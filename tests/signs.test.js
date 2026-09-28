import test from 'node:test';
import assert from 'node:assert/strict';
import { buildSigns } from '../tools/osm/signs.mjs';
import { decodeSign, isStreet } from '../web/src/signs.js';
import { realCity, realIndex } from './helpers/city.js';
import { onRoad, inBuilding } from '../web/src/map.js';

const S = 10;
// Kreuz zweier Hauptstraßen (A in Ost-West, B in Nord-Süd), je 600 m lang; Ortsteile nach Himmelsrichtung
function cross({ len = 6000, oneN = 0, oneS = 0, tags = {}, rels = [], center = null, nLen = len } = {}) {
  const vertices = [0, 0, len, 0, -len, 0, 0, -nLen, 0, len];
  const E = (a, b, n, id, o = 0) => ({ a, b, c: 3, w: 120, n, o, p: [], id, in: 1, x: [] });
  const edges = [E(0, 1, 0, 101), E(2, 0, 0, 102), E(0, 3, 1, 103, oneN), oneS ? E(4, 0, 1, 104, 1) : E(0, 4, 1, 104)];
  const districtAt = (x, y) => (x < -500 ? 'Westend' : x > 500 ? 'Ostend' : y < -500 ? 'Nordkiez' : y > 500 ? 'Südkiez' : 'Mitte-Kiez');
  return buildSigns({ edges, vertices, names: ['A-Straße', 'B-Straße'], S, districtAt, center, wayTags: (id) => tags[id], destRels: rels });
}
const deg = (a) => Math.round(a * 180 / Math.PI);

test('Kreuzung: je Zufahrt ein Schild rechts vor der Kreuzung, links → geradeaus → rechts, kein Wenden', () => {
  const { signs, stats } = cross();
  assert.equal(stats.junctions, 1);
  assert.equal(signs.length, 4);
  const fromWest = signs.find((s) => Math.abs(s.angle) < 0.1); // fährt nach Osten
  assert.deepEqual(fromWest.rows.map((r) => r.dests[0]), ['Nordkiez', 'Ostend', 'Südkiez']);
  assert.deepEqual(fromWest.rows.map((r) => deg(r.turn)), [-90, 0, 90]);
  assert.ok(Math.abs(fromWest.x + 350) < 5, `35 m vor der Kreuzung (${fromWest.x})`);
  assert.ok(fromWest.y > 60 && fromWest.y < 100, `rechts neben der 12 m breiten Fahrbahn (${fromWest.y})`);
  for (const s of signs) {
    assert.equal(s.rows.length, 3, 'keine Zeile zurück in die eigene Straße');
    assert.ok(s.vis);
  }
});

test('Einbahnstraßen: keine Zufahrt gegen die Richtung, keine Ausfahrt hinein', () => {
  const { signs } = cross({ oneN: 1, oneS: 1 }); // B nur nach Norden befahrbar (von S zur Kreuzung, von ihr nach N)
  assert.equal(signs.length, 3, 'aus Norden kommt niemand');
  for (const s of signs) assert.ok(!s.rows.some((r) => r.dests.includes('Südkiez')), 'nach Süden ist Einbahn dagegen');
});

test('Echte Beschilderung aus OSM geht vor: destination-Tag in Fahrtrichtung, Relation von–über–nach', () => {
  const { signs } = cross({ tags: { 101: { destination: 'Flughafen;Messe', ref: 'B 96' } }, rels: [{ from: 102, via: 0, to: 103, text: 'Spezialziel', ref: '' }] });
  const fromWest = signs.find((s) => Math.abs(s.angle) < 0.1);
  const east = fromWest.rows.find((r) => Math.abs(r.turn) < 0.1);
  assert.deepEqual(east.dests, ['Flughafen', 'Messe']);
  assert.equal(east.ref, 'B 96');
  assert.deepEqual(fromWest.rows.find((r) => r.turn < -1).dests, ['Spezialziel'], 'Relation nur für diese Zufahrt');
  const fromSouth = signs.find((s) => Math.abs(s.angle + Math.PI / 2) < 0.1);
  assert.equal(fromSouth.rows.find((r) => Math.abs(r.turn) < 0.1).dests[0], 'Nordkiez');
});

test('destination-Tag gilt in Weg-Richtung; gegen sie nur destination:backward', () => {
  // Weg 102 ist von Westen zur Kreuzung gezeichnet; wer nach Westen hinausfährt, fährt gegen die Weg-Richtung
  const westbound = (tags) => cross({ tags: { 102: tags } }).signs.find((s) => Math.abs(Math.abs(s.angle) - Math.PI) < 0.1).rows.find((r) => Math.abs(r.turn) < 0.1);
  assert.equal(westbound({ destination: 'Nur-Ostwärts' }).dests[0], 'Westend', 'Tag der Gegenrichtung gilt nicht');
  assert.equal(westbound({ destination: 'Nur-Ostwärts', 'destination:backward': 'Spandau' }).dests[0], 'Spandau');
});

test('Keine Zeile, die fast zurückführt – auch in eine andere Straße', () => {
  const vertices = [0, 0, 6000, 0, -6000, 0, 0, -6000, 0, 6000, -6000, 800];
  const E = (a, b, n, id) => ({ a, b, c: 3, w: 120, n, o: 0, p: [], id, in: 1, x: [] });
  const edges = [E(0, 1, 0, 101), E(2, 0, 0, 102), E(0, 3, 1, 103), E(0, 4, 1, 104), E(0, 5, 2, 105)];
  const districtAt = (x, y) => (x < -500 ? 'Westend' : x > 500 ? 'Ostend' : y < -500 ? 'Nordkiez' : y > 500 ? 'Südkiez' : 'M');
  const { signs } = buildSigns({ edges, vertices, names: ['A', 'B', 'C-Straße'], S, districtAt });
  const fromWest = signs.find((s) => Math.abs(s.angle) < 0.1);
  assert.ok(fromWest.rows.every((r) => Math.abs(r.turn) < 2.6), 'die C-Straße führt fast zurück: keine Zeile');
  const fromEast = signs.find((s) => Math.abs(Math.abs(s.angle) - Math.PI) < 0.1);
  assert.ok(fromEast.rows.length === 4, 'aus Osten ist die C-Straße eine normale Ausfahrt');
});

test('„Zentrum“, wenn die Ausfahrt deutlich auf die Mitte zuführt; bleibt die Straße im Ortsteil, der Straßenname', () => {
  const { signs } = cross({ center: [0, -40000], nLen: 15000 });
  const fromSouth = signs.find((s) => Math.abs(s.angle + Math.PI / 2) < 0.1);
  assert.deepEqual(fromSouth.rows.find((r) => Math.abs(r.turn) < 0.1).dests, ['Zentrum', 'Nordkiez']);
  assert.ok(!fromSouth.rows.find((r) => r.turn < -1).dests.includes('Zentrum'), 'nach Westen nicht');
  const short = cross({ len: 1500 }).signs; // 150 m: kein neuer Ortsteil erreicht
  assert.ok(short.every((s) => s.rows.every((r) => r.dests.every((d) => d === 'A-Straße' || d === 'B-Straße'))));
});

test('Kreisel: alle Knoten des Rings sind eine Kreuzung', () => {
  const r = 400, vertices = [r, 0, 0, r, -r, 0, 0, -r, 6000, 0, 0, 6000, -6000, 0, 0, -6000];
  const E = (a, b, n, id, extra = {}) => ({ a, b, c: 3, w: 80, n, o: 0, p: [], id, in: 1, x: [], ...extra });
  const edges = [0, 1, 2, 3].map((i) => E(i, (i + 1) % 4, 2, 200 + i, { o: 1, rb: 1 }))
    .concat([E(0, 4, 0, 301), E(1, 5, 1, 302), E(2, 6, 0, 303), E(3, 7, 1, 304)]);
  const districtAt = (x, y) => (x < -500 ? 'W' : x > 500 ? 'O' : y < -500 ? 'N' : y > 500 ? 'S' : 'M');
  const { signs, stats } = buildSigns({ edges, vertices, names: ['A', 'B', 'Platz'], S, districtAt });
  assert.equal(stats.junctions, 1);
  assert.equal(signs.length, 4);
  for (const s of signs) assert.equal(s.rows.length, 3);
});

test('Schild in einem Haus oder ohne Platz: im HUD ja, in der Welt nicht', () => {
  const vertices = [0, 0, 6000, 0, -6000, 0, 0, -6000, 0, 6000];
  const E = (a, b, n, id) => ({ a, b, c: 3, w: 120, n, o: 0, p: [], id, in: 1, x: [] });
  const edges = [E(0, 1, 0, 1), E(2, 0, 0, 2), E(0, 3, 1, 3), E(0, 4, 1, 4)];
  const { signs, stats } = buildSigns({ edges, vertices, names: ['A', 'B'], S, districtAt: (x) => (x > 500 ? 'O' : x < -500 ? 'W' : 'M'), inBuilding: () => true });
  assert.ok(signs.length > 0 && signs.every((s) => !s.vis));
  assert.equal(stats.hidden, signs.length);
});

test('Echte Karte: Kottbusser Tor weist nach Neukölln (Süden) und Richtung Zentrum; kein Schild auf der Fahrbahn oder im Haus', () => {
  const city = realCity(), all = city.list('sign');
  assert.ok(realIndex().meta.signs.junctions > 500, 'Hunderte beschilderte Kreuzungen in Berlin');
  assert.ok(all.length > 100, `${all.length} Schilder im Kerngebiet`);
  const kotti = all.filter((s) => s.name === 'Kottbusser Tor');
  assert.ok(kotti.length >= 3, `${kotti.length} Schilder am Kotti`);
  const rows = kotti.flatMap((s) => s.rows);
  const south = rows.find((r) => r.dests.includes('Neukölln'));
  assert.ok(south && Math.abs(south.dir - Math.PI / 2) < 0.6, 'Neukölln liegt im Süden (Kottbusser Damm)');
  assert.ok(rows.some((r) => r.dests.includes('Zentrum')), 'eine Ausfahrt Richtung Zentrum');
  let bad = 0;
  for (const s of all) if (s.vis && (onRoad(city, s.x, s.y) || inBuilding(city, s.x, s.y))) bad++;
  assert.equal(bad, 0, `${bad} Schilder auf der Fahrbahn oder im Haus`);
  for (const s of all) for (const r of s.rows) { assert.ok(r.dests.length >= 1 && r.dests.length <= 3); assert.ok(Number.isFinite(r.dir)); }
});

test('Kachelformat und Straßennamen-Erkennung', () => {
  const names = ['Kotti', 'Neukölln;Zentrum', 'B 96'];
  const s = decodeSign([10, 20, 1571, 0, 1, [[3142, -1571, 1, 2], [0, 0, 1, -1]]], (i) => names[i]);
  assert.equal(s.name, 'Kotti'); assert.ok(s.vis); assert.equal(s.layer, 'sign');
  assert.deepEqual(s.rows[0].dests, ['Neukölln', 'Zentrum']); assert.equal(s.rows[0].ref, 'B 96'); assert.equal(s.rows[1].ref, '');
  assert.ok(Math.abs(s.angle - Math.PI / 2) < 0.01);
  assert.ok(isStreet('Adalbertstraße') && isStreet('Kottbusser Damm') && isStreet('Mehringplatz'));
  assert.ok(!isStreet('Neukölln') && !isStreet('Zentrum'));
});

test('Im Bild: Schilder am Kotti gezeichnet, Tafel als einmal gemaltes Bild, sie reicht von der Fahrbahn weg', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  globalThis.OffscreenCanvas ??= class { constructor(w, h) { this.width = w; this.height = h; } getContext() { return new Proxy({}, { get: (t, k) => (k === 'measureText' ? () => ({ width: 30 }) : k === 'createRadialGradient' || k === 'createLinearGradient' ? () => ({ addColorStop() {} }) : () => {}) }); } };
  const { Renderer, signBoard, signBoardX } = await import('../web/src/render.js');
  const { createWorld } = await import('../web/src/world.js');
  const city = realCity(), w = createWorld({ city, cars: 0, pedestrians: 0 });
  const sg = city.list('sign').find((s) => s.name === 'Kottbusser Tor' && s.vis);
  w.camera.x = sg.x; w.camera.y = sg.y; w.player.x = sg.x; w.player.y = sg.y + 200;
  let boards = 0;
  const ctx = new Proxy({ lineWidth: 1 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'drawImage') return (img) => { if (img === sg._board?.c) boards++; };
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'getLineDash') return () => [];
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const r = new Renderer(ctx);
  r.draw(w, 1280, 720, 1.2);
  assert.ok(r.stats.signs >= 1, `${r.stats.signs} Schilder im Bild`);
  assert.equal(boards, 1, 'Tafel als Bild gestempelt');
  const b = signBoard(sg);
  assert.equal(signBoard(sg), b, 'einmal gemalt, dann zwischengespeichert');
  // Tafel reicht vom Pfosten weg von der Straße: Fahrt nach Norden → Gehweg rechts (Osten) → Tafel nach Osten
  assert.equal(signBoardX({ x: 100, angle: -Math.PI / 2 }, 80), 94);
  assert.equal(signBoardX({ x: 100, angle: Math.PI / 2 }, 80), 26);
  assert.equal(signBoardX({ x: 100, angle: 0 }, 80), 60);
});

test('Jedes Ziel nur einmal je Schild: die wichtigste Zeile bekommt es, sonst Straßenname, sonst weg', async () => {
  const { dedupeRows } = await import('../tools/osm/signs.mjs');
  const rows = [
    { turn: 0.9, dests: ['Neukölln'], street: 'Kottbusser Damm' },
    { turn: 0.05, dests: ['Neukölln', 'Britz'], street: 'Karl-Marx-Straße' },
    { turn: -1.5, dests: ['Zentrum', 'Mitte'], street: 'Skalitzer Straße' },
    { turn: -1.4, dests: ['Zentrum'], street: 'Skalitzer Straße' },
    { turn: 1.4, dests: ['Treptow'], street: 'X', osm: 1 },
    { turn: 1.2, dests: ['Treptow'], street: 'X' },
  ];
  dedupeRows(rows);
  // geradeaus (0,05) bekommt Neukölln, die schärfere Rechte fällt auf ihren Straßennamen zurück; die flachere Linke
  // (−1,4) bekommt Zentrum, die andere behält Mitte; OSM-Beschilderung geht vor, die zweite Treptow-Zeile nennt ihre Straße
  assert.deepEqual(rows.map((r) => r.dests), [['Kottbusser Damm'], ['Neukölln', 'Britz'], ['Mitte'], ['Zentrum'], ['Treptow'], ['X']]);
  const two = [{ turn: 0, dests: ['A'], street: 'S' }, { turn: 0.1, dests: ['A'], street: 'S' }];
  dedupeRows(two);
  assert.deepEqual(two.map((r) => r.dests), [['A'], ['S']]);
  const three = [{ turn: 0, dests: ['A'], street: 'S' }, { turn: 0.1, dests: ['A'], street: 'S' }, { turn: 0.2, dests: ['A'], street: 'S' }];
  dedupeRows(three);
  assert.equal(three.length, 2, 'nichts mehr übrig: Zeile entfällt');
  const all = realCity().list('sign');
  let dup = 0;
  for (const s of all) { const d = s.rows.flatMap((r) => r.dests); if (new Set(d).size !== d.length) dup++; }
  assert.equal(dup, 0, `${dup} Schilder nennen ein Ziel doppelt`);
  assert.ok(all.every((s) => s.rows.length >= 2), 'mindestens zwei Zeilen');
});

test('Verdeckte Wegweiser: Silhouette mit Beschriftung (die Tafel scheint durch)', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  globalThis.OffscreenCanvas ??= class { constructor(w, h) { this.width = w; this.height = h; } getContext() { return new Proxy({}, { get: (t, k) => (k === 'measureText' ? () => ({ width: 30 }) : () => {}) }); } };
  const { Renderer } = await import('../web/src/render.js');
  const { createWorld } = await import('../web/src/world.js');
  const city = realCity(), w = createWorld({ city, cars: 0, pedestrians: 0 });
  const ctx = new Proxy({ lineWidth: 1 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'getLineDash') return () => [];
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const r = new Renderer(ctx);
  // unter den Schildern im Kerngebiet ein verdecktes suchen (Baumkrone oder Haus davor)
  let found = null;
  for (const sg of city.list('sign').filter((s) => s.vis).slice(0, 400)) {
    w.camera.x = sg.x; w.camera.y = sg.y; w.player.x = sg.x + 500; w.player.y = sg.y + 300;
    r.draw(w, 1280, 720, 1.2);
    found = r._covered.find((c) => c.board && c.board.b === sg._board);
    if (found) break;
  }
  assert.ok(found, 'ein verdecktes Schild gefunden');
  assert.ok(found.occ.length > 0 && found.board.w > 0);
});
