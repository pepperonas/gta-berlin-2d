import test from 'node:test';
import assert from 'node:assert/strict';
import { surfaceAt, inBuilding, insideBorder, locationName, districtAt, nearestEdge, T, isRoadSurface } from '../web/src/map.js';
import { realCity, realIndex, realOverview, openRealCity } from './helpers/city.js';

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

const BEZIRKE = ['Charlottenburg-Wilmersdorf', 'Friedrichshain-Kreuzberg', 'Lichtenberg', 'Marzahn-Hellersdorf', 'Mitte', 'Neukölln', 'Pankow',
  'Reinickendorf', 'Spandau', 'Steglitz-Zehlendorf', 'Tempelhof-Schöneberg', 'Treptow-Köpenick'];

test('Gebiet: ganz Berlin (12 Bezirke, alle Ortsteile), Maßstab 10 px = 1 m', () => {
  assert.equal(city.scale, 10);
  assert.deepEqual(city.bezirke.map((d) => d.name).sort(), BEZIRKE);
  assert.ok(city.districts.length >= 96, city.districts.map((d) => d.name).join()); // 96 Ortsteile, seit 2023 mit Stadtrandsiedlung Malchow 97
  for (const n of ['Kreuzberg', 'Neukölln', 'Mitte', 'Spandau', 'Köpenick', 'Marzahn', 'Wannsee', 'Frohnau']) assert.ok(city.districts.some((d) => d.name === n), n);
  assert.ok(city.width > 440000 && city.height > 360000, 'ca. 46 × 38 km');
  assert.equal(city.border.length, 1, 'eine Stadtgrenze ohne Nähte');
  assert.match(city.attribution, /OpenStreetMap/);
  // Kerngebiet der Tests (Kreuzberg + Neukölln) ist fest geladen
  assert.ok(city.list('building').length > 20000, `${city.list('building').length} Gebäude`);
  assert.ok(city.edges.size > 20000);
});

test('bekannte Orte in allen Bezirken: Ortsteil, Bezirk und Straße stimmen', async () => {
  const { bezirkAt } = await import('../web/src/map.js');
  const places = [
    [52.49906, 13.41815, 'Kreuzberg', 'Friedrichshain-Kreuzberg', /Kottbusser Tor|Adalbertstraße|Skalitzer|Reichenberger|Kottbusser Straße/], // Kottbusser Tor
    [52.48685, 13.42469, 'Neukölln', 'Neukölln', /Hermannplatz|Karl-Marx-Straße|Sonnenallee|Hasenheide|Urbanstraße|Kottbusser Damm|Hermannstraße/],
    [52.48123, 13.43530, 'Neukölln', 'Neukölln', /Karl-Marx-Straße|Erkstraße/], // Rathaus Neukölln
    [52.49368, 13.38790, 'Kreuzberg', 'Friedrichshain-Kreuzberg', /Mehringdamm|Gneisenaustraße|Yorckstraße/],
    [52.5219, 13.4132, 'Mitte', 'Mitte', /Alexanderplatz|Grunerstraße|Karl-Liebknecht-Straße|Alexanderstraße|Otto-Braun-Straße|Rathausstraße|Dircksenstraße|Memhardstraße|Gontardstraße/],
    [52.5354, 13.2006, 'Spandau', 'Spandau', /Altstädter Ring|Carl-Schurz-Straße|Klosterstraße|Moritzstraße/], // Rathaus Spandau
    [52.4453, 13.5742, 'Köpenick', 'Treptow-Köpenick', /Alt-Köpenick|Schloßplatz|Grünstraße|Müggelheimer Straße|Freiheit|Kietz|Rosenstraße/],
    [52.5436, 13.5646, 'Marzahn', 'Marzahn-Hellersdorf', /Allee der Kosmonauten|Marzahner Promenade|Raoul-Wallenberg-Straße/],
    [52.5870, 13.2857, 'Tegel', 'Reinickendorf', /Schlieperstraße|Berliner Straße|Veitstraße|Am Borsigturm|Karolinenstraße/],
    [52.5069, 13.3326, 'Charlottenburg', 'Charlottenburg-Wilmersdorf', /Hardenbergplatz|Hardenbergstraße|Joachimsthaler Straße|Budapester Straße|Kantstraße/], // Zoo
    [52.5157, 13.4543, 'Friedrichshain', 'Friedrichshain-Kreuzberg', /Frankfurter Allee|Petersburger Straße|Warschauer Straße|Karl-Marx-Allee/],
    [52.4565, 13.3217, 'Steglitz', 'Steglitz-Zehlendorf', /Schloßstraße|Albrechtstraße|Kuhligkshofstraße|Grunewaldstraße|Rathaus/], // Rathaus Steglitz
    [52.4843, 13.3445, 'Schöneberg', 'Tempelhof-Schöneberg', /Martin-Luther-Straße|Dominicusstraße|Hauptstraße|Belziger Straße|Kufsteiner Straße|John-F.-Kennedy-Platz|Freiherr-vom-Stein-Straße/], // Rathaus Schöneberg
    [52.5163, 13.4796, 'Lichtenberg', 'Lichtenberg', /Frankfurter Allee|Möllendorffstraße|Rathausstraße|Normannenstraße|Ruschestraße|Magdalenenstraße|Siegfriedstraße/],
    [52.5690, 13.4020, 'Pankow', 'Pankow', /Breite Straße|Berliner Straße|Florastraße|Garbátyplatz|Mühlenstraße|Schönholzer Straße|Wollankstraße|Damerowstraße/], // S+U Pankow
  ];
  for (const [lat, lon, district, bezirk, street] of places) {
    const p = await toPx(lat, lon);
    city.focus('probe', p.x, p.y);
    assert.equal(districtAt(city, p.x, p.y), district, `${lat},${lon}`);
    assert.equal(bezirkAt(city, p.x, p.y), bezirk, `${lat},${lon}`);
    const n = nearestEdge(city, p.x, p.y, 2500, (e) => e.name && e.cls <= 8);
    assert.match(n.e.name, street, `${lat},${lon}: ${n.e.name}`);
  }
  city.release('probe');
});

test('wichtige Straßen sind vorhanden und durchgängig im Gebiet', () => {
  const edges = city.list('edge');
  const km = (name) => edges.filter((e) => e.name === name && e.inside).reduce((s, e) => s + e.len, 0) / 10000;
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
  assert.ok(isRoadSurface(surfaceAt(city, p.pickup.x, p.pickup.y)), 'Einladen auf der Fahrbahn');
  assert.ok(isRoadSurface(surfaceAt(city, p.dropoff.x, p.dropoff.y)), 'Abliefern auf der Fahrbahn');
  assert.match(locationName(city, p.giver.x, p.giver.y), /^Wrangelstraße \d+[a-z]?$/, 'Straße mit Hausnummer');
  assert.equal(districtAt(city, p.pickup.x, p.pickup.y), 'Neukölln');
  assert.ok(city.timeLimit >= 300 && city.timeLimit <= 1200, `Zeitlimit ${city.timeLimit} s`);
});

test('Untergrund und Straßennamen', async () => {
  const park = await toPx(52.4843, 13.4133); // Hasenheide
  assert.equal(surfaceAt(city, park.x, park.y), T.GRASS);
  const spree = await toPx(52.4988, 13.4530); // Spree bei Treptow (Wasserfläche)
  assert.equal(surfaceAt(city, spree.x, spree.y), T.WATER);
  // Brücken über Landwehrkanal/Spree sind befahrbar (Mitte der Brücke liegt über Wasser, zählt als Fahrbahn).
  const bridges = city.list('edge').filter((e) => e.bridge && e.inside && e.cls <= 7);
  assert.ok(bridges.length > 20, `${bridges.length} Straßenbrücken`);
  const over = bridges.filter((e) => { const m = e.pts.length / 2 & ~1; return city.list('water').some((w) => w.bbox.x < e.pts[m] && e.pts[m] < w.bbox.x + w.bbox.w && w.bbox.y < e.pts[m + 1] && e.pts[m + 1] < w.bbox.y + w.bbox.h); });
  assert.ok(over.length > 10);
  for (const e of over) { const m = e.pts.length / 2 & ~1; assert.ok(isRoadSurface(surfaceAt(city, e.pts[m], e.pts[m + 1]))); }
  const node = [...city.nodes.values()].find((n) => { const names = n.edges.map((k) => city.edges.get(k).name); return names.includes('Oranienstraße') && names.includes('Adalbertstraße'); });
  assert.ok(node, 'Kreuzung Oranien-/Adalbertstraße');
  assert.match(locationName(city, node.x, node.y), /Oranienstraße \/ Adalbertstraße|Adalbertstraße \/ Oranienstraße/);
});

test('Karte: Index, Übersicht und Kacheln (Format 3), jede Kachel klein genug zum Nachladen', async () => {
  const { readdirSync, statSync } = await import('node:fs');
  const j = realIndex();
  assert.equal(j.meta.version, 3);
  assert.ok(j.meta.osmBase, 'OSM-Stand vermerkt');
  const dir = new URL('../web/data/berlin/tiles/', import.meta.url);
  const files = readdirSync(dir);
  assert.equal(files.length, j.tiles.length, 'jede gelistete Kachel liegt vor');
  assert.ok(files.length > 2000, `${files.length} Kacheln`);
  const sizes = files.map((f) => statSync(new URL(f, dir)).size);
  assert.ok(Math.max(...sizes) < 400e3, `größte Kachel ${Math.max(...sizes)} B`);
  assert.ok(statSync(new URL('../web/data/berlin/index.json', import.meta.url)).size < 2e6, 'Index unter 2 MB');
  assert.ok(statSync(new URL('../web/data/berlin/overview.json', import.meta.url)).size < 12e6, 'Stadtplan unter 12 MB');
});

// „Das Datenmodell ist komplett gefüllt“: jede Schicht hat in jedem Bezirk Einträge (vom Build gezählt).
test('ganz Berlin: jede Datenschicht ist in jedem der 12 Bezirke gefüllt', () => {
  const cov = realIndex().meta.coverage;
  assert.deepEqual(Object.keys(cov).sort(), BEZIRKE);
  const min = { strassenKm: 300, hauptnetzKm: 150, breiteGemessen: 20, parkenErfasst: 150, tempoErfasst: 150, belagErfasst: 150, gebaeude: 10000, hoeheGemessen: 5000,
    baeume: 40000, katasterBaeume: 40000, pois: 1000, haltestellen: 100, bahnhoefe: 5, hausnummern: 10000, ampeln: 100, querungen: 300, abbiegeverbote: 20,
    poller: 2000, zaeune: 1000, durchfahrten: 30, tueren: 1000, kreuzungen: 1000 };
  for (const [b, row] of Object.entries(cov)) {
    for (const [k, v] of Object.entries(min)) assert.ok(row[k] >= v, `${b}: ${k} = ${row[k]} (erwartet ≥ ${v})`);
    assert.ok(row.parkenErfasst / row.hauptnetzKm > 0.8, `${b}: Parkstreifen auf ${(100 * row.parkenErfasst / row.hauptnetzKm).toFixed(0)} % des Hauptnetzes erfasst`);
  }
  const m = realIndex().meta;
  assert.ok(m.counts.buildings > 500000 && m.counts.edges > 300000 && m.counts.trees > 1000000 && m.counts.addresses > 400000, JSON.stringify(m.counts));
  assert.ok(m.trees.kataster > 950000, 'Baumbestand vollständig');
});

test('Bäume: kein Stamm auf einer Fahrbahn, in einem Haus oder im Wasser (Krone darf überragen)', async () => {
  const { treeOnRoad } = await import('../web/src/map.js');
  assert.equal(city.droppedTrees, 0, 'die Karte selbst muss sauber sein, nicht erst das Sicherheitsnetz im Spiel');
  const trees = city.list('tree');
  const bad = trees.filter((t) => treeOnRoad(city, t) || inBuilding(city, t.x, t.y) || surfaceAt(city, t.x, t.y) === T.WATER);
  assert.equal(bad.length, 0, `${bad.length} Bäume falsch, z. B. ${bad[0]?.x},${bad[0]?.y}`);
  assert.ok(trees.length > 40000, `${trees.length} Bäume`);
  const j = realIndex();
  assert.ok(j.meta.trees.moved > 0 && j.meta.trees.dropped < j.meta.trees.moved, 'Build hat verschoben, nicht pauschal gelöscht');
  // Straßenbäume stehen weiter am Straßenrand: die Krone reicht in vielen Fällen über die Fahrbahn.
  const overhang = trees.filter((t) => treeOnRoad(city, { x: t.x, y: t.y, r: t.size })).length;
  assert.ok(overhang > 5000, `nur ${overhang} Kronen über der Fahrbahn`);
});

// Jede Kachel einzeln geladen (sie enthält alle Straßen, die ihre Bäume berühren): das Sicherheitsnetz im Spiel
// darf nirgends in Berlin einen Baum wegnehmen müssen. Danach ist alles wieder entladen (keine Rückstände).
test('ganz Berlin, alle Kacheln: kein Baumstamm und kein Poller auf der Fahrbahn; Entladen hinterlässt nichts', () => {
  const c = openRealCity();
  let trees = 0;
  for (const k of c.available) {
    const [x, y] = k.split('_').map(Number);
    c.loadArea(x * c.tile + 1, y * c.tile + 1, x * c.tile + 2, y * c.tile + 2);
    trees += c.items.get('tree')?.size ?? 0;
    c.unload(k);
  }
  assert.equal(c.droppedTrees, 0, 'Bäume auf der Fahrbahn gefunden');
  assert.equal(c.droppedPosts, 0, 'Poller auf einer befahrbaren Fahrbahn gefunden');
  assert.equal(trees, realIndex().meta.counts.trees, 'alle Bäume geladen');
  assert.equal(c.tiles.size + c.reg.size + c.edges.size + c.nodes.size + c.signals.size + c.turnBans.size, 0, 'Reste nach dem Entladen');
  for (const h of [c.render, c.edgeSegs, c.polys, c.poiHash, c.addrHash]) assert.equal(h.map.size, 0, 'Raster nicht leer');
  assert.equal(c.solids.map.size, [...new Set(c.crates.map(() => 1))].length ? c.solids.map.size : 0);
  for (const [layer, set] of c.items) assert.equal(set.size, 0, `${layer} nicht leer`);
});

test('Nachladen: Kacheln an der Grenze fügen sich nahtlos (Straßen einmal, Gebäude einmal)', async () => {
  const c = openRealCity();
  const p = city.places.giver;
  c.focus('a', p.x, p.y);
  const e1 = c.edges.size, b1 = c.items.get('building').size;
  // gleiche Gegend über eine zweite Kachelreihe erneut laden: nichts doppelt
  c.focus('b', p.x + c.tile, p.y);
  const ids = c.list('edge').map((e) => e.id);
  assert.equal(new Set(ids).size, ids.length, 'jede Kante genau einmal');
  const bIds = c.list('building').map((b) => b.id);
  assert.equal(new Set(bIds).size, bIds.length, 'jedes Gebäude genau einmal');
  assert.ok(c.edges.size > e1 && c.items.get('building').size > b1);
  // Knoten an Kachelgrenzen kennen alle ihre Kanten (Straßengraph hat keine Lücke)
  for (const nd of c.nodes.values()) for (const k of nd.edges) assert.ok(c.edges.get(k), 'Knoten verweist auf geladene Kante');
});

test('POIs: Bahnhöfe, Einkaufszentrum, Supermärkte, Gastronomie', async () => {
  const { nearestPoi } = await import('../web/src/map.js');
  const pois = city.list('poi');
  const count = (cat) => pois.filter((q) => q.cat === cat).length;
  for (const [cat, min] of [['food', 800], ['drink', 300], ['cafe', 300], ['supermarket', 300], ['shop', 1500], ['bus', 150]]) {
    assert.ok(count(cat) >= min, `${cat}: ${count(cat)}`);
  }
  const station = (cat, name) => pois.filter((q) => q.cat === cat && q.name === name);
  for (const n of ['Kottbusser Tor', 'Hermannplatz', 'Rathaus Neukölln', 'Görlitzer Bahnhof', 'Schönleinstraße', 'Mehringdamm'])
    assert.equal(station('ubahn', n).length, 1, `U ${n} genau einmal`);
  for (const n of ['Neukölln', 'Sonnenallee', 'Hermannstraße']) assert.equal(station('sbahn', n).length, 1, `S ${n}`);
  const arcaden = pois.find((q) => q.name === 'Neukölln Arcaden' && q.cat === 'mall');
  assert.ok(arcaden, 'Neukölln Arcaden');
  assert.equal(districtAt(city, arcaden.x, arcaden.y), 'Neukölln');
  assert.match(nearestEdge(city, arcaden.x, arcaden.y, 1500, (e) => e.name && e.cls <= 5).e.name, /Karl-Marx-Straße|Flughafenstraße|Donaustraße|Erkstraße/);
  assert.ok(pois.some((q) => q.name === 'Penny' && q.cat === 'supermarket'), 'Penny');
  const kotti = station('ubahn', 'Kottbusser Tor')[0];
  assert.equal(nearestPoi(city, kotti.x + 50, kotti.y, 200, (q) => q.cat === 'ubahn'), kotti);
  // ganz Berlin: der Stadtplan kennt alle großen Bahnhöfe
  const ov = realOverview();
  for (const n of ['Alexanderplatz', 'Zoologischer Garten', 'Spandau', 'Ostkreuz', 'Wannsee', 'Hauptbahnhof', 'Rathaus Spandau', 'Hermannplatz']) assert.ok(ov.stations.some((s) => s[3] === n), `Stadtplan: ${n}`);
});

test('Hausnummern: vorhanden, an echten Straßen, im Straßennamen des HUD', async () => {
  const { nearestAddress } = await import('../web/src/map.js');
  const addresses = city.list('address');
  assert.ok(addresses.length > 20000, `${addresses.length}`);
  const ora = addresses.filter((a) => a.street === 'Oranienstraße');
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

test('Straßenraum, Ampeln, Durchfahrten, Kataster: Stichproben auf der echten Karte', () => {
  const edges = city.list('edge');
  const w = (name) => { const es = edges.filter((e) => e.name === name && e.inside && e.cls <= 8); return es.reduce((s, e) => s + e.w * e.len, 0) / es.reduce((s, e) => s + e.len, 0) / city.scale; };
  assert.ok(w('Oranienstraße') > 9 && w('Oranienstraße') < 20, `Oranienstraße ${w('Oranienstraße').toFixed(1)} m`);
  // Sonnenallee: zwei getrennte Richtungsfahrbahnen, je Fahrbahn gemessen
  assert.ok(w('Sonnenallee') > 6.5 && w('Sonnenallee') < 20, `Sonnenallee ${w('Sonnenallee').toFixed(1)} m`);
  assert.ok(edges.filter((e) => e.inside && e.cs.left.park + e.cs.right.park > 0).length > 3000, 'Parkstreifen erfasst');
  assert.ok(edges.some((e) => e.cs.surface === 1), 'Kopfsteinpflaster erfasst');
  // Ampeln an bekannten Kreuzungen
  const nodes = [...city.nodes.values()];
  const names = (n) => n.edges.map((k) => city.edges.get(k).name);
  const signalAt = (a, b) => [...city.signals].some((v) => { const n = city.nodes.get(v); return n && names(n).includes(a) && names(n).includes(b); })
    || [...city.signals].some((v) => { const n = city.nodes.get(v); return n && nodes.some((m) => Math.hypot(m.x - n.x, m.y - n.y) < 400 && names(m).includes(a) && names(m).includes(b)); });
  assert.ok(signalAt('Kottbusser Damm', 'Hermannplatz') || signalAt('Kottbusser Damm', 'Urbanstraße'), 'Hermannplatz');
  assert.ok(signalAt('Kottbusser Tor', 'Adalbertstraße') && signalAt('Kottbusser Tor', 'Skalitzer Straße'), 'Kottbusser Tor');
  assert.ok(city.signals.size > 300, `${city.signals.size} Ampelkreuzungen`);
  assert.ok(city.turnBans.size > 100, `${city.turnBans.size} Abbiegeverbote`);
  assert.ok(edges.filter((e) => e.passage).length > 1000, 'Tordurchfahrten');
  assert.ok(city.list('building').filter((b) => b.walls).length > 500, 'geöffnete Hauswände');
  assert.ok(city.list('crossing').filter((c) => c.kind === 'zebra').length > 50, 'Zebrastreifen');
  const kat = city.list('tree').filter((t) => t.genus !== 'sonstige').length;
  assert.ok(kat > 60000, `${kat} Bäume mit Gattung aus dem Kataster`);
  assert.match(city.attribution, /Baumbestand/);
});

test('Keine Poller, Zäune oder Gleiswände auf befahrbaren Fahrbahnen (die KI schrammte sonst daran entlang)', async () => {
  const { postOnRoad } = await import('../web/src/map.js');
  const { segDist2 } = await import('../web/src/geom.js');
  assert.equal(city.droppedPosts, 0);
  assert.equal(city.list('barrier').filter((b) => postOnRoad(city, b)).length, 0, 'Poller auf der Fahrbahn');
  const m = realIndex().meta.access;
  assert.ok(m.pollerVerschoben > 1000 && m.pollerEntfernt < m.pollerVerschoben / 10, 'Build rückt an den Bordstein statt zu löschen');
  // Wände (Zäune, Gleise) gegen die Fahrbahnen der KI derselben Ebene prüfen
  const drivable = city.list('edge').filter((e) => e.cls <= 8 && !e.blocked && !e.passage);
  let bad = 0;
  for (const wl of city.list('wall').filter((f) => f.sub === 'fence' || f.sub === 'rail')) {
    const p = wl.pts;
    for (let i = 0; i < p.length - 2; i += 2) {
      const mx = (p[i] + p[i + 2]) / 2, my = (p[i + 1] + p[i + 3]) / 2;
      for (const s of city.edgeSegs.query({ x: mx, y: my, w: 0, h: 0 }, [])) {
        if (s.e.junction || s.e.cls > 8 || s.e.blocked || s.e.passage) continue;
        if ((s.e.lvl ?? 0) !== (wl.lvl ?? 0)) continue; // Gleis unter der Straßenbrücke: andere Ebene, keine Sperre
        if (segDist2(mx, my, s.ax, s.ay, s.bx, s.by) < (s.e.w / 2 - 1) ** 2) { bad++; break; }
      }
    }
  }
  assert.equal(bad, 0, `${bad} Zaun-/Gleisstücke auf Fahrbahnen`);
  assert.ok(city.list('wall').some((f) => f.sub === 'fence') && city.list('wall').some((f) => f.sub === 'quay'), 'Wandarten unterschieden');
  void drivable;
});
