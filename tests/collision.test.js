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

test('Wandsegmente: Kreis und Box werden zur richtigen Seite hinausgeschoben', async () => {
  const { circleVsSegment, obbVsSegment } = await import('../web/src/collision.js');
  const wall = { ax: 0, ay: 0, bx: 100, by: 0 };
  const m = circleVsSegment(50, 4, 7, wall);
  assert.ok(m && m.ny > 0.99 && Math.abs(m.depth - 3) < 1e-9);
  assert.equal(circleVsSegment(50, 8, 7, wall), null);
  assert.ok(circleVsSegment(50, -4, 7, wall).ny < -0.99, 'von der anderen Seite zurück');
  const car = { x: 50, y: 8, angle: 0, hw: 21, hh: 10 };
  const b = obbVsSegment(car, wall);
  assert.ok(b && b.ny > 0.99 && Math.abs(b.depth - 2) < 1e-9);
  assert.equal(obbVsSegment({ ...car, y: 12 }, wall), null);
  // schräg stehende Box am Wandende
  assert.ok(obbVsSegment({ x: 115, y: 0, angle: 0.5, hw: 21, hh: 10 }, wall));
});

test('Raster-Hash: ein Objekt darf in mehreren Hashes stehen', async () => {
  const { SpatialHash } = await import('../web/src/collision.js');
  const a = new SpatialHash(50), b = new SpatialHash(50), it = { id: 1 };
  a.insert(it, { x: 0, y: 0, w: 10, h: 10 }); b.insert(it, { x: 0, y: 0, w: 10, h: 10 });
  assert.equal(a.query({ x: 0, y: 0, w: 5, h: 5 }).length, 1);
  assert.equal(b.query({ x: 0, y: 0, w: 5, h: 5 }).length, 1);
  assert.equal(a.query({ x: 0, y: 0, w: 200, h: 200 }).length, 1, 'keine Doppelten über Zellen');
});
