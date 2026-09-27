import test from 'node:test';
import assert from 'node:assert/strict';
import { surfaceAt, inBuilding, insideBorder, locationName, districtAt, nearestEdge, T } from '../web/src/map.js';
import { realCity, realCityJson } from './helpers/city.js';

const city = realCity();

// Bekannte Orte (WGS84) → Spielkoordinaten über dieselbe Projektion wie der Build.
async function toPx(lat, lon) {
  const { makeProjection } = await import('../tools/osm/geo.mjs');
  const { lat0, lon0, bbox: [s, w, n, e] } = city.meta.origin;
  const proj = makeProjection(lat0, lon0);
  const c = [proj(s, w), proj(s, e), proj(n, w), proj(n, e)];
  const minX = Math.min(...c.map((q) => q[0])), maxY = Math.max(...c.map((q) => q[1]));
  const [x, y] = proj(lat, lon);
  return { x: (x - minX) * city.scale, y: (maxY - y) * city.scale };
}

test('Gebiet: Kreuzberg und Nord-Neukölln, Maßstab 10 px = 1 m', () => {
  assert.equal(city.scale, 10);
  assert.deepEqual(city.districts.map((d) => d.name).sort(), ['Kreuzberg', 'Nord-Neukölln']);
  assert.ok(city.width > 70000 && city.height > 55000, 'ca. 8 × 6 km');
  assert.ok(city.buildings.length > 20000, `${city.buildings.length} Gebäude`);
  assert.ok(city.edges.length > 20000);
  assert.match(city.attribution, /OpenStreetMap/);
});

test('bekannte Orte liegen im richtigen Bezirk und an der richtigen Straße', async () => {
  const places = [
    [52.49906, 13.41815, 'Kreuzberg', /Kottbusser Tor|Adalbertstraße|Skalitzer|Reichenberger|Kottbusser Straße/], // Kottbusser Tor
    [52.48685, 13.42469, 'Nord-Neukölln', /Hermannplatz|Karl-Marx-Straße|Sonnenallee|Hasenheide|Urbanstraße|Kottbusser Damm|Hermannstraße/], // Hermannplatz
    [52.48123, 13.43530, 'Nord-Neukölln', /Karl-Marx-Straße|Erkstraße/], // Rathaus Neukölln
    [52.49368, 13.38790, 'Kreuzberg', /Mehringdamm|Gneisenaustraße|Yorckstraße/], // U Mehringdamm
  ];
  for (const [lat, lon, district, street] of places) {
    const p = await toPx(lat, lon);
    assert.equal(districtAt(city, p.x, p.y), district, `${lat},${lon}`);
    const n = nearestEdge(city, p.x, p.y, 600, (e) => e.name && e.cls <= 8);
    assert.match(n.e.name, street, `${lat},${lon}: ${n.e.name}`);
  }
});

test('wichtige Straßen sind vorhanden und durchgängig im Gebiet', () => {
  const km = (name) => city.edges.filter((e) => e.name === name && e.inside).reduce((s, e) => s + e.len, 0) / 10000;
  for (const [name, min] of [['Oranienstraße', 1.2], ['Sonnenallee', 2.5], ['Kottbusser Damm', 0.8], ['Karl-Marx-Straße', 2], ['Mehringdamm', 1], ['Wrangelstraße', 0.8]]) {
    assert.ok(km(name) >= min, `${name}: ${km(name).toFixed(2)} km`);
  }
});

test('Missionsorte: im Gebiet, nicht in Häusern, Straßenname stimmt', () => {
  const p = city.places;
  for (const k of ['giver', 'pickup', 'dropoff', 'playerSpawn', 'playerCar']) {
    assert.ok(insideBorder(city, p[k].x, p[k].y), `${k} außerhalb`);
    assert.ok(!inBuilding(city, p[k].x, p[k].y), `${k} im Haus`);
    assert.notEqual(surfaceAt(city, p[k].x, p[k].y), T.WATER, `${k} im Wasser`);
  }
  assert.equal(surfaceAt(city, p.pickup.x, p.pickup.y), T.ROAD, 'Einladen auf der Fahrbahn');
  assert.equal(surfaceAt(city, p.dropoff.x, p.dropoff.y), T.ROAD, 'Abliefern auf der Fahrbahn');
  assert.match(locationName(city, p.giver.x, p.giver.y), /^Wrangelstraße \d+[a-z]?$/, 'Straße mit Hausnummer');
  assert.equal(districtAt(city, p.pickup.x, p.pickup.y), 'Nord-Neukölln');
  assert.ok(city.timeLimit >= 300 && city.timeLimit <= 1200, `Zeitlimit ${city.timeLimit} s`);
});

test('Untergrund und Straßennamen', async () => {
  const park = await toPx(52.4843, 13.4133); // Hasenheide
  assert.equal(surfaceAt(city, park.x, park.y), T.GRASS);
  const spree = await toPx(52.4988, 13.4530); // Spree bei Treptow (Wasserfläche)
  assert.equal(surfaceAt(city, spree.x, spree.y), T.WATER);
  // Brücken über Landwehrkanal/Spree sind befahrbar (Mitte der Brücke liegt über Wasser, zählt als Fahrbahn).
  const bridges = city.edges.filter((e) => e.bridge && e.inside && e.cls <= 7);
  assert.ok(bridges.length > 20, `${bridges.length} Straßenbrücken`);
  const over = bridges.filter((e) => { const m = e.pts.length / 2 & ~1; return city.water.some((w) => w.bbox.x < e.pts[m] && e.pts[m] < w.bbox.x + w.bbox.w && w.bbox.y < e.pts[m + 1] && e.pts[m + 1] < w.bbox.y + w.bbox.h); });
  assert.ok(over.length > 10);
  for (const e of over) { const m = e.pts.length / 2 & ~1; assert.equal(surfaceAt(city, e.pts[m], e.pts[m + 1]), T.ROAD); }
  const node = city.nodes.find((n) => { const names = n.edges.map((k) => city.edges[k].name); return names.includes('Oranienstraße') && names.includes('Adalbertstraße'); });
  assert.ok(node, 'Kreuzung Oranien-/Adalbertstraße');
  assert.match(locationName(city, node.x, node.y), /Oranienstraße \/ Adalbertstraße|Adalbertstraße \/ Oranienstraße/);
});

test('Karte ist eine gültige, kompakte Datei', () => {
  const j = realCityJson();
  assert.equal(j.meta.version, 1);
  assert.ok(JSON.stringify(j).length < 8e6);
  assert.ok(j.meta.osmBase, 'OSM-Stand vermerkt');
});

test('Bäume: kein Stamm auf einer Fahrbahn, in einem Haus oder im Wasser (Krone darf überragen)', async () => {
  const { treeOnRoad } = await import('../web/src/map.js');
  assert.equal(city.droppedTrees, 0, 'die Karte selbst muss sauber sein, nicht erst das Sicherheitsnetz im Spiel');
  const bad = city.trees.filter((t) => treeOnRoad(city, t) || inBuilding(city, t.x, t.y) || surfaceAt(city, t.x, t.y) === T.WATER);
  assert.equal(bad.length, 0, `${bad.length} Bäume falsch, z. B. ${bad[0]?.x},${bad[0]?.y}`);
  assert.ok(city.trees.length > 40000, `${city.trees.length} Bäume`);
  const j = realCityJson();
  assert.ok(j.meta.trees.moved > 0 && j.meta.trees.dropped < j.meta.trees.moved, 'Build hat verschoben, nicht pauschal gelöscht');
  // Straßenbäume stehen weiter am Straßenrand: die Krone reicht in vielen Fällen über die Fahrbahn.
  const overhang = city.trees.filter((t) => treeOnRoad(city, { x: t.x, y: t.y, r: t.size })).length;
  assert.ok(overhang > 5000, `nur ${overhang} Kronen über der Fahrbahn`);
});

test('POIs: Bahnhöfe, Einkaufszentrum, Supermärkte, Gastronomie', async () => {
  const { nearestPoi } = await import('../web/src/map.js');
  const count = (cat) => city.pois.filter((q) => q.cat === cat).length;
  for (const [cat, min] of [['food', 800], ['drink', 300], ['cafe', 300], ['supermarket', 300], ['shop', 1500], ['bus', 150]]) {
    assert.ok(count(cat) >= min, `${cat}: ${count(cat)}`);
  }
  const station = (cat, name) => city.pois.filter((q) => q.cat === cat && q.name === name);
  for (const n of ['Kottbusser Tor', 'Hermannplatz', 'Rathaus Neukölln', 'Görlitzer Bahnhof', 'Schönleinstraße', 'Mehringdamm'])
    assert.equal(station('ubahn', n).length, 1, `U ${n} genau einmal`);
  for (const n of ['Neukölln', 'Sonnenallee', 'Hermannstraße', 'Treptower Park']) assert.equal(station('sbahn', n).length, 1, `S ${n}`);
  const arcaden = city.pois.find((q) => q.name === 'Neukölln Arcaden' && q.cat === 'mall');
  assert.ok(arcaden, 'Neukölln Arcaden');
  assert.equal(districtAt(city, arcaden.x, arcaden.y), 'Nord-Neukölln');
  assert.match(nearestEdge(city, arcaden.x, arcaden.y, 1500, (e) => e.name && e.cls <= 5).e.name, /Karl-Marx-Straße|Flughafenstraße|Donaustraße|Erkstraße/);
  assert.ok(city.pois.some((q) => q.name === 'Penny' && q.cat === 'supermarket'), 'Penny');
  const kotti = station('ubahn', 'Kottbusser Tor')[0];
  assert.equal(nearestPoi(city, kotti.x + 50, kotti.y, 200, (q) => q.cat === 'ubahn'), kotti);
});

test('Hausnummern: vorhanden, an echten Straßen, im Straßennamen des HUD', async () => {
  const { nearestAddress } = await import('../web/src/map.js');
  assert.ok(city.addresses.length > 20000, `${city.addresses.length}`);
  const ora = city.addresses.filter((a) => a.street === 'Oranienstraße');
  assert.ok(ora.length > 100, `${ora.length} Hausnummern Oranienstraße`);
  // Vorderhäuser liegen an der Straße; Hinterhäuser (z. B. „102A“) können 70 m und mehr zurückliegen.
  const dist = ora.map((a) => nearestEdge(city, a.x, a.y, 2000, (e) => e.name === 'Oranienstraße')?.d ?? Infinity);
  assert.ok(dist.every((d) => d < 150 * city.scale), 'jede Hausnummer höchstens 150 m von ihrer Straße');
  const median = dist.slice().sort((a, b) => a - b)[dist.length >> 1];
  assert.ok(median < 25 * city.scale, `Median ${(median / city.scale).toFixed(1)} m (Gebäudemitte bis Straßenmitte)`);
  const a = ora[10];
  const n = nearestEdge(city, a.x, a.y, 800, (e) => e.name === 'Oranienstraße');
  assert.match(locationName(city, n.x, n.y), /^Oranienstraße \d+/);
  assert.equal(nearestAddress(city, a.x, a.y, 10, 'Oranienstraße'), a);
});
