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

import { tractionOf, TRACTION, AQUA } from '../web/src/traction.js';

const brakeDist = (traction) => {
  const c = createCar({ x: 0, y: 0 }); c.traction = traction; c.vx = 50 / 0.36; c.vy = 0; c.angle = 0;
  c.controls.brake = 1;
  let x0 = c.x; for (let i = 0; i < 600 && Math.hypot(c.vx, c.vy) > 1; i++) stepCar(c, 1 / 60, null);
  return c.x - x0;
};

test('Bremsweg aus 50 km/h: trocken : nass : Schnee : Glätte ≈ 1 : 1,3 : 2 : 3', () => {
  const d0 = brakeDist(undefined), dw = brakeDist(tractionOf({ wet: 1 })), ds = brakeDist(tractionOf({ snow: 1 })), di = brakeDist(tractionOf({ ice: 1 }));
  assert.ok(Math.abs(dw / d0 - 1.3) < 0.15, `nass ${(dw / d0).toFixed(2)}`);
  assert.ok(Math.abs(ds / d0 - 2) < 0.25, `Schnee ${(ds / d0).toFixed(2)}`);
  assert.ok(Math.abs(di / d0 - 3) < 0.4, `Glätte ${(di / d0).toFixed(2)}`);
  assert.equal(brakeDist({ brake: 1, accel: 1, lat: 1, steer: 1 }), d0, 'trocken = ohne Wetter');
});

test('Anfahren auf Glätte begrenzt und als Durchdrehen markiert; Lenkung schwächer', () => {
  const run = (traction) => { const c = createCar({ x: 0, y: 0 }); c.traction = traction; c.controls.throttle = 1; for (let i = 0; i < 60; i++) stepCar(c, 1 / 60, null); return c; };
  const dry = run(undefined), ice = run(tractionOf({ ice: 1 }));
  assert.ok(Math.hypot(ice.vx, ice.vy) < Math.hypot(dry.vx, dry.vy) * 0.5, 'langsamer');
  assert.equal(ice.spin, 1); assert.equal(dry.spin ?? 0, 0);
  const turn = (traction) => { const c = createCar({ x: 0, y: 0 }); c.traction = traction; c.vx = 150; c.controls.steer = 1; c.controls.throttle = 0.3; for (let i = 0; i < 30; i++) stepCar(c, 1 / 60, null); return Math.abs(c.angle); };
  assert.ok(turn(tractionOf({ ice: 1 })) < turn(undefined), 'lenkt weniger ein');
});

test('Aquaplaning: Seitenhalt und Lenkung fast weg, solange car.aqua läuft', () => {
  const slide = (aqua) => { const c = createCar({ x: 0, y: 0 }); c.vx = 250; c.vy = 150; c.aqua = aqua; for (let i = 0; i < 12; i++) stepCar(c, 1 / 60, null); return Math.abs(-c.vx * Math.sin(c.angle) + c.vy * Math.cos(c.angle)); };
  assert.ok(slide(AQUA.time) > slide(0) * 3, `${slide(AQUA.time)} vs ${slide(0)}`);
  const c = createCar({ x: 0, y: 0 }); c.aqua = 0.1; stepCar(c, 1 / 60, null);
  assert.ok(Math.abs(c.aqua - (0.1 - 1 / 60)) < 1e-9, 'läuft ab');
});
