// Stadtmodell aus echten Daten: Kreuzberg + Nord-Neukölln (OpenStreetMap, LOR-Grenzen Berlin).
// Die Datei web/data/city.json erzeugt tools/osm/build.mjs; hier wird sie dekodiert und für schnelle
// Abfragen (Kollision, Untergrund, Straßennamen, Sichtbarkeit) in Raster-Hashes einsortiert.
import { undelta, pointInRing, pointInRings, signedArea, segDist2, bboxOf, polylineLength, projectOnPolyline } from './geom.js';
import { SpatialHash } from './collision.js';
import { AREA_KIND, BUILDING_KIND, TREE_TRUNK_M, TREE_FREE_MAX_CLASS, POI_CATS } from './citycodes.js';

export const T = { ROAD: 0, SIDEWALK: 1, BUILDING: 2, GRASS: 3, WATER: 4, PLAZA: 5 };

const GREEN = new Set([AREA_KIND.grass, AREA_KIND.wood, AREA_KIND.cemetery, AREA_KIND.allotments, AREA_KIND.pitch, AREA_KIND.sand]);

// Deterministischer Hash für Zufallswerte je Objekt (Farben, Baumgrößen).
export function hash01(n) {
  let t = (n + 0x6d2b79f5) >>> 0;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

export function decodeCity(json) {
  const S = json.meta.scale;
  const V = json.vertices;
  const city = {
    meta: json.meta, scale: S, width: json.meta.width, height: json.meta.height,
    attribution: json.meta.attribution,
    nodes: [], edges: [], paths: [], rails: [], buildings: [], water: [], areas: [], walls: [],
    trees: [], kieze: json.kieze ?? [], border: [], districts: [], places: {}, crates: [],
  };
  const bb = (pts) => bboxOf(pts, {});

  for (let i = 0; i < V.length / 2; i++) city.nodes.push({ id: i, x: V[2 * i], y: V[2 * i + 1], edges: [] });
  json.edges.forEach(([a, b, cls, w, n, o, br, inside, p], id) => {
    const pts = [V[2 * a], V[2 * a + 1], ...undelta(p), V[2 * b], V[2 * b + 1]];
    const e = { id, a, b, cls, w: w / 10 * S, name: n >= 0 ? json.names[n] : '', oneway: o, bridge: !!br, inside: !!inside,
      pts, len: polylineLength(pts), bbox: bb(pts), layer: 'edge' };
    city.edges.push(e);
    city.nodes[a].edges.push(id);
    if (b !== a) city.nodes[b].edges.push(id);
  });
  city.paths = json.paths.map(([br, p]) => { const pts = undelta(p); return { bridge: !!br, pts, bbox: bb(pts), layer: 'path' }; });
  city.rails = json.rails.map(([br, sub, p]) => { const pts = undelta(p); return { bridge: !!br, subway: !!sub, pts, bbox: bb(pts), layer: 'rail' }; });
  city.buildings = json.buildings.map(([h, kind, rings], id) => {
    const rs = rings.map(undelta);
    const o = rs[0];
    let cx = 0, cy = 0; for (let i = 0; i < o.length; i += 2) { cx += o[i]; cy += o[i + 1]; }
    return { id, kind, meters: h / 10, height: h / 10 * S, rings: rs, outer: rs.map((_, i) => i === 0), sign: rs.map(signedArea).map(Math.sign),
      cx: cx / (o.length / 2), cy: cy / (o.length / 2), bbox: bb(o), seed: Math.floor(hash01(id * 7 + 3) * 1e9), layer: 'building' };
  });
  city.water = json.water.map((rings) => {
    const rs = rings.map(([, p]) => undelta(p));
    return { rings: rs, bbox: bb(rs.flat()), layer: 'water' };
  });
  city.areas = json.areas.map(([kind, rings]) => {
    const rs = rings.map(([, p]) => undelta(p));
    return { kind, rings: rs, bbox: bb(rs.flat()), layer: 'area' };
  });
  city.walls = json.walls.map(undelta);
  city.border = json.border.map(undelta);
  city.districts = json.districts.map((d) => ({ name: d.n, rings: d.r.map(undelta) }));
  const tr = undelta(json.trees);
  for (let i = 0; i < tr.length; i += 2) {
    const u = hash01(i + 11);
    city.trees.push({ x: tr[i], y: tr[i + 1], r: TREE_TRUNK_M * S, size: (2.2 + u * 1.4) * S, layer: 'tree' });
  }
  // POIs (Geschäfte, Gastronomie, Haltestellen …) und Hausnummern
  city.pois = (json.pois ?? []).map(([x, y, c, n, k]) => ({ x, y, cat: POI_CATS[c], name: json.names[n], kind: json.names[k], layer: 'poi' }));
  city.addresses = [];
  if (json.addresses) {
    const xy = undelta(json.addresses.xy);
    json.addresses.nr.forEach((nr, i) => city.addresses.push({ x: xy[2 * i], y: xy[2 * i + 1], street: json.names[json.addresses.street[i]], nr }));
  }
  city.poiHash = new SpatialHash(400);
  for (const q of city.pois) city.poiHash.insert(q, { x: q.x, y: q.y, w: 0, h: 0 });
  city.addrHash = new SpatialHash(400);
  for (const a of city.addresses) city.addrHash.insert(a, { x: a.x, y: a.y, w: 0, h: 0 });

  const pl = json.places;
  city.places = { giver: pl.giver, playerSpawn: pl.playerSpawn, dropoff: pl.dropoff, playerCar: pl.playerCar, pickup: pl.pickup };
  city.parked = pl.parked ?? [];
  city.crates = pl.crates ?? [];
  city.timeLimit = pl.timeLimit;

  // --- Raster-Hashes ---------------------------------------------------------------------
  city.render = new SpatialHash(640);
  for (const list of [city.areas, city.water, city.edges, city.paths, city.rails, city.buildings]) for (const f of list) city.render.insert(f, f.bbox);

  city.edgeSegs = new SpatialHash(320);
  for (const e of city.edges) for (let i = 0; i < e.pts.length - 2; i += 2) {
    const s = { e, i, ax: e.pts[i], ay: e.pts[i + 1], bx: e.pts[i + 2], by: e.pts[i + 3] };
    const r = e.w / 2 + S; // 1 m Rand: Abfragen mit Abstand (Baumstamm, Gehweg) finden die Kante sicher
    city.edgeSegs.insert(s, { x: Math.min(s.ax, s.bx) - r, y: Math.min(s.ay, s.by) - r, w: Math.abs(s.bx - s.ax) + 2 * r, h: Math.abs(s.by - s.ay) + 2 * r });
  }
  city.polys = new SpatialHash(320);
  for (const list of [city.buildings, city.water, city.areas]) for (const f of list) city.polys.insert(f, f.bbox);

  // Statische Hindernisse: Wandsegmente, Bäume (Kreise), Kisten (Rechtecke).
  city.solids = new SpatialHash(128);
  const addLine = (pts, closed, kind) => {
    const n = pts.length;
    for (let i = 0; i < n - 2 + (closed ? 2 : 0); i += 2) {
      const ax = pts[i], ay = pts[i + 1], bx = pts[(i + 2) % n], by = pts[(i + 3) % n];
      if (ax === bx && ay === by) continue;
      const s = { ax, ay, bx, by, seg: true, kind };
      city.solids.insert(s, { x: Math.min(ax, bx), y: Math.min(ay, by), w: Math.abs(bx - ax), h: Math.abs(by - ay) });
    }
  };
  for (const b of city.buildings) for (const r of b.rings) addLine(r, true, 'building');
  for (const w of city.walls) addLine(w, false, 'wall');
  for (const r of city.border) addLine(r, true, 'border');
  // Sicherheitsnetz: Bäume, deren Stamm auf der Fahrbahn stünde, fallen weg (der Build verhindert das bereits).
  city.droppedTrees = city.trees.length;
  city.trees = city.trees.filter((t) => !treeOnRoad(city, t));
  city.droppedTrees -= city.trees.length;
  for (const t of city.trees) {
    city.solids.insert(t, { x: t.x - t.r, y: t.y - t.r, w: 2 * t.r, h: 2 * t.r });
    city.render.insert(t, { x: t.x - t.size, y: t.y - t.size * 2, w: t.size * 2, h: t.size * 3 });
  }
  for (const c of city.crates) city.solids.insert(c, c);
  return city;
}

// --- Abfragen ----------------------------------------------------------------------------
const tmp = [];
const pt = { x: 0, y: 0, w: 0, h: 0 };

export function onRoad(city, x, y, margin = 0) {
  pt.x = x; pt.y = y;
  for (const s of city.edgeSegs.query(pt, tmp)) {
    const r = s.e.w / 2 + margin;
    if (segDist2(x, y, s.ax, s.ay, s.bx, s.by) <= r * r) return s.e;
  }
  return null;
}

// Steht der Baumstamm (ganz oder teilweise) auf einer Fahrbahn? Liefert die Kante oder null.
export function treeOnRoad(city, t) {
  pt.x = t.x; pt.y = t.y;
  for (const s of city.edgeSegs.query(pt, tmp)) {
    if (s.e.cls > TREE_FREE_MAX_CLASS) continue;
    const r = s.e.w / 2 + t.r;
    if (segDist2(t.x, t.y, s.ax, s.ay, s.bx, s.by) < r * r) return s.e;
  }
  return null;
}

export function inBuilding(city, x, y) {
  pt.x = x; pt.y = y;
  for (const f of city.polys.query(pt, tmp)) if (f.layer === 'building' && pointInRings(x, y, f.rings)) return f;
  return null;
}

export function surfaceAt(city, x, y) {
  if (x < 0 || y < 0 || x > city.width || y > city.height) return T.BUILDING;
  if (onRoad(city, x, y)) return T.ROAD;
  pt.x = x; pt.y = y;
  let best = T.SIDEWALK;
  for (const f of city.polys.query(pt, tmp)) {
    if (!pointInRings(x, y, f.rings)) continue;
    if (f.layer === 'building') return T.BUILDING;
    if (f.layer === 'water') best = T.WATER;
    else if (best !== T.WATER) best = GREEN.has(f.kind) ? T.GRASS : T.PLAZA;
  }
  return best;
}

export function insideBorder(city, x, y) { return pointInRings(x, y, city.border); }

export function districtAt(city, x, y) {
  for (const d of city.districts) if (pointInRings(x, y, d.rings)) return d.name;
  return null;
}

// Nächste Kante (optional gefiltert) im Umkreis: { e, s, d, x, y, ux, uy }.
export function nearestEdge(city, x, y, radius, filter = () => true) {
  const box = { x: x - radius, y: y - radius, w: 2 * radius, h: 2 * radius };
  let best = null;
  for (const s of city.edgeSegs.query(box, [])) {
    if (!filter(s.e)) continue;
    const d2 = segDist2(x, y, s.ax, s.ay, s.bx, s.by);
    if (d2 > radius * radius || (best && d2 >= best.d2)) continue;
    best = { e: s.e, d2 };
  }
  if (!best) return null;
  const p = projectOnPolyline(best.e.pts, x, y);
  return { e: best.e, s: p.s, d: Math.sqrt(p.d2), x: p.x, y: p.y, ux: p.ux, uy: p.uy };
}

// Nächste Hausnummer (optional nur an einer bestimmten Straße) im Umkreis.
export function nearestAddress(city, x, y, radius, street = null) {
  let best = null, bd = radius;
  for (const a of city.addrHash.query({ x: x - radius, y: y - radius, w: 2 * radius, h: 2 * radius }, [])) {
    if (street && a.street !== street) continue;
    const d = Math.hypot(a.x - x, a.y - y);
    if (d < bd) { bd = d; best = a; }
  }
  return best;
}

// Nächster POI (optional gefiltert) im Umkreis.
export function nearestPoi(city, x, y, radius, filter = () => true) {
  let best = null, bd = radius;
  for (const q of city.poiHash.query({ x: x - radius, y: y - radius, w: 2 * radius, h: 2 * radius }, [])) {
    if (!filter(q)) continue;
    const d = Math.hypot(q.x - x, q.y - y);
    if (d < bd) { bd = d; best = q; }
  }
  return best;
}

export function locationName(city, x, y) {
  const S = city.scale;
  const near = nearestEdge(city, x, y, 25 * S, (e) => !!e.name && e.cls <= 10);
  if (near && near.d <= near.e.w / 2 + 8 * S) {
    // An einer Kreuzung beide Straßen nennen.
    for (const nid of [near.e.a, near.e.b]) {
      const nd = city.nodes[nid];
      if (Math.hypot(nd.x - x, nd.y - y) > near.e.w / 2 + 6 * S) continue;
      const other = nd.edges.map((k) => city.edges[k]).find((e) => e.name && e.name !== near.e.name && e.cls <= 8);
      if (other) return `${near.e.name} / ${other.name}`;
    }
    // Hausnummer dazu, wenn ein Haus dieser Straße in der Nähe ist
    const addr = nearestAddress(city, x, y, near.e.w / 2 + 25 * S, near.e.name);
    return addr ? `${near.e.name} ${addr.nr}` : near.e.name;
  }
  let kiez = null, kd = 700 * S;
  for (const k of city.kieze) { const d = Math.hypot(k.x - x, k.y - y); if (d < kd) { kd = d; kiez = k.n; } }
  return kiez ?? districtAt(city, x, y) ?? 'Berlin';
}

export { pointInRing, BUILDING_KIND };
