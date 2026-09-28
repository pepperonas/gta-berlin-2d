import test from 'node:test';
import assert from 'node:assert/strict';
import { vehicleState, transitNear, speedOfPattern, alightSpot, stationExit, RIDE } from '../web/src/ride.js';
import { positionAt, pointOn } from '../web/src/transit.js';
import { createWorld, updateWorld, resetPopulation } from '../web/src/world.js';
import { realCity, realTransit } from './helpers/city.js';
import { idle } from './helpers/bot.js';

const city = realCity(), tr = realTransit();
const m10 = tr.patterns.filter((p) => p.name === 'M10').sort((a, b) => b.stops.length - a.stops.length)[0];

function worldWithTram(tauAt = 'moving') {
  const mid = pointOn(m10, m10.stops[Math.floor(m10.stops.length / 2)]);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60; w.day = 1;
  w.camera.x = mid.x; w.camera.y = mid.y; w.player.x = mid.x; w.player.y = mid.y; resetPopulation(w);
  for (let i = 0; i < 30; i++) updateWorld(w, idle(), 1 / 60);
  const s = w.transit.tracked.get(m10.id);
  let tau = 0; const want = tauAt === 'moving' ? (q) => !q.dwelling : (q) => q.dwelling;
  while (!(positionAt(m10, tau).s > m10.stops[Math.floor(m10.stops.length / 2)] - 400 && want(positionAt(m10, tau)))) tau += 0.5;
  const v = { tau, delay: 0, key: 'test' }; s.veh.push(v);
  return { w, v };
}

test('Fahrzeuglage: Wagen, Tempo aus dem Fahrplan, beim Halten 0; verschwundenes Fahrzeug → null', () => {
  const { w, v } = worldWithTram('moving');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  assert.equal(st.mode, 'tram'); assert.equal(st.cars.length, 3); assert.ok(st.speed > 20, `Tempo ${st.speed}`);
  assert.equal(st.speed, speedOfPattern(m10, v.tau));
  v.gone = true; w.transit.tracked.get(m10.id).veh = w.transit.tracked.get(m10.id).veh.filter((x) => !x.gone);
  assert.equal(vehicleState(w, { pid: m10.id, key: 'test' }), null);
  assert.equal(vehicleState(w, { carId: -1 }), null);
  w.transit.tracked.get(m10.id).veh.push({ tau: m10.duration + 1, delay: 0, key: 'test-end' });
  assert.equal(vehicleState(w, { pid: m10.id, key: 'test-end' }), null, 'Linienende (pos.done) → null');
  let tau = 0; while (!positionAt(m10, tau).dwelling || tau < 30) tau += 0.5;
  assert.equal(speedOfPattern(m10, tau), 0);
});

test('In Reichweite: nur nah am Wagen-Rechteck (25 px), sortiert, mit Wagennummer und Abstand zur Spitze', () => {
  const { w } = worldWithTram('moving');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  const c2 = st.cars[2], nx = -Math.sin(c2.angle), ny = Math.cos(c2.angle);
  const near = transitNear(w, c2.x + nx * (c2.W / 2 + 10), c2.y + ny * (c2.W / 2 + 10), RIDE.reach);
  const hit = near.find((h) => h.ref.key === 'test');
  assert.ok(hit, 'neben Wagen 3'); assert.equal(hit.car, 2); assert.ok(hit.front > 200);
  assert.ok(!transitNear(w, c2.x + nx * (c2.W / 2 + 40), c2.y + ny * (c2.W / 2 + 40), RIDE.reach).some((h) => h.ref.key === 'test'), 'zu weit');
  for (let i = 1; i < near.length; i++) assert.ok(near[i].dist >= near[i - 1].dist);
});

test('Aussteigen: Platz neben dem Wagen, sonst null; Straßenausgang am Bahnhof', () => {
  const { w } = worldWithTram('dwell');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  const sp = alightSpot(w, st, 1);
  assert.ok(sp && Math.hypot(sp.x - st.cars[1].x, sp.y - st.cars[1].y) < 60);
  const u8 = tr.patterns.filter((p) => p.name === 'U8').sort((a, b) => b.stops.length - a.stops.length)[0];
  const i = u8.stopNames.findIndex((n) => n.startsWith('U Kottbusser Tor'));
  const ex = stationExit(w, u8, i), stop = pointOn(u8, u8.stops[i]);
  assert.ok(Math.hypot(ex.x - stop.x, ex.y - stop.y) < 400, 'Ausgang in der Nähe des Bahnsteigs');
});
