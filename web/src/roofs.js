// Dächer und Fassaden, rein rechnerisch aus Gebäudeart, Höhe und Seed (keine zusätzlichen Kartendaten):
//  – roofStyle: flach (Kies/Bitumen, bei Altbauten mit Ziegelrand wie das „Berliner Dach“), Satteldach, Wellblech
//  – facadeStyle: Altbau, Plattenbau, Neubau (Fensterband), Industrie
//  – roofDecor: Schornsteine, Lichtschächte, Oberlichter, Lüftungsgeräte, Solarmodule, Dachterrassen – nur innerhalb
//    des Grundrisses, ohne Überlappung, ausgerichtet an der Hauptachse des Hauses.
// Ergebnisse werden am Gebäude zwischengespeichert (b._roof) und fallen mit ihm weg, wenn seine Kachel entladen wird.
import { BUILDING_KIND } from './citycodes.js';
import { pointInRings, signedArea } from './geom.js';
import { mulberry32 } from './rng.js';

const K = BUILDING_KIND;

// Richtung der längsten Außenkante (Hauptachse), in Radiant
export function mainAxis(b) {
  const r = b.rings[0], n = r.length;
  let best = 0, a = 0;
  for (let i = 0; i < n; i += 2) {
    const ex = r[(i + 2) % n] - r[i], ey = r[(i + 3) % n] - r[i + 1], L = ex * ex + ey * ey;
    if (L > best) { best = L; a = Math.atan2(ey, ex); }
  }
  return a;
}

export function footprintM2(b, scale = 10) { return Math.abs(signedArea(b.rings[0])) / (scale * scale); }

export function roofStyle(b, scale = 10) {
  const r = (b.seed % 1000) / 1000, m = b.meters ?? b.height / scale, area = footprintM2(b, scale);
  if (b.kind === K.church) return 'pitched';
  if (b.kind === K.industrial || b.kind === K.warehouse) return 'corrugated';
  if (b.kind === K.small) return r < 0.5 ? 'pitched' : 'flat';
  if (b.kind === K.house && m <= 9 && area < 400 && r < 0.75) return 'pitched'; // Einfamilien- und Reihenhäuser
  if (b.kind === K.house && m >= 12 && m <= 24 && r < 0.6) return 'berlin'; // Altbau: Ziegelrand, flache Mitte
  return 'flat';
}

export function facadeStyle(b, scale = 10) {
  const r = ((b.seed / 1000) | 0) % 1000 / 1000, m = b.meters ?? b.height / scale;
  if (b.kind === K.industrial || b.kind === K.warehouse) return 'industry';
  if (b.kind === K.public) return r < 0.6 ? 'modern' : 'altbau';
  if (b.kind === K.house) {
    if (m > 30) return r < 0.7 ? 'platte' : 'modern';
    if (m > 24) return r < 0.4 ? 'platte' : r < 0.7 ? 'modern' : 'altbau';
    return r < 0.8 ? 'altbau' : 'modern';
  }
  return 'altbau';
}

// Aufbauten: Maße in m (Länge entlang der Hauptachse × Breite)
const DECOR = {
  chimney: [0.7, 0.7], shaft: [1.8, 1.6], skylight: [1.4, 0.9], ac: [2.2, 1.4], solar: [5, 2.2], terrace: [4.5, 3.5],
};

export function roofDecor(b, scale = 10) {
  const style = roofStyle(b, scale);
  if (style === 'pitched' || b.kind === K.small) return [];
  const area = footprintM2(b, scale);
  if (area < 60) return [];
  const rnd = mulberry32(b.seed ^ 0x5bd1e995);
  const a = mainAxis(b), ca = Math.cos(a), sa = Math.sin(a);
  const wish = [];
  const n = (per, max) => Math.min(max, Math.floor(area / per + rnd()));
  if (style === 'corrugated') { wish.push(...Array(n(250, 8)).fill('ac'), ...Array(n(400, 6)).fill('skylight')); if (rnd() < 0.2) wish.push('solar', 'solar'); }
  else {
    wish.push(...Array(n(55, 14)).fill('chimney'), ...Array(n(260, 4)).fill('shaft'), ...Array(n(180, 5)).fill('skylight'));
    if (b.kind === K.public) wish.push(...Array(n(300, 5)).fill('ac'));
    if (rnd() < 0.15) wish.push('solar', 'solar', 'solar');
    if (rnd() < 0.1) wish.push('terrace');
  }
  const { x, y, w, h } = b.bbox, out = [];
  for (const t of wish) {
    const [lm, wm] = DECOR[t], l = lm * scale, wd = wm * scale;
    for (let tries = 0; tries < 12; tries++) {
      const px = x + rnd() * w, py = y + rnd() * h;
      // ganze Fläche (plus Rand) im Grundriss? 5 × 5 Punkte – nur die Ecken zu prüfen reicht bei eingebuchteten
      // Grundrissen nicht (eine Einbuchtung kann zwischen den Ecken liegen)
      const hx = l / 2 + 0.4 * scale, hy = wd / 2 + 0.4 * scale;
      let inside = true;
      for (let i = 0; i <= 4 && inside; i++) for (let j = 0; j <= 4 && inside; j++) {
        const u = -hx + hx * i / 2, v = -hy + hy * j / 2;
        inside = pointInRings(px + ca * u - sa * v, py + sa * u + ca * v, b.rings);
      }
      if (!inside) continue;
      if (out.some((o) => Math.hypot(o.x - px, o.y - py) < (Math.max(o.l, o.w) + Math.max(l, wd)) / 2 + 0.5 * scale)) continue;
      out.push({ t, x: px, y: py, l, w: wd, a });
      break;
    }
  }
  return out;
}

// Alles zusammen, einmal je Gebäude
export function roofOf(b, scale = 10) {
  return (b._roof ??= { style: roofStyle(b, scale), facade: facadeStyle(b, scale), axis: mainAxis(b), decor: roofDecor(b, scale) });
}
