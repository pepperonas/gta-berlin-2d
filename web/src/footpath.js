// Wegfindung zu Fuß (rein): A* auf einem 8-px-Raster (0,8 m) im Rechteck um Start und Ziel. Eine Zelle ist frei, wenn
// ein Kreis mit dem Spielerradius dort weder feste Hindernisse (Wände, Zäune, Bäume, Poller, Kisten – dieselbe
// Prüfung wie die Kollision, car.js blocks) noch Häuser oder stehende Autos berührt. Zellen werden erst beim Besuch
// geprüft. Unerreichbares Ziel → Weg zur nächstgelegenen erreichbaren Zelle. Der Weg wird über Sichtlinien geglättet.
import { circleVsSegment, circleVsCircle, circleVsRect, circleVsObb } from './collision.js';
import { blocks } from './car.js';
import { inBuilding } from './map.js';
import { PLAYER } from './config.js';

export const FOOTPATH = { cell: 8, margin: 200, maxSide: 2500, maxNodes: 60000 };
const q = [];

export function footFree(world, x, y, lvl = 0, r = PLAYER.radius) {
  if (inBuilding(world.city, x, y)) return false;
  for (const s of world.solids.query({ x: x - r, y: y - r, w: 2 * r, h: 2 * r }, q)) {
    if (!blocks(world, s, lvl)) continue;
    if (s.seg ? circleVsSegment(x, y, r, s) : s.r !== undefined ? circleVsCircle(x, y, r, s.x, s.y, s.r) : circleVsRect(x, y, r, s)) return false;
  }
  for (const c of world.cars) if (Math.abs(c.vx) + Math.abs(c.vy) < 5 && Math.abs(c.x - x) < c.hw + r + 2 && Math.abs(c.y - y) < c.hw + r + 2 && circleVsObb(x, y, r, c)) return false;
  return true;
}

export function findFootPath(world, from, to, lvl = 0) {
  const C = FOOTPATH.cell, M = FOOTPATH.margin;
  let x0 = Math.min(from.x, to.x) - M, y0 = Math.min(from.y, to.y) - M, x1 = Math.max(from.x, to.x) + M, y1 = Math.max(from.y, to.y) + M;
  if (x1 - x0 > FOOTPATH.maxSide) { const c = from.x; x0 = c - FOOTPATH.maxSide / 2; x1 = c + FOOTPATH.maxSide / 2; }
  if (y1 - y0 > FOOTPATH.maxSide) { const c = from.y; y0 = c - FOOTPATH.maxSide / 2; y1 = c + FOOTPATH.maxSide / 2; }
  const nx = Math.ceil((x1 - x0) / C), ny = Math.ceil((y1 - y0) / C), N = nx * ny;
  const cx = (i) => x0 + (i % nx + 0.5) * C, cy = (i) => y0 + (Math.floor(i / nx) + 0.5) * C;
  const idx = (x, y) => { const gx = Math.floor((x - x0) / C), gy = Math.floor((y - y0) / C); return gx < 0 || gy < 0 || gx >= nx || gy >= ny ? -1 : gy * nx + gx; };
  const free = new Int8Array(N); // 0 = ungeprüft, 1 = frei, 2 = belegt
  const isFree = (i) => (free[i] ||= footFree(world, cx(i), cy(i), lvl) ? 1 : 2) === 1;
  const s = idx(from.x, from.y), t = idx(to.x, to.y);
  if (s < 0) return null;
  const g = new Float32Array(N).fill(Infinity), prev = new Int32Array(N).fill(-1), closed = new Uint8Array(N);
  const h = (i) => Math.hypot(cx(i) - to.x, cy(i) - to.y);
  // Binärer Heap über (f, Zelle)
  const heap = [], push = (f, i) => { heap.push([f, i]); let k = heap.length - 1; while (k > 0) { const p = (k - 1) >> 1; if (heap[p][0] <= heap[k][0]) break; [heap[p], heap[k]] = [heap[k], heap[p]]; k = p; } };
  const pop = () => { const top = heap[0], last = heap.pop(); if (heap.length) { heap[0] = last; let k = 0; for (;;) { const l = 2 * k + 1, r = l + 1; let m = k; if (l < heap.length && heap[l][0] < heap[m][0]) m = l; if (r < heap.length && heap[r][0] < heap[m][0]) m = r; if (m === k) break; [heap[m], heap[k]] = [heap[k], heap[m]]; k = m; } } return top; };
  g[s] = 0; push(h(s), s);
  let best = s, bestH = h(s), n = 0;
  const DIRS = [[1, 0, 1], [-1, 0, 1], [0, 1, 1], [0, -1, 1], [1, 1, Math.SQRT2], [1, -1, Math.SQRT2], [-1, 1, Math.SQRT2], [-1, -1, Math.SQRT2]];
  while (heap.length && n++ < FOOTPATH.maxNodes) {
    const [, i] = pop();
    if (closed[i]) continue;
    closed[i] = 1;
    const hi = h(i);
    if (hi < bestH) { bestH = hi; best = i; }
    if (i === t) break;
    const gx = i % nx, gy = Math.floor(i / nx);
    for (const [dx, dy, cost] of DIRS) {
      const ax = gx + dx, ay = gy + dy;
      if (ax < 0 || ay < 0 || ax >= nx || ay >= ny) continue;
      const j = ay * nx + ax;
      if (closed[j] || !isFree(j)) continue;
      if (dx && dy && (!isFree(gy * nx + ax) || !isFree(ay * nx + gx))) continue; // keine Ecken schneiden
      const ng = g[i] + cost * C;
      if (ng < g[j]) { g[j] = ng; prev[j] = i; push(ng + h(j), j); }
    }
  }
  const end = closed[t] ? t : best;
  const cells = [];
  for (let i = end; i >= 0; i = prev[i]) cells.push(i);
  cells.reverse();
  const pts = cells.map((i) => ({ x: cx(i), y: cy(i) }));
  if (end === t && isFree(t)) pts[pts.length - 1] = { x: to.x, y: to.y }; // genau aufs Ziel
  pts[0] = { x: from.x, y: from.y };
  return smooth(pts, (a, b) => lineFree(a, b, idx, isFree, C));
}

// Sichtlinie zwischen zwei Punkten: alle Rasterzellen entlang der Linie frei (Abtastung je halbe Zelle)
function lineFree(a, b, idx, isFree, C) {
  const L = Math.hypot(b.x - a.x, b.y - a.y), n = Math.ceil(L / (C / 2));
  for (let k = 1; k < n; k++) { const i = idx(a.x + (b.x - a.x) * k / n, a.y + (b.y - a.y) * k / n); if (i < 0 || !isFree(i)) return false; }
  return true;
}

function smooth(pts, see) {
  if (pts.length <= 2) return pts;
  const out = [pts[0]];
  let i = 0;
  while (i < pts.length - 1) {
    let j = pts.length - 1;
    while (j > i + 1 && !see(pts[i], pts[j])) j--;
    out.push(pts[j]); i = j;
  }
  return out;
}
