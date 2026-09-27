// Kleinteile auf der Fahrbahn: Gullys am Bordstein, Kanaldeckel, Asphaltflicken, Risse, Ölflecken auf Parkstreifen.
// Rein rechnerisch und deterministisch je Kante (e._decals); render.js zeichnet sie. Alles liegt auf der eigenen Fahrbahn.
import { pointAlong } from './geom.js';
import { mulberry32 } from './rng.js';
import { laneOffsets, parkingStrip } from './street.js';
import { PARK, SURFACE } from './citycodes.js';

export const DECAL = {
  gullyEvery: 25,          // m, je Straßenseite
  manholeEvery: 45,        // m
  patchEvery: { main: 90, side: 45 }, // m (Nebenstraßen sind flickiger)
  crackEvery: 70,          // m
  oilEvery: 14,            // m je Parkstreifen
  maxPerEdge: 120,
};

// Decals einer Kante. Jedes: { t: 'gully'|'manhole'|'patch'|'crack'|'oil', x, y, a (Winkel), l, w (Länge/Breite in px), pts? }
export function edgeDecals(city, e) {
  if (e._decals) return e._decals;
  const out = [];
  if (e.cls <= 8 && !e.bridge && !e.junction && !e.passage && e.cs && e.len > 20) {
    const S = city.scale, cs = e.cs, rnd = mulberry32(e.id * 2654435761 >>> 0);
    const cobble = cs.surface === SURFACE.cobble;
    const lo = laneOffsets(cs, S);
    const p = { x: 0, y: 0, ux: 1, uy: 0 };
    const margin = Math.min(e.len / 2, e.w / 2 + 3 * S); // nicht in die Kreuzungsfläche
    const put = (t, s, off, l, w, turn = 0) => {
      if (s < margin || s > e.len - margin || out.length >= DECAL.maxPerEdge) return null;
      pointAlong(e.pts, s, p);
      const d = { t, x: p.x - p.uy * off, y: p.y + p.ux * off, a: Math.atan2(p.uy, p.ux) + turn, l, w };
      out.push(d);
      return d;
    };
    const count = (every) => Math.floor(e.len / (every * S) + rnd());
    const along = (n) => Array.from({ length: n }, (_, i) => (i + 0.2 + rnd() * 0.6) * e.len / Math.max(1, n));
    // Gullys: am Bordstein (hinter dem Parkstreifen liegt der Rinnstein am Bordstein)
    for (const side of [-1, 1]) for (const s of along(count(DECAL.gullyEvery))) put('gully', s, side * (cs.width / 2 - 0.35 * S), 0.6 * S, 0.4 * S);
    // irgendwo zwischen den Fahrstreifenrändern (bei Einbahn-Richtungsfahrbahnen liegt die „Mitte“ am Bordstein)
    const inLanes = () => lo.xL + (lo.xR - lo.xL) * (0.15 + rnd() * 0.7);
    // Kanaldeckel: in den Fahrstreifen
    for (const s of along(count(DECAL.manholeEvery))) put('manhole', s, inLanes(), 0.7 * S, 0.7 * S);
    if (!cobble) {
      for (const s of along(count(e.cls <= 5 ? DECAL.patchEvery.main : DECAL.patchEvery.side))) {
        put('patch', s, inLanes(), (1.5 + rnd() * 2.5) * S, (0.8 + rnd() * 1.2) * S, (rnd() - 0.5) * 0.2);
      }
      for (const s of along(count(DECAL.crackEvery))) {
        const d = put('crack', s, inLanes(), (2 + rnd() * 3) * S, 0);
        if (d) { // Zickzack-Linie entlang des Risses (lokale Koordinaten, x in Längsrichtung)
          const n = 4 + Math.floor(rnd() * 3), pts = [];
          for (let i = 0; i <= n; i++) pts.push(-d.l / 2 + d.l * i / n, (rnd() - 0.5) * 0.5 * S);
          d.pts = pts;
        }
      }
    }
    // Ölflecken: auf Parkstreifen
    for (const side of [-1, 1]) {
      const ps = parkingStrip(cs, side);
      if ((ps.kind !== PARK.lane && ps.kind !== PARK.half) || ps.depth < 1.5 * S) continue;
      for (const s of along(count(DECAL.oilEvery))) if (rnd() < 0.5) put('oil', s, ps.offset + (rnd() - 0.5) * ps.depth * 0.4, (0.6 + rnd() * 0.8) * S, (0.4 + rnd() * 0.5) * S, rnd() * Math.PI);
    }
  }
  e._decals = out;
  return out;
}
