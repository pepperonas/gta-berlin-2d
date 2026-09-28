// Ebene je Objekt (rein, ohne Zufall): Autos, Personen, Räder und die Spielfigur tragen `lvl` (0 = Boden, ≥ 1 Brücke,
// < 0 offene Unterführung). Die Ebene wechselt nur in Portalen – Knoten, die Wege verschiedener Ebenen teilen
// (Brückenkopf, Rampe, Treppe). Wer eine Brücke nur unterquert oder überquert, bleibt auf seiner Ebene.
//  – Im Portal gilt die Straße bzw. der Weg, auf dem man tatsächlich ist: der Lotfußpunkt liegt innerhalb des Stücks
//    (nicht an dessen Ende). Auf der eigenen Fahrbahn bleibt man, wo man ist.
//  – Außerhalb von Portalen bleibt die Ebene; nur wenn unter einem Objekt auf Brücke/Unterführung weit und breit keine
//    Fläche seiner Ebene mehr liegt (Sicherheitsnetz), fällt es auf den Boden zurück.
import { AREA_KIND } from './citycodes.js';
import { pointInRings } from './geom.js';

export const LEVEL = { pathHalf: 12, margin: 5, check: 10, lost: 30 }; // px (10 px = 1 m) bzw. Schritte
const q = [];
const box = { x: 0, y: 0, w: 0, h: 0 };
const around = (x, y, r) => { box.x = x - r; box.y = y - r; box.w = 2 * r; box.h = 2 * r; return box; };

export const sameLvl = (a, b) => (a?.lvl ?? 0) === (b?.lvl ?? 0);

// Reichweite einer Straße zu einer Seite (+ rechts der Kantenrichtung): Fahrbahn + Lücke zur Gegenfahrbahn + Radweg
function reach(e, side) {
  const tr = side > 0 ? e.cs?.right?.track : e.cs?.left?.track;
  return e.w / 2 + (e.fill && Math.sign(e.fill) === side ? Math.abs(e.fill) : 0) + (tr ? 4 + tr : 0);
}

// Flächenstücke um (x, y): { lvl, d (Abstand zur Achse), t (Lotfußpunkt 0…1), half (Reichweite), angle }
function pieces(city, x, y, r, out) {
  out.length = 0;
  for (const s of city.edgeSegs.query(around(x, y, r), q)) {
    const e = s.e;
    if (e.junction) continue;
    const dx = s.bx - s.ax, dy = s.by - s.ay, L2 = dx * dx + dy * dy || 1;
    const t = ((x - s.ax) * dx + (y - s.ay) * dy) / L2, tc = Math.max(0, Math.min(1, t));
    const d = Math.hypot(s.ax + dx * tc - x, s.ay + dy * tc - y), side = Math.sign((x - s.ax) * -dy + (y - s.ay) * dx) || 1;
    out.push({ lvl: e.lvl ?? 0, d, t, half: reach(e, side), angle: Math.atan2(dy, dx) });
  }
  for (const f of city.render.query(around(x, y, r), q)) {
    if (f.layer !== 'path' || !f.lvl) continue; // Wege am Boden zählen nicht als eigene Fläche
    const p = f.pts;
    for (let i = 0; i < p.length - 2; i += 2) {
      const dx = p[i + 2] - p[i], dy = p[i + 3] - p[i + 1], L2 = dx * dx + dy * dy || 1;
      const t = ((x - p[i]) * dx + (y - p[i + 1]) * dy) / L2, tc = Math.max(0, Math.min(1, t));
      const d = Math.hypot(p[i] + dx * tc - x, p[i + 1] + dy * tc - y);
      if (d < r) out.push({ lvl: f.lvl, d, t, half: LEVEL.pathHalf, angle: Math.atan2(dy, dx) });
    }
  }
  return out;
}
const tmpPieces = [];

// Liegt unter (x, y) eine Fläche der Ebene lvl (Straße, Brückenweg, Kreuzungsscheibe, Brückendeck)? r = Toleranz
export function levelHere(city, x, y, lvl, r = LEVEL.margin) {
  if (lvl === 0) return true; // Boden ist überall
  for (const p of pieces(city, x, y, 60 + r, tmpPieces)) if (p.lvl === lvl && p.d <= p.half + r) return true;
  for (const s of city.edgeSegs.query(around(x, y, 1), q)) {
    const j = s.e.junction;
    if (j && lvl >= (j.lo ?? 0) && lvl <= (j.hi ?? 0) && Math.hypot(x - j.x, y - j.y) <= j.r + r) return true;
  }
  for (const f of city.polys.query(around(x, y, 1), q)) if (f.kind === AREA_KIND.bridge && (f.lvl || 1) === lvl && pointInRings(x, y, f.rings)) return true;
  return false;
}

// Ebene beim Erzeugen/Teleport: die Straße, auf der man steht und die in Blickrichtung verläuft (angle optional);
// sonst Boden, wenn Boden-Fläche da ist; sonst die einzige vorhandene Ebene.
export function initialLevel(city, x, y, angle = null, r = 30) {
  let best = null, ground = false;
  for (const p of pieces(city, x, y, 60 + r, tmpPieces)) {
    if (p.d > p.half + r) continue;
    if (p.lvl === 0) ground = true;
    let da = 0;
    if (angle !== null) { da = Math.abs(((angle - p.angle) % Math.PI + Math.PI) % Math.PI); da = Math.min(da, Math.PI - da); }
    const score = da * 100 + p.d;
    if (!best || score < best.score) best = { lvl: p.lvl, score, aligned: da < 0.35 };
  }
  if (!best) return 0;
  if (angle !== null && best.aligned) return best.lvl;
  return ground ? 0 : best.lvl;
}

// Ebene eines Objekts einen Schritt weiterführen (obj.x, obj.y, obj.lvl; obj._lvlT zählt fürs Sicherheitsnetz)
export function stepLevel(city, obj) {
  const L = obj.lvl ?? 0;
  let portal = null;
  for (const p of city.portals.query(around(obj.x, obj.y, 1), q)) if (Math.hypot(obj.x - p.x, obj.y - p.y) <= p.r) { portal = p; break; }
  if (portal) {
    // Nur Flächen zählen, auf denen man eindeutig ist (innerhalb der Breite, Lotfußpunkt innerhalb des Stücks):
    // auf einer der eigenen Ebene bleibt man; sonst gilt die, in der man am tiefsten drin ist. Auf keiner (Gehweg
    // neben der Fahrbahn, Treppenfuß daneben) ändert sich nichts.
    let keep = false, best = null;
    for (const p of pieces(city, obj.x, obj.y, portal.r + 40, tmpPieces)) {
      if (p.lvl < portal.lo || p.lvl > portal.hi || p.t <= 0.001 || p.t >= 0.999 || p.d > p.half) continue;
      if (p.lvl === L) keep = true;
      if (!best || p.d / p.half < best.d / best.half) best = p;
    }
    if (!keep && best) obj.lvl = best.lvl;
    obj._lvlT = 0;
    return obj.lvl ?? 0;
  }
  if (L !== 0 && (obj._lvlT = (obj._lvlT ?? 0) + 1) >= LEVEL.check) { // Sicherheitsnetz: keine Fläche der eigenen Ebene mehr in der Nähe
    obj._lvlT = 0;
    if (!levelHere(city, obj.x, obj.y, L, LEVEL.lost)) obj.lvl = 0;
  }
  return obj.lvl ?? 0;
}
