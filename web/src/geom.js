// Geometrie auf flachen Koordinatenlisten [x0, y0, x1, y1, …]. Geteilt von Spiel und Karten-Build.

export function pointInRing(x, y, r) {
  let inside = false;
  for (let i = 0, j = r.length - 2; i < r.length; j = i, i += 2) {
    const xi = r[i], yi = r[i + 1], xj = r[j], yj = r[j + 1];
    if ((yi > y) !== (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

// Ringe mit gerade/ungerade-Regel (Außenring + Löcher).
export function pointInRings(x, y, rings) {
  let k = 0;
  for (const r of rings) if (pointInRing(x, y, r)) k++;
  return k % 2 === 1;
}

// Vorzeichenbehaftete Fläche nach Gauß (bei y nach unten: > 0 = im Uhrzeigersinn auf dem Bildschirm).
export function signedArea(r) {
  let s = 0;
  for (let i = 0, j = r.length - 2; i < r.length; j = i, i += 2) s += r[j] * r[i + 1] - r[i] * r[j + 1];
  return s / 2;
}

export function segDist2(px, py, ax, ay, bx, by) {
  const dx = bx - ax, dy = by - ay, l2 = dx * dx + dy * dy;
  let t = l2 ? ((px - ax) * dx + (py - ay) * dy) / l2 : 0;
  t = t < 0 ? 0 : t > 1 ? 1 : t;
  const qx = ax + t * dx - px, qy = ay + t * dy - py;
  return qx * qx + qy * qy;
}

export function bboxOf(pts, out = { x: 0, y: 0, w: 0, h: 0 }) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (let i = 0; i < pts.length; i += 2) {
    if (pts[i] < x0) x0 = pts[i]; if (pts[i] > x1) x1 = pts[i];
    if (pts[i + 1] < y0) y0 = pts[i + 1]; if (pts[i + 1] > y1) y1 = pts[i + 1];
  }
  out.x = x0; out.y = y0; out.w = x1 - x0; out.h = y1 - y0;
  return out;
}

export function polylineLength(pts) {
  let L = 0;
  for (let i = 0; i < pts.length - 2; i += 2) L += Math.hypot(pts[i + 2] - pts[i], pts[i + 3] - pts[i + 1]);
  return L;
}

// Punkt bei Bogenlänge s auf einer Polylinie, dazu Richtung (Einheitsvektor).
export function pointAlong(pts, s, out = { x: 0, y: 0, ux: 1, uy: 0 }) {
  const n = pts.length;
  if (s <= 0) s = 0;
  for (let i = 0; i < n - 2; i += 2) {
    const dx = pts[i + 2] - pts[i], dy = pts[i + 3] - pts[i + 1], L = Math.hypot(dx, dy);
    if (s <= L || i === n - 4) {
      const t = L ? Math.min(1, s / L) : 0;
      out.x = pts[i] + dx * t; out.y = pts[i + 1] + dy * t;
      out.ux = L ? dx / L : 1; out.uy = L ? dy / L : 0;
      return out;
    }
    s -= L;
  }
  out.x = pts[n - 2]; out.y = pts[n - 1];
  return out;
}

// Nächster Punkt auf einer Polylinie: { s (Bogenlänge), d2, x, y, ux, uy }.
export function projectOnPolyline(pts, x, y) {
  let best = null, acc = 0;
  for (let i = 0; i < pts.length - 2; i += 2) {
    const ax = pts[i], ay = pts[i + 1], dx = pts[i + 2] - ax, dy = pts[i + 3] - ay, L2 = dx * dx + dy * dy, L = Math.sqrt(L2);
    let t = L2 ? ((x - ax) * dx + (y - ay) * dy) / L2 : 0;
    t = t < 0 ? 0 : t > 1 ? 1 : t;
    const qx = ax + dx * t, qy = ay + dy * t, d2 = (qx - x) ** 2 + (qy - y) ** 2;
    if (!best || d2 < best.d2) best = { s: acc + L * t, d2, x: qx, y: qy, ux: L ? dx / L : 1, uy: L ? dy / L : 0 };
    acc += L;
  }
  return best;
}

// Parallele Linie im Abstand d (positiv = rechts in Laufrichtung bei y nach unten).
export function offsetPolyline(pts, d) {
  const out = [];
  const n = pts.length / 2;
  for (let i = 0; i < n; i++) {
    const a = Math.max(0, i - 1), b = Math.min(n - 1, i + 1);
    let dx = pts[2 * b] - pts[2 * a], dy = pts[2 * b + 1] - pts[2 * a + 1];
    const L = Math.hypot(dx, dy) || 1; dx /= L; dy /= L;
    out.push(pts[2 * i] - dy * d, pts[2 * i + 1] + dx * d);
  }
  return out;
}

export function delta(pts) {
  const out = new Array(pts.length);
  for (let i = 0; i < pts.length; i++) out[i] = i < 2 ? pts[i] : pts[i] - pts[i - 2];
  return out;
}
export function undelta(d) {
  const out = new Array(d.length);
  for (let i = 0; i < d.length; i++) out[i] = i < 2 ? d[i] : d[i] + out[i - 2];
  return out;
}
