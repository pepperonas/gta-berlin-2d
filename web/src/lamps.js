// Straßenlaternen: deterministische Standorte je Straßenkante, am Bordstein auf dem Gehweg (nie auf einer Fahrbahn,
// in einem Haus oder im Wasser). Rein rechnerisch; render.js zeichnet Mast und Licht. Zwischengespeichert an der Kante
// (e._lamps) – nur nahe der Kamera abgefragt, wo die Kacheln vollständig geladen sind.
import { pointAlong } from './geom.js';
import { hash01, treeOnRoad, inBuilding, surfaceAt, T } from './map.js';

export const LAMP = {
  mainSpacing: 25, sideSpacing: 30,   // mittlerer Abstand in m (Hauptstraßen dichter)
  curbGap: 0.7,                        // Mast so weit hinter der Bordsteinkante (m)
  bothSidesFrom: 10,                   // ab dieser Fahrbahnbreite (m) beidseitig, versetzt
  cornerGap: 6,                        // Abstand zur Kreuzungsecke (m)
  maxClass: 8,                         // bis zu dieser Straßenklasse (Wohnstraßen)
};

// Lichtfarbe: Gaslaternen warm, Hauptstraßen hell-neutral, Nebenstraßen warmweiß
export const LAMP_RGB = { gas: '255,186,110', main: '255,236,200', side: '255,214,158' };

export function edgeLamps(city, e) {
  if (e._lamps) return e._lamps;
  const S = city.scale, out = [];
  if (e.inside && e.cls <= LAMP.maxClass && !e.bridge && !e.junction && !e.passage && e.cs) {
    const main = e.cls <= 5;
    const step = (main ? LAMP.mainSpacing : LAMP.sideSpacing) * S;
    const both = e.cs.width >= LAMP.bothSidesFrom * S;
    const sides = both ? [-1, 1] : [hash01(e.id * 131 + 7) < 0.5 ? -1 : 1];
    const corner = (n) => {
      // nur echte Straßen machen eine Kreuzung (Grundstückszufahrten und Fußwege nicht)
      const nd = city.nodes.get(n);
      let r = 0, streets = 0;
      for (const k of nd?.edges ?? []) { const o = city.edges.get(k); if (o && o !== e && o.cls <= LAMP.maxClass) { streets++; r = Math.max(r, o.w / 2); } }
      return streets >= 2 ? r + LAMP.cornerGap * S : 1 * S;
    };
    const s0 = corner(e.a), s1 = e.len - corner(e.b);
    const rgb = e.cs.gaslight ? LAMP_RGB.gas : main ? LAMP_RGB.main : LAMP_RGB.side;
    const p = { x: 0, y: 0, ux: 1, uy: 0 };
    // OSM zerlegt Straßen in viele kurze Stücke: je Stück so viele Laternen, wie der Abstand im Mittel ergibt
    // (Nachkommaanteil gewürfelt), gleichmäßig verteilt – so stimmt die Dichte auch über kurze Stücke hinweg.
    const avail = s1 - s0;
    for (const side of sides) {
      if (avail <= 0) break;
      const off = side * (e.w / 2 + LAMP.curbGap * S);
      const n = Math.floor(avail / step + hash01(e.id * 17 + side * 5 + 3));
      const gap = avail / Math.max(1, n);
      const phase = gap * (both && side > 0 ? 0.25 : 0.75) * (0.6 + 0.8 * hash01(e.id * 29 + side));
      for (let i = 0; i < n; i++) {
        const s = s0 + Math.min(avail, (phase + i * gap) % avail);
        pointAlong(e.pts, s, p);
        const x = p.x - p.uy * off, y = p.y + p.ux * off;
        // nicht auf einer befahrbaren Fahrbahn (eigene oder fremde), nicht im Haus, nicht im Wasser
        if (treeOnRoad(city, { x, y, r: 0.3 * S }) || inBuilding(city, x, y) || surfaceAt(city, x, y) === T.WATER) continue;
        // Arm zeigt zur Fahrbahn; das Licht fällt auf die Fahrbahnkante
        const nx = p.uy * side, ny = -p.ux * side;
        out.push({ x, y, nx, ny, rgb, gas: !!e.cs.gaslight, main });
      }
    }
  }
  e._lamps = out;
  return out;
}
