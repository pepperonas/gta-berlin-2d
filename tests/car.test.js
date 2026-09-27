import test from 'node:test';
import assert from 'node:assert/strict';
import { createCar, stepCar, forwardSpeed, collideCarWorld, collideCars } from '../web/src/car.js';
import { SpatialHash } from '../web/src/collision.js';
import { CAR } from '../web/src/config.js';

const run = (car, secs, ctl) => { Object.assign(car.controls, ctl); for (let i = 0; i < secs * 60; i++) stepCar(car, 1 / 60, null); };

test('beschleunigt bis nahe Höchstgeschwindigkeit, nicht darüber', () => {
  const c = createCar({ x: 0, y: 0 });
  run(c, 12, { throttle: 1 });
  const v = forwardSpeed(c);
  assert.ok(v > CAR.maxSpeed * 0.7, `v=${v}`);
  assert.ok(v <= CAR.maxSpeed + 1);
});

test('Bremse stoppt, weiter gedrückt fährt rückwärts (begrenzt)', () => {
  const c = createCar({ x: 0, y: 0 });
  run(c, 3, { throttle: 1 });
  run(c, 1.2, { throttle: 0, brake: 1 });
  assert.ok(forwardSpeed(c) <= 0.5);
  run(c, 6, { brake: 1 });
  assert.ok(forwardSpeed(c) < -50 && forwardSpeed(c) >= -CAR.maxReverse - 1);
});

test('lenkt nur in Fahrt; Rückwärts lenkt spiegelverkehrt', () => {
  const c = createCar({ x: 0, y: 0 });
  run(c, 1, { steer: 1 });
  assert.equal(c.angle, 0);
  run(c, 1.5, { throttle: 1, steer: 1 });
  assert.ok(c.angle > 0.3);
  const r = createCar({ x: 0, y: 0 });
  run(r, 2, { brake: 1 });
  run(r, 1, { brake: 1, steer: 1 });
  assert.ok(r.angle < 0);
});

test('Aufprall auf Wand: herausgeschoben, abgebremst, beschädigt', () => {
  const wall = { x: 100, y: -200, w: 50, h: 400 };
  const solids = new SpatialHash(64); solids.insert(wall, wall);
  const c = createCar({ x: 0, y: 0 });
  c.vx = 300;
  const events = [];
  for (let i = 0; i < 60; i++) { stepCar(c, 1 / 60, null); collideCarWorld(c, { solids }, events); }
  assert.ok(c.x + c.hw <= wall.x + 0.5, `x=${c.x}`);
  assert.ok(c.vx < 150);
  assert.ok(c.health < CAR.health);
  assert.ok(events.some((e) => e.type === 'crash'));
});

test('Totalschaden macht das Auto zum Wrack, das nicht mehr fährt', () => {
  const c = createCar({ x: 0, y: 0 });
  c.health = 1; c.vx = 400;
  const wall = { x: 60, y: -100, w: 20, h: 200 };
  const solids = new SpatialHash(64); solids.insert(wall, wall);
  const ev = [];
  for (let i = 0; i < 30; i++) { stepCar(c, 1 / 60, null); collideCarWorld(c, { solids }, ev); }
  assert.ok(c.wrecked);
  c.x = 0; c.vx = 0; c.controls.throttle = 1;
  for (let i = 0; i < 60; i++) stepCar(c, 1 / 60, null);
  assert.ok(Math.abs(c.x) < 1);
});

test('Auto-Auto-Stoß: beide getrennt, Impuls übertragen', () => {
  const a = createCar({ x: 0, y: 0 }), b = createCar({ x: 38, y: 0 });
  a.vx = 200;
  collideCars(a, b, []);
  assert.ok(b.vx > 50 && a.vx < 150);
  assert.ok(b.x - a.x >= 41.9);
});
