// Kreuzungsflächen mit echten Ecken (Darstellung der nativen Fassung; die Simulation nutzt weiter die runden
// Kreuzungsscheiben aus junctionsOf). Vorbild: osm2streets / A/B Street (Apache-2.0,
// https://a-b-street.github.io/docs/tech/map/geometry/index.html):
//   1. Jede Straße am Knoten als Band (halbe Fahrbahnbreite je Seite) in Richtung vom Knoten weg.
//   2. Straßen nach Winkel sortieren; die linke Kante einer Straße mit der rechten der nächsten schneiden → Ecke.
//   3. Ecke mit einem Bordsteinbogen abrunden (Radius nach Straßenart), die Berührpunkte bestimmen, wie weit jede
//      Straße am Knoten gekürzt wird (senkrecht, entlang ihrer Linie).
//   4. Fläche = Mündungen der Straßen + Eckbögen, nach Winkel geordnet. Dazu je Ecke ein Zug für Rinne, Bordstein
//      und Gehweg, der an den Mündungen genau an die gekürzten Straßenstreifen anschließt.
// Alle Werte in Kartenpixeln (S px je Meter). Rein, deterministisch, ohne Abhängigkeiten.

/// Bordsteinradius je Straßenklasse (m): Hauptstraßen weit, Wohnstraßen eng
export const CORNER_R = (c) => (c <= 3 ? 6 : c <= 5 ? 5 : c <= 8 ? 3.5 : 2.5);
/// Knick, ab dem ein Knoten mit genau zwei Straßen eine Fläche bekommt (rad); flachere Knicke schließt die Gehrung
/// der Straßenstreifen selbst
export const BEND_MIN = 0.45;
/// Gehweg- und Bordsteinmaß wie im Rust-Lader (mesh.rs road_mesh): nur bis zu dieser Klasse
export const SIDEWALK_MAX_CLASS = 9;
/// längste Ecke (Gehrung) als Vielfaches der breitesten halben Fahrbahn – spitzere Winkel werden abgeschrägt
const MITER_MAX = 4;
/// höchstens dieser Anteil einer Straße wird am Knoten weggekürzt
const TRIM_SHARE = 0.45;
/// Knoten, die über eine kürzere Straße verbunden sind, bilden eine gemeinsame Fläche (m) – sonst überlappen sich
/// ihre Flächen und dazwischen bleibt ein Stummel mit eigenem Belag und Bordstein stehen
export const MERGE_M = 8;
/// größte Ausdehnung einer solchen Knotengruppe (m): Ketten kurzer Stücke (Einfahrten im Abstand weniger Meter)
/// sollen nicht zu einer langen Fläche zusammenwachsen
export const MERGE_SPAN_M = 14;
/// Nachbararme mit kleinerem Winkel (rad) bekommen eine gerade Bordsteinnase statt einer Ecke
export const NOSE_PHI = 0.6;
/// Richtungsfahrbahnen derselben Straße: Knoten, deren Querverbindung kürzer ist (m), bilden eine Fläche …
export const DUAL_M = 35;
/// … und die ganze Gruppe darf so groß werden (m)
export const DUAL_SPAN_M = 55;
/// Wendehammer: Radius (m), falls OSM keinen Durchmesser nennt
export const TURN_R_M = 8;

const norm = (a) => { while (a <= -Math.PI) a += 2 * Math.PI; while (a > Math.PI) a -= 2 * Math.PI; return a; };

/// Punkt und Richtung auf einem Zug [x0, y0, x1, y1, …] bei Bogenlänge s (von vorn).
export function pointAt(pts, s) {
  let acc = 0;
  for (let i = 0; i + 3 < pts.length; i += 2) {
    const dx = pts[i + 2] - pts[i], dy = pts[i + 3] - pts[i + 1], l = Math.hypot(dx, dy);
    if (l === 0) continue;
    if (acc + l >= s || i + 4 >= pts.length) {
      const u = Math.min(1, Math.max(0, (s - acc) / l));
      return { x: pts[i] + dx * u, y: pts[i + 1] + dy * u, ux: dx / l, uy: dy / l };
    }
    acc += l;
  }
  return { x: pts[0], y: pts[1], ux: 1, uy: 0 };
}
export function lengthOf(pts) {
  let l = 0;
  for (let i = 0; i + 3 < pts.length; i += 2) l += Math.hypot(pts[i + 2] - pts[i], pts[i + 3] - pts[i + 1]);
  return l;
}

/// Fläche eines Knotens oder einer Knotengruppe. `arms`: je Straße { key, pts (vom Knoten weg, erster Punkt = ihr
/// Knoten – bei Gruppen verschiedene), h (halbe Breite px), c (Klasse) }. Liefert { ring: [x, y, …], corners: [[x, y, …], …], trims: Map key → px } oder null, wenn der
/// Knoten keine eigene Fläche braucht oder sie entartet wäre.
export function plateOf(arms, S) {
  if (arms.length < 2) return null;
  const a = arms.map((r) => {
    const L = lengthOf(r.pts);
    const p = pointAt(r.pts, Math.min(5 * S, L * 0.5));
    const [vx, vy] = [r.pts[0], r.pts[1]];
    const dx = p.x - vx, dy = p.y - vy, l = Math.hypot(dx, dy) || 1;
    return { ...r, L, vx, vy, dx: dx / l, dy: dy / l, th: Math.atan2(dy, dx), need: 0 };
  });
  if (a.length === 2) {
    const bend = Math.PI - Math.abs(norm(a[1].th - a[0].th));
    if (bend < BEND_MIN && Math.abs(a[0].h - a[1].h) < 0.5 * S) return null;
  }
  // Reihenfolge um die Mitte der Knoten (bei einem Knoten = dessen Lage, dann wie die Richtung)
  const mx = a.reduce((m, r) => m + r.vx, 0) / a.length, my = a.reduce((m, r) => m + r.vy, 0) / a.length;
  for (const r of a) {
    const p = pointAt(r.pts, Math.min(5 * S, r.L * 0.5));
    r.ord = Math.hypot(r.vx - mx, r.vy - my) < 0.5 ? r.th : Math.atan2(p.y - my, p.x - mx);
  }
  a.sort((p, q) => p.ord - q.ord || p.key - q.key);
  const vx = Math.round(mx), vy = Math.round(my);
  // erst mit Bordsteinbögen, sonst mit geraden Ecken; geht beides nicht auf, die Hülle der Mündungen
  return corners(a, S, true) ?? corners(a, S, false) ?? hullPlate(a, S, vx, vy);
}

/// Eckzüge und Kürzungen für sortierte Arme `a`; `arcs`: Bordsteinbögen (sonst gerade Ecken). null bei Selbstschnitt.
function corners(a, S, arcs) {
  const n = a.length, hMax = Math.max(...a.map((r) => r.h));
  const vx = Math.round(a.reduce((m, r) => m + r.vx, 0) / n), vy = Math.round(a.reduce((m, r) => m + r.vy, 0) / n);
  for (const r of a) r.need = 0;
  // Ecken zwischen Straße i (linke Kante) und der nächsten j (rechte Kante)
  const cs = [];
  for (let i = 0; i < n; i++) {
    const r = a[i], q = a[(i + 1) % n];
    // fast parallele Nachbarn (Richtungsfahrbahnen einer Straße, spitzes Y): gerade Bordsteinnase zwischen den
    // Mündungen – als Ecke gerechnet liefen Bordstein und Gehweg als Balken quer durch die Kreuzung
    if (Math.abs(norm(q.th - r.th)) < NOSE_PHI) { cs.push({ i, j: (i + 1) % n, nose: true }); continue; }
    let phi = q.th - r.th; if (phi <= 0) phi += 2 * Math.PI; // Winkel von i nach j (mathematisch positiv)
    const ni = [-r.dy, r.dx], nj = [-q.dy, q.dx];
    if (phi >= Math.PI - 0.02) { cs.push({ i, j: (i + 1) % n, convex: true }); continue; }
    // L_i: v_i + h_i·n_i + t·d_i  ∩  R_j: v_j − h_j·n_j + s·d_j  (v_i = v_j bei einem Knoten)
    const ex = q.vx - r.vx - q.h * nj[0] - r.h * ni[0], ey = q.vy - r.vy - q.h * nj[1] - r.h * ni[1]; // R_j0 − L_i0
    const det = r.dx * (-q.dy) - r.dy * (-q.dx);
    let t = (ex * (-q.dy) - ey * (-q.dx)) / det;
    let s = (r.dx * ey - r.dy * ex) / det;
    const cap = MITER_MAX * hMax;
    let R = arcs ? Math.max(0, CORNER_R(Math.min(r.c, q.c)) * S) : 0;
    const bevel = !(t >= -1 && s >= -1) || t > cap || s > cap;
    if (bevel) { t = Math.min(Math.max(t, 0), cap); s = Math.min(Math.max(s, 0), cap); R = 0; }
    // Bogen: Berührpunkte im Abstand R / tan(φ/2) vom Eckpunkt, gedeckelt
    // Eckabstand + Bogen zusammen höchstens `cap` (bei spitzen Winkeln wird der Bogen kleiner)
    let tan = R > 0 ? R / Math.tan(phi / 2) : 0;
    const room = Math.max(0, cap - Math.max(t, s));
    if (tan > room) { tan = room; R = tan * Math.tan(phi / 2); }
    r.need = Math.max(r.need, t + tan);
    q.need = Math.max(q.need, s + tan);
    cs.push({ i, j: (i + 1) % n, t, s, tan, R, phi });
  }
  const mouth = mouths(a, S);
  const ring = [], cornerLines = [];
  for (const co of cs) {
    const r = a[co.i], q = a[co.j];
    const mi = mouth[co.i], mj = mouth[co.j];
    ring.push(...mi.R, ...mi.L);
    const line = [...mi.L];
    if (co.nose) {
      // gerade von Mündung zu Mündung
    } else if (co.convex) {
      // gerade oder stumpf nach außen: an den Knoten heran, um ihn herum
      const li0 = [r.vx - r.h * r.dy, r.vy + r.h * r.dx], rj0 = [q.vx + q.h * q.dy, q.vy - q.h * q.dx];
      line.push(...li0);
      if (Math.hypot(li0[0] - rj0[0], li0[1] - rj0[1]) > 0.5) line.push(...rj0);
    } else {
      const ni = [-r.dy, r.dx], nj = [-q.dy, q.dx];
      const ti = co.t + co.tan, sj = co.s + co.tan;
      const Pi = [r.vx + r.h * ni[0] + ti * r.dx, r.vy + r.h * ni[1] + ti * r.dy];
      const Pj = [q.vx - q.h * nj[0] + sj * q.dx, q.vy - q.h * nj[1] + sj * q.dy];
      line.push(...Pi);
      if (co.R > 0.5) {
        // Mittelpunkt: vom Berührpunkt auf i um R nach links (in den Block hinein)
        const cx = Pi[0] + co.R * ni[0], cy = Pi[1] + co.R * ni[1];
        arc(cx, cy, co.R, Math.atan2(Pi[1] - cy, Pi[0] - cx), Math.atan2(Pj[1] - cy, Pj[0] - cx), line);
      }
      line.push(...Pj);
    }
    line.push(...mj.R);
    // in den Ring ohne Anfangs- und Endpunkt (die sind die Mündungen)
    ring.push(...line.slice(2, -2));
    cornerLines.push(dedupe(line));
  }
  const r2 = dedupe(ring);
  if (r2.length < 6 || selfIntersects(r2)) return null;
  const trims = new Map(a.map((r) => [r.key, r.T]));
  return { ring: r2, corners: cornerLines, trims, x: vx, y: vy };
}

/// Kürzung je Straße aus dem Bedarf der Ecken (gedeckelt: kurze Straßen behalten ihre Mitte) und die Mündungen
/// (senkrecht über die gekürzte Straße; linke Normale (−uy, ux) → links = p + h·(−uy, ux)).
function mouths(a, S) {
  for (const r of a) r.T = Math.min(Math.max(r.need, 0.5 * S), r.L * TRIM_SHARE);
  return a.map((r) => {
    const p = pointAt(r.pts, r.T);
    return { R: [p.x + r.h * p.uy, p.y - r.h * p.ux], L: [p.x - r.h * p.uy, p.y + r.h * p.ux] };
  });
}

/// Bogen um (cx, cy) von a0 nach a1 (kürzerer Weg) ohne Endpunkte an `out` anhängen.
function arc(cx, cy, rad, a0, a1, out) {
  const d = norm(a1 - a0);
  const steps = Math.max(2, Math.ceil(Math.abs(d) / 0.3));
  for (let k = 1; k < steps; k++) { const ang = a0 + d * k / steps; out.push(cx + rad * Math.cos(ang), cy + rad * Math.sin(ang)); }
}

/// Letzte Rückfallstufe: Ecken gerade von Mündung zu Mündung; gefüllt wird die konvexe Hülle aller Mündungspunkte
/// (`fill`), weil die aneinandergereihten Eckzüge sich hier schneiden. Kürzung wie bei geraden Ecken.
function hullPlate(a, S, vx, vy) {
  const mouth = mouths(a, S);
  const n = a.length;
  const cornerLines = [];
  for (let i = 0; i < n; i++) cornerLines.push([...mouth[i].L, ...mouth[(i + 1) % n].R]);
  const fill = convexHull(mouth.flatMap((m) => [m.R, m.L]));
  if (fill.length < 6) return null;
  const trims = new Map(a.map((r) => [r.key, r.T]));
  return { ring: fill, corners: cornerLines, fill, trims, x: vx, y: vy };
}

/// Konvexe Hülle (Andrew) von Punkten [[x, y], …] als [x, y, …] gegen den Uhrzeigersinn (mathematisch).
export function convexHull(points) {
  const p = points.slice().sort((u, v) => u[0] - v[0] || u[1] - v[1]);
  if (p.length < 3) return p.flat();
  const cross = (o, u, v) => (u[0] - o[0]) * (v[1] - o[1]) - (u[1] - o[1]) * (v[0] - o[0]);
  const lower = [], upper = [];
  for (const q of p) { while (lower.length >= 2 && cross(lower[lower.length - 2], lower[lower.length - 1], q) <= 0) lower.pop(); lower.push(q); }
  for (const q of p.reverse()) { while (upper.length >= 2 && cross(upper[upper.length - 2], upper[upper.length - 1], q) <= 0) upper.pop(); upper.push(q); }
  return [...lower.slice(0, -1), ...upper.slice(0, -1)].flat();
}

/// Ende einer Straße ohne Fortsetzung: Wendehammer (Kreis um den Knoten, `turnR` px) oder Bordstein quer über das
/// Ende, um den der Gehweg herumläuft. Der Eckzug läuft von der linken Mündung um das Ende zur rechten.
export function endPlate(arm, S, turnR = 0) {
  const L = lengthOf(arm.pts);
  const [vx, vy] = [arm.pts[0], arm.pts[1]];
  const d = pointAt(arm.pts, Math.min(3 * S, L * 0.5));
  let dx = d.x - vx, dy = d.y - vy; const l = Math.hypot(dx, dy) || 1; dx /= l; dy /= l;
  const h = arm.h;
  const rc = turnR > 0 ? Math.max(turnR, h + 2 * S) : 0;
  const T = Math.min(rc > 0 ? Math.sqrt(rc * rc - h * h) : 1.2 * S, L * TRIM_SHARE);
  if (T < 0.3 * S) return null;
  const p = pointAt(arm.pts, T);
  const Lm = [p.x - h * p.uy, p.y + h * p.ux], Rm = [p.x + h * p.uy, p.y - h * p.ux];
  const line = [...Lm];
  if (rc > 0) {
    // Kreis um den Knoten: von der linken Mündung über die Rückseite zur rechten
    const a0 = Math.atan2(Lm[1] - vy, Lm[0] - vx), a1 = Math.atan2(Rm[1] - vy, Rm[0] - vx);
    let sweep = a1 - a0; while (sweep <= 0) sweep += 2 * Math.PI;
    const steps = Math.max(8, Math.ceil(sweep / 0.2));
    for (let k = 1; k < steps; k++) { const ang = a0 + sweep * k / steps; line.push(vx + rc * Math.cos(ang), vy + rc * Math.sin(ang)); }
  } else {
    // gerades Ende am Knoten
    line.push(vx - h * dy, vy + h * dx, vx + h * dy, vy - h * dx);
  }
  line.push(...Rm);
  const ring = dedupe([...Rm, ...line]);
  if (ring.length < 6 || selfIntersects(ring)) return null;
  return { ring, corners: [dedupe(line)], trims: new Map([[arm.key, T]]), x: Math.round(vx), y: Math.round(vy) };
}

/// Aufeinanderfolgende Doppelpunkte (< 0,5 px) entfernen.
function dedupe(pts) {
  const out = [];
  for (let i = 0; i < pts.length; i += 2) {
    const k = out.length;
    if (k >= 2 && Math.hypot(pts[i] - out[k - 2], pts[i + 1] - out[k - 1]) < 0.5) continue;
    out.push(pts[i], pts[i + 1]);
  }
  return out;
}

/// Schneidet sich der geschlossene Ring selbst (nicht benachbarte Kanten)?
export function selfIntersects(ring) {
  const n = ring.length / 2;
  const P = (k) => [ring[2 * (k % n)], ring[2 * (k % n) + 1]];
  const cross = (o, a, b) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  for (let i = 0; i < n; i++) {
    const a1 = P(i), a2 = P(i + 1);
    for (let j = i + 2; j < n; j++) {
      if (i === 0 && j === n - 1) continue;
      const b1 = P(j), b2 = P(j + 1);
      const d1 = cross(b1, b2, a1), d2 = cross(b1, b2, a2), d3 = cross(a1, a2, b1), d4 = cross(a1, a2, b2);
      if (((d1 > 1e-9 && d2 < -1e-9) || (d1 < -1e-9 && d2 > 1e-9)) && ((d3 > 1e-9 && d4 < -1e-9) || (d3 < -1e-9 && d4 > 1e-9))) return true;
    }
  }
  return false;
}

/// Flächen aller Knoten mit ≥ 3 Straßen (oder Knick/Breitensprung bei zweien) auf einer Ebene. `edges` wie im Bau,
/// `edgePts(ed)` = Punkte von a nach b. Liefert { plates: [{ v, x, y, lvl, surface, cobble, ring, corners }],
/// trimOf(ed) → [vorn, hinten] in px (0 = nicht gekürzt) }.
export function platesOf(edges, edgePts, S, { turning = new Set() } = {}) {
  const at = new Map();
  for (const ed of edges) {
    if (ed.c > SIDEWALK_MAX_CLASS || ed.pass || ed.a === ed.b) continue;
    for (const v of [ed.a, ed.b]) (at.get(v) ?? at.set(v, []).get(v)).push(ed);
  }
  const lvlOf = (v) => {
    const es = at.get(v), lv = es[0].lvl ?? 0;
    return es.some((ed) => (ed.lvl ?? 0) !== lv) ? null : lv;
  };
  const pos = (v) => { for (const ed of at.get(v)) { const p = edgePts(ed); return ed.a === v ? [p[0], p[1]] : [p[p.length - 2], p[p.length - 1]]; } };
  // Knotengruppen über kurze Straßen (kürzeste zuerst, nur solange die Gruppe klein bleibt)
  const parent = new Map(), members = new Map();
  const find = (v) => { while (parent.get(v) !== v) v = parent.get(v); return v; };
  for (const v of at.keys()) { parent.set(v, v); members.set(v, [v]); }
  const short = [];
  for (const ed of edges) {
    if (!at.has(ed.a) || !at.has(ed.b) || ed.a === ed.b || ed.c > SIDEWALK_MAX_CLASS || ed.pass) continue;
    const L = lengthOf(edgePts(ed));
    if (L < MERGE_M * S) short.push([L, ed]);
  }
  short.sort((p, q) => p[0] - q[0] || p[1].a - q[1].a || p[1].b - q[1].b);
  const join = (ed, spanMax) => {
    const ra = find(ed.a), rb = find(ed.b);
    if (ra === rb) return;
    const lv = lvlOf(ed.a);
    if (lv === null || lvlOf(ed.b) !== lv || at.get(ed.a).length + at.get(ed.b).length < 4) return;
    const all = [...members.get(ra), ...members.get(rb)];
    if (all.some((v) => lvlOf(v) !== lv)) return;
    const ps = all.map(pos);
    const span = Math.hypot(Math.max(...ps.map((p) => p[0])) - Math.min(...ps.map((p) => p[0])), Math.max(...ps.map((p) => p[1])) - Math.min(...ps.map((p) => p[1])));
    if (span > spanMax * S) return;
    const [keep, drop] = ra < rb ? [ra, rb] : [rb, ra];
    parent.set(drop, keep);
    members.set(keep, all.sort((p, q) => p - q));
    members.delete(drop);
  };
  for (const [, ed] of short) join(ed, MERGE_SPAN_M);
  // Richtungsfahrbahnen: zwei Knoten, an denen je eine Einbahn-Fahrbahn derselben Straße in Gegenrichtung liegt und
  // die eine kurze Querverbindung haben, sind eine Kreuzung (Mittelstreifen dazwischen)
  const travel = (ed) => {
    const p = edgePts(ed), k = p.length;
    const dx = (p[k - 2] - p[0]) * ed.o, dy = (p[k - 1] - p[1]) * ed.o, l = Math.hypot(dx, dy) || 1;
    return [dx / l, dy / l];
  };
  const oneways = (v, skip) => at.get(v).filter((e) => e !== skip && e.o && e.n >= 0);
  const dual = [];
  for (const ed of edges) {
    if (!at.has(ed.a) || !at.has(ed.b) || ed.a === ed.b || ed.c > SIDEWALK_MAX_CLASS || ed.pass) continue;
    if (at.get(ed.a).length < 3 || at.get(ed.b).length < 3) continue;
    const L = lengthOf(edgePts(ed));
    if (L >= DUAL_M * S) continue;
    const pair = oneways(ed.a, ed).some((ea) => oneways(ed.b, ed).some((eb) => {
      if (ea.n !== eb.n) return false;
      const [ux, uy] = travel(ea), [wx, wy] = travel(eb);
      return ux * wx + uy * wy < -0.7;
    }));
    if (pair) dual.push([L, ed]);
  }
  dual.sort((p, q) => p[0] - q[0] || p[1].a - q[1].a || p[1].b - q[1].b);
  for (const [, ed] of dual) join(ed, DUAL_SPAN_M);
  const trims = new Map(); // ed → [vorn, hinten]
  const plates = [];
  const plate = (vs) => {
    const set = new Set(vs);
    const lv = lvlOf(vs[0]);
    if (lv === null) return false;
    const inner = new Set(), arms = [];
    for (const v of vs) for (const ed of at.get(v)) {
      if (set.has(ed.a) && set.has(ed.b)) { inner.add(ed); continue; }
      const pts = edgePts(ed);
      const fwd = ed.a === v;
      arms.push({ key: arms.length, ed, fwd, pts: fwd ? pts : reversePts(pts), h: ed.w / 10 * S / 2, c: ed.c });
    }
    if (arms.length + inner.size < 2) return false;
    const pl = plateOf(arms, S);
    if (!pl) return false;
    for (const arm of arms) {
      const t = Math.round(pl.trims.get(arm.key));
      const tr = trims.get(arm.ed) ?? [0, 0];
      tr[arm.fwd ? 0 : 1] = t;
      trims.set(arm.ed, tr);
    }
    // Straßen innerhalb der Gruppe deckt die Fläche ganz: Kürzung über die volle Länge (die Straße entfällt)
    for (const ed of inner) { const L = Math.ceil(lengthOf(edgePts(ed))) + 1; trims.set(ed, [L, L]); }
    // Belag der breitesten Straße (Code wie im Querschnitt: 1 Pflaster, 2 Platten, 3 unbefestigt)
    const es = vs.flatMap((v) => at.get(v));
    const widest = es.reduce((m, ed) => (ed.w > m.w ? ed : m), es[0]);
    const surface = widest.x?.[11] ?? 0;
    plates.push({
      v: vs[0], also: vs.slice(1), x: pl.x, y: pl.y, lvl: lv, surface,
      ring: pl.ring.map(Math.round), corners: pl.corners.map((c) => c.map(Math.round)),
      ...(pl.fill ? { fill: pl.fill.map(Math.round) } : {}),
    });
    return true;
  };
  // Straßenende ohne Fortsetzung: Wendehammer (OSM) oder Bordstein quer über das Ende
  const end = (v) => {
    const ed = at.get(v)[0];
    const fwd = ed.a === v, pts = edgePts(ed);
    const arm = { key: 0, ed, fwd, pts: fwd ? pts : reversePts(pts), h: ed.w / 10 * S / 2, c: ed.c };
    const pl = endPlate(arm, S, turning.has(v) ? TURN_R_M * S : 0);
    if (!pl) return;
    const tr = trims.get(ed) ?? [0, 0];
    tr[fwd ? 0 : 1] = Math.round(pl.trims.get(0));
    trims.set(ed, tr);
    plates.push({
      v, also: [], x: pl.x, y: pl.y, lvl: ed.lvl ?? 0, surface: ed.x?.[11] ?? 0,
      ring: pl.ring.map(Math.round), corners: pl.corners.map((c) => c.map(Math.round)), end: true,
    });
  };
  for (const v of [...at.keys()].sort((p, q) => p - q)) {
    if (find(v) !== v) continue;
    const vs = members.get(v);
    if (vs.length > 1 && plate(vs)) continue;
    // einzelne Knoten (auch die einer Gruppe, deren gemeinsame Fläche nicht aufgeht)
    for (const u of vs) {
      if (at.get(u).length >= 2) plate([u]);
      else if (at.get(u)[0].c <= 8) end(u);
    }
  }
  plates.sort((p, q) => p.v - q.v);
  return { plates, trimOf: (ed) => trims.get(ed) ?? [0, 0] };
}

function reversePts(pts) {
  const out = [];
  for (let i = pts.length - 2; i >= 0; i -= 2) out.push(pts[i], pts[i + 1]);
  return out;
}
