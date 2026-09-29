// Fahrzeug- und Schrittklang (soundscape.js): Drehzahl und Gänge, Reifen, Stimmen fremder Autos mit Doppler, Schritte.
import test from 'node:test';
import assert from 'node:assert/strict';
import { stepEngine, engineOf, tireState, carVoices, stepsBetween, strideOf, footstepKind, ENGINES } from '../web/src/soundscape.js';
import { createCar } from '../web/src/car.js';

const drive = (kind, speeds, throttle = 1) => {
  const car = createCar({ x: 0, y: 0, kind }), st = {}, out = [];
  car.driver = 'player'; car.controls.throttle = throttle;
  for (const v of speeds) { car.vx = v; for (let i = 0; i < 30; i++) stepEngine(st, car, 1 / 60); out.push({ ...st }); }
  return out;
};

test('Motor: Gänge schalten hoch, Drehzahl bleibt zwischen Leerlauf und Abregelung, Diesel dreht niedriger', () => {
  const speeds = Array.from({ length: 34 }, (_, i) => i * 10);
  const car = drive('car', speeds), truck = drive('truck', speeds);
  const E = engineOf('car');
  assert.equal(car[0].gear, 1);
  assert.ok(car.at(-1).gear >= 5, `Vollgas 330 px/s im ${car.at(-1).gear}. Gang`);
  for (let i = 1; i < car.length; i++) assert.ok(car[i].gear >= car[i - 1].gear, 'beim Beschleunigen nie zurück');
  for (const s of car) assert.ok(s.rpm >= E.idle * 0.9 && s.rpm <= E.red, `Drehzahl ${s.rpm}`);
  assert.ok(truck[20].rpm < car[20].rpm, 'Lkw-Diesel dreht niedriger');
  assert.ok(truck[0].diesel && !car[0].diesel);
  // Zündfrequenz = U/min / 60 · Zylinder / 2
  assert.ok(Math.abs(car[10].fire - car[10].rpm / 60 * 2) < 1e-9);
  // Hochschalten: kurze Lastpause, Drehzahl fällt
  const up = car.findIndex((s, i) => i > 0 && s.gear > car[i - 1].gear);
  assert.ok(up > 0);
  // Stillstand mit Gas: dreht hoch, ohne Gas Leerlauf
  const rev = drive('car', [0, 0, 0], 1).at(-1), idle = drive('car', [0, 0, 0], 0).at(-1);
  assert.ok(rev.rpm > idle.rpm + 300, 'Gas im Stand dreht hoch');
  assert.ok(Math.abs(idle.rpm - E.idle) < 50, 'Leerlauf');
  for (const k of Object.keys(ENGINES)) assert.ok(ENGINES[k].gears.every((g, i, a) => !i || g > a[i - 1]), `${k}: Gänge aufsteigend`);
});

test('Reifen: Quietschen beim Querrutschen auf trockener Straße, auf Schnee nur Rauschen; Fahrtwind wächst mit v²', () => {
  const car = createCar({ x: 0, y: 0 });
  car.vx = 200; car.vy = 150; // schräg zur Fahrzeugachse (angle 0) → Querrutschen
  const dry = tireState({ city: null, snow: 0, wet: 0 }, car), snow = tireState({ city: null, snow: 1, wet: 0 }, car);
  assert.ok(dry.skid > 0.5 && dry.slide < 0.05, 'trocken quietscht');
  assert.ok(snow.skid < 0.05 && snow.slide > 0.5, 'Schnee rauscht nur');
  assert.ok(snow.snow > 0.9, 'Schnee knirscht');
  car.vy = 0;
  const slow = tireState({ city: null }, { ...car, vx: 100 }), fast = tireState({ city: null }, { ...car, vx: 300 });
  assert.equal(slow.skid, 0);
  assert.ok(fast.wind > 8 * slow.wind, 'Fahrtwind ∝ v²');
  assert.ok(tireState({ city: null, wet: 1 }, car).wet > 0.9, 'nasse Straße zischt');
});

test('Fremde Autos: Doppler, Richtung, eigene Fahrt und eigenes Auto', () => {
  const mk = (id, x, vx, kind = 'car') => ({ ...createCar({ x, y: 0, kind }), id, vx, driver: 'npc' });
  const world = { cars: [mk(1, 200, -150), mk(2, -200, -150), mk(3, 50, 0), mk(4, 2000, 0), { ...mk(5, 10, 0), wrecked: true }, mk(6, 0, 0)] };
  const v = carVoices(world, { x: 0, y: 0 }, 4, 500, 6);
  const by = Object.fromEntries(v.map((o) => [o.id, o]));
  assert.ok(by[1].rate > 1, 'kommt von rechts näher: höher');
  assert.ok(by[2].rate < 1, 'fährt links davon: tiefer');
  assert.ok(by[1].pan > 0 && by[2].pan < 0, 'Richtung');
  assert.ok(!by[4] && !by[5] && !by[6], 'zu weit, Wrack und eigenes Auto schweigen');
  assert.ok(v.every((o, i) => !i || o.gain <= v[i - 1].gain), 'lauteste zuerst');
  assert.equal(carVoices(world, { x: 0, y: 0 }, 2).length, 2, 'höchstens n Stimmen');
  // fährt man selbst mit, gibt es keinen Doppler
  const same = carVoices({ cars: [mk(1, 200, 150)] }, { x: 0, y: 0, vx: 150, vy: 0 });
  assert.ok(Math.abs(same[0].rate - 1) < 1e-9);
  // Lkw lauter als Pkw gleicher Lage
  const t = carVoices({ cars: [mk(1, 100, 100, 'truck'), mk(2, -100, 100)] }, { x: 0, y: 0 });
  assert.ok(t[0].id === 1 && t[0].gain > t[1].gain);
});

test('Schritte: Schrittlänge nach Tempo, Zählung über den Wegzähler, Klang nach Untergrund und Wetter', () => {
  assert.ok(strideOf(15) < strideOf(35) && strideOf(35) < strideOf(70));
  assert.equal(stepsBetween(0, 7.4, 15), 0);
  assert.equal(stepsBetween(7.4, 7.6, 15), 1);
  assert.equal(stepsBetween(0, 75, 15), 10);
  assert.equal(stepsBetween(10, 5, 15), 0, 'Zähler zurückgesetzt: nichts');
  assert.equal(stepsBetween(0, 30, 0), 0, 'steht: nichts');
  assert.equal(footstepKind({ city: null, snow: 0.8 }, 0, 0), 'snow');
  assert.equal(footstepKind({ city: null, snow: 0, wet: 0.8 }, 0, 0), 'wet');
  assert.equal(footstepKind({ city: null }, 0, 0), 'hard');
});
