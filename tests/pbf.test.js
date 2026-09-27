import test from 'node:test';
import assert from 'node:assert/strict';
import { readPbf, writePbf } from '../tools/osm/pbf.mjs';
import { storeFromPbf } from '../tools/osm/store.mjs';

const elements = [
  { type: 'node', id: 5, lat: 52.5, lon: 13.4 },
  { type: 'node', id: 7, lat: 52.5001234, lon: 13.4005678, tags: { natural: 'tree' } },
  { type: 'node', id: 9, lat: 52.4999, lon: 13.3999, tags: { amenity: 'bar', name: 'Kneipe „Zur Ecke“' } },
  { type: 'node', id: 11000000001, lat: 52.51, lon: 13.41 }, // ID über 2^32
  { type: 'way', id: 100, nodes: [5, 7, 11000000001], tags: { highway: 'residential', name: 'Teststraße' } },
  { type: 'way', id: 101, nodes: [5, 9], tags: {} }, // ungetaggt, aber Mitglied einer Relation
  { type: 'way', id: 102, nodes: [7, 9] },            // ungetaggt, ohne Nutzen → verworfen
  { type: 'relation', id: 500, tags: { type: 'restriction', restriction: 'no_left_turn' }, members: [{ type: 'way', ref: 100, role: 'from' }, { type: 'node', ref: 5, role: 'via' }, { type: 'way', ref: 101, role: 'to' }] },
];

test('PBF-Leser: dichte Knoten, Tags, Wege, Relationen, große IDs, Stand der Daten', () => {
  for (const zlib of [true, false]) {
    const buf = writePbf(elements, { timestamp: '2026-09-26T20:22:51Z', zlib });
    const blocks = [...readPbf(buf)];
    assert.equal(blocks[0].header.timestamp, '2026-09-26T20:22:51Z');
    const nodes = blocks.flatMap((b) => b.nodes ?? []), ways = blocks.flatMap((b) => b.ways ?? []), rels = blocks.flatMap((b) => b.relations ?? []);
    assert.deepEqual(nodes.map((n) => n.id), [5, 7, 9, 11000000001]);
    assert.ok(Math.abs(nodes[1].lat - 52.5001234) < 1e-7 && Math.abs(nodes[1].lon - 13.4005678) < 1e-7);
    assert.equal(nodes[2].tags.name, 'Kneipe „Zur Ecke“');
    assert.equal(nodes[0].tags, undefined);
    assert.deepEqual(ways[0].nodes, [5, 7, 11000000001]);
    assert.equal(ways[0].tags.name, 'Teststraße');
    assert.deepEqual(rels[0].members, elements[7].members);
  }
});

test('OSM-Speicher: Koordinaten per ID, nur nützliche Wege, Relationsmitglieder bleiben', () => {
  const s = storeFromPbf(writePbf(elements));
  assert.deepEqual(s.coord(11000000001).map((v) => +v.toFixed(6)), [52.51, 13.41]);
  assert.equal(s.coord(6), null);
  assert.ok(s.ways.has(100) && s.ways.has(101), 'Straße und Relationsmitglied');
  assert.ok(!s.ways.has(102), 'ungetaggter Weg ohne Relation verworfen');
  assert.deepEqual(s.tagged.map((n) => n.id), [7, 9]);
  assert.equal(s.relations.length, 1);
});
