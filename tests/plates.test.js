// Kreuzungsflächen mit echten Ecken (tools/osm/plates.mjs): Geometrie an synthetischen Knoten.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { plateOf, platesOf, endPlate, convexHull, pointAt, selfIntersects, CORNER_R, lengthOf, MERGE_M, NOSE_PHI, TURN_R_M } from '../tools/osm/plates.mjs';

const S = 10;
// Straße vom Knoten (0,0) in Richtung `deg` (Grad, mathematisch), `len` m lang, `w` m breit
const arm = (key, deg, w, c = 7, len = 60) => {
  const a = deg * Math.PI / 180;
  return { key, pts: [0, 0, Math.cos(a) * len * S, Math.sin(a) * len * S], h: w * S / 2, c };
};
const pts = (flat) => { const out = []; for (let i = 0; i < flat.length; i += 2) out.push([flat[i], flat[i + 1]]); return out; };
const area = (ring) => { let s = 0; const p = pts(ring); for (let i = 0; i < p.length; i++) { const [x0, y0] = p[i], [x1, y1] = p[(i + 1) % p.length]; s += x0 * y1 - x1 * y0; } return s / 2; };
const inside = (ring, x, y) => { let c = false; const p = pts(ring); for (let i = 0, j = p.length - 1; i < p.length; j = i++) { const [xi, yi] = p[i], [xj, yj] = p[j]; if ((yi > y) !== (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi) c = !c; } return c; };
const near = (p, q, tol = 0.6) => Math.hypot(p[0] - q[0], p[1] - q[1]) < tol;

function check(arms, label) {
  const pl = plateOf(arms, S);
  assert.ok(pl, `${label}: Fläche`);
  assert.ok(!selfIntersects(pl.ring), `${label}: Ring ohne Selbstschnitt`);
  assert.ok(Math.abs(area(pl.ring)) > 0, `${label}: Fläche > 0`);
  assert.ok(inside(pl.ring, 0.3, 0.2), `${label}: Knoten liegt in der Fläche`);
  for (const r of arms) {
    const T = pl.trims.get(r.key);
    assert.ok(T > 0 && T <= lengthOf(r.pts) * 0.45 + 1e-6, `${label}: Kürzung ${T}`);
    // Mündungspunkte (Fahrbahnkanten bei T) liegen auf dem Ring
    const p = pointAt(r.pts, T);
    const L = [p.x - r.h * p.uy, p.y + r.h * p.ux], R = [p.x + r.h * p.uy, p.y - r.h * p.ux];
    const ring = pts(pl.ring);
    assert.ok(ring.some((q) => near(q, L)) && ring.some((q) => near(q, R)), `${label}: Mündung von ${r.key} im Ring`);
  }
  // Eckzüge: je Straßenpaar einer, Anfang und Ende an Mündungen
  assert.equal(pl.corners.length, arms.length);
  return pl;
}

test('rechtwinklige Kreuzung: vier gerundete Ecken, Kürzung = halbe Querstraße + Bordsteinradius', () => {
  const arms = [arm(0, 0, 8), arm(1, 90, 8), arm(2, 180, 8), arm(3, 270, 8)];
  const pl = check(arms, 'Kreuz');
  // 90°: Ecke bei t = h_quer, Bogen R / tan(45°) = R
  const want = 4 * S + CORNER_R(7) * S;
  for (const r of arms) assert.ok(Math.abs(pl.trims.get(r.key) - want) < 1e-6, `${pl.trims.get(r.key)} statt ${want}`);
  // die Ecken sind Bögen (mehr als drei Punkte), nicht spitz
  for (const c of pl.corners) assert.ok(c.length / 2 > 4, 'Bogen');
  // die scharfe Blockecke (4 m | 4 m) wird abgerundet: knapp dahinter ist Fahrbahn, tiefer im Block Gehweg
  assert.ok(inside(pl.ring, 4 * S + 3, 4 * S + 3), 'abgerundet');
  assert.ok(!inside(pl.ring, 4 * S + 20, 4 * S + 20), 'Block bleibt Gehweg');
  // der Bogen hat den Radius der Wohnstraße: Mittelpunkt (4 + R | 4 + R), alle Bogenpunkte im Abstand R
  const R = CORNER_R(7) * S, c = 4 * S + R;
  const arcPts = pl.corners.flatMap((k) => pts(k)).filter(([x, y]) => x > 4 * S + 1 && y > 4 * S + 1);
  assert.ok(arcPts.length >= 3 && arcPts.every(([x, y]) => Math.abs(Math.hypot(x - c, y - c) - R) < 0.5), 'Kreisbogen');
});

test('T-Kreuzung: die durchgehende Seite ist gerade, ihr Gehweg läuft durch', () => {
  const arms = [arm(0, 0, 8), arm(1, 180, 8), arm(2, 90, 6)];
  const pl = check(arms, 'T');
  // gerade Seite (zwischen 180° und 0° über 270°): ein Eckzug ohne Bogen, nahe der Fahrbahnkante y = −4 m
  const straight = pl.corners.find((c) => pts(c).every(([, y]) => Math.abs(y + 4 * S) < 1));
  assert.ok(straight, 'durchgehender Gehweg an der geraden Seite');
});

test('spitzes Y: Gehrung gedeckelt, kein Selbstschnitt, Kürzung begrenzt', () => {
  const arms = [arm(0, 0, 10, 4), arm(1, 25, 6), arm(2, 200, 10, 4)];
  const pl = check(arms, 'Y');
  // Eckabstand + Bogen zusammen höchstens 4 × die breiteste halbe Fahrbahn
  for (const r of arms) assert.ok(pl.trims.get(r.key) <= 4 * 5 * S + 1e-6, `gedeckelt: ${pl.trims.get(r.key)}`);
});

test('zwei Straßen: Knick bekommt eine Fläche, fast gerade nicht', () => {
  check([arm(0, 0, 7), arm(1, 120, 7)], 'Knick');
  assert.equal(plateOf([arm(0, 0, 7), arm(1, 178, 7)], S), null, 'fast gerade: keine Fläche');
  // Breitensprung bei gerader Fortsetzung bekommt eine Fläche
  check([arm(0, 0, 7), arm(1, 180, 12)], 'Breitensprung');
});

test('kurze Straße: höchstens 45 % ihrer Länge gekürzt', () => {
  const arms = [arm(0, 0, 8, 7, 6), arm(1, 90, 8), arm(2, 180, 8), arm(3, 270, 8)];
  const pl = check(arms, 'kurz');
  assert.ok(pl.trims.get(0) <= 6 * S * 0.45 + 1e-6);
});

test('deterministisch und unabhängig von der Eingabereihenfolge', () => {
  const a = [arm(0, 0, 8), arm(1, 100, 6), arm(2, 200, 9), arm(3, 290, 6)];
  const p1 = plateOf(a, S), p2 = plateOf([a[2], a[0], a[3], a[1]], S);
  assert.deepEqual([...p1.trims].sort(), [...p2.trims].sort());
  assert.equal(p1.ring.length, p2.ring.length);
});

test('die Eckzüge aneinandergereiht ergeben den Umriss (die Kacheln speichern nur sie)', () => {
  for (const arms of [
    [arm(0, 0, 8), arm(1, 90, 8), arm(2, 180, 8), arm(3, 270, 8)],
    [arm(0, 0, 8), arm(1, 180, 8), arm(2, 90, 6)],
    [arm(0, 0, 10, 4), arm(1, 25, 6), arm(2, 200, 10, 4)],
    [arm(0, 0, 7), arm(1, 120, 7)],
  ]) {
    const pl = plateOf(arms, S);
    const joined = pl.corners.flat();
    assert.ok(!selfIntersects(joined), 'zusammengesetzt ohne Selbstschnitt');
    assert.ok(Math.abs(Math.abs(area(joined)) - Math.abs(area(pl.ring))) < 1, 'gleiche Fläche');
    // aufeinanderfolgende Züge: Ende eines Zugs (rechte Mündung) und Anfang des nächsten (linke) gehören zur selben Straße
    for (let k = 0; k < pl.corners.length; k++) {
      const a = pl.corners[k], b = pl.corners[(k + 1) % pl.corners.length];
      const end = [a[a.length - 2], a[a.length - 1]], start = [b[0], b[1]];
      const gap = Math.hypot(end[0] - start[0], end[1] - start[1]);
      assert.ok(gap > 1, 'Mündung dazwischen (Breite der Straße)');
    }
  }
});

test('Knoten wenige Meter auseinander bilden eine gemeinsame Fläche, das Stück dazwischen entfällt', () => {
  // versetzte Kreuzung: Querstraße trifft von Norden bei x = 0, von Süden bei x = 3 m; dazwischen 3 m Hauptstraße
  const P = { 1: [-60, 0], 2: [0, 0], 3: [3, 0], 4: [60, 0], 5: [0, -60], 6: [3, 60] };
  const e = (id, a, b, w, c = 7) => ({ id, a, b, w: w * 10, c, x: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, id === 10 ? 1 : 0] });
  const edges = [e(10, 1, 2, 10), e(11, 2, 3, 10), e(12, 3, 4, 10), e(13, 2, 5, 7), e(14, 3, 6, 7)];
  const edgePts = (ed) => [P[ed.a][0] * S, P[ed.a][1] * S, P[ed.b][0] * S, P[ed.b][1] * S];
  const { plates: all, trimOf: trimAll } = platesOf(edges, edgePts, S);
  const plates = all.filter((p) => !p.end);
  assert.equal(plates.length, 1, 'eine Fläche statt zwei überlappender');
  assert.equal(all.filter((p) => p.end).length, 4, 'die vier Enden des Testnetzes bekommen einen Abschluss');
  const trimOf = (ed) => trimAll(ed);
  assert.equal(plates[0].v, 2);
  assert.deepEqual(plates[0].also, [3]);
  assert.equal(plates[0].surface, 1, 'Belag der breitesten Straße (erste bei Gleichstand)');
  const inner = trimOf(edges[1]);
  assert.ok(inner[0] + inner[1] >= 30, 'Zwischenstück ganz gekürzt');
  assert.ok(!selfIntersects(plates[0].ring));
  // jede äußere Mündung liegt auf dem Umriss
  const ring = pts(plates[0].ring);
  for (const ed of [edges[0], edges[2], edges[3], edges[4]]) {
    const [t0, t1] = trimOf(ed);
    assert.ok(t0 + t1 > 0, `Straße ${ed.id} gekürzt`);
    const fwd = ed.a === 2 || ed.a === 3, flat = edgePts(ed);
    const pp = fwd ? flat : [flat[2], flat[3], flat[0], flat[1]];
    const p = pointAt(pp, fwd ? t0 : t1), h = ed.w / 20 * S;
    const L = [p.x - h * p.uy, p.y + h * p.ux], R = [p.x + h * p.uy, p.y - h * p.ux];
    assert.ok(ring.some((q) => near(q, L, 1)) && ring.some((q) => near(q, R, 1)), `Mündung ${ed.id}`);
  }
  // beide Knoten liegen in der Fläche
  assert.ok(inside(plates[0].ring, 0.5, 0.5) && inside(plates[0].ring, 3 * S - 0.5, 0.5));
});

test('Ketten kurzer Stücke wachsen nicht zu einer langen Fläche zusammen', () => {
  // Hauptstraße mit Einfahrten alle 6 m (< MERGE_M) über 60 m
  const P = {}, edges = [];
  const e = (id, a, b, w) => ({ id, a, b, w: w * 10, c: 7, x: [] });
  for (let k = 0; k <= 10; k++) { P[k] = [k * 6, 0]; P[100 + k] = [k * 6, -20]; }
  for (let k = 0; k < 10; k++) edges.push(e(k, k, k + 1, 8));
  for (let k = 1; k < 10; k++) edges.push(e(100 + k, k, 100 + k, 4));
  assert.ok(6 < MERGE_M);
  const { plates } = platesOf(edges, (ed) => [P[ed.a][0] * S, P[ed.a][1] * S, P[ed.b][0] * S, P[ed.b][1] * S], S);
  for (const pl of plates) {
    const xs = pts(pl.ring).map((q) => q[0]);
    assert.ok(Math.max(...xs) - Math.min(...xs) < 30 * S, `Fläche ${pl.v} zu lang`);
  }
});

test('Mittelstreifen: parallele Richtungsfahrbahnen bekommen eine Bordsteinnase, keinen Balken quer durch die Kreuzung', () => {
  // Querstraße (Nord–Süd, zweiseitig) kreuzt eine Straße mit Mittelstreifen: Fahrbahn A (y = −8 m, nach Osten),
  // Fahrbahn B (y = +8 m, nach Westen); die Querverbindung zwischen den Fahrbahnen ist 16 m lang
  const P = { 1: [-80, -8], 2: [0, -8], 3: [80, -8], 4: [80, 8], 5: [0, 8], 6: [-80, 8], 7: [0, -80], 8: [0, 80] };
  const e = (id, a, b, w, o = 0, n = -1) => ({ id, a, b, w: w * 10, c: 5, o, n, x: [] });
  const edges = [
    e(1, 1, 2, 7, 1, 3), e(2, 2, 3, 7, 1, 3), // Fahrbahn A nach Osten
    e(3, 4, 5, 7, 1, 3), e(4, 5, 6, 7, 1, 3), // Fahrbahn B nach Westen
    e(5, 7, 2, 8), e(6, 2, 5, 8), e(7, 5, 8, 8), // Querstraße
  ];
  const edgePts = (ed) => [P[ed.a][0] * S, P[ed.a][1] * S, P[ed.b][0] * S, P[ed.b][1] * S];
  const { plates, trimOf } = platesOf(edges, edgePts, S);
  const j = plates.filter((p) => !p.end);
  assert.equal(j.length, 1, 'beide Knoten sind eine Kreuzung');
  assert.deepEqual([j[0].v, ...j[0].also].sort(), [2, 5]);
  const [t0, t1] = trimOf(edges[5]);
  assert.ok(t0 + t1 >= 160, 'das Stück über den Mittelstreifen gehört zur Fläche');
  // kein Eckzug verbindet die beiden Knoten quer durch die Kreuzung: jede Ecke bleibt auf einer Seite der Querstraße
  for (const c of j[0].corners) {
    const xs = pts(c).map((q) => q[0]);
    assert.ok(Math.max(...xs) < 1 * S || Math.min(...xs) > -1 * S, `Eckzug quert die Kreuzung: ${xs}`);
  }
  assert.ok(NOSE_PHI > 0.3);
});

test('Straßenende: Bordstein quer über das Ende, Wendehammer als Kreis', () => {
  const arm0 = { key: 0, pts: [0, 0, 60 * S, 0], h: 3 * S, c: 7 };
  const flat = endPlate(arm0, S);
  assert.ok(flat && flat.corners.length === 1);
  const c = pts(flat.corners[0]);
  // der Zug läuft von der linken Mündung über das Ende (x = 0) zur rechten
  assert.ok(c.some((q) => Math.abs(q[0]) < 0.5 && q[1] > 0) && c.some((q) => Math.abs(q[0]) < 0.5 && q[1] < 0));
  const tc = endPlate(arm0, S, TURN_R_M * S);
  const r = pts(tc.ring).map((q) => Math.hypot(q[0], q[1]));
  assert.ok(Math.max(...r) <= TURN_R_M * S + 1 && Math.max(...r) > TURN_R_M * S - 1, 'Kreis um den Knoten');
  assert.ok(!selfIntersects(tc.ring));
  assert.ok(Math.abs(area(tc.ring)) > Math.PI * (TURN_R_M * S) ** 2 * 0.7, 'fast ein ganzer Kreis');
});

test('Rückfall: die konvexe Hülle ist einfach und umfasst alle Punkte', () => {
  const hull = convexHull([[0, 0], [10, 0], [10, 10], [0, 10], [5, 5], [3, 7]]);
  assert.equal(hull.length, 8);
  assert.ok(!selfIntersects(hull));
});

test('spitzer Abzweig: die Nase sitzt, wo sich die Fahrbahnen trennen – keine Notfläche', () => {
  // Hauptstraße West–Ost, eine Zufahrt zweigt unter 18° nach Nordosten ab (häufigster Grund für die Hüllen)
  const deg = (d, len, w, c) => { const a = d * Math.PI / 180; return { pts: [0, 0, Math.cos(a) * len * S, Math.sin(a) * len * S], h: w * S / 2, c }; };
  const arms = [{ key: 0, ...deg(180, 40, 6.5, 3) }, { key: 1, ...deg(0, 40, 6.5, 3) }, { key: 2, ...deg(-18, 45, 4.5, 9) }];
  const pl = plateOf(arms, S);
  assert.ok(pl && !pl.fill, 'echte Fläche, keine Hülle');
  assert.ok(!selfIntersects(pl.ring));
  // beide Mündungen am Abzweig liegen dort, wo die Fahrbahnen nicht mehr überlappen
  const t1 = pl.trims.get(1), t2 = pl.trims.get(2);
  const p1 = pointAt(arms[1].pts, t1), p2 = pointAt(arms[2].pts, t2);
  assert.ok(Math.hypot(p1.x - p2.x, p1.y - p2.y) >= (arms[1].h + arms[2].h) * 0.9, `überlappen noch: ${t1} ${t2}`);
});

test('kurzer Arm mit freiem Ende darf fast ganz in die Fläche', () => {
  const arms = [arm(0, 0, 8), arm(1, 90, 8, 7, 3), arm(2, 180, 8), arm(3, 270, 8)];
  const tight = plateOf(arms, S);
  const free = plateOf(arms.map((r) => (r.key === 1 ? { ...r, share: 0.9 } : r)), S);
  assert.ok(free.trims.get(1) > tight.trims.get(1), `${free.trims.get(1)} > ${tight.trims.get(1)}`);
  assert.ok(free.trims.get(1) <= 3 * S * 0.9 + 1e-6);
});
