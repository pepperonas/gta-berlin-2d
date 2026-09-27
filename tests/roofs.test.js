import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { roofOf, roofStyle, facadeStyle, roofDecor, mainAxis } from '../web/src/roofs.js';
import { pointInRings } from '../web/src/geom.js';
import { BUILDING_KIND } from '../web/src/citycodes.js';

const city = realCity();
const S = city.scale;
const buildings = city.list('building');

test('Dachaufbauten liegen ganz im Grundriss, überlappen nicht und folgen der Hauptachse', () => {
  let n = 0;
  for (const b of buildings) {
    const ds = roofDecor(b, S), a = mainAxis(b);
    for (const d of ds) {
      n++;
      const ca = Math.cos(d.a), sa = Math.sin(d.a);
      for (const [u, v] of [[1, 1], [-1, 1], [-1, -1], [1, -1]]) {
        const x = d.x + ca * u * d.l / 2 - sa * v * d.w / 2, y = d.y + sa * u * d.l / 2 + ca * v * d.w / 2;
        assert.ok(pointInRings(x, y, b.rings), `${d.t} ragt aus Haus ${b.id}`);
      }
      assert.equal(d.a, a);
    }
    for (let i = 0; i < ds.length; i++) for (let j = i + 1; j < ds.length; j++) {
      const p = ds[i], q = ds[j];
      assert.ok(Math.hypot(p.x - q.x, p.y - q.y) >= (Math.max(p.l, p.w) + Math.max(q.l, q.w)) / 2, `Aufbauten überlappen auf Haus ${b.id}`);
    }
  }
  assert.ok(n > 50000, `${n} Dachaufbauten im Kerngebiet`);
});

test('Dachformen und Fassaden passen zur Gebäudeart', () => {
  const K = BUILDING_KIND, count = {};
  for (const b of buildings) {
    const st = roofStyle(b, S), fa = facadeStyle(b, S);
    count[st] = (count[st] ?? 0) + 1;
    if (b.kind === K.church) assert.equal(st, 'pitched');
    if (b.kind === K.industrial || b.kind === K.warehouse) { assert.equal(st, 'corrugated'); assert.equal(fa, 'industry'); }
    if (st === 'pitched') assert.ok(b.kind === K.church || b.kind === K.small || b.meters <= 9, `Satteldach auf ${b.meters} m hohem Haus`);
    if (st === 'pitched' || b.kind === K.small) assert.deepEqual(roofDecor(b, S), [], 'Satteldächer und Schuppen ohne Aufbauten');
    if (fa === 'platte') assert.ok(b.meters > 24, 'Plattenbau nur bei hohen Häusern');
  }
  for (const st of ['flat', 'berlin', 'pitched', 'corrugated']) assert.ok(count[st] > 100, `${st}: ${count[st]}`);
  assert.ok(count.berlin > count.pitched, 'im Kerngebiet mehr Altbau-Dächer als Satteldächer');
});

test('Dach einmal je Gebäude bestimmt und deterministisch', () => {
  const b = buildings.find((x) => roofDecor(x, S).length > 3);
  const r = roofOf(b, S);
  assert.equal(roofOf(b, S), r, 'zwischengespeichert');
  const again = JSON.stringify(roofDecor(b, S));
  assert.equal(JSON.stringify(r.decor), again);
});
