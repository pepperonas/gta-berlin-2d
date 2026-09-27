import test from 'node:test';
import assert from 'node:assert/strict';
import { buildCity } from '../tools/osm/build.mjs';
import { makeProjection } from '../tools/osm/geo.mjs';
import { decodeCity, surfaceAt, locationName, districtAt, T } from '../web/src/map.js';
import { segDist2, undelta } from '../web/src/geom.js';
import { BUILDING_KIND } from '../web/src/citycodes.js';

// Kleine künstliche Stadt: zwei aneinandergrenzende Bezirke, eine Straße mit Kanal und Brücke,
// eine Einbahnstraße, ein Wohnhaus, eine Lagerhalle, ein Baum.
function fixture() {
  const sq = (w, e) => ({ type: 'MultiPolygon', coordinates: [[[[w, 52.49], [e, 52.49], [e, 52.494], [w, 52.494], [w, 52.49]]]] });
  const lor = { features: [
    { properties: { pgr_id: '0210' }, geometry: sq(13.42, 13.425) },
    { properties: { pgr_id: '0810' }, geometry: sq(13.425, 13.43) },
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
    Wy(103, [82, 83, 84, 85, 82], { building: 'apartments' }),
    Wy(110, [80, 81], { highway: 'service', tunnel: 'building_passage' }),
    // Poller mitten auf dem Pollerweg (Modalfilter)
    N(90, 52.4935, 13.4285, { barrier: 'bollard' }), N(92, 52.4935, 13.4295),
    Wy(111, [9, 90, 92], { highway: 'residential', name: 'Pollerweg' }),
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
const json = buildCity(f.lor, f.osm, f.places, { kataster: f.kataster });
const city = decodeCity(JSON.parse(JSON.stringify(json)));

test('Build ist deterministisch', () => {
  const g = fixture();
  assert.equal(JSON.stringify(buildCity(g.lor, g.osm, g.places, { kataster: g.kataster })), JSON.stringify(json));
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
  assert.equal(city.border.length, 1);
  assert.equal(city.border[0].length, 14, '4 Ecken + 2 Nahtpunkte + Schlusspunkt, keine Naht quer durchs Gebiet');
  const p = city.places.giver, q = city.places.pickup;
  assert.equal(districtAt(city, p.x, p.y), 'Kreuzberg');
  assert.equal(districtAt(city, q.x, q.y), 'Nord-Neukölln');
});

test('Straßengraph: Knoten an Kreuzungen, Einbahnstraße, Brücke, Namen', () => {
  const byName = (n) => city.edges.filter((e) => e.name === n);
  assert.equal(byName('Teststraße').length, 3, 'an den Einmündungen geteilt');
  const cross = city.nodes.find((n) => n.edges.length === 4);
  assert.ok(cross, 'Kreuzung Teststraße/Querstraße');
  assert.deepEqual(byName('Einbahn').map((e) => e.oneway), [1]);
  assert.equal(byName('Querstraße').filter((e) => e.bridge).length, 1);
  assert.ok(Math.abs(byName('Teststraße')[1].w - 90) < 1, 'width=9 (Bordstein zu Bordstein)');
  assert.ok(Math.abs(byName('Querstraße')[0].w - 55) < 1, 'ohne width/lanes/Parkstreifen: Wohnstraße 5,5 m Fahrbahn');
  assert.equal(locationName(city, cross.x, cross.y).split(' / ').sort().join(), 'Querstraße,Teststraße');
});

test('Ufer ist Wand, an der Brücke aber offen; Brücke hat Geländer', () => {
  const bridge = city.edges.find((e) => e.bridge);
  const mx = (bridge.pts[0] + bridge.pts[2]) / 2, my = (bridge.pts[1] + bridge.pts[3]) / 2;
  let quay = 0, rail = 0;
  for (const w of city.walls) for (let i = 0; i < w.length - 2; i += 2) {
    const d = Math.sqrt(segDist2(mx, my, w[i], w[i + 1], w[i + 2], w[i + 3]));
    if (Math.abs(w[i + 1] - w[i + 3]) < 2) { quay++; assert.ok(Math.abs(w[i] - mx) > 40 || Math.abs(w[i + 2] - mx) > 40 || d > 40, 'Kaimauer quer über der Brücke'); }
    else if (d < bridge.w) rail++;
  }
  assert.ok(quay >= 4, 'Kaimauern beidseits der Brücke');
  assert.ok(rail >= 2, 'Geländer links und rechts');
  assert.equal(surfaceAt(city, mx, my), T.ROAD);
  const w = city.water[0].rings[0];
  assert.equal(surfaceAt(city, w[0] + 30, (w[1] + w[5]) / 2), T.WATER);
});

test('Gebäude: Höhe aus Geschossen, Späti und Lagerhalle markiert, Kiez übernommen', () => {
  const kinds = city.buildings.map((b) => b.kind).sort();
  assert.deepEqual(kinds, [BUILDING_KIND.house, BUILDING_KIND.spaeti, BUILDING_KIND.small, BUILDING_KIND.warehouse].sort());
  const house = city.buildings.find((b) => b.kind === BUILDING_KIND.spaeti);
  assert.equal(house.meters, 13.8);
  assert.deepEqual(city.kieze.map((k) => k.n), ['Testkiez']);
  assert.ok(undelta(json.buildings[0][2][0]).every(Number.isInteger), 'Ganzzahl-Koordinaten');
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
  assert.equal(city.trees.length, 3, 'Parkbaum + zwei verschobene Straßenbäume');
  assert.equal(city.droppedTrees, 0);
  const street = city.edges.find((e) => e.name === 'Teststraße' && e.pts[0] < 40000 && e.len > 2000);
  const y0 = street.pts[1], half = street.w / 2, trunk = 5;
  for (const t of city.trees) {
    assert.equal(treeOnRoad(city, t), null, `Stamm auf der Fahrbahn bei ${t.x},${t.y}`);
    assert.equal(inBuilding(city, t.x, t.y), null);
  }
  const moved = city.trees.filter((t) => Math.abs(t.y - y0) < half + 20);
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
  const j = JSON.parse(JSON.stringify(json));
  const e = city.edges.find((x) => x.name === 'Teststraße');
  const mid = [Math.round((e.pts[0] + e.pts[2]) / 2), Math.round((e.pts[1] + e.pts[3]) / 2)];
  // zusätzlicher Baum mitten auf der Teststraße (nach dem ersten, delta-kodiert)
  const t = j.trees;
  t.xy = [...t.xy.slice(0, 2), mid[0] - t.xy[0], mid[1] - t.xy[1], ...t.xy.slice(2)];
  t.g = [t.g[0], 0, ...t.g.slice(1)]; t.c = [t.c[0], 0, ...t.c.slice(1)]; t.r = [t.r[0], 0, ...t.r.slice(1)];
  const c2 = decodeCity(j);
  assert.equal(c2.droppedTrees, 1);
  assert.equal(c2.trees.length, city.trees.length);
});

test('POIs: Kategorien, Bahnhof nur einmal (ohne „U “-Präfix), Parkbank ignoriert', () => {
  const byName = Object.fromEntries(city.pois.map((q) => [q.name, q]));
  assert.equal(byName.Testbäcker.cat, 'shop'); assert.equal(byName.Testbäcker.kind, 'bakery');
  assert.equal(byName.Kiezkneipe.cat, 'drink');
  assert.equal(city.pois.filter((q) => q.cat === 'ubahn').length, 1);
  assert.equal(byName.Teststraße.cat, 'ubahn');
  assert.equal(byName.Einbahn.cat, 'bus');
  assert.equal(city.pois.length, 4);
});

test('Hausnummern: Gebäude und Eingang mit derselben Nummer zählen einmal, Nummer im Straßennamen', () => {
  const nrs = city.addresses.map((a) => `${a.street} ${a.nr}`).sort();
  assert.deepEqual(nrs, ['Teststraße 12', 'Teststraße 7a']);
  const a7 = city.addresses.find((a) => a.nr === '7a');
  const e = city.edges.find((x) => x.name === 'Teststraße' && x.len > 2000);
  assert.equal(locationName(city, a7.x, e.pts[1]), 'Teststraße 7a');
});

test('Tordurchfahrt: Kante bleibt erhalten, die Hauswand ist im Durchfahrtskorridor offen', () => {
  const pass = city.edges.find((e) => e.passage);
  assert.ok(pass, 'Durchfahrt als Kante vorhanden');
  const house = city.buildings.find((b) => b.walls);
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
  const pw = city.edges.filter((e) => e.name === 'Pollerweg');
  assert.ok(pw.length >= 1 && pw.every((e) => e.blocked));
  assert.ok(!buildLaneGraph(city).lanes.some((l) => l.edge.name === 'Pollerweg'), 'kein KI-Verkehr durch den Modalfilter');
  const row = city.barriers.filter((b) => Math.abs(b.y - city.barriers[0].y) < 100);
  assert.ok(row.length >= 3, `${row.length} Poller`);
  const xs = row.map((b) => b.y).sort((a, b) => a - b);
  for (let i = 1; i < xs.length; i++) assert.ok(xs[i] - xs[i - 1] >= 17, 'Abstand ≥ 1,7 m: Fußgänger passen durch, Autos (2 m) nicht');
});

test('Ampel wird der Kreuzung zugeordnet; Abbiegeverbot filtert die Folgespur', async () => {
  const { buildLaneGraph, turnAngle } = await import('../web/src/roadgraph.js');
  const cross = city.nodes.find((n) => n.edges.length === 4 && n.edges.some((k) => city.edges[k].name === 'Querstraße'));
  assert.ok(city.signals.has(cross.id), 'Signal liegt 9 m vor der Kreuzung und gehört zu ihr');
  assert.equal(city.signals.size, 1);
  const via = city.nodes.find((n) => n.edges.some((k) => city.edges[k].name === 'Einbahn') && n.edges.some((k) => city.edges[k].name === 'Teststraße'));
  const west = city.edges.find((e) => e.name === 'Teststraße' && e.b === via.id);
  const einbahn = city.edges.find((e) => e.name === 'Einbahn');
  assert.ok(city.turnBans.has(`${west.id}>${via.id}>${einbahn.id}`));
  const lane = buildLaneGraph(city).lanes.find((l) => l.edge === west && l.to === via.id);
  assert.ok(lane.next.length && !lane.next.some((m) => m.edge === einbahn), 'nicht links in die Einbahn');
  void turnAngle;
});

test('Baumkataster ersetzt den OSM-Baum an derselben Stelle (Gattung, Krone, Stamm)', () => {
  assert.equal(json.meta.trees.kataster, 1);
  const linde = city.trees.find((t) => t.genus === 'Tilia');
  assert.ok(linde, 'Winterlinde aus dem Kataster');
  assert.equal(linde.size, 40, 'Krone 8 m → Radius 40 px');
  assert.ok(Math.abs(linde.r - 1.9) < 0.1, 'Stamm aus 120 cm Umfang');
  assert.ok(!city.trees.some((t) => t !== linde && Math.hypot(t.x - linde.x, t.y - linde.y) < 40), 'OSM-Doppel entfernt');
});
