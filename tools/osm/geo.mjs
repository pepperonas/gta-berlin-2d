// Geometrie-Helfer für die Kartenpipeline (nur Node, keine Abhängigkeiten).
export { pointInRing, segDist2, delta, undelta } from '../../web/src/geom.js';

// Transversale Mercator-Projektion: liegt in web/src/projection.js (das Spiel braucht sie für Bar-Koordinaten).
export { makeProjection } from '../../web/src/projection.js';

// Umkehrung von makeProjection: Meter (x nach Osten, y nach Norden, relativ zu lat0/lon0) → [lat, lon] in Grad.
// Krüger-Rückwärtsreihe (β-Koeffizienten, 3 Terme) von (ξ, η) nach (ξ′, η′) = (konforme Breite, Länge), dann
// Rückweg von der konformen Breite zur geografischen Breite über die isometrische Breite (Fixpunktiteration).
export function makeInverseProjection(lat0, lon0) {
  const a = 6378137, f = 1 / 298.257222101;
  const n = f / (2 - f), n2 = n * n, n3 = n2 * n;
  const A = a / (1 + n) * (1 + n2 / 4 + n2 * n2 / 64);
  const al = [n / 2 - 2 * n2 / 3 + 5 * n3 / 16, 13 * n2 / 48 - 3 * n3 / 5, 61 * n3 / 240]; // wie makeProjection, nur für y0
  const be = [n / 2 - 2 * n2 / 3 + 37 * n3 / 96, n2 / 48 + n3 / 15, 17 * n3 / 480]; // Krüger-Rückwärtsreihe
  const e = Math.sqrt(f * (2 - f));
  const rad = Math.PI / 180, deg = 180 / Math.PI;

  // y0 wie in makeProjection: Hochwert am Ursprung lat0/lon0 vor der Verschiebung (Ostwert dort ist exakt 0).
  const phi0 = lat0 * rad;
  const t0 = Math.sinh(Math.atanh(Math.sin(phi0)) - e * Math.atanh(e * Math.sin(phi0)));
  const xi0raw = Math.atan2(t0, 1);
  let xi0 = xi0raw;
  for (let j = 1; j <= 3; j++) xi0 += al[j - 1] * Math.sin(2 * j * xi0raw);
  const y0 = A * xi0;

  return (x, y) => {
    const xi = (y + y0) / A, eta = x / A;
    let xip = xi, etap = eta;
    for (let j = 1; j <= 3; j++) {
      xip -= be[j - 1] * Math.sin(2 * j * xi) * Math.cosh(2 * j * eta);
      etap -= be[j - 1] * Math.cos(2 * j * xi) * Math.sinh(2 * j * eta);
    }
    const chi = Math.asin(Math.sin(xip) / Math.cosh(etap));
    const lam = Math.atan2(Math.sinh(etap), Math.cos(xip));
    // Konforme Breite chi -> geografische Breite phi: dieselbe Gleichung wie im Hinweg (isometrische Breite),
    // hier über eine Fixpunktiteration nach der konformen Breite aufgelöst (konvergiert bei GRS80 in wenigen
    // Schritten auf Maschinengenauigkeit).
    const psi = Math.atanh(Math.sin(chi));
    let phi = 2 * Math.atan(Math.exp(psi)) - Math.PI / 2;
    for (let k = 0; k < 6; k++) {
      const s = Math.sin(phi);
      phi = 2 * Math.atan(Math.exp(psi) * Math.pow((1 + e * s) / (1 - e * s), e / 2)) - Math.PI / 2;
    }
    return [phi * deg, lon0 + lam * deg];
  };
}

export function ringArea(r) {
  let s = 0;
  for (let i = 0, j = r.length - 2; i < r.length; j = i, i += 2) s += (r[j] - r[i]) * (r[j + 1] + r[i + 1]);
  return s / 2;
}

// Douglas-Peucker auf flacher Liste; closed: erster/letzter Punkt bleiben.
export function simplify(pts, tol) {
  const n = pts.length / 2;
  if (n <= 2) return pts.slice();
  const keep = new Uint8Array(n); keep[0] = keep[n - 1] = 1;
  const stack = [[0, n - 1]], t2 = tol * tol;
  while (stack.length) {
    const [a, b] = stack.pop();
    const ax = pts[2 * a], ay = pts[2 * a + 1], bx = pts[2 * b], by = pts[2 * b + 1];
    let best = -1, bd = t2;
    for (let i = a + 1; i < b; i++) {
      const d = segDist2local(pts[2 * i], pts[2 * i + 1], ax, ay, bx, by);
      if (d > bd) { bd = d; best = i; }
    }
    if (best >= 0) { keep[best] = 1; stack.push([a, best], [best, b]); }
  }
  const out = [];
  for (let i = 0; i < n; i++) if (keep[i]) out.push(pts[2 * i], pts[2 * i + 1]);
  return out;
}

// Ringe aus Wegstücken zusammensetzen (Multipolygon-Mitglieder). ways: Arrays von Knoten-IDs.
export function joinRings(ways) {
  const rings = [], open = ways.filter((w) => w.length > 1).map((w) => w.slice());
  while (open.length) {
    let cur = open.shift();
    let guard = 0;
    while (cur[0] !== cur[cur.length - 1] && guard++ < 10000) {
      const end = cur[cur.length - 1];
      let k = open.findIndex((w) => w[0] === end || w[w.length - 1] === end);
      if (k < 0) break;
      let w = open.splice(k, 1)[0];
      if (w[0] !== end) w = w.slice().reverse();
      cur = cur.concat(w.slice(1));
    }
    if (cur.length >= 4 && cur[0] === cur[cur.length - 1]) rings.push(cur);
  }
  return rings;
}

// Außenkante der Vereinigung benachbarter Polygone: gemeinsame Kanten heben sich auf.
export function unionOutline(rings, key = (x, y) => `${x},${y}`) {
  const count = new Map(), dir = [];
  for (const r of rings) for (let i = 0; i < r.length - 2; i += 2) {
    const a = key(r[i], r[i + 1]), b = key(r[i + 2], r[i + 3]);
    if (a === b) continue;
    const k = a < b ? a + '|' + b : b + '|' + a;
    count.set(k, (count.get(k) ?? 0) + 1);
    dir.push([a, b, r[i], r[i + 1], r[i + 2], r[i + 3], k]);
  }
  const next = new Map();
  for (const [a, b, ax, ay, bx, by, k] of dir) if (count.get(k) === 1) next.set(a, { b, ax, ay });
  const loops = [];
  while (next.size) {
    const [start] = next.keys();
    const loop = []; let cur = start;
    while (next.has(cur)) { const e = next.get(cur); next.delete(cur); loop.push(e.ax, e.ay); cur = e.b; }
    if (loop.length >= 6) { loop.push(loop[0], loop[1]); loops.push(loop); }
  }
  return loops;
}

function segDist2local(px, py, ax, ay, bx, by) {
  const dx = bx - ax, dy = by - ay, l2 = dx * dx + dy * dy;
  let t = l2 ? ((px - ax) * dx + (py - ay) * dy) / l2 : 0;
  t = t < 0 ? 0 : t > 1 ? 1 : t;
  const qx = ax + t * dx - px, qy = ay + t * dy - py;
  return qx * qx + qy * qy;
}

// Hüllrechteck (S, W, N, O in Grad) über GeoJSON-Polygone, erweitert um marginM Meter.
export function bboxOfFeatures(features, marginM = 250) {
  let s = 90, w = 180, n = -90, e = -180;
  for (const f of features) for (const poly of f.geometry.coordinates) for (const ring of poly) for (const [x, y] of ring) {
    s = Math.min(s, y); n = Math.max(n, y); w = Math.min(w, x); e = Math.max(e, x);
  }
  const dLat = marginM / 110574, dLon = marginM / (111320 * Math.cos(((s + n) / 2) * Math.PI / 180));
  return [s - dLat, w - dLon, n + dLat, e + dLon];
}
