// Ampeln: Jede Ampelkreuzung hat einen festen Umlauf mit zwei Achsen. Zufahrten werden nach ihrer Richtung einer
// Achse zugeordnet (Achse 0 = Richtung der ersten Zufahrt ± 45°). Versatz je Kreuzung, damit nicht alle gleich schalten.
import { hash01 } from './map.js';

export const SIGNAL = { cycle: 50, green: 20, yellow: 3, allRed: 2 }; // Sekunden

function axisOf(city, v) {
  const cache = (city._signalAxis ??= new Map());
  let a = cache.get(v);
  if (a === undefined) {
    const nd = city.nodes.get(v);
    if (!nd) return 0;
    const es = nd.edges.map((k) => city.edges.get(k)).filter(Boolean); // nach Nummer sortiert (map.js)
    const e = es.find((x) => x.cls <= 8) ?? es[0];
    if (!e) return 0;
    const p = e.pts, atStart = e.a === v;
    const [x0, y0, x1, y1] = atStart ? [p[0], p[1], p[2], p[3]] : [p[p.length - 2], p[p.length - 1], p[p.length - 4], p[p.length - 3]];
    a = Math.atan2(y1 - y0, x1 - x0);
    cache.set(v, a);
  }
  return a;
}

// Zustand für eine Zufahrt mit Fahrtrichtung heading (rad): 'green' | 'yellow' | 'red'.
export function signalState(city, v, heading, time) {
  const axis = Math.abs(Math.cos(heading - axisOf(city, v))) >= Math.SQRT1_2 ? 0 : 1;
  const { cycle, green, yellow, allRed } = SIGNAL;
  const half = green + yellow + allRed;
  const t = ((time + hash01(v * 13 + 5) * cycle) % cycle + cycle) % cycle - (axis ? half : 0);
  if (t >= 0 && t < green) return 'green';
  if (t >= green && t < green + yellow) return 'yellow';
  return 'red';
}
