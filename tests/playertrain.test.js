import test from 'node:test';
import assert from 'node:assert/strict';
import { takeTrain, trainAhead, turnAround } from '../web/src/playertrain.js';
import { transitNear, vehicleState, RIDE } from '../web/src/ride.js';
import { positionAt, pointOn } from '../web/src/transit.js';
import { createWorld, updateWorld, resetPopulation } from '../web/src/world.js';
import { realCity, realTransit } from './helpers/city.js';
import { idle } from './helpers/bot.js';

const city = realCity(), tr = realTransit();
const pat = (name, mode) => tr.patterns.filter((p) => p.name === name && p.mode === mode).sort((a, b) => b.stops.length - a.stops.length)[0];
const press = (w, patch = {}) => { updateWorld(w, { ...idle(), ...patch }, 1 / 60); w.camera.x = w.player.x; w.camera.y = w.player.y; };

function atFrontOf(p, stopIdx) {
  const at = pointOn(p, p.stops[stopIdx]);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60; w.day = 1;
  w.camera.x = at.x; w.camera.y = at.y; w.player.x = at.x; w.player.y = at.y; resetPopulation(w);
  for (let i = 0; i < 30; i++) press(w);
  const s = w.transit.tracked.get(p.id);
  let tau = 0; while (!(positionAt(p, tau).stop === stopIdx && positionAt(p, tau).dwelling)) tau += 0.5;
  s.veh.push({ tau, delay: 0, key: 'mine' });
  const st = vehicleState(w, { pid: p.id, key: 'mine' }), f = st.cars[0];
  w.player.x = f.x + Math.cos(f.angle) * (f.L / 2 + 10); w.player.y = f.y + Math.sin(f.angle) * (f.L / 2 + 10);
  return w;
}

test('Übernehmen: Y am Führerstand macht aus der Fahrplan-Bahn den Spielerzug – gleiche Lage, Fahrplan-Fahrzeug weg', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  const before = vehicleState(w, { pid: p.id, key: 'mine' });
  press(w, { enterExit: true });
  assert.ok(w.playerTrain, 'Spielerzug'); assert.equal(w.player.ride.kind, 'driver');
  assert.ok(Math.abs(w.playerTrain.s - before.s) < 1);
  assert.equal(vehicleState(w, { pid: p.id, key: 'mine' }), null, 'aus dem Fahrplan genommen');
  // weiter hinten stehend (nicht am Führerstand) übernimmt man nicht
  const w2 = atFrontOf(p, 3), st = vehicleState(w2, { pid: p.id, key: 'mine' }), c = st.cars[2];
  w2.player.x = c.x + 20; w2.player.y = c.y + 20;
  press(w2, { enterExit: true });
  assert.equal(w2.playerTrain ?? null, null);
});

test('Fahren: Gas bewegt entlang der Linie, Türen nur im Stand an der Haltestelle, Trinkgeld beim sauberen Halt', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  // ohne Straßenverkehr: geprüft wird Fahren/Türen/Trinkgeld; Hindernisse auf dem Gleis prüft der Test weiter unten.
  // (Auf der M10 liegt der Linienweg stellenweise in der Gegenspur – entgegenkommende Busse blockieren dort.)
  w.trafficScale = 0;
  const drive = (patch) => { w.cars.length = 0; press(w, patch); };
  drive({ enterExit: true });
  const t = w.playerTrain, s0 = t.s;
  for (let i = 0; i < 60 * 8; i++) drive({ throttle: 1 });
  assert.ok(t.s > s0 + 200, 'fährt'); assert.ok(t.v > 0);
  drive({ action: true });
  assert.equal(t.drive.doors, 'closed', 'Türen nicht in Fahrt');
  // sanft auf die nächste Haltestelle zu
  const target = p.stops[4]; let money = w.money;
  for (let i = 0; i < 60 * 120 && !(t.v === 0 && Math.abs(t.s - target) < 60); i++) {
    const rest = target - t.s, want = Math.sqrt(Math.max(0, 2 * 10 * Math.max(0, rest - 10)));
    drive(t.v > want ? { brake: Math.min(1, (t.v - want) / 20 + 0.3) } : { throttle: rest > 20 ? 0.6 : 0 });
  }
  assert.ok(Math.abs(t.s - target) < 60, `steht am Halt (${(t.s - target).toFixed(0)} px)`);
  drive({ action: true });
  assert.equal(t.drive.doors, 'open');
  assert.ok(w.events.some((e) => e.type === 'tip') && w.money > money, 'Trinkgeld');
  for (let i = 0; i < 60 * 25; i++) drive({ throttle: 1 });
  assert.equal(t.drive.doors, 'closed', 'schließen nach 20 s selbst');
});

test('Zug voraus: der Spielerzug hält davor und fährt nie hinein; Fahrplan-Zug dahinter wartet', () => {
  const p = pat('U1', 'ubahn'), w = atFrontOf(p, 2);
  press(w, { enterExit: true });
  const t = w.playerTrain, s = w.transit.tracked.get(p.id);
  // Fahrplan-Zug 500 px voraus (Spitze), auf demselben Muster
  let tau = 0; while (positionAt(p, tau).s < t.s + 500 + 6 * 165) tau += 0.2;
  const ahead = { tau, delay: 0, key: 'ahead' }; s.veh.push(ahead);
  const ahead0 = positionAt(p, tau).s;
  assert.ok(trainAhead(w, t) < 700);
  for (let i = 0; i < 60 * 30; i++) { ahead.tau = tau; press(w, { throttle: 1 }); }
  const tail = ahead0 - 6 * 165;
  assert.ok(t.s < tail - 50, `Abstand gehalten (${(tail - t.s).toFixed(0)} px)`);
  // Fahrplan-Zug hinter dem Spielerzug holt nicht auf
  let tb = 0; while (positionAt(p, tb).s < t.s - 6 * 165 - 700) tb += 0.2;
  const behind = { tau: tb, delay: 0, key: 'behind' }; s.veh.push(behind);
  for (let i = 0; i < 60 * 60; i++) press(w);
  assert.ok(positionAt(p, behind.tau).s < t.s - 6 * 165 - 400, 'wartet hinter dem Spielerzug');
});

test('Linienende: Zug hält, Wenden auf die Gegenrichtung, sonst nur aussteigen', () => {
  const p = pat('M10', 'tram'), n = p.stops.length, w = atFrontOf(p, n - 2);
  press(w, { enterExit: true });
  const t = w.playerTrain;
  for (let i = 0; i < 60 * 180; i++) press(w, { throttle: 1 });
  assert.ok(t.s <= p.stops[n - 1] + 1 && t.v === 0, 'steht am Endhalt');
  press(w, { action: true }); assert.equal(t.drive.doors, 'open', 'erst Türen');
  press(w, { action: true }); assert.equal(t.drive.doors, 'closed');
  const ok = turnAround(w);
  const back = tr.patterns.find((q) => q.name === p.name && q.id !== p.id && Math.hypot(pointOn(q, q.stops[0]).x - pointOn(p, p.stops[n - 1]).x, pointOn(q, q.stops[0]).y - pointOn(p, p.stops[n - 1]).y) < 600);
  assert.equal(ok, !!back);
  if (back) { assert.equal(w.playerTrain.pid, back.id); assert.ok(Math.abs(w.playerTrain.s - back.stops[0]) < 5); }
});
test('Eigene Straßenbahn hält vor einem Auto auf dem Gleis', async () => {
  const { createCar } = await import('../web/src/car.js');
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  const t = w.playerTrain, q = pointOn(p, t.s + 900);
  const car = createCar({ x: q.x, y: q.y, angle: q.angle }); car.driver = null; w.cars.push(car);
  for (let i = 0; i < 60 * 30; i++) press(w, { throttle: 1 });
  const head = pointOn(p, t.s);
  assert.ok(Math.hypot(head.x - car.x, head.y - car.y) > 30, `Spitze ${Math.hypot(head.x - car.x, head.y - car.y).toFixed(0)} px vor dem Auto`);
  assert.equal(t.v, 0);
});
