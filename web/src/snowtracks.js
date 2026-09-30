// Reifenspuren im Schnee (rein, nur Darstellung): je Auto zwei Linien der Hinterräder, als Stücke in einem Ringpuffer
// mit Zeitstempel. Sie verblassen mit der Zeit und schneller, solange es schneit (Neuschnee deckt zu); ohne Schneedecke
// sind sie unsichtbar. Aufgezeichnet wird mit der Weltzeit, damit Pause und Zeitlupe stimmen – die Simulation bleibt
// davon unberührt.
// Radstellung wie gezeichnet (vehicles.js drawCarBody): rear/front = Achsabstand von der Mitte als Anteil der halben Länge (car.hw), inset = Rad innen von der
// Flanke (car.hh = halbe Breite). Ein Auto zieht vier Spuren (geradeaus decken sich vorn und hinten, in Kurven und beim
// Driften laufen sie auseinander), ein Zweirad eine.
export const TRAILS = { max: 12000, step: 6, inset: 1.2, rear: 0.57, front: 0.62, jump: 150, life: 240, minDepth: 0.05 };

export function createTrails() {
  const M = TRAILS.max;
  return { ax: new Float32Array(M), ay: new Float32Array(M), bx: new Float32Array(M), by: new Float32Array(M), t: new Float64Array(M), n: 0, head: 0, last: new Map(), gen: 0 };
}

const TWO = new Set(['bicycle', 'escooter', 'motorcycle', 'scooter']);
// Aufstandspunkte der Räder als [x0, y0, x1, y1, …] – car.hw ist die halbe Länge, car.hh die halbe Breite (car.js)
export function wheels(c) {
  const ca = Math.cos(c.angle), sa = Math.sin(c.angle), r = c.hw * TRAILS.rear, f = c.hw * TRAILS.front;
  if (TWO.has(c.kind)) return [c.x - ca * r, c.y - sa * r, c.x + ca * f, c.y + sa * f];
  const off = Math.max(1, c.hh - TRAILS.inset), ox = -sa * off, oy = ca * off;
  const rx = c.x - ca * r, ry = c.y - sa * r, fx = c.x + ca * f, fy = c.y + sa * f;
  return [rx + ox, ry + oy, rx - ox, ry - oy, fx + ox, fy + oy, fx - ox, fy - oy];
}

function push(tr, ax, ay, bx, by, t) {
  const i = tr.head;
  tr.ax[i] = ax; tr.ay[i] = ay; tr.bx[i] = bx; tr.by[i] = by; tr.t[i] = t;
  tr.head = (i + 1) % TRAILS.max; tr.n = Math.min(TRAILS.max, tr.n + 1);
}

// Einmal je Bild: neue Stücke für alle Autos am Boden (Ebene 0 – Brücken liegen darüber), die seit dem letzten Stück
// weit genug gefahren sind. Liegende Zweiräder und Wracks ohne Fahrt ziehen nichts (Stillstand).
export function recordTrails(tr, cars, t, depth) {
  const gen = ++tr.gen;
  if (!(depth >= TRAILS.minDepth)) { tr.last.clear(); return; }
  for (const c of cars) {
    if (c.hw === undefined || c.hh === undefined) continue;
    if ((c.lvl ?? 0) !== 0) { tr.last.delete(c.id); continue; }
    const w = wheels(c), p = tr.last.get(c.id);
    if (!p || p.w.length !== w.length) { tr.last.set(c.id, { w, gen }); continue; }
    p.gen = gen;
    let d = 0;
    for (let k = 0; k < w.length; k += 2) d = Math.max(d, Math.hypot(w[k] - p.w[k], w[k + 1] - p.w[k + 1]));
    if (d < TRAILS.step) continue;
    if (d < TRAILS.jump) for (let k = 0; k < w.length; k += 2) push(tr, p.w[k], p.w[k + 1], w[k], w[k + 1], t);
    p.w = w;
  }
  if (gen % 60 === 0) for (const [id, p] of tr.last) if (p.gen !== gen) tr.last.delete(id); // verschwundene Autos
}

// Sichtbarkeit eines Stücks: blasser mit dem Alter, Lebenszeit kürzer bei Schneefall (snowfall 0..1+).
export function trailAlpha(age, depth, snowfall) {
  if (!(depth >= TRAILS.minDepth)) return 0;
  const life = TRAILS.life / (1 + 4 * Math.max(0, snowfall));
  if (age >= life) return 0;
  return Math.min(1, depth * 1.4) * 0.55 * (1 - age / life) ** 1.5;
}

// Stücke im Sichtfeld v mit ihrer Deckkraft a.
export function visibleTrails(tr, v, now, depth, snowfall, out = []) {
  out.length = 0;
  const M = TRAILS.max, x0 = v.x, y0 = v.y, x1 = v.x + v.w, y1 = v.y + v.h;
  for (let k = 0; k < tr.n; k++) {
    const i = (tr.head - 1 - k + M) % M;
    const a = trailAlpha(now - tr.t[i], depth, snowfall);
    if (a <= 0.01) continue;
    const ax = tr.ax[i], ay = tr.ay[i], bx = tr.bx[i], by = tr.by[i];
    if (Math.max(ax, bx) < x0 || Math.min(ax, bx) > x1 || Math.max(ay, by) < y0 || Math.min(ay, by) > y1) continue;
    out.push({ ax, ay, bx, by, a });
  }
  return out;
}
