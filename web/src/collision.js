// Kollisionen: Kreis, achsenparallele Rechtecke (AABB) und gedrehte Boxen (OBB, für Autos).
// Alle Tests liefern { nx, ny, depth }: Normale zeigt vom Hindernis weg zum ersten Objekt.
// Das erste Objekt wird um depth entlang (nx, ny) herausgeschoben.

export function circleVsRect(cx, cy, r, rect) {
  const px = Math.max(rect.x, Math.min(cx, rect.x + rect.w));
  const py = Math.max(rect.y, Math.min(cy, rect.y + rect.h));
  let dx = cx - px, dy = cy - py;
  const d2 = dx * dx + dy * dy;
  if (d2 > r * r) return null;
  if (d2 > 1e-9) {
    const d = Math.sqrt(d2);
    return { nx: dx / d, ny: dy / d, depth: r - d };
  }
  // Mittelpunkt im Rechteck: kürzester Weg hinaus.
  const left = cx - rect.x, right = rect.x + rect.w - cx, top = cy - rect.y, bottom = rect.y + rect.h - cy;
  const m = Math.min(left, right, top, bottom);
  if (m === left) return { nx: -1, ny: 0, depth: left + r };
  if (m === right) return { nx: 1, ny: 0, depth: right + r };
  if (m === top) return { nx: 0, ny: -1, depth: top + r };
  return { nx: 0, ny: 1, depth: bottom + r };
}

export function circleVsCircle(ax, ay, ar, bx, by, br) {
  const dx = ax - bx, dy = ay - by, rr = ar + br;
  const d2 = dx * dx + dy * dy;
  if (d2 >= rr * rr) return null;
  const d = Math.sqrt(d2);
  if (d < 1e-9) return { nx: 1, ny: 0, depth: rr };
  return { nx: dx / d, ny: dy / d, depth: rr - d };
}

// OBB: { x, y, angle, hw (halbe Länge, Fahrtrichtung), hh (halbe Breite) }
export function obbAxes(o) {
  const c = Math.cos(o.angle), s = Math.sin(o.angle);
  return [c, s, -s, c]; // vorwärts (fx, fy), rechts (rx, ry)
}

export function obbCorners(o) {
  const [fx, fy, rx, ry] = obbAxes(o);
  const pts = [];
  for (const [a, b] of [[1, 1], [1, -1], [-1, -1], [-1, 1]]) {
    pts.push({ x: o.x + fx * o.hw * a + rx * o.hh * b, y: o.y + fy * o.hw * a + ry * o.hh * b });
  }
  return pts;
}

export function obbBounds(o) {
  const [fx, fy, rx, ry] = obbAxes(o);
  const ex = Math.abs(fx) * o.hw + Math.abs(rx) * o.hh;
  const ey = Math.abs(fy) * o.hw + Math.abs(ry) * o.hh;
  return { x: o.x - ex, y: o.y - ey, w: ex * 2, h: ey * 2 };
}

function projObb(o, axes, nx, ny) {
  const r = o.hw * Math.abs(axes[0] * nx + axes[1] * ny) + o.hh * Math.abs(axes[2] * nx + axes[3] * ny);
  const c = o.x * nx + o.y * ny;
  return [c - r, c + r];
}

function sat(axisList, projA, projB, ax, ay, bx, by) {
  let best = Infinity, bnx = 0, bny = 0;
  for (const [nx, ny] of axisList) {
    const [a0, a1] = projA(nx, ny);
    const [b0, b1] = projB(nx, ny);
    const overlap = Math.min(a1, b1) - Math.max(a0, b0);
    if (overlap <= 0) return null;
    if (overlap < best) { best = overlap; bnx = nx; bny = ny; }
  }
  if ((ax - bx) * bnx + (ay - by) * bny < 0) { bnx = -bnx; bny = -bny; }
  return { nx: bnx, ny: bny, depth: best };
}

export function obbVsRect(o, rect) {
  const axes = obbAxes(o);
  const cx = rect.x + rect.w / 2, cy = rect.y + rect.h / 2;
  return sat(
    [[axes[0], axes[1]], [axes[2], axes[3]], [1, 0], [0, 1]],
    (nx, ny) => projObb(o, axes, nx, ny),
    (nx, ny) => {
      const r = (rect.w / 2) * Math.abs(nx) + (rect.h / 2) * Math.abs(ny);
      const c = cx * nx + cy * ny;
      return [c - r, c + r];
    },
    o.x, o.y, cx, cy,
  );
}

export function obbVsObb(a, b) {
  const aa = obbAxes(a), ba = obbAxes(b);
  return sat(
    [[aa[0], aa[1]], [aa[2], aa[3]], [ba[0], ba[1]], [ba[2], ba[3]]],
    (nx, ny) => projObb(a, aa, nx, ny),
    (nx, ny) => projObb(b, ba, nx, ny),
    a.x, a.y, b.x, b.y,
  );
}

// Normale zeigt von der Box zum Kreis (der Kreis wird herausgeschoben).
export function circleVsObb(cx, cy, r, o) {
  const [fx, fy, rx, ry] = obbAxes(o);
  const dx = cx - o.x, dy = cy - o.y;
  const lx = dx * fx + dy * fy, ly = dx * rx + dy * ry;
  const qx = Math.max(-o.hw, Math.min(lx, o.hw));
  const qy = Math.max(-o.hh, Math.min(ly, o.hh));
  let ex = lx - qx, ey = ly - qy;
  const d2 = ex * ex + ey * ey;
  let nlx, nly, depth;
  if (d2 > r * r) return null;
  if (d2 > 1e-9) {
    const d = Math.sqrt(d2);
    nlx = ex / d; nly = ey / d; depth = r - d;
  } else {
    const px = o.hw - Math.abs(lx), py = o.hh - Math.abs(ly);
    if (px < py) { nlx = Math.sign(lx) || 1; nly = 0; depth = px + r; }
    else { nlx = 0; nly = Math.sign(ly) || 1; depth = py + r; }
  }
  return { nx: nlx * fx + nly * rx, ny: nlx * fy + nly * ry, depth };
}

// Raster-Hash für statische Hindernisse.
export class SpatialHash {
  constructor(cell = 96) { this.cell = cell; this.map = new Map(); this.stamp = 0; }
  _key(cx, cy) { return cx * 73856093 ^ cy * 19349663; }
  insert(item, box) {
    const c = this.cell;
    for (let y = Math.floor(box.y / c); y <= Math.floor((box.y + box.h) / c); y++) {
      for (let x = Math.floor(box.x / c); x <= Math.floor((box.x + box.w) / c); x++) {
        const k = this._key(x, y);
        let list = this.map.get(k);
        if (!list) { list = []; this.map.set(k, list); }
        list.push(item);
      }
    }
  }
  query(box, out = []) {
    const c = this.cell, stamp = ++this.stamp;
    out.length = 0;
    for (let y = Math.floor(box.y / c); y <= Math.floor((box.y + box.h) / c); y++) {
      for (let x = Math.floor(box.x / c); x <= Math.floor((box.x + box.w) / c); x++) {
        const list = this.map.get(this._key(x, y));
        if (!list) continue;
        for (const it of list) if (it._stamp !== stamp) { it._stamp = stamp; out.push(it); }
      }
    }
    return out;
  }
}
