import test from 'node:test';
import assert from 'node:assert/strict';
import { buildCity, GATE_M } from '../tools/osm/build.mjs';
import { geoToPx } from './helpers/city.js';
import { makeProjection } from '../tools/osm/geo.mjs';
import { decodeCity, surfaceAt, locationName, districtAt, T } from '../web/src/map.js';
import { segDist2, undelta, delta } from '../web/src/geom.js';
import { BUILDING_KIND, ROOF_SHAPE, WALL_MAT, BUILDING_SUB, unpackLook } from '../web/src/citycodes.js';

// Kleine künstliche Stadt: zwei aneinandergrenzende Bezirke, eine Straße mit Kanal und Brücke,
// eine Einbahnstraße, ein Wohnhaus, eine Lagerhalle, ein Baum.
function fixture() {
  const sq = (w, e) => ({ type: 'MultiPolygon', coordinates: [[[[w, 52.49], [e, 52.49], [e, 52.494], [w, 52.494], [w, 52.49]]]] });
  const lor = { features: [
    { properties: { pgr_id: '0210', pgr_name: 'Kreuzberg', bez: '02 - Friedrichshain-Kreuzberg' }, geometry: sq(13.42, 13.425) },
    { properties: { pgr_id: '0810', pgr_name: 'Nord-Neukölln', bez: '08 - Neukölln' }, geometry: sq(13.425, 13.43) },
  ] };
  const N = (id, lat, lon, tags) => ({ type: 'node', id, lat, lon, ...(tags ? { tags } : {}) });
  const Wy = (id, nodes, tags) => ({ type: 'way', id, nodes, tags });
  const elements = [
    N(1, 52.492, 13.4205), N(2, 52.492, 13.4225), N(3, 52.492, 13.4275), N(4, 52.492, 13.4295),
    N(5, 52.4905, 13.4225), N(7, 52.4908, 13.4225), N(8, 52.4916, 13.4225), N(6, 52.4935, 13.4225), N(9, 52.4935, 13.4275),
    N(20, 52.491, 13.4202), N(21, 52.491, 13.4298), N(22, 52.4914, 13.4298), N(23, 52.4914, 13.4202),
    N(30, 52.4925, 13.423), N(31, 52.4925, 13.424), N(32, 52.493, 13.424), N(33, 52.493, 13.423),
    N(40, 52.4924, 13.428), N(41, 52.4924, 13.429), N(42, 52.4929, 13.429), N(43, 52.4929, 13.428),
    N(50, 52.4935, 13.421, { natural: 'tree' }), N(51, 52.4932, 13.4265, { place: 'quarter', name: 'Testkiez' }),
    // Bäume auf der Teststraße: mitten auf der Fahrbahn (Nordhälfte), knapp am Rand (Südhälfte), und einer vor dem
    // Wohnhaus, dessen Bordsteinplatz im Haus läge (Haus reicht in diesem Test bis an die Fahrbahn heran).
    N(52, 52.49201, 13.4250, { natural: 'tree' }), N(53, 52.49197, 13.4260, { natural: 'tree' }),
    N(54, 52.49203, 13.4212, { natural: 'tree' }),
    N(60, 52.49204, 13.4208), N(61, 52.49204, 13.4216), N(62, 52.4924, 13.4216), N(63, 52.4924, 13.4208),
    Wy(102, [60, 61, 62, 63, 60], { building: 'garage' }),
    // POIs und Hausnummern
    N(70, 52.4922, 13.4262, { shop: 'bakery', name: 'Testbäcker' }),
    N(71, 52.4921, 13.4240, { amenity: 'bar', name: 'Kiezkneipe' }),
    N(72, 52.4921, 13.4250, { amenity: 'bench' }),                               // kein POI
    N(73, 52.4919, 13.4226, { railway: 'station', station: 'subway', name: 'U Teststraße' }),
    N(74, 52.4921, 13.4228, { public_transport: 'station', subway: 'yes', name: 'Teststraße' }), // derselbe Bahnhof
    N(75, 52.4919, 13.4270, { highway: 'bus_stop', name: 'Einbahn' }),
    N(76, 52.49252, 13.4235, { 'addr:street': 'Teststraße', 'addr:housenumber': '12' }), // Eingang = Gebäudenummer
    N(77, 52.4918, 13.4250, { 'addr:street': 'Teststraße', 'addr:housenumber': '7a' }),
    // Tordurchfahrt durch ein Haus
    N(80, 52.49228, 13.4254), N(81, 52.4934, 13.4254),
    N(82, 52.4925, 13.4250), N(83, 52.4925, 13.4258), N(84, 52.4931, 13.4258), N(85, 52.4931, 13.4250),
    Wy(103, [82, 83, 84, 85, 82], { building: 'apartments', 'roof:shape': 'gabled', 'roof:colour': '#a04a3a', 'building:material': 'brick' }),
    Wy(110, [80, 81], { highway: 'service', tunnel: 'building_passage' }),
    // Poller mitten auf dem Pollerweg (Modalfilter)
    N(90, 52.4935, 13.4285, { barrier: 'bollard' }), N(92, 52.4935, 13.4295),
    Wy(111, [9, 90, 92], { highway: 'residential', name: 'Pollerweg' }),
    // Zaun, den ein Fußweg ohne Tor-Knoten kreuzt (bekommt eine Öffnung), und eine Hecke ohne Querung (bleibt zu)
    N(120, 52.4938, 13.4240), N(121, 52.4938, 13.4270), Wy(112, [120, 121], { barrier: 'fence' }),
    N(122, 52.49365, 13.4255), N(123, 52.49395, 13.4255), Wy(113, [122, 123], { highway: 'footway' }),
    N(124, 52.4940, 13.4228), N(125, 52.4940, 13.4236), Wy(114, [124, 125], { barrier: 'hedge' }),
    // Pollerreihe als Linie (OSM-Weg barrier=bollard): wird zu einzelnen Pollern, die man umfahren kann
    N(126, 52.4943, 13.4240), N(127, 52.4943, 13.4250), Wy(115, [126, 127], { barrier: 'bollard' }),
    // Ampel an der Kreuzung Teststraße/Querstraße (Signal-Knoten auf der Zufahrt, nicht auf dem Kreuzungsknoten)
    N(93, 52.49212, 13.4225, { highway: 'traffic_signals' }),
    // Abbiegeverbot: von der Teststraße nicht links in die Einbahn
    Wy(10, [1, 2, 3, 4], { highway: 'residential', name: 'Teststraße', width: '9' }),
    Wy(11, [5, 7], { highway: 'residential', name: 'Querstraße' }),
    Wy(12, [7, 8], { highway: 'residential', name: 'Querstraße', bridge: 'yes' }),
    Wy(13, [8, 2, 6], { highway: 'residential', name: 'Querstraße' }),
    Wy(14, [3, 9], { highway: 'residential', name: 'Einbahn', oneway: 'yes' }),
    Wy(15, [20, 21, 22, 23, 20], { natural: 'water', water: 'canal' }),
    Wy(100, [30, 31, 32, 33, 30], { building: 'apartments', 'building:levels': '4', 'addr:street': 'Teststraße', 'addr:housenumber': '12' }),
    Wy(101, [40, 41, 42, 43, 40], { building: 'warehouse' }),
  ];
  elements.push({ type: 'relation', id: 500, tags: { type: 'restriction', restriction: 'no_left_turn' },
    members: [{ type: 'way', ref: 10, role: 'from' }, { type: 'node', ref: 3, role: 'via' }, { type: 'way', ref: 14, role: 'to' }] });
  // Baumkataster: Winterlinde genau dort, wo auch der OSM-Parkbaum steht (Knoten 50)
  const kataster = [[13.421, 52.4935, 'Tilia', 'Winter-Linde', 8, 120, 15, 1]];
  const places = { giver: { lat: 52.49225, lon: 13.4235, name: 'Späti' }, pickup: { osmWay: 101, name: 'Lager' }, dropoff: { name: 'Parkplatz' } };
  return { lor, osm: { elements }, places, kataster };
}

const f = fixture();
const built = buildCity(f.lor, f.osm, f.places, { kataster: f.kataster });
const ser = (b) => JSON.stringify({ index: b.index, overview: b.overview, tiles: [...b.tiles] });
const reparse = (b) => { const j = JSON.parse(ser(b)); return { index: j.index, overview: j.overview, tiles: new Map(j.tiles) }; };
const json = { meta: built.index.meta, tiles: built.tiles };
const city = decodeCity(reparse(built));
const L = (layer) => city.list(layer);
const nodes = () => [...city.nodes.values()];

test('Build ist deterministisch', () => {
  const g = fixture();
  assert.equal(ser(buildCity(g.lor, g.osm, g.places, { kataster: g.kataster })), ser(built));
});

test('Projektion: Abstände stimmen auf < 0,5 % mit der Kugel überein', () => {
  const proj = makeProjection(52.49, 13.425);
  const [ax, ay] = proj(52.492, 13.4205), [bx, by] = proj(52.492, 13.4295);
  const hav = 6371008.8 * 2 * Math.asin(Math.sqrt(Math.cos(52.492 * Math.PI / 180) ** 2 * Math.sin(0.009 * Math.PI / 360) ** 2));
  assert.ok(Math.abs(Math.hypot(bx - ax, by - ay) / hav - 1) < 0.005);
  const [, ny] = proj(52.493, 13.425), [, sy] = proj(52.492, 13.425);
  assert.ok(ny > sy, 'Norden hat größeres y (im Spiel: oben)');
});

test('Grenze: gemeinsame Kante der Bezirke verschwindet, Bezirke bleiben benannt', () => {
  assert.equal(city.border.length, 1, 'ein Ring, keine Naht quer durchs Gebiet');
  assert.ok(city.border[0].length <= 14, '4 Ecken (+ Nahtpunkte auf der Außenkante)');
  assert.deepEqual(city.bezirke.map((b) => b.name).sort(), ['Friedrichshain-Kreuzberg', 'Neukölln']);
  const p = city.places.giver, q = city.places.pickup;
  assert.equal(districtAt(city, p.x, p.y), 'Kreuzberg');
  assert.equal(districtAt(city, q.x, q.y), 'Nord-Neukölln');
});

test('Straßengraph: Knoten an Kreuzungen, Einbahnstraße, Brücke, Namen', () => {
  const byName = (n) => L('edge').filter((e) => e.name === n);
  assert.equal(byName('Teststraße').length, 3, 'an den Einmündungen geteilt');
  const cross = nodes().find((n) => n.edges.length === 4);
  assert.ok(cross, 'Kreuzung Teststraße/Querstraße');
  assert.deepEqual(byName('Einbahn').map((e) => e.oneway), [1]);
  assert.equal(byName('Querstraße').filter((e) => e.bridge).length, 1);
  assert.ok(Math.abs(byName('Teststraße')[1].w - 90) < 1, 'width=9 (Bordstein zu Bordstein)');
  assert.ok(Math.abs(byName('Querstraße')[0].w - 55) < 1, 'ohne width/lanes/Parkstreifen: Wohnstraße 5,5 m Fahrbahn');
  assert.equal(locationName(city, cross.x, cross.y).split(' / ').sort().join(), 'Querstraße,Teststraße');
});

test('Ufer ist Wand, an der Brücke aber offen; Brücke hat Geländer', () => {
  const bridge = L('edge').find((e) => e.bridge);
  const mx = (bridge.pts[0] + bridge.pts[2]) / 2, my = (bridge.pts[1] + bridge.pts[3]) / 2;
  let quay = 0, rail = 0;
  for (const w of L('wall').filter((f) => f.kind === 'wall').map((f) => f.pts)) for (let i = 0; i < w.length - 2; i += 2) {
    const d = Math.sqrt(segDist2(mx, my, w[i], w[i + 1], w[i + 2], w[i + 3]));
    if (Math.abs(w[i + 1] - w[i + 3]) < 2) { quay++; assert.ok(Math.abs(w[i] - mx) > 40 || Math.abs(w[i + 2] - mx) > 40 || d > 40, 'Kaimauer quer über der Brücke'); }
    else if (d < bridge.w) rail++;
  }
  assert.ok(quay >= 4, 'Kaimauern beidseits der Brücke');
  assert.ok(rail >= 2, 'Geländer links und rechts');
  assert.equal(surfaceAt(city, mx, my), T.ROAD);
  const w = L('water')[0].rings[0];
  assert.equal(surfaceAt(city, w[0] + 30, (w[1] + w[5]) / 2), T.WATER);
});

test('Gebäude: Höhe aus Geschossen, Späti und Lagerhalle markiert, Kiez übernommen', () => {
  const kinds = L('building').map((b) => b.kind).sort();
  assert.deepEqual(kinds, [BUILDING_KIND.house, BUILDING_KIND.spaeti, BUILDING_KIND.small, BUILDING_KIND.warehouse].sort());
  const house = L('building').find((b) => b.kind === BUILDING_KIND.spaeti);
  assert.equal(house.meters, 13.8);
  assert.deepEqual(city.kieze.map((k) => k.n), ['Testkiez']);
  for (const t of built.tiles.values()) for (const b of t.buildings) assert.ok(undelta(b[3][0]).every(Number.isInteger), 'Ganzzahl-Koordinaten');
});

test('Gebäude: Aussehen aus OSM (Dachform, Farbe, Material, Typ, Bezirk) kommt im Spiel an', () => {
  const withLook = L('building').filter((b) => b.roofRgb >= 0);
  assert.equal(withLook.length, 1, 'genau das Haus mit roof:colour');
  const b = withLook[0], lk = unpackLook(b.look);
  assert.equal(b.roofRgb, 0xa04a3a);
  assert.equal(b.wallRgb, -1, 'keine Fassadenfarbe angegeben');
  assert.equal(lk.shape, ROOF_SHAPE.gabled);
  assert.equal(lk.wmat, WALL_MAT.brick);
  assert.equal(lk.sub, BUILDING_SUB.apartments);
  assert.ok(lk.bez > 0, 'Bezirk eingetragen');
  // Häuser ohne OSM-Farben kommen ohne die Farbfelder (Kachelgröße)
  for (const t of built.tiles.values()) for (const row of t.buildings) assert.ok(row.length <= 9 && (row.length < 8 || row[row.length - 1] !== 0));
});

test('Missionsorte eingerastet, Zeitlimit aus der Route', () => {
  const p = city.places;
  assert.equal(surfaceAt(city, p.giver.x, p.giver.y), T.SIDEWALK);
  assert.equal(surfaceAt(city, p.dropoff.x, p.dropoff.y), T.ROAD);
  assert.equal(surfaceAt(city, p.pickup.x, p.pickup.y), T.ROAD);
  assert.ok(city.timeLimit >= 40 && city.timeLimit < 200, `${city.timeLimit} s`);
  assert.ok(city.crates.length >= 1);
});

test('Bäume auf der Fahrbahn rücken an den Bordstein ihrer Seite oder entfallen', async () => {
  const { treeOnRoad, inBuilding } = await import('../web/src/map.js');
  assert.deepEqual([json.meta.trees.moved, json.meta.trees.dropped], [2, 1]);
  assert.equal(L('tree').length, 3, 'Parkbaum + zwei verschobene Straßenbäume');
  assert.equal(city.droppedTrees, 0);
  const street = L('edge').find((e) => e.name === 'Teststraße' && e.pts[0] < 40000 && e.len > 2000);
  const y0 = street.pts[1], half = street.w / 2, trunk = 5;
  for (const t of L('tree')) {
    assert.equal(treeOnRoad(city, t), null, `Stamm auf der Fahrbahn bei ${t.x},${t.y}`);
    assert.equal(inBuilding(city, t.x, t.y), null);
  }
  const moved = L('tree').filter((t) => Math.abs(t.y - y0) < half + 20);
  assert.equal(moved.length, 2);
  // knapp außerhalb der Fahrbahn: Stamm am Bordstein, Krone ragt über die Straße
  for (const t of moved) {
    assert.ok(Math.abs(Math.abs(t.y - y0) - (half + trunk + 3)) <= 1.5, `Abstand ${Math.abs(t.y - y0)}`);
    assert.ok(Math.abs(t.y - y0) < half + t.size, 'Krone überragt die Fahrbahn');
  }
  // jeder bleibt auf seiner Straßenseite (Norden = kleineres y)
  const north = moved.find((t) => t.y < y0), south = moved.find((t) => t.y > y0);
  assert.ok(north && south, 'je ein Baum nördlich und südlich');
});

test('Schutz im Spiel: ein Baum auf der Fahrbahn in einer fremden Karte wird beim Laden verworfen', () => {
  const j = reparse(built);
  const e = L('edge').find((x) => x.name === 'Teststraße');
  const mid = [Math.round((e.pts[0] + e.pts[2]) / 2), Math.round((e.pts[1] + e.pts[3]) / 2)];
  // zusätzlicher Baum mitten auf der Teststraße, in der Kachel, in der er steht
  const key = `${Math.floor(mid[0] / j.index.meta.tile)}_${Math.floor(mid[1] / j.index.meta.tile)}`;
  const t = j.tiles.get(key).trees;
  const xy = undelta(t.xy); xy.push(...mid);
  t.xy = delta(xy); t.g.push(0); t.c.push(0); t.r.push(0);
  const c2 = decodeCity(j);
  assert.equal(c2.droppedTrees, 1);
  assert.equal(c2.list('tree').length, L('tree').length);
});

test('POIs: Kategorien, Bahnhof nur einmal (ohne „U “-Präfix), Parkbank ignoriert', () => {
  const byName = Object.fromEntries(L('poi').map((q) => [q.name, q]));
  assert.equal(byName.Testbäcker.cat, 'shop'); assert.equal(byName.Testbäcker.kind, 'bakery');
  assert.equal(byName.Kiezkneipe.cat, 'drink');
  assert.equal(L('poi').filter((q) => q.cat === 'ubahn').length, 1);
  assert.equal(byName.Teststraße.cat, 'ubahn');
  assert.equal(byName.Einbahn.cat, 'bus');
  assert.equal(L('poi').length, 4);
});

test('Hausnummern: Gebäude und Eingang mit derselben Nummer zählen einmal, Nummer im Straßennamen', () => {
  const nrs = L('address').map((a) => `${a.street} ${a.nr}`).sort();
  assert.deepEqual(nrs, ['Teststraße 12', 'Teststraße 7a']);
  const a7 = L('address').find((a) => a.nr === '7a');
  const e = L('edge').find((x) => x.name === 'Teststraße' && x.len > 2000);
  assert.equal(locationName(city, a7.x, e.pts[1]), 'Teststraße 7a');
});

test('Tordurchfahrt: Kante bleibt erhalten, die Hauswand ist im Durchfahrtskorridor offen', () => {
  const pass = L('edge').find((e) => e.passage);
  assert.ok(pass, 'Durchfahrt als Kante vorhanden');
  const house = L('building').find((b) => b.walls);
  assert.ok(house, 'Haus hat eigene Wandzüge');
  const [ax, ay, bx, by] = [pass.pts[0], pass.pts[1], pass.pts[pass.pts.length - 2], pass.pts[pass.pts.length - 1]];
  for (const w of house.walls) for (let i = 0; i < w.length - 2; i += 2) {
    const mx = (w[i] + w[i + 2]) / 2, my = (w[i + 1] + w[i + 3]) / 2;
    assert.ok(Math.sqrt(segDist2(mx, my, ax, ay, bx, by)) > 10, 'Wandstück liegt in der Durchfahrt');
  }
  assert.ok(house.walls.length >= 2, 'Nord- und Südwand sind aufgeschnitten');
});

test('Poller auf der Straße sperren sie für Autos (Reihe quer über die Fahrbahn, Fußgänger kommen durch)', async () => {
  const { buildLaneGraph } = await import('../web/src/roadgraph.js');
  const pw = L('edge').filter((e) => e.name === 'Pollerweg');
  assert.ok(pw.length >= 1 && pw.every((e) => e.blocked));
  assert.ok(![...buildLaneGraph(city).lanes].some((l) => l.edge.name === 'Pollerweg'), 'kein KI-Verkehr durch den Modalfilter');
  const [px, py] = geoToPx(built.index.meta, 52.4935, 13.4285);
  const row = L('barrier').filter((b) => Math.abs(b.x - px) < 30 && Math.abs(b.y - py) < 100); // die Reihe am Pollerweg
  assert.ok(row.length >= 3, `${row.length} Poller`);
  const xs = row.map((b) => b.y).sort((a, b) => a - b);
  for (let i = 1; i < xs.length; i++) assert.ok(xs[i] - xs[i - 1] >= 17, 'Abstand ≥ 1,7 m: Fußgänger passen durch, Autos (2 m) nicht');
});

test('Ampel wird der Kreuzung zugeordnet; Abbiegeverbot filtert die Folgespur', async () => {
  const { buildLaneGraph, turnAngle } = await import('../web/src/roadgraph.js');
  const cross = nodes().find((n) => n.edges.length === 4 && n.edges.some((k) => city.edges.get(k).name === 'Querstraße'));
  assert.ok(city.signals.has(cross.id), 'Signal liegt 9 m vor der Kreuzung und gehört zu ihr');
  assert.equal(city.signals.size, 1);
  const via = nodes().find((n) => n.edges.some((k) => city.edges.get(k).name === 'Einbahn') && n.edges.some((k) => city.edges.get(k).name === 'Teststraße'));
  const west = L('edge').find((e) => e.name === 'Teststraße' && e.b === via.id);
  const einbahn = L('edge').find((e) => e.name === 'Einbahn');
  assert.ok(city.turnBans.has(`${west.id}>${via.id}>${einbahn.id}`));
  const lane = [...buildLaneGraph(city).lanes].find((l) => l.edge === west && l.to === via.id);
  assert.ok(lane.next.length && !lane.next.some((m) => m.edge === einbahn), 'nicht links in die Einbahn');
  void turnAngle;
});

test('Baumkataster ersetzt den OSM-Baum an derselben Stelle (Gattung, Krone, Stamm)', () => {
  assert.equal(json.meta.trees.kataster, 1);
  const linde = L('tree').find((t) => t.genus === 'Tilia');
  assert.ok(linde, 'Winterlinde aus dem Kataster');
  assert.equal(linde.size, 40, 'Krone 8 m → Radius 40 px');
  assert.ok(Math.abs(linde.r - 1.9) < 0.1, 'Stamm aus 120 cm Umfang');
  assert.ok(!L('tree').some((t) => t !== linde && Math.hypot(t.x - linde.x, t.y - linde.y) < 40), 'OSM-Doppel entfernt');
});

test('Gebäude-Regeln: Brückenbauwerke und schwebende Teile sind keine Häuser, Bauteile nur ohne Umriss', async () => {
  const { buildingTreatment, mergeParts } = await import('../tools/osm/build.mjs');
  assert.equal(buildingTreatment({ building: 'yes' }), 'building');
  assert.equal(buildingTreatment({ highway: 'residential' }), null);
  assert.equal(buildingTreatment({ building: 'bridge', 'bridge:support': 'pier' }), 'skip', 'Brückenpfeiler');
  assert.equal(buildingTreatment({ building: 'bridge', min_height: '9.5', note: 'Kreuzgang' }), 'skip');
  assert.equal(buildingTreatment({ building: 'roof' }), 'skip');
  assert.equal(buildingTreatment({ building: 'watchtower', min_height: '15' }), 'upper', 'Turmspitze schwebt: hebt das Haus darunter');
  assert.equal(buildingTreatment({ building: 'yes', 'building:min_level': '2' }), 'upper', 'Obergeschosse: heben das Haus darunter (ohne Haus darunter: man fährt durch)');
  assert.equal(buildingTreatment({ 'building:part': 'retail', min_height: '3.14', height: '22' }), 'upper', 'Obergeschosse über der Arkade');
  assert.equal(buildingTreatment({ building: 'yes', layer: '-1' }), 'skip');
  assert.equal(buildingTreatment({ 'building:part': 'yes', height: '34' }), 'part');
  assert.equal(buildingTreatment({ building: 'yes', 'building:part': 'yes' }), 'part');
  const sq = (id, x, y, s, h) => ({ id, h, k: 0, measured: true, rings: [{ outer: true, pts: [x, y, x + s, y, x + s, y + s, x, y + s] }] });
  // Haus mit Umriss und Teil darin: Teil bleibt unbeachtet
  const buildings = [sq(1, 0, 0, 100, 160)];
  // Brückenpfeiler (groß, 5 m) mit Turm (klein, 34 m) darauf, daneben ein einzelnes Teil
  const parts = [sq(2, 10, 10, 20, 170), sq(10, 500, 500, 300, 50), sq(11, 600, 600, 60, 340), sq(12, 610, 610, 30, 150), sq(20, 2000, 0, 50, 120)];
  const r = mergeParts(buildings, parts, [], (p) => p.id === 10); // Teil 10 steht im Wasser (Pfeiler)
  const byId = new Map(buildings.map((b) => [b.id, b]));
  assert.ok(!byId.has(2), 'Teil im Umriss, nicht höher: kein eigenes Gebäude');
  assert.ok(byId.has(11) && byId.get(11).h === 340, 'auf dem Pfeiler steht der Turm (höchstes Teil, eigener Umriss)');
  assert.ok(!byId.has(10) && !byId.has(12), 'kein Pfeiler im Wasser, keine Doppelung (12 liegt im gleich hohen Turm)');
  assert.ok(byId.has(20), 'einzelnes Teil wird Gebäude');
  assert.equal(r.added, 2);
});

test('Gebäude aus Bauteilen: Sockel mit Hochhaus, Turm auf dem Block, Obergeschosse über der Arkade, Überbauung', async () => {
  const { mergeParts, buildingType } = await import('../tools/osm/build.mjs');
  const sq = (id, x, y, s, h) => ({ id, h, k: 0, measured: true, rings: [{ outer: true, pts: [x, y, x + s, y, x + s, y + s, x, y + s] }] });
  // Einkaufssockel 3,1 m mit 25-m-Hochhaus darauf: beide Gebäude (vorher blieb nur das Hochhaus, der Sockel fehlte)
  const bs = [sq(1, 5000, 0, 200, 120)];               // Block mit Umriss, 12 m
  const parts = [sq(10, 0, 0, 400, 31), sq(11, 50, 50, 100, 250), sq(12, 60, 60, 40, 60), // Sockel, Hochhaus, Vordach
    sq(20, 5050, 50, 60, 400)];                            // Turm im Umriss des Blocks, 40 m
  const uppers = [sq(30, 200, 200, 150, 220),              // Obergeschosse 3,1–22 m über dem Sockel
    sq(31, 9000, 0, 50, 180)];                             // Überbauung einer Straße: nichts darunter
  const r = mergeParts(bs, parts, uppers);
  const byId = new Map(bs.map((b) => [b.id, b]));
  assert.ok(byId.has(10) && byId.has(11), 'Sockel und Hochhaus');
  assert.ok(!byId.has(12), 'niedriges Teil im Sockel: kein eigenes Gebäude');
  assert.equal(byId.get(10).h, 220, 'Obergeschosse heben den Sockel darunter (vorher: flache 3-m-Platte)');
  assert.equal(byId.get(11).h, 250, 'Hochhaus bleibt');
  assert.ok(byId.has(20) && byId.get(1).h === 120, 'Turm auf dem Block als eigenes Gebäude, Block bleibt 12 m');
  assert.ok(!byId.has(31) && !byId.has(30), 'Obergeschosse werden kein eigenes Gebäude; ohne etwas darunter entfallen sie');
  assert.deepEqual([r.towers, r.raised], [1, 1]);
  assert.equal(buildingType({ 'building:part': 'retail' }), 'retail', 'Art eines Bauteils');
  assert.equal(buildingType({ building: 'office', 'building:part': 'yes' }), 'office');
});

test('Echte Karte: Oberbaumbrücke mit Brückendeck, Türmen und ohne Häuser auf der Fahrbahn; Krankenhäuser in ganz Berlin', async () => {
  const { openRealCity, realIndex } = await import('./helpers/city.js');
  const { pointInRings } = await import('../web/src/geom.js');
  const { AREA_KIND } = await import('../web/src/citycodes.js');
  const idx = realIndex();
  assert.ok(idx.hospitals.length >= 40, `${idx.hospitals.length} Krankenhäuser`);
  const c = openRealCity();
  const { bezirkAt } = await import('../web/src/map.js');
  const bz = new Set(idx.hospitals.map(([x, y]) => bezirkAt(c, x, y)).filter(Boolean));
  assert.equal(bz.size, 12, `Krankenhäuser in ${bz.size} von 12 Bezirken`);
  const q = c.list('poi').find((p) => p.name === 'Oberbaumbrücke' && p.cat === 'culture') ?? null;
  const [px, py] = q ? [q.x, q.y] : [246018, 196469];
  c.loadArea(px - 800, py - 800, px + 800, py + 800, { pin: true });
  const near = (f) => Math.hypot(f.cx - px, f.cy - py) < 600;
  const towers = c.list('building').filter((b) => near(b) && b.meters >= 30);
  assert.ok(towers.length >= 2, `Türme der Oberbaumbrücke (${towers.length})`);
  const edges = c.list('edge').filter((e) => e.name === 'Oberbaumbrücke' && e.cls <= 8);
  assert.ok(edges.length >= 2);
  for (const b of c.list('building')) for (const e of edges) for (let i = 0; i < e.pts.length; i += 2) assert.ok(!pointInRings(e.pts[i], e.pts[i + 1], b.rings), `Haus ${b.id} steht auf der Fahrbahn der Oberbaumbrücke`);
  const deck = c.list('area').filter((a) => a.kind === AREA_KIND.bridge && pointInRings(px, py, a.rings));
  assert.equal(deck.length, 1, 'Brückendeck unter dem POI');
  // ein Punkt auf dem Deck, der keine Fahrbahn ist, gilt nicht als Wasser
  let checked = 0;
  for (let dx = -300; dx <= 300 && !checked; dx += 20) for (let dy = -300; dy <= 300 && !checked; dy += 20) {
    const x = px + dx, y = py + dy;
    if (!pointInRings(x, y, deck[0].rings) || c.list('water').every((wa) => !pointInRings(x, y, wa.rings))) continue;
    if (surfaceAt(c, x, y) === T.ROAD) continue;
    assert.notEqual(surfaceAt(c, x, y), T.WATER, 'Deck über dem Wasser ist kein Wasser'); checked++;
  }
  assert.ok(checked, 'Deckpunkt über Wasser gefunden');
});

test('Zäune: Öffnung, wo ein Weg sie kreuzt (auch ohne Tor-Knoten), breit genug für ein Auto; sonst geschlossen', () => {
  const S = city.scale, meta = built.index.meta;
  const [cx, cy] = geoToPx(meta, 52.4938, 13.4255), [hx, hy] = geoToPx(meta, 52.4940, 13.4232);
  const fence = L('wall').filter((w) => w.sub === 'fence');
  const pieces = fence.filter((w) => Math.abs(w.pts[1] - cy) < 20);
  assert.equal(pieces.length, 2, 'Zaun in zwei Stücke geteilt');
  const xs = pieces.flatMap((w) => [w.pts[0], w.pts[w.pts.length - 2]]).sort((a, b) => Math.abs(a - cx) - Math.abs(b - cx));
  const gap = Math.abs(xs[0] - xs[1]);
  assert.ok(gap >= 3.5 * S && gap <= 6 * S, `Lücke ${gap / S} m: ein Auto (2 m) passt durch`);
  assert.ok(GATE_M >= 3.5, 'Torbreite reicht für ein Auto');
  assert.ok(xs[0] < cx !== xs[1] < cx, 'Lücke liegt auf dem Weg');
  const hedge = L('wall').filter((w) => w.sub === 'fence' && Math.abs(w.pts[1] - hy) < 20 && Math.abs((w.pts[0] + w.pts[w.pts.length - 2]) / 2 - hx) < 60);
  assert.equal(hedge.length, 1, 'Hecke ohne Querung bleibt ganz');
  assert.ok(Math.abs(hedge[0].pts[0] - hedge[0].pts[hedge[0].pts.length - 2]) > 7 * S, 'in voller Länge');
  // gezeichnet wird der Zaun genau so (mit Lücke)
  assert.equal(L('fence').filter((f) => Math.abs(f.pts[1] - cy) < 20).length, 2);
});

test('Pollerreihe als Linie wird zu einzelnen Pollern (umfahrbar), nicht zur Wand', () => {
  const S = city.scale, meta = built.index.meta;
  const [ax, ay] = geoToPx(meta, 52.4943, 13.4240), [bx] = geoToPx(meta, 52.4943, 13.4250);
  const posts = L('barrier').filter((b) => Math.abs(b.y - ay) < 5 && b.x >= ax - 5 && b.x <= bx + 5).sort((a, b) => a.x - b.x);
  const len = (bx - ax) / S;
  assert.ok(posts.length >= Math.floor(len / 1.6), `${posts.length} Poller auf ${len.toFixed(0)} m`);
  for (let i = 1; i < posts.length; i++) assert.ok(posts[i].x - posts[i - 1].x < 1.8 * S, 'Lücken kleiner als ein Auto');
  const wall = L('wall').filter((w) => Math.abs(w.pts[1] - ay) < 5 && Math.abs(w.pts[0] - ax) < (bx - ax) + 5);
  assert.equal(wall.length, 0, 'keine feste Wand');
  assert.equal(L('fence').filter((f) => f.kind === 3 && Math.abs(f.pts[1] - ay) < 5).length, 0, 'keine gezeichnete Linie');
});
