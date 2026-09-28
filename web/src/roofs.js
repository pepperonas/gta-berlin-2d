// Dächer und Fassaden aus OSM-Daten (Dachform, Material, Gebäudetyp, Bezirk; citycodes.js unpackLook) – wo die
// fehlen, geschätzt aus Art, Höhe, Grundriss und Seed:
//  – roofStyle: flach, Berliner Dach (geneigter Streifen zu Straße und Hof, flache Mitte), Sattel-, Walm-, Zelt-,
//    Mansard-, Pult-, Tonnen- und Kuppeldach, Wellblech (Industrie)
//  – roofGeometry: Dachflächen als Vielecke mit Fallrichtung (für die Schattierung nach Sonnenstand), Ziegelreihen,
//    First- und Gratlinien, Gauben. Kompakte Grundrisse bekommen das Dach über dem ausgerichteten Hüllrechteck,
//    verwinkelte (Blockrand mit Hof, L-Form) einen Streifen entlang jeder Außen- und Hofkante mit Gehrung an den
//    Ecken – bei voller Tiefe ist das ein Walmdach über jedem Flügel.
//  – facadeStyle: Altbau, Plattenbau, Neubau (Fensterband), Industrie
//  – roofDecor: Schornsteine, Lichtschächte, Oberlichter, Lüftungsgeräte, Solarmodule, Dachterrassen – nur auf der
//    flachen Dachfläche, ohne Überlappung, ausgerichtet an der Hauptachse des Hauses.
// Alles ist rein rechnerisch und deterministisch; das Ergebnis wird am Gebäude zwischengespeichert (b._roof) und fällt
// mit ihm weg, wenn seine Kachel entladen wird.
import { BUILDING_KIND, ROOF_SHAPE, ROOF_MAT, WALL_MAT, BUILDING_SUB, unpackLook } from './citycodes.js';
import { pointInRings, signedArea } from './geom.js';
import { mulberry32 } from './rng.js';

const K = BUILDING_KIND, RS = ROOF_SHAPE, SUB = BUILDING_SUB;
export const PITCHED = new Set(['gabled', 'hipped', 'pyramidal', 'mansard', 'skillion', 'round']);
// Bezirke (Index + 1 wie im Bitfeld) mit großen Plattenbausiedlungen
const PLATTE_BEZ = new Set([3, 4]); // Lichtenberg, Marzahn-Hellersdorf
const NEUKOELLN = 6;

export const lookOf = (b) => (b._look && b._look.v === b.look ? b._look : (b._look = { ...unpackLook(b.look), v: b.look }));

// Richtung der längsten Außenkante (Hauptachse), in Radiant
export function mainAxis(b) {
  const r = b.rings[0], n = r.length;
  let best = 0, a = 0;
  for (let i = 0; i < n; i += 2) {
    const ex = r[(i + 2) % n] - r[i], ey = r[(i + 3) % n] - r[i + 1], L = ex * ex + ey * ey;
    if (L > best) { best = L; a = Math.atan2(ey, ex); }
  }
  return a;
}

export function footprintM2(b, scale = 10) {
  let a = Math.abs(signedArea(b.rings[0]));
  for (let i = 1; i < b.rings.length; i++) a -= Math.abs(signedArea(b.rings[i]));
  return Math.max(0, a) / (scale * scale);
}

// Ausgerichtetes Hüllrechteck um den Schwerpunkt: Länge t0..t1 entlang der Hauptachse, Breite w0..w1 quer dazu
export function orientedBox(b, a = mainAxis(b)) {
  const ux = Math.cos(a), uy = Math.sin(a), r = b.rings[0];
  let t0 = Infinity, t1 = -Infinity, w0 = Infinity, w1 = -Infinity;
  for (let i = 0; i < r.length; i += 2) {
    const qx = r[i] - b.cx, qy = r[i + 1] - b.cy, t = qx * ux + qy * uy, w = -qx * uy + qy * ux;
    if (t < t0) t0 = t; if (t > t1) t1 = t; if (w < w0) w0 = w; if (w > w1) w1 = w;
  }
  return { a, ux, uy, nx: -uy, ny: ux, t0, t1, w0, w1 };
}

export function roofStyle(b, scale = 10) {
  const r = (b.seed % 1000) / 1000, m = b.meters ?? b.height / scale, area = footprintM2(b, scale), lk = lookOf(b);
  if (b.kind === K.spaeti || b.kind === K.warehouse) return b.kind === K.warehouse ? 'corrugated' : 'flat';
  switch (lk.shape) {
    case RS.flat: return b.kind === K.industrial && lk.rmat !== ROOF_MAT.green ? 'corrugated' : 'flat';
    case RS.gabled: return 'gabled';
    case RS.hipped: return 'hipped';
    case RS.pyramidal: return 'pyramidal';
    case RS.mansard: return 'mansard';
    case RS.skillion: return 'skillion';
    case RS.dome: return 'dome';
    case RS.round: return 'round';
    default: break;
  }
  if (lk.rmat === ROOF_MAT.green || lk.rmat === ROOF_MAT.tar) return 'flat';
  if (b.kind === K.church) return r < 0.85 ? 'gabled' : 'hipped';
  if (b.kind === K.industrial) return r < 0.12 ? 'gabled' : 'corrugated';
  if (b.kind === K.small) return r < 0.45 ? 'gabled' : r < 0.55 ? 'skillion' : 'flat';
  if (lk.sub === SUB.garage) return r < 0.2 ? 'gabled' : 'flat';
  if (b.kind === K.house) {
    if (lk.sub === SUB.terrace) return r < 0.8 ? 'gabled' : 'flat';
    const villa = lk.sub === SUB.villa || (m <= 10 && area < 350);
    if (villa && m <= 14) return r < 0.42 ? 'hipped' : r < 0.84 ? 'gabled' : r < 0.9 ? 'mansard' : 'flat';
    if (PLATTE_BEZ.has(lk.bez) && m >= 14) return 'flat';
    // Nord-Neukölln: Mietshäuser meist mit durchgehendem Steildach über Vorderhaus und Flügeln
    if (lk.bez === NEUKOELLN && m >= 12 && m <= 26) return r < 0.85 ? 'gabled' : r < 0.93 ? 'berlin' : 'flat';
    if (m >= 12 && m <= 26 && r < 0.62) return 'berlin'; // Altbau: Ziegelstreifen zur Straße und zum Hof
    if (m > 9 && m < 12 && r < 0.35) return 'hipped';
  }
  if (b.kind === K.public && m <= 22 && r < 0.3) return r < 0.15 ? 'hipped' : 'mansard';
  return 'flat';
}

export function facadeStyle(b, scale = 10) {
  const r = ((b.seed / 1000) | 0) % 1000 / 1000, m = b.meters ?? b.height / scale, lk = lookOf(b);
  if (b.kind === K.industrial || b.kind === K.warehouse) return 'industry';
  if (lk.wmat === WALL_MAT.glass) return 'modern';
  if (lk.wmat === WALL_MAT.concrete && b.kind === K.house && m >= 14) return 'platte';
  if (b.kind === K.public) return r < 0.6 ? 'modern' : 'altbau';
  if (b.kind === K.house) {
    if (PLATTE_BEZ.has(lk.bez) && m >= 14 && lk.sub !== SUB.villa) return r < 0.85 ? 'platte' : 'modern';
    if (m > 30) return r < 0.7 ? 'platte' : 'modern';
    if (m > 24) return r < 0.4 ? 'platte' : r < 0.7 ? 'modern' : 'altbau';
    return r < 0.8 ? 'altbau' : 'modern';
  }
  return 'altbau';
}

// --- Dachflächen ---------------------------------------------------------------------------------------------------

const COURSE = 7;      // Abstand der Ziegelreihen, px
const MAX_COURSE = 320; // Linien je Dach (größere Dächer bekommen weitere Reihen)

// Streifen entlang der Kanten eines Rings nach innen (Tiefe d), mit Gehrung: [{ pts: [p0, p1, q1, q0], nx, ny }]
// nx, ny = Fallrichtung (nach außen). Innenseite je Ring über eine Punktprobe bestimmt (Löcher = Höfe umgekehrt).
// Abstand von (x, y) in Richtung (nx, ny) bis zur nächsten Wand des Grundrisses (Strahl gegen alle Ringe), sonst Infinity
function rayToWall(rings, x, y, nx, ny) {
  let best = Infinity;
  for (const r of rings) for (let i = 0; i < r.length; i += 2) {
    const ax = r[i], ay = r[i + 1], ex = r[(i + 2) % r.length] - ax, ey = r[(i + 3) % r.length] - ay;
    const den = nx * ey - ny * ex;
    if (Math.abs(den) < 1e-9) continue;
    const t = ((ax - x) * ey - (ay - y) * ex) / den, u = ((ax - x) * ny - (ay - y) * nx) / den;
    if (t > 0.5 && u >= 0 && u <= 1 && t < best) best = t;
  }
  return best;
}

// depthOf(D): Dachtiefe einer Kante aus der Hausdicke D dahinter (gemessen per Strahl von der Kante nach innen, Median
// aus drei Punkten) – so bekommt jeder Flügel seinen eigenen First, auch bei Vorderhaus mit schmalen Seitenflügeln.
function bandFacets(ring, depthOf, inside, out, allRings = [ring]) {
  const n = ring.length / 2;
  if (n < 3) return;
  // Innenseite: links oder rechts der Kanten?
  let side = 0;
  for (let i = 0; i < n && !side; i++) {
    const x0 = ring[2 * i], y0 = ring[2 * i + 1], x1 = ring[(2 * i + 2) % ring.length], y1 = ring[(2 * i + 3) % ring.length];
    const L = Math.hypot(x1 - x0, y1 - y0);
    if (L < 4) continue;
    const mx = (x0 + x1) / 2, my = (y0 + y1) / 2, lx = -(y1 - y0) / L * 1.5, ly = (x1 - x0) / L * 1.5;
    const a = inside(mx + lx, my + ly), c = inside(mx - lx, my - ly);
    if (a !== c) side = a ? 1 : -1;
  }
  if (!side) return;
  const nin = [];
  for (let i = 0; i < n; i++) {
    const x0 = ring[2 * i], y0 = ring[2 * i + 1], x1 = ring[(2 * i + 2) % ring.length], y1 = ring[(2 * i + 3) % ring.length];
    const L = Math.hypot(x1 - x0, y1 - y0) || 1;
    nin.push(-(y1 - y0) / L * side, (x1 - x0) / L * side);
  }
  const de = [];
  for (let i = 0; i < n; i++) {
    const x0 = ring[2 * i], y0 = ring[2 * i + 1], x1 = ring[(2 * i + 2) % ring.length], y1 = ring[(2 * i + 3) % ring.length];
    const D = [0.25, 0.5, 0.75].map((f) => rayToWall(allRings, x0 + (x1 - x0) * f, y0 + (y1 - y0) * f, nin[2 * i], nin[2 * i + 1])).sort((a, c) => a - c)[1];
    de.push(depthOf(D));
  }
  const q = [];
  for (let i = 0; i < n; i++) {
    const j = (i + n - 1) % n, ax = nin[2 * j], ay = nin[2 * j + 1], bx = nin[2 * i], by = nin[2 * i + 1];
    const d = Math.min(de[j], de[i]); // an der Ecke gilt der flachere Flügel (Grat auf der Winkelhalbierenden)
    let mx = ax + bx, my = ay + by;
    const dot = 1 + ax * bx + ay * by;
    if (dot < 0.15) { mx = bx; my = by; } else { mx /= dot; my /= dot; }
    const ml = Math.hypot(mx, my);
    if (ml > 2.2) { mx *= 2.2 / ml; my *= 2.2 / ml; }
    q.push(ring[2 * i] + mx * d, ring[2 * i + 1] + my * d);
  }
  for (let i = 0; i < n; i++) {
    const k = (i + 1) % n;
    const x0 = ring[2 * i], y0 = ring[2 * i + 1], x1 = ring[2 * k], y1 = ring[2 * k + 1];
    if (Math.hypot(x1 - x0, y1 - y0) < 1) continue;
    let ax = q[2 * i], ay = q[2 * i + 1], bx = q[2 * k], by = q[2 * k + 1];
    // Spitze Ecke: der Gehrungspunkt kann auf der Außenseite dieser Traufe liegen – dann senkrecht einrücken
    const nx = nin[2 * i], ny = nin[2 * i + 1], dd = Math.min(de[i], de[(i + n - 1) % n], de[k]);
    if ((ax - x0) * nx + (ay - y0) * ny < 0.2 * dd) { ax = x0 + nx * dd; ay = y0 + ny * dd; }
    if ((bx - x1) * nx + (by - y1) * ny < 0.2 * dd) { bx = x1 + nx * dd; by = y1 + ny * dd; }
    // Kante kürzer als die doppelte Tiefe: die Innenkante kehrt sich um (Fliege) – dann ist es ein Walmdreieck
    if ((bx - ax) * (x1 - x0) + (by - ay) * (y1 - y0) < 0) { ax = bx = (ax + bx) / 2; ay = by = (ay + by) / 2; }
    out.push({ pts: [x0, y0, x1, y1, bx, by, ax, ay], nx: -nin[2 * i], ny: -nin[2 * i + 1], edge: true });
  }
}

const boxPt = (o, b, t, w) => [b.cx + o.ux * t + o.nx * w, b.cy + o.uy * t + o.ny * w];
function boxRing(o, b, m = 0) {
  return [...boxPt(o, b, o.t0 - m, o.w0 - m), ...boxPt(o, b, o.t1 + m, o.w0 - m), ...boxPt(o, b, o.t1 + m, o.w1 + m), ...boxPt(o, b, o.t0 - m, o.w1 + m)];
}

// Ziegelreihen in einer Fläche [p0, p1, q1, q0]: Linien parallel zur Traufe p0–p1
function coursesOf(f, sp, out) {
  const [x0, y0, x1, y1, x2, y2, x3, y3] = f.pts;
  const depth = Math.hypot((x3 + x2) / 2 - (x0 + x1) / 2, (y3 + y2) / 2 - (y0 + y1) / 2);
  if (depth < sp * 1.5) return;
  for (let s = sp; s < depth - sp * 0.4; s += sp) {
    const t = s / depth;
    const ax = x0 + (x3 - x0) * t, ay = y0 + (y3 - y0) * t, bx = x1 + (x2 - x1) * t, by = y1 + (y2 - y1) * t;
    if (Math.hypot(bx - ax, by - ay) > 3) out.push(ax, ay, bx, by);
  }
}

// Dachgeometrie (rein): { facets, courses, ridges, dormers, dome, flatCenter }
export function roofGeometry(b, style = roofStyle(b), scale = 10) {
  const g = { facets: [], courses: [], ridges: [], dormers: [], dome: null, flatCenter: false };
  if (!PITCHED.has(style) && style !== 'berlin' && style !== 'dome') return g;
  const o = orientedBox(b), L = o.t1 - o.t0, W = o.w1 - o.w0, halfW = W / 2, wm = (o.w0 + o.w1) / 2;
  const boxArea = L * W, area = footprintM2(b, scale) * scale * scale;
  const box = b.rings.length === 1 && boxArea > 0 && area / boxArea >= 0.82;
  const rnd = mulberry32(b.seed ^ 0x2c1b3c6d);
  if (style === 'dome') {
    g.dome = { x: b.cx + o.ux * (o.t0 + o.t1) / 2 + o.nx * wm, y: b.cy + o.uy * (o.t0 + o.t1) / 2 + o.ny * wm, r: Math.min(L, W) / 2 * 0.92 };
    return g;
  }
  if (box && (style === 'gabled' || style === 'round' || style === 'skillion')) {
    const m = 2; // über den Rand hinaus, der Grundriss schneidet ab
    if (style === 'skillion') {
      const down = rnd() < 0.5 ? -1 : 1;
      const f = down < 0
        ? { pts: [...boxPt(o, b, o.t0 - m, o.w0 - m), ...boxPt(o, b, o.t1 + m, o.w0 - m), ...boxPt(o, b, o.t1 + m, o.w1 + m), ...boxPt(o, b, o.t0 - m, o.w1 + m)], nx: -o.nx, ny: -o.ny }
        : { pts: [...boxPt(o, b, o.t1 + m, o.w1 + m), ...boxPt(o, b, o.t0 - m, o.w1 + m), ...boxPt(o, b, o.t0 - m, o.w0 - m), ...boxPt(o, b, o.t1 + m, o.w0 - m)], nx: o.nx, ny: o.ny };
      g.facets.push(f);
    } else {
      g.facets.push({ pts: [...boxPt(o, b, o.t0 - m, o.w0 - m), ...boxPt(o, b, o.t1 + m, o.w0 - m), ...boxPt(o, b, o.t1 + m, wm), ...boxPt(o, b, o.t0 - m, wm)], nx: -o.nx, ny: -o.ny, edge: true });
      g.facets.push({ pts: [...boxPt(o, b, o.t1 + m, o.w1 + m), ...boxPt(o, b, o.t0 - m, o.w1 + m), ...boxPt(o, b, o.t0 - m, wm), ...boxPt(o, b, o.t1 + m, wm)], nx: o.nx, ny: o.ny, edge: true });
      g.ridges.push(...boxPt(o, b, o.t0, wm), ...boxPt(o, b, o.t1, wm));
    }
  } else {
    let depthOf, rings;
    const lo = 0.8 * scale;
    if (box) {
      rings = [boxRing(o, b)];
      const d = Math.max(lo, style === 'mansard' ? Math.min(halfW * 0.35, 2.8 * scale) : style === 'berlin' ? Math.min(halfW * 0.6, 4.5 * scale) : halfW);
      depthOf = () => d;
    } else {
      rings = b.rings;
      // Tiefe je Kante aus der Hausdicke dahinter: Steildach bis zum First in der Mitte (sehr tiefe Häuser ab 16 m
      // behalten eine flache Mitte), Berliner Dach und Mansarde als Streifen
      const f = style === 'mansard' ? [0.22, 2.8] : style === 'berlin' ? [0.38, 4.5] : [0.5, 8];
      depthOf = (D) => Math.max(lo, Math.min(Number.isFinite(D) ? D * f[0] : f[1] * scale, f[1] * scale, halfW));
    }
    const inside = box ? (x, y) => pointInRings(x, y, rings) : (x, y) => pointInRings(x, y, b.rings);
    for (const r of rings) bandFacets(r, depthOf, inside, g.facets, rings);
    for (const f of g.facets) { const p = f.pts; g.ridges.push(p[0], p[1], p[6], p[7], p[6], p[7], p[4], p[5]); } // Grate und Innenkante
    g.flatCenter = style === 'berlin' || style === 'mansard';
  }
  // Ziegelreihen (Abstand wächst bei sehr großen Dächern, damit es bei MAX_COURSE Linien bleibt)
  let sp = style === 'round' ? COURSE * 0.8 : COURSE;
  for (let tries = 0; tries < 4; tries++) {
    g.courses.length = 0;
    for (const f of g.facets) coursesOf(f, sp, g.courses);
    if (g.courses.length / 4 <= MAX_COURSE) break;
    sp *= 1.6;
  }
  if (g.courses.length / 4 > MAX_COURSE) g.courses.length = MAX_COURSE * 4;
  // Gauben auf Wohnhäusern: auf langen Traufseiten, zur Hälfte der Tiefe, ganz im Grundriss
  if (b.kind === K.house && style !== 'skillion' && style !== 'round' && rnd() < (style === 'berlin' ? 0.45 : 0.55)) {
    const gw = 1.6 * scale, gl = 1.5 * scale;
    for (const f of g.facets) {
      if (!f.edge) continue;
      const [x0, y0, x1, y1, x2, y2, x3, y3] = f.pts;
      const len = Math.hypot(x1 - x0, y1 - y0), depth = Math.hypot((x3 + x2) / 2 - (x0 + x1) / 2, (y3 + y2) / 2 - (y0 + y1) / 2);
      if (len < 7 * scale || depth < 2.4 * scale) continue;
      const ux = (x1 - x0) / len, uy = (y1 - y0) / len, a = Math.atan2(uy, ux);
      const step = (4.5 + rnd() * 2) * scale, n = Math.floor((len - 3 * scale) / step);
      const off = (len - (n - 1) * step) / 2, t = 0.45;
      for (let k = 0; k < n; k++) {
        const s = off + k * step;
        const ex = x0 + ux * s, ey = y0 + uy * s;
        const x = ex + (-f.nx) * depth * t, y = ey + (-f.ny) * depth * t;
        let ok = true;
        for (const [u, v] of [[1, 1], [-1, 1], [-1, -1], [1, -1]]) {
          if (!pointInRings(x + ux * u * gw / 2 - f.nx * v * gl / 2, y + uy * u * gw / 2 - f.ny * v * gl / 2, b.rings)) { ok = false; break; }
        }
        if (ok && !g.dormers.some((q) => Math.hypot(q.x - x, q.y - y) < gw * 1.6)) g.dormers.push({ x, y, a, w: gw, l: gl, nx: f.nx, ny: f.ny });
        if (g.dormers.length >= 24) break;
      }
      if (g.dormers.length >= 24) break;
    }
  }
  return g;
}

// --- Aufbauten -----------------------------------------------------------------------------------------------------

// Maße in m (Länge entlang der Hauptachse × Breite)
const DECOR = {
  chimney: [0.7, 0.7], shaft: [1.8, 1.6], skylight: [1.4, 0.9], ac: [2.2, 1.4], solar: [5, 2.2], terrace: [4.5, 3.5],
};

// Abstand eines Punkts zur nächsten Kante des Grundrisses
function edgeDist(b, x, y) {
  let best = Infinity;
  for (const r of b.rings) for (let i = 0; i < r.length; i += 2) {
    const ax = r[i], ay = r[i + 1], bx = r[(i + 2) % r.length], by = r[(i + 3) % r.length];
    const ex = bx - ax, ey = by - ay, l2 = ex * ex + ey * ey || 1;
    const t = Math.max(0, Math.min(1, ((x - ax) * ex + (y - ay) * ey) / l2));
    best = Math.min(best, Math.hypot(ax + ex * t - x, ay + ey * t - y));
  }
  return best;
}

export function roofDecor(b, scale = 10, style = roofStyle(b, scale), geo = null) {
  const area = footprintM2(b, scale);
  const rnd = mulberry32(b.seed ^ 0x5bd1e995);
  const a = mainAxis(b), ca = Math.cos(a), sa = Math.sin(a);
  if (b.kind === K.small || area < 40) return [];
  if (PITCHED.has(style) || style === 'dome') {
    // Schornsteine auf dem First bzw. den Graten (Wohnhäuser); sonst nichts
    if (b.kind !== K.house || style === 'skillion' || style === 'round' || !geo) return [];
    const out = [], rid = geo.ridges, n = Math.min(3, 1 + Math.floor(area / 160 * rnd()));
    const l = DECOR.chimney[0] * scale;
    for (let k = 0; k < n && rid.length >= 4; k++) {
      const s = 4 * Math.floor(rnd() * (rid.length / 4)), u = 0.2 + rnd() * 0.6;
      const x = rid[s] + (rid[s + 2] - rid[s]) * u, y = rid[s + 1] + (rid[s + 3] - rid[s + 1]) * u;
      const h = l / 2 + 0.3 * scale;
      if ([[1, 1], [-1, 1], [-1, -1], [1, -1]].some(([u, v]) => !pointInRings(x + ca * u * h - sa * v * h, y + sa * u * h + ca * v * h, b.rings))) continue;
      if (out.some((o) => Math.hypot(o.x - x, o.y - y) < 3 * scale)) continue;
      out.push({ t: 'chimney', x, y, l, w: l, a });
    }
    return out;
  }
  if (area < 60) return [];
  const wish = [];
  const n = (per, max) => Math.min(max, Math.floor(area / per + rnd()));
  if (style === 'corrugated') { wish.push(...Array(n(250, 8)).fill('ac'), ...Array(n(400, 6)).fill('skylight')); if (rnd() < 0.2) wish.push('solar', 'solar'); }
  else {
    wish.push(...Array(n(55, 14)).fill('chimney'), ...Array(n(260, 4)).fill('shaft'), ...Array(n(180, 5)).fill('skylight'));
    if (b.kind === K.public) wish.push(...Array(n(300, 5)).fill('ac'));
    if (rnd() < 0.15) wish.push('solar', 'solar', 'solar');
    if (rnd() < 0.1) wish.push('terrace');
  }
  // Berliner Dach: nur auf der flachen Mitte, hinter dem geneigten Streifen
  const band = style === 'berlin' && geo?.facets.length ? Math.min(4.5 * scale, Math.max(...geo.facets.map((f) => Math.hypot(f.pts[6] - f.pts[0], f.pts[7] - f.pts[1])))) : 0;
  const { x, y, w, h } = b.bbox, out = [];
  for (const t of wish) {
    const [lm, wm] = DECOR[t], l = lm * scale, wd = wm * scale;
    for (let tries = 0; tries < 12; tries++) {
      const px = x + rnd() * w, py = y + rnd() * h;
      // ganze Fläche (plus Rand) im Grundriss? 5 × 5 Punkte – nur die Ecken zu prüfen reicht bei eingebuchteten
      // Grundrissen nicht (eine Einbuchtung kann zwischen den Ecken liegen)
      const hx = l / 2 + 0.4 * scale, hy = wd / 2 + 0.4 * scale;
      let inside = true;
      for (let i = 0; i <= 4 && inside; i++) for (let j = 0; j <= 4 && inside; j++) {
        const u = -hx + hx * i / 2, v = -hy + hy * j / 2;
        inside = pointInRings(px + ca * u - sa * v, py + sa * u + ca * v, b.rings);
      }
      if (!inside) continue;
      if (band && edgeDist(b, px, py) < band + Math.max(l, wd) / 2 + 0.3 * scale) continue;
      if (out.some((o) => Math.hypot(o.x - px, o.y - py) < (Math.max(o.l, o.w) + Math.max(l, wd)) / 2 + 0.5 * scale)) continue;
      out.push({ t, x: px, y: py, l, w: wd, a });
      break;
    }
  }
  return out;
}

// Alles zusammen, einmal je Gebäude
export function roofOf(b, scale = 10) {
  if (b._roof) return b._roof;
  const style = roofStyle(b, scale), geo = roofGeometry(b, style, scale);
  return (b._roof = { style, facade: facadeStyle(b, scale), axis: mainAxis(b), geo, decor: roofDecor(b, scale, style, geo) });
}
