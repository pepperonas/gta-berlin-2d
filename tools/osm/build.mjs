// Baut aus den Rohdaten (data/raw/) die Spielkarte von ganz Berlin: web/data/berlin/index.json (Grenzen, Orte,
// Missionsorte, Abdeckung), web/data/berlin/overview.json (Stadtplan) und je 640 × 640 m eine Kachel
// web/data/berlin/tiles/<x>_<y>.json, die das Spiel um die Kamera nachlädt (web/src/map.js).
//   node --max-old-space-size=12000 tools/osm/build.mjs [--scale 10]
// Einheit im Spiel: px; Standard 10 px = 1 m (siehe web/src/config.js).
import { readFile, writeFile, mkdir, rm } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { crossSection, maxspeedOf, surfaceOf } from './crosssection.mjs';
import { makeProjection, pointInRing, ringArea, simplify, segDist2, joinRings, unionOutline, bboxOfFeatures } from './geo.mjs';
import { storeFromPbf, storeFromElements } from './store.mjs';
import { tileCity, TILE_PX } from './tiles.mjs';
import { ringIndex, insideIndex, pointInRings } from '../../web/src/geom.js';
import { WALL_KIND, ROAD_CLASS, ROAD_CLASSES, TRAFFIC_MAX_CLASS, AREA_KIND, BUILDING_KIND, TREE_TRUNK_M, TREE_FREE_MAX_CLASS, POI_CAT, PARK, PARK_ORIENT, TREE_GENERA } from '../../web/src/citycodes.js';

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

// Was aus einem Gebäude-Objekt wird: 'building' (Umriss), 'part' (Bauteil, nur ohne umgebenden Umriss), 'skip', null
// (kein Gebäude). Nie am Boden stehende Teile werden übersprungen – sie sind keine Mauer, man fährt darunter hindurch:
// Dächer, Brückenbauwerke (Pfeiler im Wasser, Kreuzgänge), alles ab 3 m über Grund bzw. ab dem 1. Geschoss.
export function buildingTreatment(t) {
  const b = t.building, part = t['building:part'];
  if ((!b || b === 'no') && (!part || part === 'no')) return null;
  if (num(t.layer) < 0 || t.location === 'underground') return 'skip';
  if (b === 'roof' || part === 'roof' || b === 'bridge' || part === 'bridge' || t.man_made === 'bridge') return 'skip';
  if (num(t.min_height) >= 3 || num(t['building:min_level']) >= 1) return 'skip';
  if (part && part !== 'no') return 'part';
  return 'building';
}

// Bauteile einordnen: Teile, deren Mittelpunkt in einem Gebäudeumriss liegt, gehören zu diesem (nichts zu tun).
// Übrige Teile bilden Gruppen (Mittelpunkt liegt in einem größeren Teil der Gruppe); je Gruppe wird das höchste Teil
// mit seinem eigenen Umriss ein Gebäude – der Turm auf dem Brückenpfeiler, nicht ein turmhoher Pfeiler.
export function mergeParts(buildings, parts) {
  const grid = boxGrid(400);
  const inside = (list, x, y) => { for (const g of list) if (g.rings.filter((r) => pointInRing(x, y, r.pts)).length % 2 === 1) return g; return null; };
  const centroid = (b) => { const r = b.rings[0].pts; let x = 0, y = 0; for (let i = 0; i < r.length; i += 2) { x += r[i]; y += r[i + 1]; } return [x / (r.length / 2), y / (r.length / 2)]; };
  for (const b of buildings) { const [x0, y0, x1, y1] = ringBox(b.rings[0].pts); grid.add(b, x0, y0, x1, y1); }
  const pgrid = boxGrid(400), roots = [];
  let merged = 0;
  const area = (b) => Math.abs(ringArea(b.rings[0].pts));
  parts.sort((a, b) => area(b) - area(a) || a.id - b.id);
  for (const p of parts) {
    const [cx, cy] = centroid(p);
    if (inside(grid.at(cx, cy), cx, cy)) continue;
    const host = inside(pgrid.at(cx, cy), cx, cy);
    if (host) { (host.root ?? host).group.push(p); p.root = host.root ?? host; merged++; }
    else { p.group = [p]; roots.push(p); }
    const [x0, y0, x1, y1] = ringBox(p.rings[0].pts); pgrid.add(p, x0, y0, x1, y1);
  }
  for (const r of roots) {
    let best = r;
    for (const q of r.group) if (q.h > best.h || (q.h === best.h && area(q) > area(best))) best = q;
    buildings.push({ id: best.id, h: best.h, k: best.k, rings: best.rings, measured: best.measured, fromParts: true });
  }
  return { added: roots.length, merged };
}

// Gebäudehöhe in m; measured = aus Höhe/Geschossen (sonst Schätzwert nach Gebäudeart).
function buildingHeight(t) {
  const h = num(t.height);
  if (h > 2 && h < 400) return [h, true];
  const lv = num(t['building:levels']), rl = num(t['roof:levels']);
  if (lv > 0 && lv < 80) return [lv * 3.2 + (rl > 0 ? rl * 2.4 : 1), true];
  switch (t.building) {
    case 'garage': case 'garages': case 'shed': case 'kiosk': case 'carport': case 'hut': case 'container': case 'toilets': return [3, false];
    case 'industrial': case 'warehouse': case 'retail': case 'supermarket': case 'commercial': case 'service': return [8, false];
    case 'church': case 'cathedral': return [24, false];
    case 'house': case 'detached': case 'semidetached_house': case 'bungalow': case 'terrace': return [8, false];
    default: return [16, false];
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
  if (t.man_made === 'bridge' && !(num(t.layer) < 0)) return AREA_KIND.bridge; // Brückendeck (über dem Wasser)
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
const ringBox = (r) => { let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity; for (let i = 0; i < r.length; i += 2) { x0 = Math.min(x0, r[i]); x1 = Math.max(x1, r[i]); y0 = Math.min(y0, r[i + 1]); y1 = Math.max(y1, r[i + 1]); } return [x0, y0, x1, y1]; };

// Raster für Kästchen-Abfragen im Build (Kantenlänge CELL px).
function boxGrid(CELL) {
  const g = new Map();
  return {
    add(item, x0, y0, x1, y1) {
      for (let gx = Math.floor(x0 / CELL); gx <= Math.floor(x1 / CELL); gx++) for (let gy = Math.floor(y0 / CELL); gy <= Math.floor(y1 / CELL); gy++) {
        const k = gx * 1000000 + gy; let l = g.get(k); if (!l) g.set(k, l = []); l.push(item);
      }
    },
    at(x, y) { return g.get(Math.floor(x / CELL) * 1000000 + Math.floor(y / CELL)) ?? []; },
    box(x0, y0, x1, y1, out = new Set()) {
      for (let gx = Math.floor(x0 / CELL); gx <= Math.floor(x1 / CELL); gx++) for (let gy = Math.floor(y0 / CELL); gy <= Math.floor(y1 / CELL); gy++) for (const it of g.get(gx * 1000000 + gy) ?? []) out.add(it);
      return out;
    },
  };
}

// Kleinster Binärheap für Dijkstra: [Priorität, Wert]
class Heap {
  constructor() { this.a = []; }
  get size() { return this.a.length; }
  push(p, v) { const a = this.a; a.push([p, v]); let i = a.length - 1; while (i > 0) { const j = (i - 1) >> 1; if (a[j][0] <= a[i][0]) break; [a[i], a[j]] = [a[j], a[i]]; i = j; } }
  pop() {
    const a = this.a, top = a[0], last = a.pop();
    if (a.length) { a[0] = last; let i = 0; for (;;) { const l = 2 * i + 1, r = l + 1; let m = i; if (l < a.length && a[l][0] < a[m][0]) m = l; if (r < a.length && a[r][0] < a[m][0]) m = r; if (m === i) break; [a[i], a[m]] = [a[m], a[i]]; i = m; } }
    return top;
  }
}

// lor: LOR-Prognoseräume (GeoJSON, Grenze = ihre Vereinigung); osm: Store (store.mjs) oder { elements } (Overpass-Format);
// places: Missionsorte; kataster: Berliner Baumbestand. Liefert { index, overview, tiles: Map Schlüssel → Kachel }.
export function buildCity(lor, osmIn, places, { scale = 10, kataster = [], tile = TILE_PX, marginM = 250, log = () => {} } = {}) {
  const S = scale;
  const osm = osmIn.coord ? osmIn : storeFromElements(osmIn.elements, { timestamp: osmIn.osm3s?.timestamp_osm_base ?? null });
  const [s, w, n, e] = bboxOfFeatures(lor.features, marginM);
  const lat0 = (s + n) / 2, lon0 = (w + e) / 2;
  const proj = makeProjection(lat0, lon0);
  const corners = [proj(s, w), proj(s, e), proj(n, w), proj(n, e)];
  const minX = Math.min(...corners.map((c) => c[0])), maxX = Math.max(...corners.map((c) => c[0]));
  const minY = Math.min(...corners.map((c) => c[1])), maxY = Math.max(...corners.map((c) => c[1]));
  const W = Math.round((maxX - minX) * S), H = Math.round((maxY - minY) * S);
  const toPx = (lat, lon) => { const [x, y] = proj(lat, lon); return [Math.round((x - minX) * S), Math.round((maxY - y) * S)]; };
  const P = (id) => { const c = osm.coord(id); return c ? toPx(c[0], c[1]) : null; };
  const flat = (ids) => { const out = []; for (const id of ids) { const p = P(id); if (p) out.push(p[0], p[1]); } return out; };
  const inBounds = (pts) => { for (let i = 0; i < pts.length; i += 2) if (pts[i] >= 0 && pts[i] <= W && pts[i + 1] >= 0 && pts[i + 1] <= H) return true; return false; };
  const t0 = Date.now(), step = (msg) => log(`  ${((Date.now() - t0) / 1000).toFixed(0).padStart(4)} s  ${msg}`);

  // --- Grenze, Bezirke, Ortsteile -------------------------------------------------------------
  const pgrRings = lor.features.map((f) => ({ f, rings: f.geometry.coordinates.map((poly) => poly[0].flatMap(([lon, lat]) => toPx(lat, lon))) }));
  const keepLoops = (loops) => loops.filter((r) => Math.abs(ringArea(r)) > 20000 * S * S); // Splitter aus Rundungsnähten weg
  const border = keepLoops(unionOutline(pgrRings.flatMap((p) => p.rings)));
  const borderIx = ringIndex(border);
  const insideBorder = (x, y) => insideIndex(borderIx, x, y);
  const bezName = (f) => (f.properties.bez ?? f.properties.pgr_id?.slice(0, 2) ?? '?').replace(/^\d+\s*-\s*/, '');
  const bezGroups = new Map();
  for (const p of pgrRings) { const k = bezName(p.f); (bezGroups.get(k) ?? bezGroups.set(k, []).get(k)).push(...p.rings); }
  const bezirke = [...bezGroups].map(([name, rings]) => ({ name, rings: keepLoops(unionOutline(rings)) })).sort((a, b) => a.name.localeCompare(b.name));
  const bezIx = bezirke.map((b) => ({ name: b.name, ix: ringIndex(b.rings) }));
  const bezirkAt = (x, y) => { for (const b of bezIx) if (insideIndex(b.ix, x, y)) return b.name; return null; };
  // Ortsteile aus OSM (admin_level 10); ohne sie (Test-Fixture) die LOR-Prognoseräume.
  let districts = [];
  for (const r of osm.relations) {
    const t = r.tags;
    if (t?.type !== 'boundary' || t.boundary !== 'administrative' || t.admin_level !== '10' || !t.name) continue;
    const outer = joinRings(r.members.filter((m) => m.type === 'way' && m.role !== 'inner').map((m) => osm.ways.get(m.ref)?.nodes).filter(Boolean));
    const rings = outer.map((ids) => flat(ids)).filter((pts) => pts.length >= 8);
    if (!rings.length) continue;
    const [x0, y0, x1, y1] = ringBox(rings[0]);
    if (!insideBorder((x0 + x1) / 2, (y0 + y1) / 2) && !rings.some((rr) => { for (let i = 0; i < rr.length; i += 40) if (insideBorder(rr[i], rr[i + 1])) return true; return false; })) continue;
    districts.push({ name: t.name, rings });
  }
  if (!districts.length) districts = pgrRings.map((p) => ({ name: p.f.properties.pgr_name ?? p.f.properties.pgr_id, rings: p.rings }));
  districts.sort((a, b) => a.name.localeCompare(b.name));
  step(`Grenze ${border.length} Ring(e), ${bezirke.length} Bezirke, ${districts.length} Ortsteile`);

  // --- Polygone -------------------------------------------------------------------------------
  const polygons = [];
  for (const el of osm.ways.values()) {
    if (el.tags && el.nodes.length >= 4 && el.nodes[0] === el.nodes[el.nodes.length - 1]) polygons.push({ id: el.id, tags: el.tags, rings: [{ pts: flat(el.nodes), outer: true }] });
  }
  for (const el of osm.relations) {
    if (el.tags?.type !== 'multipolygon') continue;
    const get = (role) => el.members.filter((m) => m.type === 'way' && (m.role === role || (role === 'outer' && m.role === ''))).map((m) => osm.ways.get(m.ref)?.nodes).filter(Boolean);
    const outer = joinRings(get('outer')), inner = joinRings(get('inner'));
    if (!outer.length) continue;
    polygons.push({ id: el.id + 1e12, tags: el.tags, rings: [...outer.map((r) => ({ pts: flat(r), outer: true })), ...inner.map((r) => ({ pts: flat(r), outer: false }))] });
  }
  // Außenringe gegen den Uhrzeigersinn (im px-System mit y nach unten: Fläche < 0), Löcher andersherum.
  const orient = (pts, outer) => {
    const a = ringArea(pts);
    if ((a > 0) === outer) { const rev = []; for (let i = pts.length - 2; i >= 0; i -= 2) rev.push(pts[i], pts[i + 1]); return rev; }
    return pts;
  };
  const cleanRings = (poly, tol) => poly.rings.map((r) => {
    const sp = simplify(r.pts, tol);
    return sp.length >= 8 ? { outer: r.outer, pts: orient(sp, r.outer) } : null;
  }).filter(Boolean);

  const buildings = [], water = [], areas = [], parts = [];
  let heightMeasured = 0;
  for (const poly of polygons) {
    const t = poly.tags;
    if (!poly.rings.some((r) => inBounds(r.pts))) continue;
    const bt = buildingTreatment(t);
    if (bt) {
      if (bt === 'skip') continue;
      const rings = cleanRings(poly, 0.3 * S);
      if (!rings.length || !rings[0].outer) continue;
      if (Math.abs(ringArea(rings[0].pts)) < 4 * S * S) continue; // < 4 m²
      const [h, measured] = buildingHeight(t);
      const b = { id: poly.id, h: Math.round(h * 10), k: buildingKind(t), rings, measured };
      if (bt === 'part') parts.push(b);
      else { if (measured) heightMeasured++; buildings.push(b); }
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
  polygons.length = 0;
  areas.sort((a, b) => a.k - b.k || a.id - b.id);
  // Bauteile (building:part) ohne umgebenden Gebäudeumriss werden selbst zu Gebäuden (etwa die Türme der
  // Oberbaumbrücke, die nur als Teile erfasst sind); ineinanderliegende Teile zu einem, mit der größten Höhe.
  const partStats = mergeParts(buildings, parts);
  for (const b of buildings) if (b.fromParts && b.measured) heightMeasured++;
  buildings.sort((a, b) => a.id - b.id);
  water.sort((a, b) => a.id - b.id);
  step(`${buildings.length} Gebäude (davon ${partStats.added} aus Bauteilen), ${water.length} Wasserflächen, ${areas.length} Flächen`);

  // --- Straßen und Graph ---------------------------------------------------------------
  const roadWays = [], pathWays = [], railWays = [];
  const PATHS = new Set(['footway', 'cycleway', 'path', 'steps', 'bridleway']);
  for (const el of osm.ways.values()) {
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
  const nameOf = (x) => { if (!x) return -1; let k = nameIdx.get(x); if (k === undefined) { k = names.length; names.push(x); nameIdx.set(x, k); } return k; };
  const edges = [];
  const tagStats = { width: 0, parking: 0, maxspeed: 0, surface: 0, lanes: 0, main: 0 };
  for (const { el, base } of roadWays) {
    const t = el.tags;
    const ids = el.nodes.filter((id) => osm.has(id));
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
    // Erfasste Merkmale (für den Abdeckungsbericht): gemessen/getaggt statt Standardwert
    const tagged = { width: !!(t['width:carriageway'] || t.width), parking: Object.keys(t).some((k) => k.startsWith('parking')), maxspeed: !!t.maxspeed, surface: !!t.surface, lanes: !!t.lanes };
    let start = 0;
    for (let i = 1; i < ids.length; i++) {
      if (i === ids.length - 1 || use.get(ids[i]) > 1) {
        const seg = ids.slice(start, i + 1);
        const pts = simplify(flat(seg), 0.5 * S);
        if (seg.length >= 2 && (pts[0] !== pts[pts.length - 2] || pts[1] !== pts[pts.length - 1])) {
          edges.push({ a: vertex(seg[0]), b: vertex(seg[seg.length - 1]), c: cls, w: width, n: name, o: oneway, br: bridge, p: pts.slice(2, -2), id: el.id, x, ids: seg, pass: t.tunnel === 'building_passage' ? 1 : 0, tagged });
        }
        start = i;
      }
    }
  }
  use.clear();
  // Nur Verkehrskanten innerhalb der Grenze kommen in den Fahr-/Gehgraphen.
  const vx = (k) => vertices[2 * k], vy = (k) => vertices[2 * k + 1];
  const edgePts = (ed) => [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)];
  for (const ed of edges) {
    const mx = (vx(ed.a) + vx(ed.b)) / 2, my = (vy(ed.a) + vy(ed.b)) / 2;
    ed.in = insideBorder(mx, my) ? 1 : 0;
  }
  step(`${edges.length} Straßenkanten, ${vertices.length / 2} Knoten`);

  const paths = pathWays.map((el) => ({ br: el.tags.bridge && el.tags.bridge !== 'no' ? 1 : 0, pass: el.tags.tunnel === 'building_passage' ? 1 : 0, p: simplify(flat(el.nodes), 0.8 * S) })).filter((x) => x.p.length >= 4 && inBounds(x.p));
  const rails = railWays.map((el) => ({ br: el.tags.bridge && el.tags.bridge !== 'no' ? 1 : 0, sub: el.tags.railway === 'subway' ? 1 : 0, ids: el.nodes, p: simplify(flat(el.nodes), 0.5 * S) })).filter((x) => x.p.length >= 4 && inBounds(x.p));

  // --- Wände: Ufer und Gleise, an Brücken/Übergängen aufgeschnitten --------------------
  const corridors = []; // [ax, ay, bx, by, r]
  const addCorridor = (list, pts, r, ext) => {
    for (let i = 0; i < pts.length - 2; i += 2) {
      let ax = pts[i], ay = pts[i + 1], bx = pts[i + 2], by = pts[i + 3];
      const L = Math.hypot(bx - ax, by - ay) || 1, ux = (bx - ax) / L, uy = (by - ay) / L;
      if (i === 0) { ax -= ux * ext; ay -= uy * ext; }
      if (i === pts.length - 4) { bx += ux * ext; by += uy * ext; }
      list.push([ax, ay, bx, by, r]);
    }
  };
  for (const ed of edges) if (ed.br) addCorridor(corridors, edgePts(ed), ed.w / 10 * S / 2 + 1.5 * S, 6 * S);
  for (const pa of paths) if (pa.br) addCorridor(corridors, pa.p, 2.5 * S, 4 * S);
  // Straßen, die Gleise ebenerdig kreuzen (gemeinsamer Knoten), öffnen die Gleiswand.
  const railNodes = new Set(); for (const r of rails) if (!r.br) for (const id of r.ids) railNodes.add(id);
  for (const { el, base } of roadWays) for (const id of el.nodes) if (railNodes.has(id)) {
    const p = P(id); if (p) corridors.push([p[0], p[1], p[0], p[1], (DEFAULT_WIDTH[base] ?? 8) / 2 * S + 2 * S]);
  }
  const cut = makeCutter(corridors, S);
  // Tordurchfahrten öffnen die Hauswände (eigener Korridor-Satz, nur für Gebäude).
  const passages = [];
  for (const ed of edges) if (ed.pass) addCorridor(passages, edgePts(ed), Math.max(ed.w / 10 * S / 2, 1.6 * S), 2 * S);
  for (const pa of paths) if (pa.pass) addCorridor(passages, pa.p, 1.4 * S, 2 * S);
  const passCut = makeCutter(passages, S);
  const passGrid = boxGrid(4000);
  for (const c of passages) passGrid.add(c, Math.min(c[0], c[2]) - c[4], Math.min(c[1], c[3]) - c[4], Math.max(c[0], c[2]) + c[4], Math.max(c[1], c[3]) + c[4]);
  const lenOf = (p) => { let L = 0; for (let i = 0; i < p.length - 2; i += 2) L += Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]); return L; };
  for (const b of buildings) {
    const [x0, y0, x1, y1] = ringBox(b.rings[0].pts);
    let hit = false;
    for (const c of passGrid.box(x0, y0, x1, y1)) if (Math.max(c[0], c[2]) + c[4] >= x0 && Math.min(c[0], c[2]) - c[4] <= x1 && Math.max(c[1], c[3]) + c[4] >= y0 && Math.min(c[1], c[3]) - c[4] <= y1) { hit = true; break; }
    if (!hit) continue;
    const ws = b.rings.flatMap((r) => passCut([...r.pts, r.pts[0], r.pts[1]]));
    const full = b.rings.reduce((acc, r) => acc + lenOf([...r.pts, r.pts[0], r.pts[1]]), 0);
    if (ws.reduce((acc, x) => acc + lenOf(x), 0) < full - 0.5 * S) b.walls = ws; // nur wenn wirklich ein Stück Wand fehlt
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
  // Wandzüge mit Art (WALL_KIND in citycodes.js): Ufer, Gleis, Geländer, Zaun – das Spiel und die Tests unterscheiden sie
  const walls = [], wallKind = [];
  const addWall = (k) => (p) => { walls.push(p); wallKind.push(k); };
  for (const wa of water) for (const r of wa.rings) cut([...r.pts, r.pts[0], r.pts[1]]).forEach(addWall(WALL_KIND.quay));
  const railWalls = [];
  for (const r of rails) if (!r.br && !r.sub) for (const d of [-2.5 * S, 2.5 * S]) railWalls.push(...cut(offsetLine(r.p, d)));
  // Brückengeländer
  for (const ed of edges) if (ed.br) { const pts = edgePts(ed); for (const d of [-1, 1]) addWall(WALL_KIND.railing)(offsetLine(pts, d * (ed.w / 10 * S / 2 + 0.6 * S))); }
  step(`${walls.length} Wandzüge, ${buildings.filter((b) => b.walls).length} geöffnete Hauswände`);

  const fenceWalls = [];
  const access = accessAndRules(osm, { P, S, edges, vertices, vIndex, buildings, W, H, walls: fenceWalls });
  // Zäune und Gleiswände nicht auf befahrbaren Fahrbahnen: wo die (oft geschätzte) Fahrbahn einen Zaun längs überdeckt,
  // schrammte die KI daran entlang. Quer sperrende Zäune haben ihre Straße bereits gesperrt (dort wird nichts
  // ausgeschnitten). Ufer bleiben unangetastet, sonst ginge es dort ins Wasser.
  const driveCut = makeCutter(edges.filter((ed) => ed.c <= 8 && !ed.blocked && !ed.pass).flatMap((ed) => { const p = edgePts(ed), out = []; for (let i = 0; i < p.length - 2; i += 2) out.push([p[i], p[i + 1], p[i + 2], p[i + 3], ed.w / 10 * S / 2 + 0.3 * S]); return out; }), S);
  let cutWalls = 0;
  for (const [list, k] of [[railWalls, WALL_KIND.rail], [fenceWalls, WALL_KIND.fence]]) for (const w0 of list) {
    const pieces = driveCut(w0);
    if (pieces.reduce((a, q) => a + lenOf(q), 0) < lenOf(w0) - 0.5 * S) cutWalls++;
    pieces.forEach(addWall(k));
  }
  access.out.fences = access.out.fences.flatMap(([k, p]) => driveCut(p).map((q) => [k, q]));
  access.stats.waendeAufFahrbahnGekuerzt = cutWalls;
  step(`Zugänge und Regeln ${JSON.stringify(access.stats)}`);

  // --- Kreuzungsflächen und Spurkürzung je Knoten (Spiel und Baumregel nutzen dieselben Werte) ----------
  const junctions = junctionsOf(edges, vertices, S);
  const trim = laneTrim(edges, vertices, S, edgePts);
  const postStats = keepPostsOffCarriageway(access.out, { edges, vertices, junctions, S });
  access.stats.pollerVerschoben = postStats.moved; access.stats.pollerEntfernt = postStats.dropped;

  // --- Bäume, Kiez-Namen ----------------------------------------------------------------
  const trees = [], kieze = [];
  for (const nd of osm.tagged) {
    const t = nd.tags;
    if (t.natural === 'tree') { const p = toPx(nd.lat, nd.lon); if (p[0] >= 0 && p[1] >= 0 && p[0] <= W && p[1] <= H) trees.push({ x: p[0], y: p[1], g: 0, c: 0, r: 0, osm: true }); }
    else if (['neighbourhood', 'quarter'].includes(t.place) && t.name) { const p = toPx(nd.lat, nd.lon); if (insideBorder(p[0], p[1])) kieze.push({ n: t.name, x: p[0], y: p[1] }); }
  }
  // Kieze, die in OSM als Fläche statt als Punkt eingetragen sind (Schwerpunkt der Außenpunkte)
  const kiezNames = new Set(kieze.map((k) => k.n));
  for (const el of [...osm.ways.values(), ...osm.relations]) {
    const t = el.tags;
    if (!t || !['neighbourhood', 'quarter'].includes(t.place) || !t.name || kiezNames.has(t.name)) continue;
    const ids = el.type === 'way' ? el.nodes : el.members.filter((m) => m.type === 'way' && m.role !== 'inner').flatMap((m) => osm.ways.get(m.ref)?.nodes ?? []);
    let x = 0, y = 0, k = 0;
    for (const id of new Set(ids)) { const p = P(id); if (p) { x += p[0]; y += p[1]; k++; } }
    if (!k) continue;
    x = Math.round(x / k); y = Math.round(y / k);
    if (insideBorder(x, y)) { kieze.push({ n: t.name, x, y }); kiezNames.add(t.name); }
  }
  const katStats = mergeKataster(trees, kataster, { toPx, W, H, S });
  const treeStats = { ...katStats, ...keepTreesOffRoads(trees, { edges, vertices, buildings, water, junctions, S }) };
  trees.sort((a, b) => a.y - b.y || a.x - b.x);
  kieze.sort((a, b) => a.n.localeCompare(b.n) || a.x - b.x);
  step(`${trees.length} Bäume (${JSON.stringify(treeStats)})`);
  const { pois, addresses } = extractPoisAndAddresses(osm, { P, toPx, W, H, S, nameOf });
  step(`${pois.length} POIs, ${addresses.length} Hausnummern`);

  const missionPlaces = placeMission({ places, toPx, edges, vertices, buildings, S, insideBorder });
  step(`Mission: Route ${missionPlaces.routeMeters} m → ${missionPlaces.timeLimit} s`);

  const g = {
    S, W, H, names, vertices, edges, paths, rails, buildings, water, areas, walls, wallKind, trees, kieze, pois, addresses, junctions, trim,
    border, bezirke, districts, access: access.out,
  };
  const meta = {
    version: 3, scale: S, width: W, height: H, origin: { lat0, lon0, bbox: [s, w, n, e] },
    osmBase: osm.timestamp ?? null,
    attribution: 'Kartendaten © OpenStreetMap-Mitwirkende (ODbL) · Grenzen und Baumbestand: Geoportal Berlin (dl-de/zero-2.0)',
    classes: ROAD_CLASSES, trafficMaxClass: TRAFFIC_MAX_CLASS,
    trees: treeStats, access: access.stats,
    counts: { buildings: buildings.length, heightMeasured, edges: edges.length, vertices: vertices.length / 2, trees: trees.length, pois: pois.length, addresses: addresses.length, water: water.length, areas: areas.length, walls: walls.length },
    coverage: coverage({ bezirkAt, edges, vx, vy, buildings, trees, pois, addresses, access: access.out, junctions }),
  };
  const out = tileCity(g, { tile, meta, places: missionPlaces });
  step(`${out.tiles.size} Kacheln`);
  return out;
}

// --- Abdeckung je Bezirk: Ist jede Datenschicht überall gefüllt? ----------------------------------
function coverage({ bezirkAt, edges, vx, vy, buildings, trees, pois, addresses, access, junctions }) {
  const cov = {};
  const row = (b) => (cov[b] ??= { strassenKm: 0, hauptnetzKm: 0, breiteGemessen: 0, parkenErfasst: 0, tempoErfasst: 0, belagErfasst: 0, gebaeude: 0, hoeheGemessen: 0,
    baeume: 0, katasterBaeume: 0, pois: 0, haltestellen: 0, bahnhoefe: 0, hausnummern: 0, ampeln: 0, querungen: 0, abbiegeverbote: 0,
    poller: 0, zaeune: 0, durchfahrten: 0, tueren: 0, kreuzungen: 0 });
  const add = (x, y, k, v = 1) => { const b = bezirkAt(x, y); if (b) row(b)[k] += v; };
  for (const ed of edges) {
    if (!ed.in) continue;
    const pts = [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)];
    let L = 0; for (let i = 0; i < pts.length - 2; i += 2) L += Math.hypot(pts[i + 2] - pts[i], pts[i + 3] - pts[i + 1]);
    const km = L / 10000, mx = (vx(ed.a) + vx(ed.b)) / 2, my = (vy(ed.a) + vy(ed.b)) / 2;
    add(mx, my, 'strassenKm', km);
    if (ed.pass) add(mx, my, 'durchfahrten');
    if (ed.c > 8) continue;
    add(mx, my, 'hauptnetzKm', km);
    if (ed.tagged.width) add(mx, my, 'breiteGemessen', km);
    if (ed.tagged.parking) add(mx, my, 'parkenErfasst', km);
    if (ed.tagged.maxspeed) add(mx, my, 'tempoErfasst', km);
    if (ed.tagged.surface) add(mx, my, 'belagErfasst', km);
  }
  for (const b of buildings) { const [x0, y0, x1, y1] = ringBox(b.rings[0].pts); add((x0 + x1) / 2, (y0 + y1) / 2, 'gebaeude'); if (b.measured) add((x0 + x1) / 2, (y0 + y1) / 2, 'hoeheGemessen'); if (b.doors) add((x0 + x1) / 2, (y0 + y1) / 2, 'tueren', b.doors.length); }
  for (const t of trees) { add(t.x, t.y, 'baeume'); if (!t.osm) add(t.x, t.y, 'katasterBaeume'); }
  for (const q of pois) { add(q.x, q.y, 'pois'); if (q.cat === 'bus') add(q.x, q.y, 'haltestellen'); if (['ubahn', 'sbahn', 'bahn'].includes(q.cat)) add(q.x, q.y, 'bahnhoefe'); }
  for (const a of addresses) add(a.x, a.y, 'hausnummern');
  for (const v of access.signals) add(access.vertexXY(v)[0], access.vertexXY(v)[1], 'ampeln');
  for (let i = 0; i < access.crossings.length; i += 4) add(access.crossings[i], access.crossings[i + 1], 'querungen');
  for (let i = 0; i < access.turnBans.length; i += 3) { const [x, y] = access.vertexXY(access.turnBans[i + 1]); add(x, y, 'abbiegeverbote'); }
  for (let i = 0; i < access.barriers.length; i += 6) add(access.barriers[i], access.barriers[i + 1], 'poller', access.barriers[i + 5]);
  for (let i = 0; i < access.posts.length; i += 3) add(access.posts[i], access.posts[i + 1], 'poller');
  for (const [, pts] of access.fences) add(pts[0], pts[1], 'zaeune');
  for (const j of junctions) add(j.x, j.y, 'kreuzungen');
  for (const r of Object.values(cov)) for (const k of Object.keys(r)) r[k] = Math.round(r[k] * 10) / 10;
  return cov;
}

// --- Kreuzungsflächen: Knoten mit ≥ 3 Straßen oder einem Knick, Radius = größte halbe Fahrbahnbreite + 2 m ----------
export function junctionsOf(edges, vertices, S) {
  const vx = (k) => vertices[2 * k], vy = (k) => vertices[2 * k + 1];
  const at = new Map();
  for (const ed of edges) if (ed.c <= 8 && !ed.pass) for (const v of ed.a === ed.b ? [ed.a] : [ed.a, ed.b]) (at.get(v) ?? at.set(v, []).get(v)).push(ed);
  const heading = (ed, v) => { const pts = [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)]; const i = ed.a === v ? 0 : pts.length - 4; const a = Math.atan2(pts[i + 3] - pts[i + 1], pts[i + 2] - pts[i]); return ed.a === v ? a : a + Math.PI; };
  const out = [];
  for (const [v, es] of at) {
    let corner = es.length >= 3;
    if (es.length === 2) { let d = heading(es[1], v) - heading(es[0], v) - Math.PI; while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI; corner = Math.abs(d) > 0.5; }
    if (!corner) continue;
    const r = Math.round(Math.max(...es.map((ed) => ed.w / 10 * S / 2)) + 2 * S);
    out.push({ v, x: vx(v), y: vy(v), r, bridge: es.some((ed) => ed.br) ? 1 : 0, cobble: es.every((ed) => ed.x[11] === 1) ? 1 : 0, cls: Math.min(...es.map((ed) => ed.c)) });
  }
  return out.sort((a, b) => a.v - b.v);
}

// Spurkürzung an Kreuzungen (px): Knoten mit ≥ 3 befahrbaren Kanten → größte halbe Breite + 1 m (wie roadgraph.js).
export function laneTrim(edges, vertices, S, edgePts) {
  const deg = new Map(), rad = new Map();
  for (const ed of edges) {
    if (!(ed.in && ed.c <= TRAFFIC_MAX_CLASS && !ed.blocked && !ed.pass)) continue;
    let L = 0; const p = edgePts(ed); for (let i = 0; i < p.length - 2; i += 2) L += Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]);
    if (L <= 5) continue;
    for (const v of [ed.a, ed.b]) { deg.set(v, (deg.get(v) ?? 0) + 1); rad.set(v, Math.max(rad.get(v) ?? 0, ed.w / 10 * S / 2)); }
  }
  const trim = new Map();
  for (const [v, d] of deg) if (d >= 3) trim.set(v, Math.round(rad.get(v) + 1 * S));
  return trim;
}

// --- POIs und Hausnummern ----------------------------------------------------------------
// Position: Knoten direkt, Wege/Relationen über den Schwerpunkt ihrer (Außen-)Punkte.
function extractPoisAndAddresses(osm, { P, toPx, W, H, S, nameOf }) {
  const center = (el) => {
    if (el.type === 'node') return toPx(el.lat, el.lon);
    const ids = el.type === 'way' ? el.nodes : el.members.filter((m) => m.type === 'way' && m.role !== 'inner').flatMap((m) => osm.ways.get(m.ref)?.nodes ?? []);
    let x = 0, y = 0, n = 0;
    for (const id of new Set(ids)) { const p = P(id); if (p) { x += p[0]; y += p[1]; n++; } }
    return n ? [Math.round(x / n), Math.round(y / n)] : null;
  };
  const inside = (p) => p && p[0] >= 0 && p[1] >= 0 && p[0] <= W && p[1] <= H;
  const pois = [], addresses = [];
  const seenStation = new Map(); // Kategorie|Name → Positionen
  for (const el of osm.elements()) {
    const t = el.tags;
    if (!t) continue;
    const cat = poiCategory(t);
    if (cat && (t.name || cat === 'bus')) {
      const p = center(el);
      if (inside(p)) {
        const name = (t.name ?? 'Haltestelle').replace(/^(S\+U|U\+S|[SU]) (?=\S)/, '').replace(/^Berlin /, '');
        // Bahnhöfe und Bushaltestellen haben oft mehrere Knoten (Bahnsteige, Richtungen): einmal je Name und Umkreis.
        const dedupe = { ubahn: 400 * S, sbahn: 400 * S, bahn: 400 * S, bus: 80 * S }[cat];
        const key = cat + '|' + name, seen = seenStation.get(key) ?? [];
        if (!dedupe || !seen.some((q) => Math.hypot(q[0] - p[0], q[1] - p[1]) < dedupe)) {
          if (dedupe) { seen.push(p); seenStation.set(key, seen); }
          pois.push({ x: p[0], y: p[1], cat, n: nameOf(name), k: nameOf(t.shop ?? t.amenity ?? t.tourism ?? t.station ?? t.railway ?? t.highway ?? ''), id: el.id, type: el.type, name });
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
export function makeCutter(corridors, S) {
  const CELL = 400, cgrid = new Map();
  corridors.forEach((c, i) => {
    const x0 = Math.floor((Math.min(c[0], c[2]) - c[4]) / CELL), x1 = Math.floor((Math.max(c[0], c[2]) + c[4]) / CELL);
    const y0 = Math.floor((Math.min(c[1], c[3]) - c[4]) / CELL), y1 = Math.floor((Math.max(c[1], c[3]) + c[4]) / CELL);
    for (let gx = x0; gx <= x1; gx++) for (let gy = y0; gy <= y1; gy++) { const k = gx * 1000000 + gy; (cgrid.get(k) ?? cgrid.set(k, []).get(k)).push(i); }
  });
  const inCorridor = (x, y) => {
    for (const i of cgrid.get(Math.floor(x / CELL) * 1000000 + Math.floor(y / CELL)) ?? []) {
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

export function accessAndRules(osm, { P, S, edges, vertices, vIndex, buildings, W, H, walls }) {
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
  for (const nd of osm.tagged) {
    const t = nd.tags;
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
          // hinter die Kreuzungsfläche (Radius = größte halbe Breite der Straßen am Knoten + 2 m, siehe junctionsOf)
          let other = 0; for (const o of allOn.get(nd.id) ?? []) if (edges[o.k].c <= 8 && !edges[o.k].pass) other = Math.max(other, edges[o.k].w / 10 * S / 2);
          const shift = other ? other + 2.6 * S : 0;
          bx = Math.round(bx + ux * inward * shift); by = Math.round(by + uy * inward * shift);
        }
        barriers.push([bx, by, kind, Math.round(-uy * 1000), Math.round(ux * 1000), n]);
      } else posts.push([p[0], p[1], kind]);
    }
    if (t.highway === 'crossing' || (t.crossing && onEdge.has(nd.id))) {
      const hit = onEdge.get(nd.id);
      if (hit && edges[hit.k].c <= 8) {
        const c = t.crossing, ref = t.crossing_ref, mk = t['crossing:markings'];
        const kind = c === 'traffic_signals' || t['crossing:signals'] === 'yes' ? 1 : (c === 'zebra' || ref === 'zebra' || mk === 'zebra') ? 0 : (c === 'marked' || c === 'uncontrolled' || (mk && mk !== 'no')) ? 2 : -1;
        if (kind >= 0) crossings.push([p[0], p[1], hit.k, kind]);
      }
    }
    if (t.highway === 'traffic_signals') signalNodes.push({ id: nd.id, x: p[0], y: p[1] });
  }
  // Zäune und Mauern als Wände (Tore darin bleiben offen)
  const gates = [], gateNodes = new Set();
  for (const nd of osm.tagged) if (['gate', 'swing_gate', 'kissing_gate', 'entrance', 'lift_gate'].includes(nd.tags.barrier)) { gateNodes.add(nd.id); const p = P(nd.id); if (p) gates.push([p[0], p[1], p[0], p[1], 0.9 * S]); }
  const gateCut = makeCutter(gates, S);
  // Straßenstücke für den Schnitt mit Sperrlinien (Poller-Reihen wie Diagonalsperren, Mauern, Zäune quer über die Straße)
  const G = 400, roadGrid = new Map();
  edges.forEach((ed, k) => {
    if (ed.c <= 2 || ed.c > 8) return;
    const pts = [vertices[2 * ed.a], vertices[2 * ed.a + 1], ...ed.p, vertices[2 * ed.b], vertices[2 * ed.b + 1]];
    for (let i = 0; i < pts.length - 2; i += 2) {
      const x0 = Math.floor(Math.min(pts[i], pts[i + 2]) / G), x1 = Math.floor(Math.max(pts[i], pts[i + 2]) / G);
      const y0 = Math.floor(Math.min(pts[i + 1], pts[i + 3]) / G), y1 = Math.floor(Math.max(pts[i + 1], pts[i + 3]) / G);
      for (let gx = x0; gx <= x1; gx++) for (let gy = y0; gy <= y1; gy++) { const key = gx * 1000000 + gy; (roadGrid.get(key) ?? roadGrid.set(key, []).get(key)).push([k, pts[i], pts[i + 1], pts[i + 2], pts[i + 3]]); }
    }
  });
  const crossesSeg = (ax, ay, bx, by, cx, cy, dx, dy) => {
    const d = (bx - ax) * (dy - cy) - (by - ay) * (dx - cx);
    if (Math.abs(d) < 1e-9) return false;
    const t = ((cx - ax) * (dy - cy) - (cy - ay) * (dx - cx)) / d, u = ((cx - ax) * (by - ay) - (cy - ay) * (bx - ax)) / d;
    return t >= -0.01 && t <= 1.01 && u >= -0.01 && u <= 1.01;
  };
  for (const el of [...osm.ways.values()].sort((a, b) => a.id - b.id)) {
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
      const key = Math.floor(pts[i] / G) * 1000000 + Math.floor(pts[i + 1] / G);
      for (let gx = -1; gx <= 1; gx++) for (let gy = -1; gy <= 1; gy++) for (const [kk, ax, ay, bx, by] of roadGrid.get(key + gx * 1000000 + gy) ?? []) {
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
  const bgrid = boxGrid(400);
  buildings.forEach((b) => { const [x0, y0, x1, y1] = ringBox(b.rings[0].pts); bgrid.add(b, x0, y0, x1, y1); });
  for (const nd of osm.tagged) {
    if (!nd.tags.entrance && !nd.tags.door) continue;
    const p = P(nd.id); if (!inside(p)) continue;
    let best = null;
    for (const b of bgrid.at(p[0], p[1])) {
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
  const jgrid = boxGrid(600);
  for (const [v, d] of deg) if (d >= 3) jgrid.add(v, vertices[2 * v], vertices[2 * v + 1], vertices[2 * v], vertices[2 * v + 1]);
  const signals = new Set();
  for (const sn of signalNodes) {
    const v0 = vIndex.get(sn.id);
    if (v0 !== undefined && deg.get(v0) >= 3) { signals.add(v0); continue; }
    let best = null;
    for (const v of jgrid.box(sn.x - 35 * S, sn.y - 35 * S, sn.x + 35 * S, sn.y + 35 * S)) {
      const d = Math.hypot(vertices[2 * v] - sn.x, vertices[2 * v + 1] - sn.y);
      if (d < 35 * S && (!best || d < best.d || (d === best.d && v < best.v))) best = { d, v };
    }
    if (best) signals.add(best.v);
  }
  // Abbiegeverbote: (von Kante, über Knoten, nach Kante)
  const bans = [];
  const edgesAt = new Map();
  edges.forEach((ed, k) => { for (const v of [ed.a, ed.b]) (edgesAt.get(v) ?? edgesAt.set(v, []).get(v)).push(k); });
  let restrictions = 0;
  for (const el of osm.relations) {
    if (el.tags?.type !== 'restriction') continue;
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
  return {
    out: { barriers: barriers.flat(), posts: posts.flat(), fences, crossings: crossings.flat(), signals: [...signals].sort((a, b) => a - b), turnBans: bans,
      vertexXY: (v) => [vertices[2 * v], vertices[2 * v + 1]] },
    stats: { sperren: blockedEdges, poller: barriers.reduce((acc, b) => acc + b[5], 0) + posts.length, zaeune: fences.length, tueren: doors, ampeln: signals.size, querungen: crossings.length, abbiegeverbote: restrictions, durchfahrten: edges.filter((ed) => ed.pass).length },
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
  const CELL = 100, katPos = new Map();
  const key = (x, y) => Math.floor(x / CELL) * 1000000 + Math.floor(y / CELL);
  for (const t of kat) { const k = key(t.x, t.y); (katPos.get(k) ?? katPos.set(k, []).get(k)).push(t); }
  const close = (x, y) => { for (let gx = -1; gx <= 1; gx++) for (let gy = -1; gy <= 1; gy++) for (const t of katPos.get(key(x + gx * CELL, y + gy * CELL)) ?? []) if (Math.hypot(t.x - x, t.y - y) < 4 * S) return true; return false; };
  const osm = trees.splice(0, trees.length).filter((t) => !close(t.x, t.y));
  for (const t of kat) trees.push(t);
  for (const t of osm) trees.push(t);
  return { kataster: kat.length, osmKept: osm.length };
}

// --- Bäume von der Fahrbahn --------------------------------------------------------------
// OSM-Bäume stehen oft auf der geschätzten Fahrbahnbreite (Parkstreifen, Schätzwerte). Ein Stamm auf der Fahrbahn
// wird quer zur Straße an den Bordstein geschoben (auf seiner Seite, die Baumreihe bleibt eine Reihe). Findet sich
// dort kein freier Platz (Haus, Wasser, andere Fahrbahn), entfällt der Baum; ebenso Bäume, die in einem Haus oder
// im Wasser stünden. Kreuzungsflächen zählen als Fahrbahn (junctionsOf, dieselben Werte wie im Spiel).
// Danach wird die Regel geprüft; ein Verstoß bricht den Build ab.
export function keepTreesOffRoads(trees, { edges, vertices, buildings, water, junctions, S }) {
  const trunkOf = (t) => (t.r ? t.r / 100 * S : TREE_TRUNK_M * S), gap = 0.3 * S, trunk = 0.6 * S; // trunk: größter Stamm fürs Raster
  const vx = (k) => vertices[2 * k], vy = (k) => vertices[2 * k + 1];
  const grid = boxGrid(300);
  for (const ed of edges) {
    if (ed.c > TREE_FREE_MAX_CLASS) continue;
    const pts = [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)], half = ed.w / 10 * S / 2;
    for (let i = 0; i < pts.length - 2; i += 2) {
      const sgm = { road: true, half, ax: pts[i], ay: pts[i + 1], bx: pts[i + 2], by: pts[i + 3] };
      grid.add(sgm, Math.min(sgm.ax, sgm.bx) - half - trunk, Math.min(sgm.ay, sgm.by) - half - trunk, Math.max(sgm.ax, sgm.bx) + half + trunk, Math.max(sgm.ay, sgm.by) + half + trunk);
    }
  }
  for (const j of junctions) {
    const sgm = { road: true, half: j.r, ax: j.x, ay: j.y, bx: j.x, by: j.y };
    grid.add(sgm, j.x - j.r - trunk, j.y - j.r - trunk, j.x + j.r + trunk, j.y + j.r + trunk);
  }
  for (const f of [...buildings, ...water]) { const rs = f.rings.map((r) => r.pts); const [x0, y0, x1, y1] = ringBox(rs[0]); grid.add({ rings: rs }, x0, y0, x1, y1); }
  const worstRoad = (x, y, tr) => {
    let worst = null;
    for (const g of grid.at(x, y)) {
      if (!g.road) continue;
      const need = g.half + tr, d = Math.sqrt(segDist2(x, y, g.ax, g.ay, g.bx, g.by));
      if (d < need + 1 && (!worst || need - d > worst.pen)) worst = { g, d, pen: need - d }; // 1 px Reserve gegen Rundung
    }
    return worst;
  };
  const blocked = (x, y) => grid.at(x, y).some((g) => g.rings && g.rings.filter((r) => pointInRing(x, y, r)).length % 2 === 1);
  let moved = 0, dropped = 0;
  const keep = [];
  for (const tree of trees) {
    const tr = trunkOf(tree);
    let { x, y } = tree;
    let hit = worstRoad(x, y, tr);
    if (!hit) { if (blocked(x, y)) dropped++; else keep.push(tree); continue; } // Baum in Haus/Wasser (OSM-Fehler)
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
    if (hit || blocked(x, y)) { dropped++; continue; }
    tree.x = Math.round(x); tree.y = Math.round(y);
    if (worstRoad(tree.x, tree.y, tr)) { dropped++; continue; } // Rundung
    moved++;
    keep.push(tree);
  }
  trees.length = 0; for (const t of keep) trees.push(t);
  for (const t of trees) if (worstRoad(t.x, t.y, trunkOf(t)) || blocked(t.x, t.y)) throw new Error(`Baum bei ${t.x},${t.y} steht auf der Fahrbahn, in einem Haus oder im Wasser`);
  return { moved, dropped };
}

// --- Poller nicht auf die Fahrbahn --------------------------------------------------------------
// Einzelne Poller stehen in Berlin meist auf der Gehwegkante (gegen Falschparker). Wo die (oft geschätzte) Fahrbahn
// sie überdeckt, stünden sie mitten im Verkehr: Sie rücken wie Bäume quer zur Straße an den Bordstein ihrer Seite,
// ohne freien Platz entfallen sie. Maßgeblich sind befahrbare Straßen (Klasse ≤ 8, nicht gesperrt, keine Durchfahrt)
// und Kreuzungsflächen. Pollerreihen auf gesperrten Straßen bleiben stehen (sie SIND die Sperre).
export const POST_R = 0.15; // m, wie im Spiel (map.js)
export function keepPostsOffCarriageway(out, { edges, vertices, junctions, S }) {
  const vx = (k) => vertices[2 * k], vy = (k) => vertices[2 * k + 1];
  const grid = boxGrid(300), r = POST_R * S, gap = 0.4 * S;
  for (const ed of edges) {
    if (ed.c > 8 || ed.blocked || ed.pass) continue;
    const pts = [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)], half = ed.w / 10 * S / 2;
    for (let i = 0; i < pts.length - 2; i += 2) {
      const g = { half, ax: pts[i], ay: pts[i + 1], bx: pts[i + 2], by: pts[i + 3] };
      grid.add(g, Math.min(g.ax, g.bx) - half - r, Math.min(g.ay, g.by) - half - r, Math.max(g.ax, g.bx) + half + r, Math.max(g.ay, g.by) + half + r);
    }
  }
  for (const j of junctions) grid.add({ half: j.r, ax: j.x, ay: j.y, bx: j.x, by: j.y }, j.x - j.r - r, j.y - j.r - r, j.x + j.r + r, j.y + j.r + r);
  const worst = (x, y) => {
    let w = null;
    for (const g of grid.at(x, y)) { const need = g.half + r, d = Math.sqrt(segDist2(x, y, g.ax, g.ay, g.bx, g.by)); if (d < need + 1 && (!w || need - d > w.pen)) w = { g, pen: need - d }; }
    return w;
  };
  const place = (x, y) => { // → [x, y] frei, oder null
    let hit = worst(x, y);
    for (let k = 0; k < 6 && hit; k++) {
      const { g } = hit, dx = g.bx - g.ax, dy = g.by - g.ay, L2 = dx * dx + dy * dy || 1;
      const t = Math.max(0, Math.min(1, ((x - g.ax) * dx + (y - g.ay) * dy) / L2)), px = g.ax + dx * t, py = g.ay + dy * t;
      let nx = x - px, ny = y - py, n = Math.hypot(nx, ny);
      if (n < 1e-6) { const L = Math.sqrt(L2); nx = -dy / L; ny = dx / L; n = 1; }
      x = px + nx / n * (g.half + r + gap); y = py + ny / n * (g.half + r + gap);
      hit = worst(x, y);
    }
    if (hit) return null;
    const rx = Math.round(x), ry = Math.round(y);
    return worst(rx, ry) ? null : [rx, ry];
  };
  let moved = 0, dropped = 0;
  const posts = [];
  for (let i = 0; i < out.posts.length; i += 3) {
    const x = out.posts[i], y = out.posts[i + 1], kind = out.posts[i + 2];
    if (!worst(x, y)) { posts.push(x, y, kind); continue; }
    const p = place(x, y);
    if (p) { posts.push(p[0], p[1], kind); moved++; } else dropped++;
  }
  // Reihen: Poller einzeln prüfen; muss einer weichen, wird die Reihe in Einzelpoller aufgelöst
  const rows = [];
  for (let i = 0; i < out.barriers.length; i += 6) {
    const [x, y, kind, qx, qy, n] = out.barriers.slice(i, i + 6);
    const poles = []; let bad = false;
    for (let k = 0; k < n; k++) { const o = (k - (n - 1) / 2) * 1.8 * S, px = Math.round(x + qx / 1000 * o), py = Math.round(y + qy / 1000 * o); poles.push([px, py]); if (worst(px, py)) bad = true; }
    if (!bad) { rows.push(x, y, kind, qx, qy, n); continue; }
    for (const [px, py] of poles) {
      if (!worst(px, py)) { posts.push(px, py, kind); continue; }
      const p = place(px, py);
      if (p) { posts.push(p[0], p[1], kind); moved++; } else dropped++;
    }
  }
  out.posts = posts; out.barriers = rows;
  for (let i = 0; i < posts.length; i += 3) if (worst(posts[i], posts[i + 1])) throw new Error(`Poller bei ${posts[i]},${posts[i + 1]} steht auf der Fahrbahn`);
  return { moved, dropped };
}

// --- Missionsorte ---------------------------------------------------------------------
function placeMission({ places, toPx, edges, vertices, buildings, S, insideBorder }) {
  const vx = (k) => vertices[2 * k], vy = (k) => vertices[2 * k + 1];
  const edgePts = (ed) => [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)];
  const boxes = buildings.map((b) => ringBox(b.rings[0].pts));
  const inBuilding = (x, y) => buildings.some((b, i) => {
    const [x0, y0, x1, y1] = boxes[i];
    if (x < x0 || x > x1 || y < y0 || y > y1) return false;
    return pointInRing(x, y, b.rings[0].pts) && !b.rings.slice(1).some((r) => pointInRing(x, y, r.pts));
  });
  const snap = (x, y, filter) => {
    let best = null;
    for (const ed of edges) {
      if (!ed.in || !filter(ed)) continue;
      if (Math.abs(vx(ed.a) - x) > 20000 && Math.abs(vx(ed.b) - x) > 20000) continue; // grob vorfiltern (Kanten sind kurz)
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
  const at = (sn, along, out) => ({ x: Math.round(sn.px + sn.ux * along + sn.nx * out), y: Math.round(sn.py + sn.uy * along + sn.ny * out) });
  const angleOn = (sn) => Math.atan2(sn.uy * sn.side, sn.ux * sn.side); // Rechtsverkehr: rechte Seite fährt a→b

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
  buildings.forEach((b, bi) => {
    const [x0, y0, x1, y1] = boxes[bi];
    if (giver.x < x0 - 5000 || giver.x > x1 + 5000 || giver.y < y0 - 5000 || giver.y > y1 + 5000) return;
    const r = b.rings[0].pts;
    for (let i = 0; i < r.length; i += 2) {
      const j = (i + 2) % r.length;
      const d = segDist2(giver.x, giver.y, r[i], r[i + 1], r[j], r[j + 1]);
      if (!best || d < best.d) best = { d, b };
    }
  });
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
    const D = new Map([[from, 0]]), done = new Set(), q = new Heap();
    q.push(0, from);
    while (q.size) {
      const [d, u] = q.pop();
      if (done.has(u)) continue; done.add(u);
      if (u === to) return d;
      for (const [v, l] of graph.get(u) ?? []) if (!D.has(v) || d + l < D.get(v)) { D.set(v, d + l); q.push(d + l, v); }
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

// --- Kommandozeile -------------------------------------------------------------------------
export async function writeCity(out, dir) {
  await rm(dir, { recursive: true, force: true });
  await mkdir(new URL('tiles/', dir), { recursive: true });
  let bytes = 0;
  const put = async (rel, obj) => { const s = JSON.stringify(obj); bytes += s.length; await writeFile(new URL(rel, dir), s); return s.length; };
  const sizes = [];
  for (const [k, t] of out.tiles) sizes.push(await put(`tiles/${k}.json`, t));
  await put('overview.json', out.overview);
  await put('index.json', out.index);
  return { bytes, tiles: sizes.length, maxTile: Math.max(...sizes) };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const arg = (k, d) => { const i = process.argv.indexOf(k); return i > 0 ? process.argv[i + 1] : d; };
  const root = new URL('../../', import.meta.url);
  const dir = new URL('web/data/berlin/', root);
  const t0 = Date.now();
  const lor = JSON.parse(await readFile(new URL('data/raw/lor.json', root)));
  console.log('OSM-Auszug lesen …');
  const osm = storeFromPbf(await readFile(new URL('data/raw/berlin.osm.pbf', root)));
  console.log(`  ${osm.n} Knoten, ${osm.tagged.length} getaggt, ${osm.ways.size} Wege, ${osm.relations.length} Relationen, Stand ${osm.timestamp} (${((Date.now() - t0) / 1000).toFixed(0)} s)`);
  const places = JSON.parse(await readFile(new URL('data/places.json', root)));
  let kataster = [];
  try { kataster = JSON.parse(await readFile(new URL('data/raw/baeume.json', root))); } catch { console.warn('data/raw/baeume.json fehlt – nur OSM-Bäume'); }
  const out = buildCity(lor, osm, places, { scale: Number(arg('--scale', 10)), kataster, log: console.log });
  const r = await writeCity(out, dir);
  const m = out.index.meta;
  console.log(`${fileURLToPath(dir)}: ${(r.bytes / 1e6).toFixed(1)} MB in ${r.tiles} Kacheln (größte ${(r.maxTile / 1e6).toFixed(2)} MB), ${m.width}×${m.height} px, ` +
    `${JSON.stringify(m.counts)}, Bäume ${JSON.stringify(m.trees)}, ${JSON.stringify(m.access)}, ${((Date.now() - t0) / 1000).toFixed(0)} s`);
  console.log('Abdeckung je Bezirk:');
  const keys = Object.keys(Object.values(m.coverage)[0]);
  console.log('  ' + ['Bezirk'.padEnd(28), ...keys.map((k) => k.slice(0, 9).padStart(9))].join(' '));
  for (const [b, row] of Object.entries(m.coverage)) console.log('  ' + [b.padEnd(28), ...keys.map((k) => String(Math.round(row[k])).padStart(9))].join(' '));
}
