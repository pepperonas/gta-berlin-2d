// Reifenspuren im Schnee (rein, nur Darstellung): je Auto zwei Linien der Hinterräder, als Stücke in einem Ringpuffer
// mit Zeitstempel. Sie verblassen mit der Zeit und schneller, solange es schneit (Neuschnee deckt zu); ohne Schneedecke
// sind sie unsichtbar. Aufgezeichnet wird mit der Weltzeit, damit Pause und Zeitlupe stimmen – die Simulation bleibt
// davon unberührt.
export const TRAILS = { max: 6000, step: 6, inset: 2, rear: 0.62, jump: 150, life: 240, minDepth: 0.05 };

export function createTrails() {
  const M = TRAILS.max;
  return { ax: new Float32Array(M), ay: new Float32Array(M), bx: new Float32Array(M), by: new Float32Array(M), t: new Float64Array(M), n: 0, head: 0, last: new Map(), gen: 0 };
}

// Hinterräder links/rechts: [lx, ly, rx, ry]
function wheels(c) {
  const ca = Math.cos(c.angle), sa = Math.sin(c.angle), bx = c.x - ca * c.hh * TRAILS.rear, by = c.y - sa * c.hh * TRAILS.rear;
  const off = Math.max(1, c.hw - TRAILS.inset);
  return [bx - sa * off, by + ca * off, bx + sa * off, by - ca * off];
}

function push(tr, ax, ay, bx, by, t) {
  const i = tr.head;
  tr.ax[i] = ax; tr.ay[i] = ay; tr.bx[i] = bx; tr.by[i] = by; tr.t[i] = t;
  tr.head = (i + 1) % TRAILS.max; tr.n = Math.min(TRAILS.max, tr.n + 1);
}

// Einmal je Bild: neue Stücke für alle Autos, die seit dem letzten Stück weit genug gefahren sind.
export function recordTrails(tr, cars, t, depth) {
  const gen = ++tr.gen;
  if (!(depth >= TRAILS.minDepth)) { tr.last.clear(); return; }
  for (const c of cars) {
    if (c.hw === undefined || c.hh === undefined) continue;
    const w = wheels(c), p = tr.last.get(c.id);
    if (!p) { tr.last.set(c.id, { w, gen }); continue; }
    p.gen = gen;
    const d = Math.hypot(w[0] - p.w[0], w[1] - p.w[1]);
    if (d < TRAILS.step) continue;
    if (d < TRAILS.jump) { push(tr, p.w[0], p.w[1], w[0], w[1], t); push(tr, p.w[2], p.w[3], w[2], w[3], t); }
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
