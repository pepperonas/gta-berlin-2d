// Stadtmodell aus echten Daten: ganz Berlin (OpenStreetMap, LOR-Grenzen, Baumbestand).
// tools/osm/build.mjs legt die Karte in web/data/berlin/ ab: index.json (Grenzen, Bezirke, Ortsteile, Missionsorte),
// overview.json (Stadtplan) und je 640 × 640 m eine Kachel. Die Stadt hier lädt nur die Kacheln um die Kamera
// (city.focus), setzt sie zu einem Modell zusammen und gibt ferne Kacheln wieder frei. Abfragen (Kollision,
// Untergrund, Straßennamen, Sichtbarkeit) laufen über Raster-Hashes wie bei einer fest geladenen Karte.
//
// Zusammensetzen (Gegenstück: tools/osm/tiles.mjs): Straßen, Gebäude, Linien und kleine Flächen liegen in jeder
// Kachel, die sie berühren, und tragen eine globale Nummer – sie werden nur einmal angelegt und erst entfernt, wenn
// keine Kachel mehr sie hält. Punkte gehören genau einer Kachel, große Flächen sind je Kachel abgeschnitten.
import { undelta, pointInRings, signedArea, segDist2, bboxOf, polylineLength, projectOnPolyline, ringIndex, insideIndex, pointInRing } from './geom.js';
import { SpatialHash } from './collision.js';
import { WALL_KIND, AREA_KIND, BUILDING_KIND, TREE_TRUNK_M, TREE_FREE_MAX_CLASS, POI_CATS, PARK_ORIENT, SURFACE, TREE_GENERA } from './citycodes.js';

const WALL_NAMES = Object.fromEntries(Object.entries(WALL_KIND).map(([k, v]) => [v, k]));

export const T = { ROAD: 0, SIDEWALK: 1, BUILDING: 2, GRASS: 3, WATER: 4, PLAZA: 5, COBBLE: 6 };
export const isRoadSurface = (t) => t === T.ROAD || t === T.COBBLE; // Fahrbahn (Asphalt oder Pflaster)

// Nachladen (px um den Fokus): bis READY muss alles da sein, bis LOAD wird vorgeladen, jenseits KEEP freigegeben.
export const STREAM = { ready: 4000, load: 7000, keep: 11000, ttl: 900 };

const GREEN = new Set([AREA_KIND.grass, AREA_KIND.wood, AREA_KIND.cemetery, AREA_KIND.allotments, AREA_KIND.pitch, AREA_KIND.sand]);

// Deterministischer Hash für Zufallswerte je Objekt (Farben, Baumgrößen).
export function hash01(n) {
  let t = (n + 0x6d2b79f5) >>> 0;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

// index: web/data/berlin/index.json; loadTile(key) liefert die Kachel (Objekt) oder ein Promise darauf.
export function openCity(index, loadTile, { overview = null } = {}) {
  const m = index.meta;
  if (m.version !== 3) throw new Error(`Kartenformat ${m.version} wird nicht unterstützt (erwartet 3) – npm run map:build`);
  const S = m.scale;
  const pl = index.places;
  const city = {
    meta: m, scale: S, width: m.width, height: m.height, tile: m.tile, attribution: m.attribution,
    border: index.border.map(undelta),
    bezirke: index.bezirke.map((d) => ({ name: d.n, rings: d.r.map(undelta) })),
    districts: index.districts.map((d) => ({ name: d.n, rings: d.r.map(undelta) })),
    kieze: index.kieze ?? [],
    places: { giver: pl.giver, playerSpawn: pl.playerSpawn, dropoff: pl.dropoff, playerCar: pl.playerCar, pickup: pl.pickup },
    parked: pl.parked ?? [], crates: pl.crates ?? [], timeLimit: pl.timeLimit,
    overview,
    nodes: new Map(), edges: new Map(), signals: new Set(), turnBans: new Set(),
    render: new SpatialHash(640), edgeSegs: new SpatialHash(320), polys: new SpatialHash(320), solids: new SpatialHash(128),
    poiHash: new SpatialHash(400), addrHash: new SpatialHash(400),
    droppedTrees: 0, droppedPosts: 0, gen: 0,
    hooks: { edgeAdd: [], edgeRemove: [] },
    // Nachladen
    available: new Set(index.tiles), tiles: new Map(), inbox: [], reg: new Map(), items: new Map(), focuses: new Map(), clock: 0, loader: loadTile, pinned: new Set(),
    _sig: new Map(), _ban: new Map(),
  };
  city.borderIx = ringIndex(city.border);
  for (const d of [...city.districts, ...city.bezirke]) d.ix = ringIndex(d.rings);
  for (const c of city.crates) city.solids.insert(c, c); // Missionskisten liegen immer da
  city.focus = (key, x, y) => focus(city, key, x, y);
  city.release = (key) => city.focuses.delete(key);
  city.loadArea = (x0, y0, x1, y1, opts) => loadArea(city, x0, y0, x1, y1, opts);
  city.loadAll = (opts) => loadArea(city, 0, 0, city.width, city.height, opts);
  city.status = (x, y) => loadStatus(city, x, y);
  city.pump = (ms = PUMP_MS.playing) => pump(city, ms);
  city.unload = (key) => { city.pinned.delete(key); uninstall(city, key); };
  city.list = (layer) => [...(city.items.get(layer) ?? [])];
  city.ready = (x, y, r = STREAM.ready) => tilesAround(city, x, y, r).every((k) => city.tiles.get(k)?.state === 'ready');
  return city;
}

// Build-Ergebnis im Speicher ({ index, tiles: Map }) vollständig geladen – für Tests und Werkzeuge.
export function decodeCity(built) {
  const city = openCity(built.index, (k) => built.tiles.get(k), { overview: built.overview ?? null });
  city.loadAll({ pin: true });
  return city;
}

// --- Nachladen -------------------------------------------------------------------------------
function tilesAround(city, x, y, r) {
  const T = city.tile, out = [];
  const nx = city.meta.tilesX, ny = city.meta.tilesY;
  for (let tx = Math.max(0, Math.floor((x - r) / T)); tx <= Math.min(nx - 1, Math.floor((x + r) / T)); tx++) {
    for (let ty = Math.max(0, Math.floor((y - r) / T)); ty <= Math.min(ny - 1, Math.floor((y + r) / T)); ty++) {
      const k = `${tx}_${ty}`;
      if (!city.available.has(k)) continue;
      const dx = Math.max(tx * T - x, 0, x - (tx + 1) * T), dy = Math.max(ty * T - y, 0, y - (ty + 1) * T);
      if (dx * dx + dy * dy <= r * r) out.push(k);
    }
  }
  return out;
}

// Fehlgeschlagene Kacheln erst nach einer kurzen Pause erneut anfragen (sonst jedes Bild eine Anfrage und eine
// Fehlermeldung); kurz genug, dass das Spiel nach einem Neustart des Servers gleich weiterläuft.
export const RETRY_MS = [500, 1000, 2000, 3000];
// Eingetroffene Kacheln werden nicht auf einmal eingebaut, sondern in Zeitscheiben (ms je Aufruf von focus): beim
// Fahren kaum spürbar, solange die Welt wartet deutlich mehr.
export const PUMP_MS = { playing: 4, waiting: 30 };
const now = () => (globalThis.performance?.now ? performance.now() : Date.now());

function request(city, key) {
  const old = city.tiles.get(key);
  if (old && !(old.state === 'failed' && Date.now() >= old.retryAt)) return;
  const res = city.loader(key);
  if (res && typeof res.then === 'function') {
    const entry = { state: 'loading', fails: old?.fails ?? 0 };
    city.tiles.set(key, entry);
    res.then((json) => { if (city.tiles.get(key) === entry) { entry.state = 'arrived'; city.inbox.push({ key, entry, json }); } })
      .catch((err) => { if (city.tiles.get(key) === entry) failed(city, key, entry, err); });
  } else if (res) install(city, key, res);
}

function failed(city, key, entry, err) {
  const fails = entry.fails + 1;
  city.tiles.set(key, { state: 'failed', fails, retryAt: Date.now() + RETRY_MS[Math.min(fails, RETRY_MS.length) - 1], error: String(err?.message ?? err) });
  if (fails === 1) console.error(`Kachel ${key}:`, err); // einmal melden, nicht bei jedem neuen Versuch
}

// Eingetroffene Kacheln einbauen, die nächsten zuerst, bis das Zeitbudget verbraucht ist (mindestens eine).
function pump(city, budgetMs) {
  if (!city.inbox.length) return 0;
  const fs = [...city.focuses.values()], T = city.tile;
  const dist = ({ key }) => { const [tx, ty] = key.split('_').map(Number), cx = (tx + 0.5) * T, cy = (ty + 0.5) * T; let d = Infinity; for (const f of fs) d = Math.min(d, Math.hypot(f.x - cx, f.y - cy)); return d; };
  city.inbox.sort((a, b) => dist(a) - dist(b));
  const t0 = now();
  let n = 0;
  while (city.inbox.length && (n === 0 || now() - t0 < budgetMs)) {
    const { key, entry, json } = city.inbox.shift();
    if (city.tiles.get(key) !== entry) continue; // inzwischen verworfen
    try { install(city, key, json); n++; } catch (err) { failed(city, key, entry, err); }
  }
  return n;
}

// Fokus (Kamera einer Welt, Teleport-Ziel …): lädt ringsum nach und gibt Fernes frei. true = alles Nötige geladen.
function focus(city, key, x, y) {
  city.focuses.set(key, { x, y, stamp: ++city.clock });
  for (const k of tilesAround(city, x, y, STREAM.load)) request(city, k);
  if (city.clock % 30 === 0) evict(city);
  if (city.inbox.length) pump(city, city.ready(x, y) ? PUMP_MS.playing : PUMP_MS.waiting);
  return city.ready(x, y);
}

// Stand um einen Punkt (für die Ladeanzeige): wie viele Kacheln nötig, schon da, fehlgeschlagen.
function loadStatus(city, x, y) {
  const keys = tilesAround(city, x, y, STREAM.ready);
  const st = keys.map((k) => city.tiles.get(k));
  const failedTiles = st.filter((t) => t?.state === 'failed');
  return { needed: keys.length, ready: st.filter((t) => t?.state === 'ready').length, failed: failedTiles.length,
    retryIn: failedTiles.length ? Math.max(0, Math.min(...failedTiles.map((t) => t.retryAt)) - Date.now()) : 0 };
}

function evict(city) {
  const live = [...city.focuses.values()].filter((f) => f.stamp > city.clock - STREAM.ttl);
  for (const [key, f] of city.focuses) if (f.stamp <= city.clock - STREAM.ttl) city.focuses.delete(key);
  const keep = new Set(city.pinned);
  for (const f of live) for (const k of tilesAround(city, f.x, f.y, STREAM.keep)) keep.add(k);
  for (const [k, t] of city.tiles) if (!keep.has(k)) { if (t.state === 'ready') uninstall(city, k); else city.tiles.delete(k); }
}

function loadArea(city, x0, y0, x1, y1, { pin = false } = {}) {
  const T = city.tile;
  for (let tx = Math.max(0, Math.floor(x0 / T)); tx <= Math.floor(x1 / T); tx++) for (let ty = Math.max(0, Math.floor(y0 / T)); ty <= Math.floor(y1 / T); ty++) {
    const k = `${tx}_${ty}`;
    if (!city.available.has(k)) continue;
    if (pin) city.pinned.add(k);
    if (city.tiles.get(k)?.state === 'ready') continue;
    const res = city.loader(k);
    if (res && typeof res.then === 'function') throw new Error('loadArea braucht einen synchronen Kachel-Lader');
    if (res) install(city, k, res);
  }
  return city;
}

// --- Einträge anlegen und wieder entfernen ---------------------------------------------------
// Ein Eintrag hält ein Kartenobjekt samt seinen Hash-Einträgen; drop() räumt Verweise außerhalb der Hashes auf.
function put(entry, hash, item, box) { entry.ins.push(hash, item, hash.insert(item, box)); }
function track(city, layer, item) { let s = city.items.get(layer); if (!s) city.items.set(layer, s = new Set()); s.add(item); }
function untrack(city, layer, item) { city.items.get(layer)?.delete(item); }

function acquire(city, t, key, create) {
  let r = city.reg.get(key);
  if (!r) { r = { refs: 0, ins: [], drop: null }; city.reg.set(key, r); create(r); }
  r.refs++;
  t.shared.push(key);
}
function own(t, create) { const r = { ins: [], drop: null }; t.local.push(r); create(r); }
function destroy(r) {
  for (let i = 0; i < r.ins.length; i += 3) r.ins[i].remove(r.ins[i + 1], r.ins[i + 2]);
  r.drop?.();
}

function install(city, key, json) {
  const S = city.scale, names = json.names;
  const t = { state: 'ready', shared: [], local: [], signals: json.signals ?? [], bans: [] };
  city.tiles.set(key, t);
  const [tx, ty] = json.t, T = city.tile;
  const clipRect = { x0: tx * T - S, y0: ty * T - S, x1: (tx + 1) * T + S, y1: (ty + 1) * T + S };
  const nm = (i) => (i >= 0 ? names[i] : '');

  // Knoten der Kachel (global nummeriert) – angelegt erst mit ihrer ersten Kante
  const vid = json.vertices.id, vxy = undelta(json.vertices.xy), vtrim = json.vertices.trim;
  const node = (i) => {
    const id = vid[i];
    let nd = city.nodes.get(id);
    if (!nd) city.nodes.set(id, nd = { id, x: vxy[2 * i], y: vxy[2 * i + 1], edges: [], trim: vtrim[i] });
    return nd;
  };

  for (const [gid, ia, ib, cls, w, n, o, flags, p, x0] of json.edges) acquire(city, t, 'e' + gid, (r) => {
    const A = node(ia), B = node(ib);
    const pts = [A.x, A.y, ...undelta(p), B.x, B.y];
    const d = (dm) => dm / 10 * S; // dm → px
    // Querschnitt in px: Fahrstreifen je Richtung, Park-/Radstreifen je Seite (links/rechts in Kantenrichtung)
    const x = x0.length === 2 ? [o === -1 ? 0 : 1, o === 1 ? 0 : 1, 0, 0, 0, 0, 0, 0, 0, 0, x0[0], x0[1], 0] : x0; // Nebenweg: Standardquerschnitt
    const cs = { width: d(w), fwd: x[0], bwd: x[1],
      left: { park: x[2], parkW: d(x[3]), orient: PARK_ORIENT[x[4]] ?? 'parallel', cycle: d(x[8]) },
      right: { park: x[5], parkW: d(x[6]), orient: PARK_ORIENT[x[7]] ?? 'parallel', cycle: d(x[9]) },
      maxspeed: x[10], surface: x[11], lit: !!(x[12] & 1), gaslight: !!(x[12] & 2) };
    const e = { id: gid, a: A.id, b: B.id, cls, w: d(w), cs, name: nm(n), oneway: o, bridge: !!(flags & 1), inside: !!(flags & 2),
      blocked: !!(flags & 4), passage: !!(flags & 8), pts, len: polylineLength(pts), bbox: bboxOf(pts, {}), layer: 'edge' };
    city.edges.set(gid, e);
    addSorted(A.edges, gid); // nach Nummer sortiert: gleiche Reihenfolge, egal in welcher Folge Kacheln laden
    if (B !== A) addSorted(B.edges, gid);
    put(r, city.render, e, e.bbox);
    for (let i = 0; i < pts.length - 2; i += 2) {
      const sg = { e, i, ax: pts[i], ay: pts[i + 1], bx: pts[i + 2], by: pts[i + 3] };
      const rr = e.w / 2 + S; // 1 m Rand: Abfragen mit Abstand (Baumstamm, Gehweg) finden die Kante sicher
      put(r, city.edgeSegs, sg, { x: Math.min(sg.ax, sg.bx) - rr, y: Math.min(sg.ay, sg.by) - rr, w: Math.abs(sg.bx - sg.ax) + 2 * rr, h: Math.abs(sg.by - sg.ay) + 2 * rr });
    }
    track(city, 'edge', e);
    for (const h of city.hooks.edgeAdd) h(e);
    r.drop = () => {
      for (const h of city.hooks.edgeRemove) h(e);
      for (const nd of [A, B]) {
        const i = nd.edges.indexOf(gid);
        if (i >= 0) nd.edges.splice(i, 1);
        if (!nd.edges.length && city.nodes.get(nd.id) === nd) city.nodes.delete(nd.id);
      }
      city.edges.delete(gid);
      untrack(city, 'edge', e);
    };
  });

  // Kreuzungsflächen: Asphalt bis zum Eckradius, sonst schneiden abbiegende Autos über den Bordstein.
  for (const [v, x, y, rad, fl, cls] of json.junctions) acquire(city, t, 'j' + v, (r) => {
    const j = { x, y, r: rad, node: v, bridge: !!(fl & 1), cobble: !!(fl & 2), layer: 'junction' };
    const box = { x: x - rad, y: y - rad, w: 2 * rad, h: 2 * rad };
    put(r, city.render, j, box);
    put(r, city.edgeSegs, { e: { w: 2 * rad, cls, cs: { surface: j.cobble ? SURFACE.cobble : SURFACE.asphalt }, junction: j }, ax: x, ay: y, bx: x, by: y }, box);
    track(city, 'junction', j);
    r.drop = () => untrack(city, 'junction', j);
  });

  const line = (layer, props, p) => { const pts = undelta(p); return { ...props, pts, bbox: bboxOf(pts, {}), layer }; };
  for (const [gid, fl, p] of json.paths) acquire(city, t, 'g' + gid, (r) => { const f = line('path', { bridge: !!(fl & 1), passage: !!(fl & 2) }, p); put(r, city.render, f, f.bbox); track(city, 'path', f); r.drop = () => untrack(city, 'path', f); });
  for (const [gid, br, sub, p] of json.rails) acquire(city, t, 'g' + gid, (r) => { const f = line('rail', { bridge: !!br, subway: !!sub }, p); put(r, city.render, f, f.bbox); track(city, 'rail', f); r.drop = () => untrack(city, 'rail', f); });
  for (const [gid, k, p] of json.fences) acquire(city, t, 'g' + gid, (r) => { const f = line('fence', { kind: k }, p); put(r, city.render, f, f.bbox); track(city, 'fence', f); r.drop = () => untrack(city, 'fence', f); });

  // Wände (Ufer, Gleise, Geländer, Zäune) und die Stadtgrenze: nur Kollision
  const addLine = (r, pts, closed, kind) => {
    const n = pts.length;
    for (let i = 0; i < n - 2 + (closed ? 2 : 0); i += 2) {
      const ax = pts[i], ay = pts[i + 1], bx = pts[(i + 2) % n], by = pts[(i + 3) % n];
      if (ax === bx && ay === by) continue;
      put(r, city.solids, { ax, ay, bx, by, seg: true, kind }, { x: Math.min(ax, bx), y: Math.min(ay, by), w: Math.abs(bx - ax), h: Math.abs(by - ay) });
    }
  };
  for (const [gid, kind, p] of json.walls) acquire(city, t, 'g' + gid, (r) => {
    // kind: Kollisionsart ('border' für die Stadtgrenze), sub: Art der Wand (Ufer, Gleis, Geländer, Zaun)
    const pts = undelta(p), f = { pts, kind: kind === WALL_KIND.border ? 'border' : 'wall', sub: WALL_NAMES[kind] ?? 'other', layer: 'wall' };
    addLine(r, pts, false, f.kind); track(city, 'wall', f); r.drop = () => untrack(city, 'wall', f);
  });

  for (const [gid, h, kind, rings, walls, doors] of json.buildings) acquire(city, t, 'g' + gid, (r) => {
    const rs = rings.map(undelta), o = rs[0];
    let cx = 0, cy = 0; for (let i = 0; i < o.length; i += 2) { cx += o[i]; cy += o[i + 1]; }
    const b = { id: gid, kind, meters: h / 10, height: h / 10 * S, rings: rs, outer: rs.map((_, i) => i === 0), sign: rs.map(signedArea).map(Math.sign),
      cx: cx / (o.length / 2), cy: cy / (o.length / 2), bbox: bboxOf(o, {}), seed: Math.floor(hash01(gid * 7 + 3) * 1e9), layer: 'building',
      walls: walls ? walls.map(undelta) : null, // eigene Wandzüge, wo eine Tordurchfahrt die Fassade öffnet
      doors: doors || null }; // [Ring, Kante, Anteil ×1000]
    put(r, city.render, b, b.bbox); put(r, city.polys, b, b.bbox);
    if (b.walls) for (const w of b.walls) addLine(r, w, false, 'building');
    else for (const ring of b.rings) addLine(r, ring, true, 'building');
    track(city, 'building', b); r.drop = () => untrack(city, 'building', b);
  });

  // Flächen: kleine mehrfach abgelegt (gid), große je Kachel abgeschnitten (gid −1, nur in dieser Kachel)
  const area = (gid, props, rings, layer) => {
    const create = (r) => {
      const rs = rings.map(([, p]) => undelta(p));
      const f = { ...props, rings: rs, bbox: bboxOf(rs.flat(), {}), layer, clip: gid < 0 ? clipRect : null };
      put(r, city.render, f, f.bbox); put(r, city.polys, f, f.bbox);
      track(city, layer, f); r.drop = () => untrack(city, layer, f);
    };
    if (gid >= 0) acquire(city, t, 'g' + gid, create); else own(t, create);
  };
  for (const [gid, rings] of json.water) area(gid, {}, rings, 'water');
  for (const [gid, kind, rings] of json.areas) area(gid, { kind }, rings, 'area');

  // Poller und Tore
  const B = json.barriers;
  for (let i = 0; i < B.length; i += 6) {
    const [x, y, kind, qx, qy, n] = B.slice(i, i + 6);
    for (let k = 0; k < n; k++) {
      const o = (k - (n - 1) / 2) * 1.8 * S;
      own(t, (r) => barrier(city, r, x + qx / 1000 * o, y + qy / 1000 * o, kind));
    }
  }
  for (let i = 0; i < json.posts.length; i += 3) own(t, (r) => barrier(city, r, json.posts[i], json.posts[i + 1], json.posts[i + 2]));

  // Querungen (Zebrastreifen, Ampelübergänge, markierte Überwege)
  const C = json.crossings;
  for (let i = 0; i < C.length; i += 4) {
    const e = city.edges.get(C[i + 2]);
    if (!e) continue;
    own(t, (r) => {
      const x = C[i], y = C[i + 1], pr = projectOnPolyline(e.pts, x, y);
      const c = { x, y, edge: e, s: pr.s, ux: pr.ux, uy: pr.uy, kind: ['zebra', 'signal', 'marked'][C[i + 3]], layer: 'crossing' };
      put(r, city.render, c, { x: x - 10, y: y - 10, w: 20, h: 20 });
      (e.crossings ??= []).push(c);
      track(city, 'crossing', c); r.drop = () => { untrack(city, 'crossing', c); e.crossings.splice(e.crossings.indexOf(c), 1); };
    });
  }

  // Ampeln und Abbiegeverbote (Knoten gehören genau einer Kachel)
  for (const v of t.signals) countAdd(city._sig, city.signals, v);
  for (let i = 0; i < json.turnBans.length; i += 3) { const k = `${json.turnBans[i]}>${json.turnBans[i + 1]}>${json.turnBans[i + 2]}`; t.bans.push(k); countAdd(city._ban, city.turnBans, k); }

  // POIs und Hausnummern
  for (const [x, y, c, n, k] of json.pois) own(t, (r) => {
    const q = { x, y, cat: POI_CATS[c], name: nm(n), kind: nm(k), layer: 'poi' };
    put(r, city.poiHash, q, { x, y, w: 0, h: 0 }); track(city, 'poi', q); r.drop = () => untrack(city, 'poi', q);
  });
  const axy = undelta(json.addresses.xy);
  json.addresses.nr.forEach((nr, i) => own(t, (r) => {
    const a = { x: axy[2 * i], y: axy[2 * i + 1], street: nm(json.addresses.street[i]), nr, layer: 'address' };
    put(r, city.addrHash, a, { x: a.x, y: a.y, w: 0, h: 0 }); track(city, 'address', a); r.drop = () => untrack(city, 'address', a);
  }));

  // Bäume: Kataster mit Gattung, Krone und Stamm; OSM-Bäume (Gattung 0, Maße 0) mit Standardwerten.
  // Sicherheitsnetz: Bäume, deren Stamm auf der Fahrbahn stünde, fallen weg (der Build verhindert das bereits).
  const tr = undelta(json.trees.xy);
  for (let i = 0; i < json.trees.g.length; i++) {
    const x = tr[2 * i], y = tr[2 * i + 1], c = json.trees.c[i], rr = json.trees.r[i];
    const seed = Math.floor(hash01(x * 7919 + y) * 1e6);
    const tree = { x, y, r: rr ? rr / 100 * S : TREE_TRUNK_M * S, size: c ? c / 20 * S : (2.2 + hash01(seed + 11) * 1.4) * S,
      genus: TREE_GENERA[json.trees.g[i]] ?? 'sonstige', seed, layer: 'tree' };
    if (treeOnRoad(city, tree)) { city.droppedTrees++; continue; }
    own(t, (r) => {
      put(r, city.solids, tree, { x: x - tree.r, y: y - tree.r, w: 2 * tree.r, h: 2 * tree.r });
      put(r, city.render, tree, { x: x - tree.size, y: y - tree.size * 2, w: tree.size * 2, h: tree.size * 3 });
      track(city, 'tree', tree); r.drop = () => untrack(city, 'tree', tree);
    });
  }
  city.gen++;
}

function barrier(city, r, x, y, kind) {
  const b = { x, y, kind, r: 0.15 * city.scale, layer: 'barrier' };
  // Sicherheitsnetz: kein Poller auf einer befahrbaren Fahrbahn (der Build rückt sie an den Bordstein)
  if (postOnRoad(city, b)) { city.droppedPosts++; return; }
  put(r, city.render, b, { x: x - 10, y: y - 10, w: 20, h: 20 });
  put(r, city.solids, b, { x: x - b.r, y: y - b.r, w: 2 * b.r, h: 2 * b.r });
  track(city, 'barrier', b); r.drop = () => untrack(city, 'barrier', b);
}

function addSorted(list, v) { let i = list.length; while (i > 0 && list[i - 1] > v) i--; list.splice(i, 0, v); }

function countAdd(counts, set, k) { counts.set(k, (counts.get(k) ?? 0) + 1); set.add(k); }
function countDel(counts, set, k) { const n = (counts.get(k) ?? 1) - 1; if (n > 0) counts.set(k, n); else { counts.delete(k); set.delete(k); } }

function uninstall(city, key) {
  const t = city.tiles.get(key);
  city.tiles.delete(key);
  if (!t || t.state !== 'ready') return;
  for (const r of t.local) destroy(r);
  // In umgekehrter Reihenfolge: Kanten zuletzt (Querungen verweisen auf sie)
  for (let i = t.shared.length - 1; i >= 0; i--) {
    const k = t.shared[i], r = city.reg.get(k);
    if (!r || --r.refs > 0) continue;
    city.reg.delete(k);
    destroy(r);
  }
  for (const v of t.signals) countDel(city._sig, city.signals, v);
  for (const k of t.bans) countDel(city._ban, city.turnBans, k);
  city.gen++;
}

// Winkel zwischen zwei Kanten an einem Knoten (0 = geradeaus weiter).
export function turnBetween(a, b, n) {
  const dir = (e, atStart) => { const p = e.pts, i = atStart ? 0 : p.length - 4; return Math.atan2(p[i + 3] - p[i + 1], p[i + 2] - p[i]); };
  const inA = a.b === n ? dir(a, false) : dir(a, true) + Math.PI, outB = b.a === n ? dir(b, true) : dir(b, false) + Math.PI;
  let d = outB - inA; while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI;
  return d;
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

// Steht ein Poller auf einer Fahrbahn, auf der die KI fährt (oder einer Kreuzungsfläche)? Pollerreihen auf gesperrten
// Straßen zählen nicht – sie sind die Sperre.
export function postOnRoad(city, b) {
  pt.x = b.x; pt.y = b.y;
  for (const s of city.edgeSegs.query(pt, tmp)) {
    const e = s.e;
    if (!e.junction && (e.cls > 8 || e.blocked || e.passage)) continue;
    const r = e.w / 2 + b.r;
    if (segDist2(b.x, b.y, s.ax, s.ay, s.bx, s.by) < r * r) return e;
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
  const road = onRoad(city, x, y);
  if (road) return road.cs.surface === SURFACE.cobble ? T.COBBLE : T.ROAD;
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

export function insideBorder(city, x, y) { return insideIndex(city.borderIx, x, y); }

// Ortsteil (z. B. „Kreuzberg“), sonst Bezirk
export function districtAt(city, x, y) {
  for (const d of city.districts) if (insideIndex(d.ix, x, y)) return d.name;
  return null;
}
export function bezirkAt(city, x, y) {
  for (const d of city.bezirke) if (insideIndex(d.ix, x, y)) return d.name;
  return null;
}

// Nächste Kante (optional gefiltert) im Umkreis: { e, s, d, x, y, ux, uy }.
export function nearestEdge(city, x, y, radius, filter = () => true) {
  const box = { x: x - radius, y: y - radius, w: 2 * radius, h: 2 * radius };
  let best = null;
  for (const s of city.edgeSegs.query(box, [])) {
    if (s.e.junction || !filter(s.e)) continue;
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
      const nd = city.nodes.get(nid);
      if (!nd || Math.hypot(nd.x - x, nd.y - y) > near.e.w / 2 + 6 * S) continue;
      const other = nd.edges.map((k) => city.edges.get(k)).find((e) => e && e.name && e.name !== near.e.name && e.cls <= 8);
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
