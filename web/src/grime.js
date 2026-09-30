// Gebrauchsspuren und Bildwirkung (nur Darstellung): großräumiger Schmutz und ausgeblichene Stellen über dem ganzen
// Boden (Rauschmuster in Weltkoordinaten, zwei Maßstäbe gegeneinander gedreht – keine sichtbare Kachel), dunkles
// Ölband in der Mitte jedes Fahrstreifens (wie auf Luftbildern), weicher Kontaktschatten um den Fuß jedes Hauses,
// Moos/Ruß auf Dächern, bewegte Lichtreflexe auf dem Wasser und eine leichte Vignette.
// Alles rein aus Ort/Zeit abgeleitet (kein world.rng); ohne Canvas-Bilddaten (Tests) fällt jede Ebene still weg.
import { noisePattern, scrolled } from './wetfx.js';
import { offsetPolyline, polylineLength } from './geom.js';
import { cutPolyline } from './roadgraph.js';
import { laneOffsets } from './street.js';
import { SURFACE } from './citycodes.js';

export const GRIME = {
  dirt: 0.3,        // Deckkraft der Schmutzflecken (großer und kleiner Maßstab zusammen)
  bleach: 0.14,     // ausgeblichene/staubige Stellen
  roof: 0.32,       // Moos und Ruß auf Dächern
  laneW: 0.9,       // m, Breite des Ölbands in der Spurmitte
  laneAlpha: 0.12,
  ao: [[30, 0.05], [16, 0.07], [7, 0.09]], // Kontaktschatten ums Haus: [Strichbreite px, Deckkraft], außen → innen
  vignette: 0.26,
};

const clamp01 = (x) => Math.max(0, Math.min(1, x));
const smooth = (u) => { const x = clamp01(u); return x * x * (3 - 2 * x); };

// Farbe und Deckkraft (0…255) je Rauschwert n (0…1): Schmutz nur in den „schmutzigen“ Senken, ausgeblichen nur auf
// den Kuppen; dazwischen durchsichtig, damit die Bodentextur unverändert bleibt.
export function grimeRGBA(kind, n) {
  switch (kind) {
    case 'dirt': return [46, 38, 28, Math.round(smooth((n - 0.42) / 0.3) * 255)];
    case 'bleach': return [236, 230, 214, Math.round(smooth((0.46 - n) / 0.26) * 255)];
    case 'roof': { // Moos (grünlich) in den Senken, Ruß (grau) auf den Kuppen
      const moss = smooth((n - 0.55) / 0.25), soot = smooth((0.38 - n) / 0.25);
      return moss >= soot ? [58, 70, 36, Math.round(moss * 255)] : [34, 34, 36, Math.round(soot * 255)];
    }
    case 'water': return [220, 235, 245, Math.round(smooth((n - 0.52) / 0.2) * 255)];
    default: throw new Error(`unbekannte Art ${kind}`);
  }
}
const pattern = (ctx, kind) => noisePattern(ctx, 'grime-' + kind, (n) => grimeRGBA(kind, n));

// Schmutz und ausgeblichene Stellen über dem sichtbaren Ausschnitt v (Weltkoordinaten). k = Stärke (Schnee deckt zu).
// Liefert die Zahl der gezeichneten Ebenen.
// water: Wasserflächen (Path2D), die ausgespart werden; low: nur eine Ebene (Qualität niedrig)
export function drawGrime(ctx, v, k = 1, water = [], low = false) {
  if (k < 0.02) return 0;
  const dirt = pattern(ctx, 'dirt'), bleach = pattern(ctx, 'bleach');
  if (!dirt || !bleach) return 0;
  const a0 = ctx.globalAlpha, x = v.x - 5, y = v.y - 5, w = v.w + 10, h = v.h + 10;
  ctx.save();
  if (water.length && typeof Path2D !== 'undefined') { // Schmutz liegt nicht auf dem Wasser
    const clip = new Path2D(); clip.rect(x, y, w, h);
    for (const wp of water) clip.addPath(wp);
    ctx.clip(clip, 'evenodd');
  }
  let n = 0;
  // ein Kachel = 256 × scale px: 3,5 → ≈ 90 m (Flecken), 13 → ≈ 330 m (ganze Ecken schmutziger), gedreht gegeneinander
  const layers = [[dirt, 3.5, 0.37, GRIME.dirt * 0.55], [dirt, 13, 1.21, GRIME.dirt * 0.45], [bleach, 6, 2.3, GRIME.bleach]];
  for (const [pat, scale, rot, a] of low ? [layers[0]] : layers) {
    ctx.globalAlpha = a0 * a * k * (low ? 1.6 : 1);
    ctx.fillStyle = scrolled(pat, scale, 0, 0, scale, rot);
    ctx.fillRect(x, y, w, h);
    n++;
  }
  ctx.restore();
  return n;
}

// Mittellinien (Querversatz in px) der Fahrstreifen einer Kante, auf denen sich Öl und Reifenabrieb sammeln. Rein
// rechnerisch, je Kante zwischengespeichert. Keine Bänder auf Kopfsteinpflaster, Wegen, Kreuzungsflächen, Brücken-Füllung.
export function laneWear(city, e) {
  if (e._wear) return e._wear;
  const out = [];
  if (e.cls <= 8 && e.cs && !e.junction && e.cs.surface !== SURFACE.cobble && e.len > 15) {
    const lo = laneOffsets(e.cs, city.scale);
    for (const off of [...lo.fwd, ...lo.bwd]) if (!out.some((o) => Math.abs(o - off) < 1)) out.push(off);
  }
  e._wear = out;
  return out;
}

// Ölbänder aller Kanten E (ein Pfad je Kante, zwischengespeichert). Liefert die Zahl der Kanten mit Band.
export function drawLaneWear(ctx, city, E) {
  if (typeof Path2D === 'undefined') return 0;
  ctx.save();
  ctx.lineCap = 'butt'; ctx.lineJoin = 'round';
  ctx.strokeStyle = `rgba(18,16,14,${GRIME.laneAlpha})`; ctx.lineWidth = GRIME.laneW * city.scale;
  let n = 0;
  for (const e of E) {
    const offs = laneWear(city, e);
    if (!offs.length) continue;
    if (!e._wearPath) { // endet vor der Kreuzung (wie Gullys und Flicken: halbe Fahrbahnbreite + 3 m)
      const p = new Path2D(), margin = Math.min(e.len / 2, e.w / 2 + 3 * city.scale);
      for (const off of offs) {
        const full = off ? offsetPolyline(e.pts, off) : e.pts, L = polylineLength(full);
        const pts = cutPolyline(full, margin, L - margin);
        if (pts.length < 4) continue;
        p.moveTo(pts[0], pts[1]);
        for (let i = 2; i < pts.length; i += 2) p.lineTo(pts[i], pts[i + 1]);
      }
      e._wearPath = p;
    }
    ctx.stroke(e._wearPath);
    n++;
  }
  ctx.restore();
  return n;
}

// Weicher Kontaktschatten ums Haus am Boden (Umgebungsverdeckung): drei Striche um den Grundriss, außen breit und
// schwach, innen schmal und dichter – das Haus selbst deckt die innere Hälfte ab.
export function drawContactShadows(ctx, buildings, pathOf, k = 1, low = false) {
  if (k < 0.02 || !buildings.length) return 0;
  ctx.save();
  ctx.lineJoin = 'round';
  for (const [w, a] of low ? [[12, 0.12]] : GRIME.ao) { // niedrige Qualität: ein Strich statt drei
    ctx.strokeStyle = `rgba(20,18,16,${(a * k).toFixed(3)})`; ctx.lineWidth = w;
    for (const b of buildings) ctx.stroke(pathOf(b));
  }
  ctx.restore();
  return buildings.length;
}

// Moos und Ruß auf einem Dach (Pfad p, schon an seinem Platz); das Muster liegt in Weltkoordinaten, seine
// Transformation ist fest (einmal gesetzt, nicht je Dach neu)
export function roofGrime(ctx, p) {
  const pat = pattern(ctx, 'roof');
  if (!pat) return false;
  if (!pat._set) { scrolled(pat, 1.6, 0, 0, 1.6, 0.8); pat._set = true; }
  const a0 = ctx.globalAlpha;
  ctx.globalAlpha = a0 * GRIME.roof;
  ctx.fillStyle = pat; ctx.fill(p, 'evenodd');
  ctx.globalAlpha = a0;
  return true;
}

// Lichtreflexe auf dem Wasser: zwei gegeneinander treibende Rauschmuster (glitzernde, wandernde Wellenfelder)
export function waterGlint(ctx, p, t, sun = 1) {
  const pat = pattern(ctx, 'water');
  if (!pat) return false;
  const a0 = ctx.globalAlpha;
  for (const [scale, vx, vy, rot, a] of [[0.9, 6, 2.5, 0.2, 0.2], [1.7, -3.5, 4, 1.4, 0.14]]) {
    ctx.globalAlpha = a0 * a * (0.45 + 0.55 * sun);
    ctx.fillStyle = scrolled(pat, scale, t * vx, t * vy, scale, rot); ctx.fill(p, 'evenodd');
  }
  ctx.globalAlpha = a0;
  return true;
}

// Vignette in Bildschirmkoordinaten (Verlauf je Bildgröße zwischengespeichert)
const vignettes = new WeakMap();
export function drawVignette(ctx, W, H, k = 1) {
  if (k < 0.02 || !ctx.createRadialGradient) return false;
  let c = vignettes.get(ctx);
  if (!c || c.W !== W || c.H !== H) {
    const g = ctx.createRadialGradient(W / 2, H / 2, Math.min(W, H) * 0.45, W / 2, H / 2, Math.hypot(W, H) / 2);
    if (!g?.addColorStop) return false; // Test-Attrappe
    g.addColorStop(0, 'rgba(8,10,16,0)');
    g.addColorStop(1, `rgba(8,10,16,${GRIME.vignette})`);
    vignettes.set(ctx, c = { W, H, g });
  }
  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha = k; ctx.fillStyle = c.g; ctx.fillRect(0, 0, W, H);
  ctx.restore();
  return true;
}
