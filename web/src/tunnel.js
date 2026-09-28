// Unter Tage (rein): U- und S-Bahn fahren dort im Tunnel, wo die Karte kein sichtbares Gleis hat (der Build verwirft
// Tunnelgleise). Dieselbe Regel entscheidet, ob Züge oben gezeichnet werden (render.js) und wann die Tunnelansicht
// kommt. Eine noch nicht geladene Kachel gilt als oberirdisch – sonst blitzte beim Nachladen die Tunnelansicht auf.
// Reine Nähe reicht nicht: an Kottbusser Tor liegt der U8-Tunnel nur ~5 m unter dem QUERENDEN U1-Viadukt (Skalitzer
// Straße) – ein Abstandstest allein hält die eigene (Nord-Süd-)Strecke für sichtbar, weil er die andere (Ost-West-)
// Strecke danebenmisst. Deshalb zählt eine Schiene nur, wenn ihre Richtung zur Fahrtrichtung passt (±35°, ohne
// Vorzeichen – ein Gleis hat keine Fahrtrichtung). Ohne Fahrtrichtung (railAt ohne dx/dy, so wie render.js es ruft)
// bleibt die reine Abstandsprüfung wie zuvor.
import { segDist2 } from './geom.js';
import { pointOn } from './transit.js';

export const TUNNEL = { probe: 50, step: 60, maxAngle: 35 };
const MAX_COS = Math.cos(TUNNEL.maxAngle * Math.PI / 180);
const q = [];

export function railAt(city, x, y, dx, dy) {
  const r = TUNNEL.probe, r2 = r * r;
  const hl = dx === undefined || dy === undefined ? 0 : Math.hypot(dx, dy);
  const hx = hl > 1e-9 ? dx / hl : 0, hy = hl > 1e-9 ? dy / hl : 0;
  for (const f of city.render.query({ x: x - r, y: y - r, w: 2 * r, h: 2 * r }, q)) {
    if (f.layer !== 'rail') continue;
    const p = f.pts;
    for (let i = 0; i < p.length - 2; i += 2) {
      if (segDist2(x, y, p[i], p[i + 1], p[i + 2], p[i + 3]) >= r2) continue;
      if (hl <= 1e-9) return f; // keine Fahrtrichtung: reine Nähe zählt (Bestandsverhalten)
      const sx = p[i + 2] - p[i], sy = p[i + 3] - p[i + 1], sl = Math.hypot(sx, sy);
      if (sl > 1e-9 && Math.abs((sx * hx + sy * hy) / sl) >= MAX_COS) return f;
    }
  }
  return null;
}

export function undergroundAt(city, mode, x, y, dx, dy) {
  if (mode !== 'ubahn' && mode !== 'sbahn') return false;
  if (!city.ready(x, y, TUNNEL.probe)) return false;
  return !railAt(city, x, y, dx, dy);
}

// Lage auf einem Muster; Ergebnis je 60-px-Stück zwischengespeichert, bis sich der Kachelstand ändert.
// Die Fahrtrichtung am Abtastpunkt (aus pointOn) geht als Heading mit, damit eine querende Schiene nicht zählt.
export function undergroundAtS(city, p, s) {
  if (p.mode !== 'ubahn' && p.mode !== 'sbahn') return false;
  const c = p._ug && p._ug.gen === city.gen ? p._ug : (p._ug = { gen: city.gen, m: new Map() });
  const k = Math.round(s / TUNNEL.step);
  let v = c.m.get(k);
  if (v === undefined) {
    const pt = pointOn(p, k * TUNNEL.step);
    v = undergroundAt(city, p.mode, pt.x, pt.y, Math.cos(pt.angle), Math.sin(pt.angle));
    if (city.ready(pt.x, pt.y, TUNNEL.probe)) c.m.set(k, v); // Unfertiges nicht merken
  }
  return v;
}
