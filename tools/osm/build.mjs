// Baut aus den Rohdaten (data/raw/) die Spielkarte web/data/city.json.
//   node tools/osm/build.mjs [--scale 10] [--out web/data/city.json]
// Einheit im Spiel: px; Standard 10 px = 1 m (siehe web/src/config.js).
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';
import { crossSection, maxspeedOf, surfaceOf } from './crosssection.mjs';
import { makeProjection, pointInRing, ringArea, simplify, segDist2, joinRings, unionOutline, delta } from './geo.mjs';
import { ROAD_CLASS, ROAD_CLASSES, TRAFFIC_MAX_CLASS, AREA_KIND, BUILDING_KIND, TREE_TRUNK_M, TREE_FREE_MAX_CLASS, POI_CAT, PARK, PARK_ORIENT, TREE_GENERA } from '../../web/src/citycodes.js';

const DISTRICT_OF = { '0210': 'Kreuzberg', '0220': 'Kreuzberg', '0230': 'Kreuzberg', '0810': 'Nord-Neukölln' };

const num = (v) => { const m = /^\s*(-?\d+(?:[.,]\d+)?)/.exec(v ?? ''); return m ? parseFloat(m[1].replace(',', '.')) : NaN; };

// Standardbreite der Fahrbahn inkl. Parkstreifen in Metern.
const DEFAULT_WIDTH = { motorway: 11, trunk: 15, primary: 16, secondary: 13, tertiary: 11, unclassified: 8, residential: 9,
  living_street: 6, busway: 7, service: 4.5, pedestrian: 6, track: 3.5 };

function roadWidth(t, base) {
  let w = DEFAULT_WIDTH[base] ?? 7;
  if (t.service === 'parking_aisle') w = 5.5; else if (t.service === 'driveway' || t.service === 'alley') w = 3.5;
  const lanes = num(t.lanes);
  if (lanes > 0 && lanes < 9) w = Math.max(w * 0.8, lanes * 3.3 + (['residential', 'tertiary', 'secondary', 'unclassified'].includes(base) ? 3 : 0));
  const tw = num(t['width:carriageway']) || num(t.width);
  if (tw >= 3 && tw <= 40) w = Math.max(tw, 3);
  return w;
}

function buildingHeight(t) {
  const h = num(t.height);
  if (h > 2 && h < 200) return h;
  const lv = num(t['building:levels']), rl = num(t['roof:levels']);
  if (lv > 0 && lv < 60) return lv * 3.2 + (rl > 0 ? rl * 2.4 : 1);
  switch (t.building) {
    case 'garage': case 'garages': case 'shed': case 'kiosk': case 'carport': case 'hut': case 'container': case 'toilets': return 3;
    case 'industrial': case 'warehouse': case 'retail': case 'supermarket': case 'commercial': case 'service': return 8;
    case 'church': case 'cathedral': return 24;
    default: return 16;
  }
}

function buildingKind(t) {
  const b = t.building;
  if (['industrial', 'warehouse', 'manufacture', 'hangar'].includes(b)) return BUILDING_KIND.industrial;
  if (['church', 'cathedral', 'chapel', 'religious', 'mosque'].includes(b)) return BUILDING_KIND.church;
  if (['garage', 'garages', 'shed', 'kiosk', 'carport', 'hut', 'container', 'toilets'].includes(b)) return BUILDING_KIND.small;
  if (['school', 'public', 'civic', 'government', 'hospital', 'college', 'university', 'train_station', 'museum', 'library', 'office', 'commercial', 'retail', 'hotel', 'supermarket'].includes(b)) return BUILDING_KIND.public;
  return BUILDING_KIND.house;
}

function areaKind(t) {
  const lu = t.landuse, le = t.leisure, na = t.natural;
  if (['park', 'garden', 'playground', 'dog_park', 'common'].includes(le) || ['grass', 'meadow', 'village_green', 'recreation_ground', 'greenfield', 'flowerbed'].includes(lu) || ['grassland', 'heath'].includes(na)) return AREA_KIND.grass;
  if (['wood', 'scrub', 'shrubbery'].includes(na) || lu === 'forest') return AREA_KIND.wood;
  if (['pitch', 'track', 'stadium'].includes(le)) return AREA_KIND.pitch;
  if (lu === 'cemetery' || t.amenity === 'grave_yard') return AREA_KIND.cemetery;
  if (lu === 'allotments') return AREA_KIND.allotments;
  if (na === 'sand' || na === 'beach') return AREA_KIND.sand;
  if (lu === 'railway') return AREA_KIND.rail;
  if (t.place === 'square' || (t.highway === 'pedestrian' && t.area === 'yes') || t.amenity === 'parking') return AREA_KIND.plaza;
  return -1;
}

// POI-Kategorie aus OSM-Tags (null = kein POI fürs Spiel).
export function poiCategory(t) {
  const station = ['station', 'halt'].includes(t.railway) || t.public_transport === 'station';
  if (station) {
    if (t.station === 'subway' || t.subway === 'yes') return 'ubahn';
    if (t.station === 'light_rail' || t.light_rail === 'yes') return 'sbahn';
    if (t.railway) return 'bahn';
    return null;
  }
  if (t.highway === 'bus_stop') return 'bus';
  if (t.shop) {
    if (t.shop === 'mall' || t.shop === 'department_store') return 'mall';
    if (['supermarket', 'convenience', 'kiosk'].includes(t.shop)) return 'supermarket';
    return 'shop';
  }
  const a = t.amenity;
  if (['restaurant', 'fast_food', 'food_court', 'ice_cream'].includes(a)) return 'food';
  if (['bar', 'pub', 'biergarten', 'nightclub'].includes(a)) return 'drink';
  if (a === 'cafe') return 'cafe';
  if (['pharmacy', 'bank', 'post_office', 'fuel', 'police', 'hospital', 'clinic', 'doctors', 'dentist', 'townhall',
    'library', 'marketplace', 'school', 'kindergarten', 'community_centre', 'place_of_worship', 'fire_station', 'car_wash', 'car_rental', 'bicycle_rental'].includes(a)) return 'service';
  if (['cinema', 'theatre', 'arts_centre', 'events_venue'].includes(a) || ['museum', 'gallery', 'attraction'].includes(t.tourism)) return 'culture';
  if (['hotel', 'hostel', 'guest_house'].includes(t.tourism)) return 'hotel';
  return null;
}

const isWater = (t) => t.natural === 'water' || t.waterway === 'riverbank' || ['basin', 'reservoir'].includes(t.landuse);

export function buildCity(lor, osm, places, { scale = 10, kataster = [] } = {}) {
  const S = scale;
  const nodes = new Map();
  let s = 90, w = 180, n = -90, e = -180;
  for (const el of osm.elements) if (el.type === 'node') {
    nodes.set(el.id, el);
    s = Math.min(s, el.lat); n = Math.max(n, el.lat); w = Math.min(w, el.lon); e = Math.max(e, el.lon);
  }
  if (osm.bbox) [s, w, n, e] = osm.bbox;
  const lat0 = (s + n) / 2, lon0 = (w + e) / 2;
  const proj = makeProjection(lat0, lon0);
  const corners = [proj(s, w), proj(s, e), proj(n, w), proj(n, e)];
  const minX = Math.min(...corners.map((c) => c[0])), maxX = Math.max(...corners.map((c) => c[0]));
  const minY = Math.min(...corners.map((c) => c[1])), maxY = Math.max(...corners.map((c) => c[1]));
  const W = Math.round((maxX - minX) * S), H = Math.round((maxY - minY) * S);
  const toPx = (lat, lon) => { const [x, y] = proj(lat, lon); return [Math.round((x - minX) * S), Math.round((maxY - y) * S)]; };
  const pxCache = new Map();
  const P = (id) => {
    let p = pxCache.get(id);
    if (!p) { const nd = nodes.get(id); if (!nd) return null; p = toPx(nd.lat, nd.lon); pxCache.set(id, p); }
    return p;
  };
  const flat = (ids) => { const out = []; for (const id of ids) { const p = P(id); if (p) out.push(p[0], p[1]); } return out; };
  const inBounds = (pts) => { for (let i = 0; i < pts.length; i += 2) if (pts[i] >= 0 && pts[i] <= W && pts[i + 1] >= 0 && pts[i + 1] <= H) return true; return false; };

  // --- Grenze -------------------------------------------------------------------------
  const lorRings = {};
  for (const f of lor.features) {
    const name = DISTRICT_OF[f.properties.pgr_id];
    if (!name) continue;
    for (const poly of f.geometry.coordinates) {
      const r = poly[0].flatMap(([lon, lat]) => toPx(lat, lon));
      (lorRings[name] ??= []).push(r);
    }
  }
  const border = unionOutline(Object.values(lorRings).flat());
  const districts = Object.entries(lorRings).map(([name, rings]) => ({ name, rings: unionOutline(rings) }));
  const insideBorder = (x, y) => { let k = 0; for (const r of border) if (pointInRing(x, y, r)) k++; return k % 2 === 1; };

  // --- Multipolygone ------------------------------------------------------------------
  const ways = new Map();
  for (const el of osm.elements) if (el.type === 'way') ways.set(el.id, el);
  const polygons = []; // { tags, rings:[{pts, outer}] , id }
  for (const el of osm.elements) {
    if (el.type === 'way' && el.tags && el.nodes.length >= 4 && el.nodes[0] === el.nodes[el.nodes.length - 1]) {
      polygons.push({ id: el.id, tags: el.tags, rings: [{ pts: flat(el.nodes), outer: true }] });
    } else if (el.type === 'relation' && el.tags?.type === 'multipolygon') {
      const get = (role) => el.members.filter((m) => m.type === 'way' && (m.role === role || (role === 'outer' && m.role === ''))).map((m) => ways.get(m.ref)?.nodes).filter(Boolean);
      const outer = joinRings(get('outer')), inner = joinRings(get('inner'));
      if (!outer.length) continue;
      polygons.push({ id: el.id, tags: el.tags, rings: [...outer.map((r) => ({ pts: flat(r), outer: true })), ...inner.map((r) => ({ pts: flat(r), outer: false }))] });
    }
  }
  // Außenringe gegen den Uhrzeigersinn (im px-System mit y nach unten: Fläche < 0), Löcher andersherum.
  const orient = (r, outer) => {
    const a = ringArea(r.pts ?? r);
    const pts = r.pts ?? r;
    if ((a > 0) === outer) { const rev = []; for (let i = pts.length - 2; i >= 0; i -= 2) rev.push(pts[i], pts[i + 1]); return rev; }
    return pts;
  };
  const cleanRings = (poly, tol) => poly.rings.map((r) => {
    const sp = simplify(r.pts, tol);
    return sp.length >= 8 ? { outer: r.outer, pts: orient(sp, r.outer) } : null;
  }).filter(Boolean);

  const buildings = [], water = [], areas = [];
  for (const poly of polygons) {
    const t = poly.tags;
    if (!poly.rings.some((r) => inBounds(r.pts))) continue;
    if (t.building && t.building !== 'no' && t.building !== 'roof' && !(num(t.layer) < 0) && t.location !== 'underground' && !t['building:part']) {
      const rings = cleanRings(poly, 0.3 * S);
      if (!rings.length || !rings[0].outer) continue;
      if (Math.abs(ringArea(rings[0].pts)) < 4 * S * S) continue; // < 4 m²
      buildings.push({ id: poly.id, h: Math.round(buildingHeight(t) * 10), k: buildingKind(t), rings });
    } else if (isWater(t)) {
      const rings = cleanRings(poly, 0.5 * S);
      if (rings.length && rings.some((r) => r.outer)) water.push({ id: poly.id, rings });
    } else {
      const k = areaKind(t);
      if (k < 0) continue;
      const rings = cleanRings(poly, 0.5 * S);
      // winzige Flächen (Beete, Baumscheiben < 15 m²) weglassen – im Spiel nicht sichtbar, kosten aber Platz
      if (rings.length && rings.some((r) => r.outer) && Math.abs(ringArea(rings[0].pts)) >= 15 * S * S) areas.push({ id: poly.id, k, rings });
    }
  }
  areas.sort((a, b) => a.k - b.k || a.id - b.id);
  buildings.sort((a, b) => a.id - b.id);
  water.sort((a, b) => a.id - b.id);

  // --- Straßen und Graph ---------------------------------------------------------------
  const roadWays = [], pathWays = [], railWays = [];
  const PATHS = new Set(['footway', 'cycleway', 'path', 'steps', 'bridleway']);
  for (const el of ways.values()) {
    const t = el.tags;
    if (!t) continue;
    if (t.highway) {
      if (t.area === 'yes' || t.tunnel === 'culvert' || t.tunnel === 'yes') continue; // Tordurchfahrten (building_passage) bleiben
      if (num(t.layer) < 0 && !t.bridge) continue;
      const base = t.highway.replace(/_link$/, '');
      if (ROAD_CLASS[base] !== undefined) roadWays.push({ el, base });
      else if (PATHS.has(t.highway) && t.footway !== 'sidewalk' && t.footway !== 'crossing' && t.cycleway !== 'crossing') pathWays.push(el);
    } else if (t.railway && ['rail', 'light_rail', 'subway'].includes(t.railway) && t.tunnel !== 'yes' && !(num(t.layer) < 0)) {
      railWays.push(el);
    }
  }
  roadWays.sort((a, b) => a.el.id - b.el.id);
  pathWays.sort((a, b) => a.id - b.id);
  railWays.sort((a, b) => a.id - b.id);

  const use = new Map();
  for (const { el } of roadWays) el.nodes.forEach((id, i) => use.set(id, (use.get(id) ?? 0) + (i === 0 || i === el.nodes.length - 1 ? 2 : 1)));
  const vIndex = new Map(), vertices = [];
  const vertex = (id) => {
    let k = vIndex.get(id);
    if (k === undefined) { k = vertices.length / 2; vIndex.set(id, k); const p = P(id); vertices.push(p[0], p[1]); }
    return k;
  };
  const names = [], nameIdx = new Map();
  const nameOf = (s) => { if (!s) return -1; let k = nameIdx.get(s); if (k === undefined) { k = names.length; names.push(s); nameIdx.set(s, k); } return k; };
  const edges = [];
  for (const { el, base } of roadWays) {
    const t = el.tags;
    const ids = el.nodes.filter((id) => nodes.has(id));
    if (ids.length < 2) continue;
    let oneway = 0;
    if (['yes', 'true', '1'].includes(t.oneway) || t.junction === 'roundabout' || base === 'motorway') oneway = 1;
    if (t.oneway === '-1' || t.oneway === 'reverse') oneway = -1;
    if (t.oneway === 'no') oneway = 0;
    const cls = ROAD_CLASS[base];
    // Querschnitt: Hauptnetz aus Breite/Spuren/Park- und Radstreifen, Nebenflächen (Zufahrt, Fußgängerzone) geschätzt.
    const cs = cls <= 8 ? crossSection(t, base, oneway)
      : { width: roadWidth(t, base), fwd: oneway === -1 ? 0 : 1, bwd: oneway === 1 ? 0 : 1,
        left: { park: PARK.none, parkW: 0, orient: 'parallel', cycle: 0 }, right: { park: PARK.none, parkW: 0, orient: 'parallel', cycle: 0 },
        maxspeed: maxspeedOf(t, base), surface: surfaceOf(t), lit: t.lit === 'yes' ? 1 : 0, gaslight: 0 };
    const width = Math.round(cs.width * 10); // dm
    const dm = (m) => Math.round(m * 10);
    const x = [cs.fwd, cs.bwd, cs.left.park, dm(cs.left.parkW), PARK_ORIENT.indexOf(cs.left.orient), cs.right.park, dm(cs.right.parkW),
      PARK_ORIENT.indexOf(cs.right.orient), dm(cs.left.cycle), dm(cs.right.cycle), cs.maxspeed, cs.surface, cs.lit | (cs.gaslight << 1)];
    const name = nameOf(t.name ?? t.ref ?? '');
    const bridge = t.bridge && t.bridge !== 'no' ? 1 : 0;
    let start = 0;
    for (let i = 1; i < ids.length; i++) {
      if (i === ids.length - 1 || use.get(ids[i]) > 1) {
        const seg = ids.slice(start, i + 1);
        const pts = simplify(flat(seg), 0.5 * S);
        if (seg.length >= 2 && (pts[0] !== pts[pts.length - 2] || pts[1] !== pts[pts.length - 1])) {
          edges.push({ a: vertex(seg[0]), b: vertex(seg[seg.length - 1]), c: cls, w: width, n: name, o: oneway, br: bridge, p: pts.slice(2, -2), id: el.id, x, ids: seg, pass: t.tunnel === 'building_passage' ? 1 : 0 });
        }
        start = i;
      }
    }
  }
  // Nur Verkehrskanten innerhalb der Grenze kommen in den Fahr-/Gehgraphen.
  const vx = (k) => vertices[2 * k], vy = (k) => vertices[2 * k + 1];
  for (const ed of edges) {
    const mx = (vx(ed.a) + vx(ed.b)) / 2, my = (vy(ed.a) + vy(ed.b)) / 2;
    ed.in = insideBorder(mx, my) ? 1 : 0;
  }

  const paths = pathWays.map((el) => ({ br: el.tags.bridge && el.tags.bridge !== 'no' ? 1 : 0, pass: el.tags.tunnel === 'building_passage' ? 1 : 0, p: simplify(flat(el.nodes), 0.8 * S) })).filter((x) => x.p.length >= 4 && inBounds(x.p));
  const rails = railWays.map((el) => ({ br: el.tags.bridge && el.tags.bridge !== 'no' ? 1 : 0, sub: el.tags.railway === 'subway' ? 1 : 0, ids: el.nodes, p: simplify(flat(el.nodes), 0.5 * S) })).filter((x) => x.p.length >= 4 && inBounds(x.p));

  // --- Wände: Ufer und Gleise, an Brücken/Übergängen aufgeschnitten --------------------
  const corridors = []; // [ax, ay, bx, by, r]
  const edgePts = (ed) => [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)];
  const addCorridor = (pts, r, ext) => {
    for (let i = 0; i < pts.length - 2; i += 2) {
      let ax = pts[i], ay = pts[i + 1], bx = pts[i + 2], by = pts[i + 3];
      const L = Math.hypot(bx - ax, by - ay) || 1, ux = (bx - ax) / L, uy = (by - ay) / L;
      if (i === 0) { ax -= ux * ext; ay -= uy * ext; }
      if (i === pts.length - 4) { bx += ux * ext; by += uy * ext; }
      corridors.push([ax, ay, bx, by, r]);
    }
  };
  for (const ed of edges) if (ed.br) addCorridor(edgePts(ed), ed.w / 10 * S / 2 + 1.5 * S, 6 * S);
  for (const pa of paths) if (pa.br) addCorridor(pa.p, 2.5 * S, 4 * S);
  // Straßen, die Gleise ebenerdig kreuzen (gemeinsamer Knoten), öffnen die Gleiswand.
  const railNodes = new Set(); for (const r of rails) if (!r.br) for (const id of r.ids) railNodes.add(id);
  for (const { el, base } of roadWays) for (const id of el.nodes) if (railNodes.has(id)) {
    const p = P(id); if (p) corridors.push([p[0], p[1], p[0], p[1], (DEFAULT_WIDTH[base] ?? 8) / 2 * S + 2 * S]);
  }
  const cut = makeCutter(corridors, S);
  // Tordurchfahrten öffnen die Hauswände (eigener Korridor-Satz, nur für Gebäude).
  const passages = [];
  const addPassage = (pts, r) => {
    for (let i = 0; i < pts.length - 2; i += 2) {
      let ax = pts[i], ay = pts[i + 1], bx = pts[i + 2], by = pts[i + 3];
      const L = Math.hypot(bx - ax, by - ay) || 1, ux = (bx - ax) / L, uy = (by - ay) / L;
      if (i === 0) { ax -= ux * 2 * S; ay -= uy * 2 * S; }
      if (i === pts.length - 4) { bx += ux * 2 * S; by += uy * 2 * S; }
      passages.push([ax, ay, bx, by, r]);
    }
  };
  for (const ed of edges) if (ed.pass) addPassage(edgePts(ed), Math.max(ed.w / 10 * S / 2, 1.6 * S));
  for (const pa of paths) if (pa.pass) addPassage(pa.p, 1.4 * S);
  const passCut = makeCutter(passages, S);
  for (const b of buildings) {
    const [x0, y0, x1, y1] = ringBox(b.rings[0].pts);
    if (!passages.some((c) => Math.max(c[0], c[2]) + c[4] >= x0 && Math.min(c[0], c[2]) - c[4] <= x1 && Math.max(c[1], c[3]) + c[4] >= y0 && Math.min(c[1], c[3]) - c[4] <= y1)) continue;
    const ws = b.rings.flatMap((r) => passCut([...r.pts, r.pts[0], r.pts[1]]));
    const len = (p) => { let L = 0; for (let i = 0; i < p.length - 2; i += 2) L += Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]); return L; };
    const full = b.rings.reduce((n, r) => n + len([...r.pts, r.pts[0], r.pts[1]]), 0);
    if (ws.reduce((n, w) => n + len(w), 0) < full - 0.5 * S) b.walls = ws; // nur wenn wirklich ein Stück Wand fehlt
  }
  const offsetLine = (pts, d) => {
    const out = [];
    for (let i = 0; i < pts.length; i += 2) {
      const pa = i > 0 ? [pts[i - 2], pts[i - 1]] : [pts[i], pts[i + 1]];
      const pb = i < pts.length - 2 ? [pts[i + 2], pts[i + 3]] : [pts[i], pts[i + 1]];
      const dx = pb[0] - pa[0], dy = pb[1] - pa[1], L = Math.hypot(dx, dy) || 1;
      out.push(Math.round(pts[i] - dy / L * d), Math.round(pts[i + 1] + dx / L * d));
    }
    return out;
  };
  const walls = [];
  for (const wa of water) for (const r of wa.rings) walls.push(...cut([...r.pts, r.pts[0], r.pts[1]]));
  for (const r of rails) if (!r.br && !r.sub) for (const d of [-2.5 * S, 2.5 * S]) walls.push(...cut(offsetLine(r.p, d)));
  // Brückengeländer
  for (const ed of edges) if (ed.br) { const pts = edgePts(ed); for (const d of [-1, 1]) walls.push(offsetLine(pts, d * (ed.w / 10 * S / 2 + 0.6 * S))); }

  const access = accessAndRules(osm.elements, { nodes, ways, P, S, edges, vertices, vIndex, buildings, W, H, walls, makeCutter });

  // --- Bäume, Kiez-Namen ----------------------------------------------------------------
  const trees = [], kieze = [];
  for (const nd of nodes.values()) {
    if (!nd.tags) continue;
    if (nd.tags.natural === 'tree') { const p = toPx(nd.lat, nd.lon); if (p[0] >= 0 && p[1] >= 0 && p[0] <= W && p[1] <= H) trees.push({ x: p[0], y: p[1], g: 0, c: 0, r: 0, osm: true }); }
    else if (['neighbourhood', 'quarter'].includes(nd.tags.place) && nd.tags.name) { const p = toPx(nd.lat, nd.lon); kieze.push({ n: nd.tags.name, x: p[0], y: p[1] }); }
  }
  const katStats = mergeKataster(trees, kataster, { toPx, W, H, S });
  const treeStats = { ...katStats, ...keepTreesOffRoads(trees, { edges, vertices, buildings, water, S }) };
  trees.sort((a, b) => a.y - b.y || a.x - b.x);
  kieze.sort((a, b) => a.n.localeCompare(b.n));
  const { pois, addresses } = extractPoisAndAddresses(osm.elements, { nodes, ways, P, toPx, W, H, S, nameOf });

  const missionPlaces = placeMission({ places, toPx, edges, vertices, buildings, S, insideBorder });
  const city = {
    meta: {
      version: 2, scale: S, width: W, height: H, origin: { lat0, lon0, bbox: [s, w, n, e] },
      osmBase: osm.osm3s?.timestamp_osm_base ?? null,
      attribution: 'Kartendaten © OpenStreetMap-Mitwirkende (ODbL) · Grenzen und Baumbestand: Geoportal Berlin (dl-de/zero-2.0)',
      classes: ROAD_CLASSES, trafficMaxClass: TRAFFIC_MAX_CLASS,
    },
    border: border.map(delta),
    districts: districts.map((d) => ({ n: d.name, r: d.rings.map(delta) })),
    names,
    vertices,
    // Kante: a, b, Klasse, Breite dm, Name, Einbahn, Brücke, im Gebiet, Zwischenpunkte, Querschnitt (siehe decodeCity)
    edges: edges.map((ed) => [ed.a, ed.b, ed.c, ed.w, ed.n, ed.o, ed.br, ed.in, ed.p.length ? delta(ed.p) : [], ed.c <= 8 ? ed.x : [ed.x[10], ed.x[11]]]), // Nebenwege: nur Tempo + Belag
    paths: paths.map((x) => [x.br, delta(x.p)]),
    rails: rails.map((x) => [x.br, x.sub, delta(x.p)]),
    // Gebäude: Höhe dm, Art, Ringe, eigene Wandzüge (bei Tordurchfahrten) oder 0, Türen [Ring, Kante, Anteil ×1000]
    buildings: buildings.map((b) => [b.h, b.k, b.rings.map((r) => delta(r.pts)), b.walls ? b.walls.map(delta) : 0, b.doors ?? 0]),
    water: water.map((wa) => wa.rings.map((r) => [r.outer ? 1 : 0, delta(r.pts)])),
    areas: areas.map((a) => [a.k, a.rings.map((r) => [r.outer ? 1 : 0, delta(r.pts)])]),
    walls: walls.map(delta),
    // Bäume: Koordinaten delta-kodiert; Gattung (TREE_GENERA), Kronendurchmesser dm, Stammradius cm (0 = Standard)
    trees: { xy: delta(trees.flatMap((t) => [t.x, t.y])), g: trees.map((t) => t.g), c: trees.map((t) => t.c), r: trees.map((t) => t.r) },
    kieze,
    ...access.out,
    // POIs: [x, y, Kategorie (POI_CATS), Name-Index, Art-Index]; Art = OSM-Wert (z. B. „bakery“) in names
    pois: pois.map((q) => [q.x, q.y, POI_CAT[q.cat], q.n, q.k]),
    // Hausnummern: Koordinaten delta-kodiert, dazu Straßen-Index (names) und Nummer als Text
    addresses: { xy: delta(addresses.flatMap((q) => [q.x, q.y])), street: addresses.map((q) => q.s), nr: addresses.map((q) => q.nr) },
  };
  city.places = missionPlaces;
  city.meta.trees = treeStats;
  city.meta.access = access.stats;
  return city;
}

// --- POIs und Hausnummern ----------------------------------------------------------------
// Position: Knoten direkt, Wege/Relationen über den Schwerpunkt ihrer (Außen-)Punkte.
function extractPoisAndAddresses(elements, { nodes, ways, P, toPx, W, H, S, nameOf }) {
  const center = (el) => {
    if (el.type === 'node') return toPx(el.lat, el.lon);
    const ids = el.type === 'way' ? el.nodes : el.members.filter((m) => m.type === 'way' && m.role !== 'inner').flatMap((m) => ways.get(m.ref)?.nodes ?? []);
    let x = 0, y = 0, n = 0;
    for (const id of new Set(ids)) { const p = P(id); if (p) { x += p[0]; y += p[1]; n++; } }
    return n ? [Math.round(x / n), Math.round(y / n)] : null;
  };
  const inside = (p) => p && p[0] >= 0 && p[1] >= 0 && p[0] <= W && p[1] <= H;
  const pois = [], addresses = [];
  const seenStation = [];
  for (const el of elements) {
    const t = el.tags;
    if (!t) continue;
    const cat = poiCategory(t);
    if (cat && (t.name || cat === 'bus')) {
      const p = center(el);
      if (inside(p)) {
        const name = (t.name ?? 'Haltestelle').replace(/^(S\+U|U\+S|[SU]) (?=\S)/, '').replace(/^Berlin /, '');
        // Bahnhöfe und Bushaltestellen haben oft mehrere Knoten (Bahnsteige, Richtungen): einmal je Name und Umkreis.
        const dedupe = { ubahn: 400 * S, sbahn: 400 * S, bahn: 400 * S, bus: 80 * S }[cat];
        if (!dedupe || !seenStation.some((q) => q.cat === cat && q.name === name && Math.hypot(q.x - p[0], q.y - p[1]) < dedupe)) {
          if (dedupe) seenStation.push({ cat, name, x: p[0], y: p[1] });
          pois.push({ x: p[0], y: p[1], cat, n: nameOf(name), k: nameOf(t.shop ?? t.amenity ?? t.tourism ?? t.station ?? t.railway ?? t.highway ?? ''), id: el.id, type: el.type });
        }
      }
    }
    const nr = t['addr:housenumber'], street = t['addr:street'] ?? t['addr:place'];
    if (nr && street) {
      const p = center(el);
      if (inside(p)) addresses.push({ x: p[0], y: p[1], s: nameOf(street), nr: String(nr), id: el.id, type: el.type });
    }
  }
  const byPos = (a, b) => a.y - b.y || a.x - b.x || a.type.localeCompare(b.type) || a.id - b.id;
  pois.sort(byPos); addresses.sort(byPos);
  // Doppelte Adressen (Gebäude + Eingangsknoten mit derselben Nummer) zusammenfassen.
  const seen = new Map(), out = [];
  for (const a of addresses) {
    const k = a.s + '|' + a.nr, prev = seen.get(k);
    if (prev && Math.hypot(prev.x - a.x, prev.y - a.y) < 40 * S) continue;
    seen.set(k, a); out.push(a);
  }
  return { pois, addresses: out };
}

// --- Korridore: Polylinien in 1-m-Stücke zerlegen und Stücke in einem Korridor weglassen --------------
function ringBox(r) { let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity; for (let i = 0; i < r.length; i += 2) { x0 = Math.min(x0, r[i]); x1 = Math.max(x1, r[i]); y0 = Math.min(y0, r[i + 1]); y1 = Math.max(y1, r[i + 1]); } return [x0, y0, x1, y1]; }

export function makeCutter(corridors, S) {
  const CELL = 400, cgrid = new Map();
  corridors.forEach((c, i) => {
    const x0 = Math.floor((Math.min(c[0], c[2]) - c[4]) / CELL), x1 = Math.floor((Math.max(c[0], c[2]) + c[4]) / CELL);
    const y0 = Math.floor((Math.min(c[1], c[3]) - c[4]) / CELL), y1 = Math.floor((Math.max(c[1], c[3]) + c[4]) / CELL);
    for (let gx = x0; gx <= x1; gx++) for (let gy = y0; gy <= y1; gy++) { const k = gx * 100000 + gy; (cgrid.get(k) ?? cgrid.set(k, []).get(k)).push(i); }
  });
  const inCorridor = (x, y) => {
    for (const i of cgrid.get(Math.floor(x / CELL) * 100000 + Math.floor(y / CELL)) ?? []) {
      const c = corridors[i];
      if (segDist2(x, y, c[0], c[1], c[2], c[3]) < c[4] * c[4]) return true;
    }
    return false;
  };
  return (pts) => {
    const out = []; let cur = [];
    const step = 1 * S;
    for (let i = 0; i < pts.length - 2; i += 2) {
      const ax = pts[i], ay = pts[i + 1], bx = pts[i + 2], by = pts[i + 3];
      const k = Math.max(1, Math.ceil(Math.hypot(bx - ax, by - ay) / step));
      for (let j = 0; j < k; j++) {
        const x0 = ax + (bx - ax) * j / k, y0 = ay + (by - ay) * j / k, x1 = ax + (bx - ax) * (j + 1) / k, y1 = ay + (by - ay) * (j + 1) / k;
        if (inCorridor((x0 + x1) / 2, (y0 + y1) / 2)) { if (cur.length >= 4) out.push(cur); cur = []; continue; }
        if (!cur.length) cur.push(Math.round(x0), Math.round(y0));
        cur.push(Math.round(x1), Math.round(y1));
      }
    }
    if (cur.length >= 4) out.push(cur);
    return out.map((c) => simplify(c, 0.2 * S)).filter((c) => c.length >= 4);
  };
}

// --- Zugänge und Verkehrsregeln -------------------------------------------------------------
const CAR_BLOCKING = new Set(['bollard', 'block', 'post', 'cycle_barrier', 'jersey_barrier', 'planter', 'lift_gate', 'gate', 'swing_gate', 'chain', 'bar']);
const FENCES = { fence: 0, wall: 1, hedge: 2, retaining_wall: 1, city_wall: 1, guard_rail: 0, handrail: 0, bollard: 3, block: 3, jersey_barrier: 1, planter: 2 };

export function accessAndRules(elements, { nodes, ways, P, S, edges, vertices, vIndex, buildings, W, H, walls, makeCutter }) {
  const inside = (p) => p && p[0] >= 0 && p[1] >= 0 && p[0] <= W && p[1] <= H;
  // Knoten → Kante (mit Position in der Kante) für Barrieren und Querungen auf Straßen
  const onEdge = new Map(), allOn = new Map();
  edges.forEach((ed, k) => ed.ids.forEach((id, i) => {
    if (!onEdge.has(id)) onEdge.set(id, { k, i });
    (allOn.get(id) ?? allOn.set(id, []).get(id)).push({ k, i });
  }));
  const dirAt = (ed, i) => {
    const a = P(ed.ids[Math.max(0, i - 1)]), b = P(ed.ids[Math.min(ed.ids.length - 1, i + 1)]);
    const dx = b[0] - a[0], dy = b[1] - a[1], L = Math.hypot(dx, dy) || 1;
    return [dx / L, dy / L];
  };
  const barriers = [], posts = [], fences = [], crossings = [], signalNodes = [];
  let blockedEdges = 0;
  for (const nd of nodes.values()) {
    const t = nd.tags;
    if (!t) continue;
    const p = P(nd.id);
    if (!inside(p)) continue;
    const open = ['yes', 'destination', 'permissive', 'designated'];
    const carsAllowed = open.includes(t.motor_vehicle) || open.includes(t.motorcar) || (open.includes(t.access) && !['no', 'private'].includes(t.motor_vehicle));
    if (t.barrier && CAR_BLOCKING.has(t.barrier) && !carsAllowed) {
      const kind = ['lift_gate', 'gate', 'swing_gate', 'bar', 'chain'].includes(t.barrier) ? 1 : 0; // 0 Poller, 1 Schranke/Tor
      const on = (allOn.get(nd.id) ?? []).filter((h) => edges[h.k].c <= 10);
      if (on.some((h) => edges[h.k].c <= 2)) continue; // Autobahn/Schnellstraße: Nottore in der Mitte, keine Sperre der Fahrbahn
      if (on.length) {
        // Die Sperre gilt für jede Kante durch diesen Knoten (steht sie am Übergang Straße → Fußgängerzone, endet die
        // Straße davor). Poller im Abstand von 1,8 m quer über den schmalsten Weg (Fußgänger kommen durch, Autos nicht).
        for (const h of on) { if (!edges[h.k].blocked) blockedEdges++; edges[h.k].blocked = 1; }
        const hit = on.reduce((a, b) => (edges[b.k].c > edges[a.k].c ? b : a));
        const ed = edges[hit.k], [ux, uy] = dirAt(ed, hit.i), half = ed.w / 10 * S / 2;
        // Reihe: Mitte, Richtung quer zur Fahrbahn (×1000), Anzahl Poller, Abstand 1,8 m
        const n = Math.max(1, Math.floor((2 * half - 1 * S) / (1.8 * S)) + 1);
        // Steht die Sperre auf einem Kreuzungsknoten, die Reihe in den gesperrten Weg hineinrücken (nicht in die Kreuzung).
        let [bx, by] = p;
        if (hit.i === 0 || hit.i === ed.ids.length - 1) {
          const inward = hit.i === 0 ? 1 : -1;
          let other = 0; for (const o of allOn.get(nd.id) ?? []) if (o.k !== hit.k) other = Math.max(other, edges[o.k].w / 10 * S / 2);
          const shift = other ? other + 1 * S : 0;
          bx = Math.round(bx + ux * inward * shift); by = Math.round(by + uy * inward * shift);
        }
        barriers.push([bx, by, kind, Math.round(-uy * 1000), Math.round(ux * 1000), n]);
      } else posts.push([p[0], p[1], kind]);
    }
    if (t.highway === 'crossing' || (t.crossing && onEdge.has(nd.id))) {
      const hit = onEdge.get(nd.id);
      if (!hit || edges[hit.k].c > 8) continue;
      const c = t.crossing, ref = t.crossing_ref, mk = t['crossing:markings'];
      const kind = c === 'traffic_signals' || t['crossing:signals'] === 'yes' ? 1 : (c === 'zebra' || ref === 'zebra' || mk === 'zebra') ? 0 : (c === 'marked' || c === 'uncontrolled' || (mk && mk !== 'no')) ? 2 : -1;
      if (kind >= 0) crossings.push([p[0], p[1], hit.k, kind]);
    }
    if (t.highway === 'traffic_signals') signalNodes.push({ id: nd.id, x: p[0], y: p[1] });
  }
  // Zäune und Mauern als Wände (Tore darin bleiben offen)
  const gates = [];
  for (const nd of nodes.values()) if (nd.tags && ['gate', 'swing_gate', 'kissing_gate', 'entrance', 'lift_gate'].includes(nd.tags.barrier)) { const p = P(nd.id); if (p) gates.push([p[0], p[1], p[0], p[1], 0.9 * S]); }
  const gateCut = makeCutter(gates, S);
  // Straßenstücke für den Schnitt mit Sperrlinien (Poller-Reihen wie Diagonalsperren, Mauern, Zäune quer über die Straße)
  const G = 400, roadGrid = new Map();
  edges.forEach((ed, k) => {
    if (ed.c <= 2 || ed.c > 8) return;
    const pts = [vertices[2 * ed.a], vertices[2 * ed.a + 1], ...ed.p, vertices[2 * ed.b], vertices[2 * ed.b + 1]];
    for (let i = 0; i < pts.length - 2; i += 2) {
      const x0 = Math.floor(Math.min(pts[i], pts[i + 2]) / G), x1 = Math.floor(Math.max(pts[i], pts[i + 2]) / G);
      const y0 = Math.floor(Math.min(pts[i + 1], pts[i + 3]) / G), y1 = Math.floor(Math.max(pts[i + 1], pts[i + 3]) / G);
      for (let gx = x0; gx <= x1; gx++) for (let gy = y0; gy <= y1; gy++) { const key = gx * 100000 + gy; (roadGrid.get(key) ?? roadGrid.set(key, []).get(key)).push([k, pts[i], pts[i + 1], pts[i + 2], pts[i + 3]]); }
    }
  });
  const crossesSeg = (ax, ay, bx, by, cx, cy, dx, dy) => {
    const d = (bx - ax) * (dy - cy) - (by - ay) * (dx - cx);
    if (Math.abs(d) < 1e-9) return false;
    const t = ((cx - ax) * (dy - cy) - (cy - ay) * (dx - cx)) / d, u = ((cx - ax) * (by - ay) - (cy - ay) * (bx - ax)) / d;
    return t >= -0.01 && t <= 1.01 && u >= -0.01 && u <= 1.01;
  };
  const gateNodes = new Set();
  for (const nd of nodes.values()) if (nd.tags && ['gate', 'swing_gate', 'lift_gate', 'entrance', 'kissing_gate'].includes(nd.tags.barrier)) gateNodes.add(nd.id);
  for (const el of ways.values()) {
    const k = FENCES[el.tags?.barrier];
    if (k === undefined || el.tags.building) continue;
    const raw = []; for (const id of el.nodes) { const q = P(id); if (q) raw.push(q[0], q[1]); }
    const pts = simplify(raw, 0.3 * S);
    if (pts.length < 4 || !inside([pts[0], pts[1]])) continue;
    fences.push([k, pts]);
    walls.push(...gateCut(pts));
    if (el.nodes.some((id) => gateNodes.has(id))) continue; // Zaun mit Tor: Durchfahrt möglich
    // Kreuzt die Sperrlinie eine Straße, ist diese für Autos gesperrt (z. B. Diagonalsperre im Kiez).
    for (let i = 0; i < pts.length - 2; i += 2) {
      const key = Math.floor(pts[i] / G) * 100000 + Math.floor(pts[i + 1] / G);
      for (let gx = -1; gx <= 1; gx++) for (let gy = -1; gy <= 1; gy++) for (const [kk, ax, ay, bx, by] of roadGrid.get(key + gx * 100000 + gy) ?? []) {
        if (edges[kk].blocked) continue;
        // Poller-Reihen (Diagonalsperren): auch Straßen, die knapp daran vorbeiführen, meidet der KI-Verkehr
        const near = el.tags.traffic_intervention === 'diagonal_diverter' && Math.min(segDist2(ax, ay, pts[i], pts[i + 1], pts[i + 2], pts[i + 3]), segDist2(bx, by, pts[i], pts[i + 1], pts[i + 2], pts[i + 3]),
          segDist2(pts[i], pts[i + 1], ax, ay, bx, by), segDist2(pts[i + 2], pts[i + 3], ax, ay, bx, by)) < (3 * S) ** 2;
        if (!near && !crossesSeg(pts[i], pts[i + 1], pts[i + 2], pts[i + 3], ax, ay, bx, by)) continue;
        edges[kk].blocked = 1; blockedEdges++;
      }
    }
  }
  // Türen: Eingangsknoten auf einem Gebäudeumriss
  let doors = 0;
  const bgrid = new Map(), CELL = 400;
  buildings.forEach((b, bi) => { const [x0, y0, x1, y1] = ringBox(b.rings[0].pts); for (let gx = Math.floor(x0 / CELL); gx <= Math.floor(x1 / CELL); gx++) for (let gy = Math.floor(y0 / CELL); gy <= Math.floor(y1 / CELL); gy++) { const key = gx * 100000 + gy; (bgrid.get(key) ?? bgrid.set(key, []).get(key)).push(bi); } });
  for (const nd of nodes.values()) {
    if (!nd.tags?.entrance && !(nd.tags?.door)) continue;
    const p = P(nd.id); if (!inside(p)) continue;
    let best = null;
    for (const bi of bgrid.get(Math.floor(p[0] / CELL) * 100000 + Math.floor(p[1] / CELL)) ?? []) {
      const b = buildings[bi];
      b.rings.forEach((r, ri) => { const pts = r.pts, n = pts.length;
        for (let i = 0; i < n; i += 2) {
          const ax = pts[i], ay = pts[i + 1], bx = pts[(i + 2) % n], by = pts[(i + 3) % n];
          const d = Math.sqrt(segDist2(p[0], p[1], ax, ay, bx, by));
          if (d < 1.5 * S && (!best || d < best.d)) {
            const L2 = (bx - ax) ** 2 + (by - ay) ** 2 || 1, t = Math.max(0, Math.min(1, ((p[0] - ax) * (bx - ax) + (p[1] - ay) * (by - ay)) / L2));
            best = { d, b, ri, ei: i / 2, t };
          }
        } });
    }
    if (best) { (best.b.doors ??= []).push([best.ri, best.ei, Math.round(best.t * 1000)]); doors++; }
  }
  // Ampelkreuzungen: Kreuzungsknoten (≥ 3 befahrbare Kanten) mit Signal auf oder vor der Kreuzung
  const deg = new Map();
  for (const ed of edges) if (ed.c <= 8) for (const v of [ed.a, ed.b]) deg.set(v, (deg.get(v) ?? 0) + 1);
  const junctions = [...deg.entries()].filter(([, d]) => d >= 3).map(([v]) => v);
  const jgrid = new Map(), JC = 600;
  for (const v of junctions) { const key = Math.floor(vertices[2 * v] / JC) * 100000 + Math.floor(vertices[2 * v + 1] / JC); (jgrid.get(key) ?? jgrid.set(key, []).get(key)).push(v); }
  const signals = new Set();
  for (const sn of signalNodes) {
    const v0 = vIndex.get(sn.id);
    if (v0 !== undefined && deg.get(v0) >= 3) { signals.add(v0); continue; }
    let best = null;
    for (let gx = -1; gx <= 1; gx++) for (let gy = -1; gy <= 1; gy++) for (const v of jgrid.get((Math.floor(sn.x / JC) + gx) * 100000 + Math.floor(sn.y / JC) + gy) ?? []) {
      const d = Math.hypot(vertices[2 * v] - sn.x, vertices[2 * v + 1] - sn.y);
      if (d < 35 * S && (!best || d < best.d)) best = { d, v };
    }
    if (best) signals.add(best.v);
  }
  // Abbiegeverbote: (von Kante, über Knoten, nach Kante)
  const bans = [];
  const edgesAt = new Map();
  edges.forEach((ed, k) => { for (const v of [ed.a, ed.b]) (edgesAt.get(v) ?? edgesAt.set(v, []).get(v)).push(k); });
  let restrictions = 0;
  for (const el of elements) {
    if (el.type !== 'relation' || el.tags?.type !== 'restriction') continue;
    const r = el.tags.restriction ?? el.tags['restriction:motorcar'];
    if (!r) continue;
    const from = el.members.find((m) => m.role === 'from' && m.type === 'way'), via = el.members.find((m) => m.role === 'via' && m.type === 'node'), to = el.members.find((m) => m.role === 'to' && m.type === 'way');
    if (!from || !via || !to) continue;
    const v = vIndex.get(via.ref); if (v === undefined) continue;
    const at = edgesAt.get(v) ?? [];
    // Ist der Von-/Nach-Weg am Knoten nicht geteilt, gehören beide Hälften dazu (Fahrtrichtung ergibt sich aus der Spur).
    const fes = at.filter((k) => edges[k].id === from.ref), tes = at.filter((k) => edges[k].id === to.ref);
    if (!fes.length || !tes.length) continue;
    restrictions++;
    for (const fe of fes) {
      if (r.startsWith('no_')) { for (const te of tes) if (te !== fe) bans.push(fe, v, te); }
      else if (r.startsWith('only_')) for (const k of at) if (!tes.includes(k) && k !== fe) bans.push(fe, v, k);
    }
  }
  const edgeFlags = edges.map((ed) => (ed.blocked ? 1 : 0) | (ed.pass ? 2 : 0));
  return {
    out: { barriers: barriers.flat(), posts: posts.flat(), fences: fences.map(([k, pts]) => [k, delta(pts)]), crossings: crossings.flat(), signals: [...signals].sort((a, b) => a - b), turnBans: bans, edgeFlags },
    stats: { sperren: blockedEdges, poller: barriers.reduce((n, b) => n + b[5], 0) + posts.length, zaeune: fences.length, tueren: doors, ampeln: signals.size, querungen: crossings.length, abbiegeverbote: restrictions, durchfahrten: edges.filter((e) => e.pass).length },
  };
}

// --- Baumbestand Berlin (Geoportal) zusammenführen ----------------------------------------------
export function genusCode(g) {
  const i = TREE_GENERA.indexOf(g);
  if (i >= 0) return i;
  if (['Pinus', 'Picea', 'Abies', 'Taxus', 'Larix', 'Thuja', 'Pseudotsuga', 'Metasequoia'].includes(g)) return TREE_GENERA.indexOf('Nadel');
  return 0;
}

export function mergeKataster(trees, kataster, { toPx, W, H, S }) {
  const kat = [];
  for (const [lon, lat, genus, , crown, girth, height] of kataster) {
    const p = toPx(lat, lon);
    if (p[0] < 0 || p[1] < 0 || p[0] > W || p[1] > H) continue;
    let c = crown > 0 ? crown : height > 0 ? height * 0.55 : girth > 0 ? 2.5 + girth / 30 : 6;
    c = Math.max(2.5, Math.min(18, c));
    const r = girth > 0 ? Math.max(10, Math.min(60, Math.round(girth / (2 * Math.PI)))) : 25; // cm
    kat.push({ x: p[0], y: p[1], g: genusCode(genus), c: Math.round(c * 10), r });
  }
  // OSM-Bäume nur behalten, wo das Kataster keinen Baum innerhalb von 4 m kennt (private Bäume).
  const CELL = 100, grid = new Set();
  const key = (x, y) => Math.floor(x / CELL) * 1000000 + Math.floor(y / CELL);
  for (const t of kat) grid.add(key(t.x, t.y));
  const near = (x, y) => { for (let gx = -1; gx <= 1; gx++) for (let gy = -1; gy <= 1; gy++) if (grid.has(key(x + gx * CELL, y + gy * CELL))) return true; return false; };
  const katPos = new Map();
  for (const t of kat) { const k = key(t.x, t.y); (katPos.get(k) ?? katPos.set(k, []).get(k)).push(t); }
  const close = (x, y) => { if (!near(x, y)) return false; for (let gx = -1; gx <= 1; gx++) for (let gy = -1; gy <= 1; gy++) for (const t of katPos.get(key(x + gx * CELL, y + gy * CELL)) ?? []) if (Math.hypot(t.x - x, t.y - y) < 4 * S) return true; return false; };
  const osm = trees.splice(0, trees.length).filter((t) => !close(t.x, t.y));
  trees.push(...kat, ...osm);
  return { kataster: kat.length, osmKept: osm.length };
}

// --- Bäume von der Fahrbahn --------------------------------------------------------------
// OSM-Bäume stehen oft auf der geschätzten Fahrbahnbreite (Parkstreifen, Schätzwerte). Ein Stamm auf der Fahrbahn
// wird quer zur Straße an den Bordstein geschoben (auf seiner Seite, die Baumreihe bleibt eine Reihe). Findet sich
// dort kein freier Platz (Haus, Wasser, andere Fahrbahn), entfällt der Baum; ebenso Bäume, die in einem Haus oder
// im Wasser stünden. Danach wird die Regel geprüft; ein
// Verstoß bricht den Build ab.
export function keepTreesOffRoads(trees, { edges, vertices, buildings, water, S }) {
  const trunkOf = (t) => (t.r ? t.r / 100 * S : TREE_TRUNK_M * S), gap = 0.3 * S, trunk = 0.6 * S; // trunk: größter Stamm fürs Raster
  const vx = (k) => vertices[2 * k], vy = (k) => vertices[2 * k + 1];
  const CELL = 300, grid = new Map();
  const add = (item, x0, y0, x1, y1) => {
    for (let gx = Math.floor(x0 / CELL); gx <= Math.floor(x1 / CELL); gx++) for (let gy = Math.floor(y0 / CELL); gy <= Math.floor(y1 / CELL); gy++) {
      const k = gx * 100000 + gy; (grid.get(k) ?? grid.set(k, []).get(k)).push(item);
    }
  };
  for (const ed of edges) {
    if (ed.c > TREE_FREE_MAX_CLASS) continue;
    const pts = [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)], half = ed.w / 10 * S / 2;
    for (let i = 0; i < pts.length - 2; i += 2) {
      const sgm = { road: true, half, ax: pts[i], ay: pts[i + 1], bx: pts[i + 2], by: pts[i + 3] };
      add(sgm, Math.min(sgm.ax, sgm.bx) - half - trunk, Math.min(sgm.ay, sgm.by) - half - trunk, Math.max(sgm.ax, sgm.bx) + half + trunk, Math.max(sgm.ay, sgm.by) + half + trunk);
    }
  }
  // Kreuzungsflächen wie im Spiel (map.js decodeCity): Knoten mit ≥ 3 Straßen oder einem Knick, Radius = größte
  // halbe Fahrbahnbreite + 2 m. Auch dort darf kein Stamm stehen.
  const at = new Map();
  for (const ed of edges) if (ed.c <= 8 && !ed.pass) for (const v of [ed.a, ed.b]) (at.get(v) ?? at.set(v, []).get(v)).push(ed);
  const heading = (ed, v) => { const pts = [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)]; const i = ed.a === v ? 0 : pts.length - 4; const a = Math.atan2(pts[i + 3] - pts[i + 1], pts[i + 2] - pts[i]); return ed.a === v ? a : a + Math.PI; };
  for (const [v, es] of at) {
    let corner = es.length >= 3;
    if (es.length === 2) { let d = heading(es[1], v) - heading(es[0], v) - Math.PI; while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI; corner = Math.abs(d) > 0.5; }
    if (!corner) continue;
    const half = Math.max(...es.map((ed) => ed.w / 10 * S / 2)) + 2 * S;
    const sgm = { road: true, half, ax: vx(v), ay: vy(v), bx: vx(v), by: vy(v) };
    add(sgm, sgm.ax - half - trunk, sgm.ay - half - trunk, sgm.ax + half + trunk, sgm.ay + half + trunk);
  }
  const bbox = (r) => { let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity; for (let i = 0; i < r.length; i += 2) { x0 = Math.min(x0, r[i]); x1 = Math.max(x1, r[i]); y0 = Math.min(y0, r[i + 1]); y1 = Math.max(y1, r[i + 1]); } return [x0, y0, x1, y1]; };
  for (const f of [...buildings, ...water]) { const rs = f.rings.map((r) => r.pts); const [x0, y0, x1, y1] = bbox(rs[0]); add({ rings: rs }, x0, y0, x1, y1); }
  const near = (x, y) => grid.get(Math.floor(x / CELL) * 100000 + Math.floor(y / CELL)) ?? [];
  const worstRoad = (x, y, tr) => {
    let worst = null;
    for (const g of near(x, y)) {
      if (!g.road) continue;
      const need = g.half + tr, d = Math.sqrt(segDist2(x, y, g.ax, g.ay, g.bx, g.by));
      if (d < need + 1 && (!worst || need - d > worst.pen)) worst = { g, d, pen: need - d }; // 1 px Reserve gegen Rundung
    }
    return worst;
  };
  const blocked = (x, y) => near(x, y).some((g) => g.rings && g.rings.filter((r) => pointInRing(x, y, r)).length % 2 === 1);
  let moved = 0, dropped = 0;
  for (let i = trees.length - 1; i >= 0; i--) {
    const tree = trees[i], tr = trunkOf(tree);
    let { x, y } = tree;
    let hit = worstRoad(x, y, tr);
    if (!hit) { if (blocked(x, y)) { trees.splice(i, 1); dropped++; } continue; } // Baum in Haus/Wasser (OSM-Fehler)
    for (let k = 0; k < 6 && hit; k++) {
      const { g } = hit;
      const dx = g.bx - g.ax, dy = g.by - g.ay, L2 = dx * dx + dy * dy || 1;
      const t = Math.max(0, Math.min(1, ((x - g.ax) * dx + (y - g.ay) * dy) / L2));
      const px = g.ax + dx * t, py = g.ay + dy * t;
      let nx = x - px, ny = y - py, n = Math.hypot(nx, ny);
      if (n < 1e-6) { const L = Math.sqrt(L2); nx = -dy / L; ny = dx / L; n = 1; }
      const out = g.half + tr + gap;
      x = px + nx / n * out; y = py + ny / n * out;
      hit = worstRoad(x, y, tr);
    }
    if (hit || blocked(x, y)) { trees.splice(i, 1); dropped++; continue; }
    tree.x = Math.round(x); tree.y = Math.round(y);
    if (worstRoad(tree.x, tree.y, tr)) { trees.splice(i, 1); dropped++; continue; } // Rundung
    moved++;
  }
  for (const t of trees) if (worstRoad(t.x, t.y, trunkOf(t)) || blocked(t.x, t.y)) throw new Error(`Baum bei ${t.x},${t.y} steht auf der Fahrbahn, in einem Haus oder im Wasser`);
  return { moved, dropped };
}

// --- Missionsorte ---------------------------------------------------------------------
function placeMission({ places, toPx, edges, vertices, buildings, S, insideBorder }) {
  const vx = (k) => vertices[2 * k], vy = (k) => vertices[2 * k + 1];
  const edgePts = (ed) => [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)];
  const inBuilding = (x, y) => buildings.some((b) => {
    const o = b.rings[0].pts;
    return pointInRing(x, y, o) && !b.rings.slice(1).some((r) => pointInRing(x, y, r.pts));
  });
  const snap = (x, y, filter) => {
    let best = null;
    for (const ed of edges) {
      if (!ed.in || !filter(ed)) continue;
      const pts = edgePts(ed);
      for (let i = 0; i < pts.length - 2; i += 2) {
        const d = segDist2(x, y, pts[i], pts[i + 1], pts[i + 2], pts[i + 3]);
        if (!best || d < best.d) best = { d, ed, i, pts };
      }
    }
    const { pts, i } = best;
    const ax = pts[i], ay = pts[i + 1], bx = pts[i + 2], by = pts[i + 3];
    const L = Math.hypot(bx - ax, by - ay) || 1, ux = (bx - ax) / L, uy = (by - ay) / L;
    const t = Math.max(0.15 * L, Math.min(0.85 * L, (x - ax) * ux + (y - ay) * uy));
    const px = ax + ux * t, py = ay + uy * t;
    const side = Math.sign((x - px) * -uy + (y - py) * ux) || 1; // +1 = rechts in Fahrtrichtung a→b (y nach unten)
    return { ed: best.ed, px, py, ux, uy, nx: -uy * side, ny: ux * side, side, half: best.ed.w / 10 * S / 2 };
  };
  const at = (s, along, out) => ({ x: Math.round(s.px + s.ux * along + s.nx * out), y: Math.round(s.py + s.uy * along + s.ny * out) });
  const angleOn = (s) => Math.atan2(s.uy * s.side, s.ux * s.side); // Rechtsverkehr: rechte Seite fährt a→b

  const gp = places.giver;
  const [gx, gy] = toPx(gp.lat, gp.lon);
  const sg = snap(gx, gy, (ed) => ed.c >= 3 && ed.c <= 8 && !ed.pass);
  let off = sg.half + 2.2 * S;
  while (off > sg.half && inBuilding(at(sg, 0, off).x, at(sg, 0, off).y)) off -= 0.3 * S;
  const giver = { ...at(sg, 0, off), name: gp.name };
  const playerSpawn = at(sg, 3.4 * S, off);
  const curb = sg.half - 1.3 * S;
  const dropoff = { ...at(sg, 18 * S, curb), name: places.dropoff.name };
  const playerCar = { ...at(sg, 30 * S, curb), angle: angleOn(sg) };
  const parked = [{ ...at(sg, 7 * S, curb), angle: angleOn(sg) }];

  const pk = places.pickup;
  const wh = buildings.find((b) => b.id === pk.osmWay);
  if (!wh) throw new Error(`Lagerhalle (OSM-Weg ${pk.osmWay}) nicht gefunden`);
  wh.k = BUILDING_KIND.warehouse;
  const o = wh.rings[0].pts; let cx = 0, cy = 0; for (let i = 0; i < o.length; i += 2) { cx += o[i]; cy += o[i + 1]; }
  cx /= o.length / 2; cy /= o.length / 2;
  const sp = snap(cx, cy, (ed) => ed.c <= 9 && !ed.pass);
  const pickup = { ...at(sp, 0, Math.max(0, sp.half - 1.5 * S)), name: pk.name };
  const crates = [];
  for (let k = -2; k <= 2 && crates.length < 4; k++) {
    const c = at(sp, k * 2.6 * S, sp.half + 2 * S);
    if (!inBuilding(c.x, c.y)) crates.push({ x: c.x - S, y: c.y - S, w: 2 * S, h: 2 * S });
  }

  // Späti-Gebäude = nächstes Gebäude zum Auftraggeber
  let best = null;
  for (const b of buildings) {
    const r = b.rings[0].pts;
    for (let i = 0; i < r.length; i += 2) {
      const j = (i + 2) % r.length;
      const d = segDist2(giver.x, giver.y, r[i], r[i + 1], r[j], r[j + 1]);
      if (!best || d < best.d) best = { d, b };
    }
  }
  best.b.k = BUILDING_KIND.spaeti;

  // Zeitlimit aus der kürzesten Route (ungerichtet, nur Straßen im Gebiet)
  const graph = new Map();
  const link = (a, b, d) => (graph.get(a) ?? graph.set(a, []).get(a)).push([b, d]);
  for (const ed of edges) if (ed.in && ed.c <= 9 && !ed.blocked && !ed.pass) { // wie die Autos: keine Sperren, keine Tordurchfahrten
    const pts = edgePts(ed); let L = 0;
    for (let i = 0; i < pts.length - 2; i += 2) L += Math.hypot(pts[i + 2] - pts[i], pts[i + 3] - pts[i + 1]);
    link(ed.a, ed.b, L); link(ed.b, ed.a, L);
  }
  const dist = (from, to) => {
    const D = new Map([[from, 0]]), done = new Set(), q = [[0, from]];
    while (q.length) {
      q.sort((a, b) => b[0] - a[0]);
      const [d, u] = q.pop();
      if (done.has(u)) continue; done.add(u);
      if (u === to) return d;
      for (const [v, l] of graph.get(u) ?? []) if (!D.has(v) || d + l < D.get(v)) { D.set(v, d + l); q.push([d + l, v]); }
    }
    return Infinity;
  };
  const route = dist(sg.ed.a, sp.ed.a) + dist(sp.ed.a, sg.ed.a);
  if (!Number.isFinite(route)) throw new Error('Späti und Lagerhalle sind nicht über Straßen verbunden');
  const meters = route / S;
  const timeLimit = Math.ceil((meters / 10 + 60) / 10) * 10; // 10 m/s Schnitt: überwiegend Tempo 30, Ampeln

  for (const [k, p] of Object.entries({ giver, playerSpawn, dropoff, playerCar, pickup })) {
    if (!insideBorder(p.x, p.y)) throw new Error(`${k} liegt außerhalb des Gebiets`);
    if (inBuilding(p.x, p.y)) throw new Error(`${k} liegt in einem Gebäude`);
  }
  return { giver, playerSpawn, dropoff, playerCar, parked, pickup, crates, timeLimit, routeMeters: Math.round(meters) };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const arg = (k, d) => { const i = process.argv.indexOf(k); return i > 0 ? process.argv[i + 1] : d; };
  const root = new URL('../../', import.meta.url);
  const out = fileURLToPath(new URL(arg('--out', 'web/data/city.json'), root));
  const t0 = Date.now();
  const lor = JSON.parse(await readFile(new URL('data/raw/lor.json', root)));
  const osm = JSON.parse(await readFile(new URL('data/raw/osm.json', root)));
  const fetched = JSON.parse(await readFile(new URL('data/raw/fetched.json', root)));
  osm.bbox = fetched.bbox;
  const places = JSON.parse(await readFile(new URL('data/places.json', root)));
  let kataster = [];
  try { kataster = JSON.parse(await readFile(new URL('data/raw/baeume.json', root))); } catch { console.warn('data/raw/baeume.json fehlt – nur OSM-Bäume'); }
  const city = buildCity(lor, osm, places, { scale: Number(arg('--scale', 10)), kataster });
  const json = JSON.stringify(city);
  await mkdir(dirname(out), { recursive: true });
  await writeFile(out, json);
  console.log(`${out}: ${(json.length / 1e6).toFixed(2)} MB, ${city.meta.width}×${city.meta.height} px, ` +
    `${city.buildings.length} Gebäude, ${city.edges.length} Straßenkanten, ${city.vertices.length / 2} Knoten, ` +
    `${city.trees.g.length} Bäume (${city.meta.trees.kataster} Kataster, ${city.meta.trees.osmKept} OSM; ${city.meta.trees.moved} an den Bordstein gerückt, ${city.meta.trees.dropped} entfernt), ${JSON.stringify(city.meta.access)}, ${city.pois.length} POIs, ${city.addresses.nr.length} Hausnummern, ${city.walls.length} Wandzüge, Route ${city.places.routeMeters} m → ${city.places.timeLimit} s, ` +
    `${((Date.now() - t0) / 1000).toFixed(1)} s`);
}
