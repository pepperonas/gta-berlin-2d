import test from 'node:test';
import assert from 'node:assert/strict';
import { createDrive, stepDrive, stopInfo, tipFor, brakeDistance } from '../web/src/trainphysics.js';
import { TRAIN_DRIVE } from '../web/src/config.js';

const run = (d, input, sec, fps = 60) => { for (let i = 0; i < sec * fps; i++) stepDrive(d, { limit: Infinity, ...input }, 1 / fps); return d; };

test('Zugfahrt: Höchsttempo je Art, nie rückwärts, bildratenunabhängig', () => {
  for (const [mode, kmh] of [['tram', 60], ['ubahn', 70], ['sbahn', 100]]) {
    const d = run(createDrive(mode), { throttle: 1 }, 120);
    assert.ok(Math.abs(d.v * 0.36 - kmh) < 0.5, `${mode} ${d.v * 0.36}`);
  }
  const d = run(createDrive('ubahn', 50), { brake: 1 }, 30);
  assert.equal(d.v, 0);
  const a = run(createDrive('sbahn'), { throttle: 1 }, 10, 30), b = run(createDrive('sbahn'), { throttle: 1 }, 10, 144);
  assert.ok(Math.abs(a.v - b.v) < 1, 'gleich bei 30 und 144 Hz');
  const roll = run(createDrive('tram', 100), {}, 5);
  assert.ok(roll.v < 100 && roll.v > 80, 'rollt aus, langsam');
});

test('Zugfahrt: Bremsweg, Notbremse kürzer, Zwangsbremsung vor Hindernis, steht bei offenen Türen', () => {
  const v0 = 70 / 0.36;
  const bd = (input) => { const d = createDrive('ubahn', v0); let s = 0; while (d.v > 0) { stepDrive(d, { limit: Infinity, ...input }, 1 / 60); s += d.v / 60; } return s; };
  const normal = bd({ brake: 1 }), emergency = bd({ emergency: true });
  assert.ok(Math.abs(normal - brakeDistance(v0, TRAIN_DRIVE.ubahn.brake)) < 25, `Bremsweg ${normal}`);
  assert.ok(emergency < normal * 0.6, 'Notbremse');
  // Hindernis 400 px voraus bei voller Fahrt: steht davor
  const d = createDrive('sbahn', 100 / 0.36); let s = 0;
  for (let i = 0; i < 60 * 30 && (d.v > 0 || i < 10); i++) { stepDrive(d, { throttle: 1, limit: 1800 - s }, 1 / 60); s += d.v / 60; }
  assert.ok(s <= 1800 + 1 && d.v === 0, `hält vor dem Hindernis (${s.toFixed(0)} px)`);
  const o = createDrive('tram'); o.doors = 'open';
  run(o, { throttle: 1 }, 2); assert.equal(o.v, 0);
});

test('Haltestellen und Trinkgeld', () => {
  const p = { stops: [0, 3000, 6000] };
  assert.deepEqual(stopInfo(p, 2900), { i: 1, dist: -100 });
  assert.deepEqual(stopInfo(p, 3120), { i: 1, dist: 120 });
  assert.equal(stopInfo(p, 4500), null);
  assert.equal(tipFor(0, 10), 10); assert.equal(tipFor(29, 12), 10);
  assert.ok(tipFor(120, 10) > 0 && tipFor(120, 10) < 10, 'anteilig');
  assert.equal(tipFor(260, 10), 0);
  assert.ok(tipFor(0, 25) < 5, 'harte Bremsung kostet');
});
