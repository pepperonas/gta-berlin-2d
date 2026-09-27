import test from 'node:test';
import assert from 'node:assert/strict';
import { circleVsRect, obbVsRect, obbVsObb, circleVsObb, SpatialHash, obbBounds } from '../web/src/collision.js';

const rect = { x: 0, y: 0, w: 100, h: 100 };

test('Kreis gegen Rechteck: außen, Kante, innen', () => {
  assert.equal(circleVsRect(150, 50, 10, rect), null);
  const m = circleVsRect(105, 50, 10, rect);
  assert.deepEqual([m.nx, m.ny], [1, 0]);
  assert.ok(Math.abs(m.depth - 5) < 1e-9);
  const inside = circleVsRect(95, 50, 10, rect);
  assert.equal(inside.nx, 1);
  assert.ok(Math.abs(inside.depth - 15) < 1e-9);
});

test('OBB gegen Rechteck: Normale zeigt vom Hindernis weg', () => {
  const car = { x: 115, y: 50, angle: 0, hw: 21, hh: 10 };
  const m = obbVsRect(car, rect);
  assert.ok(m);
  assert.ok(m.nx > 0.99);
  assert.ok(Math.abs(m.depth - 6) < 1e-6);
  assert.equal(obbVsRect({ ...car, x: 200 }, rect), null);
});

test('gedrehte OBB (45°) berührt Ecke erst später als ihr Umriss vermuten lässt', () => {
  const car = { x: 100 + 18, y: -18, angle: Math.PI / 4, hw: 21, hh: 10 };
  const b = obbBounds(car);
  assert.ok(b.x < 100); // Umriss überlappt
  assert.equal(obbVsRect(car, rect), null); // echte Box nicht
});

test('OBB gegen OBB und Kreis gegen OBB', () => {
  const a = { x: 0, y: 0, angle: 0, hw: 21, hh: 10 };
  const m = obbVsObb(a, { x: 40, y: 0, angle: 0, hw: 21, hh: 10 });
  assert.ok(m && m.nx < -0.99 && Math.abs(m.depth - 2) < 1e-6);
  const c = circleVsObb(0, 15, 7, a);
  assert.ok(c && c.ny > 0.99 && Math.abs(c.depth - 2) < 1e-6);
});

test('SpatialHash liefert jedes Objekt nur einmal', () => {
  const h = new SpatialHash(50);
  const big = { x: 0, y: 0, w: 300, h: 300 };
  h.insert(big, big);
  const out = h.query({ x: 0, y: 0, w: 300, h: 300 });
  assert.equal(out.length, 1);
  assert.equal(h.query({ x: 1000, y: 1000, w: 10, h: 10 }).length, 0);
});
