// Zerlegt die fertig berechnete Stadt (build.mjs) in Kacheln, die das Spiel um die Kamera nachlädt.
// Regeln (siehe web/src/map.js, dort wird zusammengesetzt):
//  - Punkte (Bäume, POIs, Hausnummern, Poller, Querungen, Ampeln, Abbiegeverbote) gehören genau einer Kachel.
//  - Linien und Gebäude liegen in jeder Kachel, die ihr Hüllrechteck berührt, und tragen eine globale Nummer (gid);
//    das Spiel hält sie nur einmal im Speicher. Lange Linien werden vorher in Stücke zerlegt.
//  - Große Flächen (Wasser, Grün) werden an den Kachelgrenzen abgeschnitten (jede Kachel hat ihr eigenes Stück).
// Koordinaten bleiben global (px), Linien sind delta-kodiert, Namen je Kachel in einer eigenen Tabelle.
import { simplify, ringArea } from './geo.mjs';
import { delta, clipRing } from '../../web/src/geom.js';
import { AREA_KIND, POI_CAT, WALL_KIND } from '../../web/src/citycodes.js';

export const TILE_PX = 6400; // 640 m; Vielfaches aller Rasterweiten im Spiel (map.js)

const bboxOf = (pts, pad = 0) => {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (let i = 0; i < pts.length; i += 2) { x0 = Math.min(x0, pts[i]); x1 = Math.max(x1, pts[i]); y0 = Math.min(y0, pts[i + 1]); y1 = Math.max(y1, pts[i + 1]); }
  return [x0 - pad, y0 - pad, x1 + pad, y1 + pad];
};

// Polylinie in Stücke von höchstens maxLen zerlegen (Stücke teilen ihren Endpunkt).
export function chunkPolyline(pts, maxLen) {
  const out = [];
  let cur = [pts[0], pts[1]], L = 0;
  for (let i = 2; i < pts.length; i += 2) {
    const d = Math.hypot(pts[i] - pts[i - 2], pts[i + 1] - pts[i - 1]);
    if (L + d > maxLen && cur.length >= 4) { out.push(cur); cur = [pts[i - 2], pts[i - 1]]; L = 0; }
    cur.push(pts[i], pts[i + 1]); L += d;
  }
  if (cur.length >= 4) out.push(cur);
  return out;
}

export function tileCity(g, { tile, meta, places }) {
  const { S, W, H } = g;
  const NX = Math.max(1, Math.ceil(W / tile)), NY = Math.max(1, Math.ceil(H / tile));
  const tiles = new Map();
  const get = (tx, ty) => {
    const k = `${tx}_${ty}`;
    let t = tiles.get(k);
    if (!t) tiles.set(k, t = { tx, ty, names: [], nameIdx: new Map(), vmap: new Map(), vid: [], vxy: [], vtrim: [],
      edges: [], junctions: [], paths: [], rails: [], buildings: [], water: [], areas: [], walls: [], fences: [],
      trees: { xy: [], g: [], c: [], r: [] }, barriers: [], posts: [], crossings: [], signals: [], turnBans: [], pois: [], addresses: { xy: [], street: [], nr: [] }, furn: [], dens: null });
    return t;
  };
  const cx = (x) => Math.min(NX - 1, Math.max(0, Math.floor(x / tile))), cy = (y) => Math.min(NY - 1, Math.max(0, Math.floor(y / tile)));
  const home = (x, y) => get(cx(x), cy(y));
  const each = ([x0, y0, x1, y1], fn) => { for (let tx = cx(x0); tx <= cx(x1); tx++) for (let ty = cy(y0); ty <= cy(y1); ty++) fn(get(tx, ty), tx, ty); };
  const nm = (t, s) => { if (!s) return -1; let k = t.nameIdx.get(s); if (k === undefined) { k = t.names.length; t.names.push(s); t.nameIdx.set(s, k); } return k; };
  const vtx = (t, v) => {
    let k = t.vmap.get(v);
    if (k === undefined) { k = t.vid.length; t.vmap.set(v, k); t.vid.push(v); t.vxy.push(g.vertices[2 * v], g.vertices[2 * v + 1]); t.vtrim.push(g.trim.get(v) ?? 0); }
    return k;
  };

  // Straßenkanten: [gid, a, b (Knoten der Kachel), Klasse, Breite dm, Name, Einbahn, Merkmale, Zwischenpunkte, Querschnitt]
  g.edges.forEach((ed, gid) => {
    const pts = [g.vertices[2 * ed.a], g.vertices[2 * ed.a + 1], ...ed.p, g.vertices[2 * ed.b], g.vertices[2 * ed.b + 1]];
    const flags = ed.br | (ed.in << 1) | ((ed.blocked ? 1 : 0) << 2) | (ed.pass << 3);
    const x = ed.c <= 8 ? ed.x : [ed.x[10], ed.x[11]]; // Nebenwege: nur Tempo + Belag
    each(bboxOf(pts, ed.w / 10 * S / 2 + S), (t) => {
      t.edges.push([gid, vtx(t, ed.a), vtx(t, ed.b), ed.c, ed.w, nm(t, g.names[ed.n]), ed.o, flags, ed.p.length ? delta(ed.p) : [], x, Math.round((ed.dtv ?? 0) / 100) * (ed.dtvMeasured ? 1 : -1)]);
    });
  });
  // Kreuzungsflächen: [Knoten-gid, x, y, Radius px, Brücke | Pflaster << 1, kleinste Klasse]
  for (const j of g.junctions) each([j.x - j.r, j.y - j.r, j.x + j.r, j.y + j.r], (t) => { vtx(t, j.v); t.junctions.push([j.v, j.x, j.y, j.r, j.bridge | (j.cobble << 1), j.cls]); });

  let gid = 0; // gemeinsame Nummernfolge für alle mehrfach abgelegten Linien und Flächen
  const lines = (list, maxLen, rec) => { for (const it of list) for (const piece of chunkPolyline(it.p, maxLen)) { const id = gid++; each(bboxOf(piece), (t) => rec(t, id, it, piece)); } };
  lines(g.paths, tile, (t, id, it, p) => t.paths.push([id, it.br | (it.pass << 1), delta(p)]));
  lines(g.rails, tile, (t, id, it, p) => t.rails.push([id, it.br, it.sub, delta(p)]));
  lines(g.walls.map((p, i) => ({ p, kind: g.wallKind?.[i] ?? 0 })), tile / 2, (t, id, it, p) => t.walls.push([id, it.kind, delta(p)]));
  lines(g.border.map((r) => ({ p: [...r, r[0], r[1]], kind: WALL_KIND.border })), tile / 2, (t, id, it, p) => t.walls.push([id, it.kind, delta(p)]));
  lines(g.access.fences.map(([k, p]) => ({ p, k })), tile / 2, (t, id, it, p) => t.fences.push([id, it.k, delta(p)]));

  // Gebäude: [gid, Höhe dm, Art, Ringe, eigene Wandzüge oder 0, Türen oder 0]
  g.buildings.forEach((b) => {
    const id = gid++;
    each(bboxOf(b.rings[0].pts), (t) => t.buildings.push([id, b.h, b.k, b.rings.map((r) => delta(r.pts)), b.walls ? b.walls.map(delta) : 0, b.doors ?? 0]));
    b.gid = id; // für Tests/Statistik
  });
  // Flächen: kleine mehrfach abgelegt (gid), große je Kachel abgeschnitten (gid −1)
  const polys = (list, push) => {
    for (const f of list) {
      const box = bboxOf(f.rings.flatMap((r) => r.pts));
      if (box[2] - box[0] <= tile && box[3] - box[1] <= tile) {
        const id = gid++;
        each(box, (t) => push(t, id, f, f.rings.map((r) => [r.outer ? 1 : 0, delta(r.pts)])));
        continue;
      }
      each(box, (t, tx, ty) => {
        const pad = S; // 1 m Überlappung gegen Haarrisse zwischen den Stücken
        const [x0, y0, x1, y1] = [tx * tile - pad, ty * tile - pad, (tx + 1) * tile + pad, (ty + 1) * tile + pad];
        const rings = [];
        for (const r of f.rings) {
          const c = clipRing(r.pts, x0, y0, x1, y1).map(Math.round);
          if (c.length >= 6 && Math.abs(ringArea(c)) >= S * S) rings.push([r.outer ? 1 : 0, delta(c)]);
        }
        if (rings.some((r) => r[0])) push(t, -1, f, rings);
      });
    }
  };
  polys(g.water, (t, id, f, rings) => t.water.push([id, rings]));
  polys(g.areas, (t, id, f, rings) => t.areas.push([id, f.k, rings]));

  // Punkte
  for (const tr of g.trees) { const t = home(tr.x, tr.y); t.trees.xy.push(tr.x, tr.y); t.trees.g.push(tr.g); t.trees.c.push(tr.c); t.trees.r.push(tr.r); }
  const B = g.access.barriers;
  for (let i = 0; i < B.length; i += 6) home(B[i], B[i + 1]).barriers.push(...B.slice(i, i + 6));
  const PO = g.access.posts;
  for (let i = 0; i < PO.length; i += 3) home(PO[i], PO[i + 1]).posts.push(...PO.slice(i, i + 3));
  const C = g.access.crossings;
  for (let i = 0; i < C.length; i += 4) home(C[i], C[i + 1]).crossings.push(...C.slice(i, i + 4));
  for (const v of g.access.signals) home(g.vertices[2 * v], g.vertices[2 * v + 1]).signals.push(v);
  const TB = g.access.turnBans;
  for (let i = 0; i < TB.length; i += 3) { const v = TB[i + 1]; home(g.vertices[2 * v], g.vertices[2 * v + 1]).turnBans.push(TB[i], v, TB[i + 2]); }
  for (const q of g.pois) { const t = home(q.x, q.y); t.pois.push([q.x, q.y, POI_CAT[q.cat], nm(t, g.names[q.n]), nm(t, g.names[q.k])]); }
  for (const f of g.furniture ?? []) home(f.x, f.y).furn.push([f.x, f.y, f.k]);
  // Einwohnerdichte: je Kachel das Teilraster (Zeilen von oben), nur wenn dort jemand wohnt
  if (g.dens) {
    const { cell, nx, ny, v } = g.dens, per = Math.round(tile / cell);
    for (const t of tiles.values()) {
      const tx = t.tx, ty = t.ty, vals = [];
      let any = false;
      for (let j = 0; j < per; j++) for (let i = 0; i < per; i++) {
        const cx = tx * per + i, cy = ty * per + j, val = cx < nx && cy < ny ? v[cy * nx + cx] : 0;
        vals.push(val); if (val) any = true;
      }
      if (any) t.dens = { cell, vals };
    }
  }
  for (const a of g.addresses) { const t = home(a.x, a.y); t.addresses.xy.push(a.x, a.y); t.addresses.street.push(nm(t, g.names[a.s])); t.addresses.nr.push(a.nr); }

  // Serialisieren
  const out = new Map();
  for (const [k, t] of [...tiles].sort((a, b) => (a[1].ty - b[1].ty) || (a[1].tx - b[1].tx))) {
    const empty = !t.edges.length && !t.buildings.length && !t.water.length && !t.areas.length && !t.trees.g.length && !t.walls.length && !t.paths.length && !t.rails.length;
    if (empty) continue;
    out.set(k, {
      v: 3, t: [t.tx, t.ty], names: t.names,
      vertices: { id: t.vid, xy: delta(t.vxy), trim: t.vtrim },
      edges: t.edges, junctions: t.junctions, paths: t.paths, rails: t.rails, buildings: t.buildings, water: t.water, areas: t.areas,
      walls: t.walls, fences: t.fences,
      trees: { xy: delta(t.trees.xy), g: t.trees.g, c: t.trees.c, r: t.trees.r },
      barriers: t.barriers, posts: t.posts, crossings: t.crossings, signals: t.signals, turnBans: t.turnBans,
      pois: t.pois, furn: t.furn, ...(t.dens ? { dens: t.dens } : {}),
      addresses: { xy: delta(t.addresses.xy), street: t.addresses.street, nr: t.addresses.nr },
    });
  }

  const simp = (r, tol) => { const p = simplify([...r], tol); return p.length >= 8 ? p : r; };
  const index = {
    meta: { ...meta, tile, tilesX: NX, tilesY: NY },
    tiles: [...out.keys()],
    border: g.border.map((r) => delta(simp(r, 0.5 * S))),
    bezirke: g.bezirke.map((b) => ({ n: b.name, r: b.rings.map((r) => delta(simp(r, 2 * S))) })),
    districts: g.districts.map((d) => ({ n: d.name, r: d.rings.map((r) => delta(simp(r, 2 * S))) })),
    kieze: g.kieze,
    places,
    // Krankenhäuser (ganz Berlin, auch ungeladene Kacheln): hier wacht man auf, wenn die Lebenspunkte ausgehen
    hospitals: g.pois.filter((q) => g.names[q.k] === 'hospital').map((q) => [q.x, q.y, g.names[q.n]]),
  };
  return { index, overview: overviewOf(g), tiles: out };
}

// Stadtplan (Übersicht über ganz Berlin): vereinfachte Flächen, Hauptstraßen, Bahnen, Bahnhöfe.
export function overviewOf(g) {
  const { S } = g;
  const vx = (k) => g.vertices[2 * k], vy = (k) => g.vertices[2 * k + 1];
  const GREEN = new Set([AREA_KIND.grass, AREA_KIND.wood, AREA_KIND.cemetery, AREA_KIND.allotments]);
  const rings = (f, tol, minArea) => f.rings.map((r) => { const p = simplify(r.pts, tol); return p.length >= 8 && Math.abs(ringArea(p)) >= minArea ? [r.outer ? 1 : 0, delta(p)] : null; }).filter(Boolean);
  const water = g.water.map((f) => rings(f, 6 * S, 1500 * S * S)).filter((r) => r.some((x) => x[0]));
  const areas = g.areas.filter((a) => GREEN.has(a.k)).map((a) => [a.k, rings(a, 8 * S, 8000 * S * S)]).filter(([, r]) => r.some((x) => x[0]));
  // Straßen mit Namen (eigene Namenstabelle) – der Stadtplan beschriftet sie beim Heranzoomen
  const names = [], nameIdx = new Map();
  const nm = (s) => { if (!s) return -1; let k = nameIdx.get(s); if (k === undefined) { k = names.length; names.push(s); nameIdx.set(s, k); } return k; };
  // Kanten gleichen Namens und gleicher Klasse zu durchgehenden Straßenzügen verketten (eine Kante reicht nur von
  // Kreuzung zu Kreuzung – zu kurz für einen Namen); an Abzweigen geht es in der geradesten Richtung weiter.
  const roads = chainStreets(g.edges.filter((ed) => ed.c <= 7 && !ed.pass), g, vx, vy)
    .map(({ c, n, pts }) => [c, nm(g.names[n]), delta(simplify(pts, (c <= 5 ? 3 : 6) * S))]);
  const rails = g.rails.filter((r) => !r.sub).map((r) => delta(simplify(r.p, 8 * S)));
  const stations = g.pois.filter((q) => q.cat === 'ubahn' || q.cat === 'sbahn').map((q) => [q.x, q.y, q.cat, g.names[q.n]]);
  // Beschriftungspunkte: Bezirke, Ortsteile (im Inneren der Fläche, nicht am Schwerpunkt) und Kieze
  const place = (list) => list.map((d) => { const [x, y, area] = labelPoint(d.rings); return [x, y, d.name, Math.round(area / (S * S) / 1e4)]; }); // Fläche in ha
  const labels = place(g.bezirke), ortsteile = place(g.districts);
  const kieze = g.kieze.map((k) => [k.x, k.y, k.n]);
  return { water, areas, roads, names, rails, stations, labels, ortsteile, kieze };
}

// Punkt im Inneren einer Fläche, möglichst weit vom Rand (für die Beschriftung; der Schwerpunkt kann bei
// gebogenen Ortsteilen außerhalb liegen). Grobes Raster, dann verfeinert. Liefert [x, y, Fläche].
export function labelPoint(rings) {
  const outer = rings.reduce((a, r) => (Math.abs(ringArea(r)) > Math.abs(ringArea(a)) ? r : a), rings[0]);
  const area = Math.abs(ringArea(outer));
  const [x0, y0, x1, y1] = bboxOf(outer);
  const inside = (x, y) => { let c = false; for (let i = 0, j = outer.length - 2; i < outer.length; j = i, i += 2) { const xi = outer[i], yi = outer[i + 1], xj = outer[j], yj = outer[j + 1]; if ((yi > y) !== (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi) c = !c; } return c; };
  const edgeDist = (x, y) => { let d = Infinity; for (let i = 0; i < outer.length - 2; i += 2) { const ax = outer[i], ay = outer[i + 1], bx = outer[i + 2], by = outer[i + 3], dx = bx - ax, dy = by - ay, L = dx * dx + dy * dy; let t = L ? ((x - ax) * dx + (y - ay) * dy) / L : 0; t = Math.max(0, Math.min(1, t)); d = Math.min(d, Math.hypot(ax + t * dx - x, ay + t * dy - y)); } return d; };
  let best = null, cell = Math.max(x1 - x0, y1 - y0) / 16;
  let cx = (x0 + x1) / 2, cy = (y0 + y1) / 2, span = 8;
  for (let round = 0; round < 4; round++) {
    for (let i = -span; i <= span; i++) for (let j = -span; j <= span; j++) {
      const x = cx + i * cell, y = cy + j * cell;
      if (!inside(x, y)) continue;
      const d = edgeDist(x, y);
      if (!best || d > best.d) best = { x, y, d };
    }
    if (!best) break;
    cx = best.x; cy = best.y; cell /= 4; span = 4;
  }
  return best ? [Math.round(best.x), Math.round(best.y), area] : [Math.round((x0 + x1) / 2), Math.round((y0 + y1) / 2), area];
}

export function chainStreets(edges, g, vx, vy) {
  const groups = new Map();
  for (const ed of edges) { const k = ed.n >= 0 ? `${ed.n}|${ed.c}` : null; if (k === null) continue; (groups.get(k) ?? groups.set(k, []).get(k)).push(ed); }
  const out = edges.filter((ed) => ed.n < 0).map((ed) => ({ c: ed.c, n: ed.n, pts: [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)] }));
  const ptsOf = (ed, fromA) => { const p = [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)]; if (fromA) return p; const r = []; for (let i = p.length - 2; i >= 0; i -= 2) r.push(p[i], p[i + 1]); return r; };
  for (const list of groups.values()) {
    const at = new Map();
    for (const ed of list) for (const v of [ed.a, ed.b]) (at.get(v) ?? at.set(v, []).get(v)).push(ed);
    const used = new Set();
    const dirAt = (pts, end) => { const n = pts.length; return end ? Math.atan2(pts[n - 1] - pts[n - 3], pts[n - 2] - pts[n - 4]) : Math.atan2(pts[3] - pts[1], pts[2] - pts[0]); };
    const extend = (pts, v) => { // am Ende (Knoten v) weiterverketten
      for (;;) {
        const heading = dirAt(pts, true);
        let best = null;
        for (const ed of at.get(v) ?? []) {
          if (used.has(ed)) continue;
          const p = ptsOf(ed, ed.a === v);
          let d = Math.abs(dirAt(p, false) - heading); if (d > Math.PI) d = 2 * Math.PI - d;
          if (d < 1.2 && (!best || d < best.d)) best = { ed, p, d };
        }
        if (!best) return pts;
        used.add(best.ed);
        for (let i = 2; i < best.p.length; i++) pts.push(best.p[i]);
        v = best.ed.a === v ? best.ed.b : best.ed.a;
      }
    };
    // Start an Enden (Knoten mit nur einer Kante dieses Namens), dann Reste (Ringe)
    const order = [...list].sort((a, b) => ((at.get(a.a).length === 1 || at.get(a.b).length === 1) ? 0 : 1) - ((at.get(b.a).length === 1 || at.get(b.b).length === 1) ? 0 : 1) || a.a - b.a);
    for (const ed of order) {
      if (used.has(ed)) continue;
      used.add(ed);
      const fromA = at.get(ed.a).length === 1 || at.get(ed.b).length !== 1;
      let pts = ptsOf(ed, fromA);
      pts = extend(pts, fromA ? ed.b : ed.a);
      // auch nach hinten weiter (falls nicht an einem Ende begonnen)
      const rev = []; for (let i = pts.length - 2; i >= 0; i -= 2) rev.push(pts[i], pts[i + 1]);
      pts = extend(rev, fromA ? ed.a : ed.b);
      out.push({ c: ed.c, n: ed.n, pts });
    }
  }
  return out;
}
