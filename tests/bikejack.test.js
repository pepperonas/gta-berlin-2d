// Radfahrer als Ziel und Fahrzeug: abschießen, umhauen, kapern (F/Y oder Doppelklick), selbst Rad fahren, absteigen.
import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { createBike, bikeSpawn } from '../web/src/bikes.js';
import { castRay, shoot, strike, clickIntent, WEAPONS, KICK } from '../web/src/combat.js';
import { KINDS, isBikeKind } from '../web/src/fleet.js';
import { speedOf } from '../web/src/car.js';
import { createStats, createTracker, trackStep } from '../web/src/stats.js';

const city = realCity();
const PISTOL = WEAPONS.find((q) => q.id === 'pistol');

function withBike(kind = 'bike') {
  const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle';
  let sp = null;
  for (let r = 300; !sp && r < 4000; r += 300) sp = bikeSpawn(city, w.rng, city.places.giver.x, city.places.giver.y, 0, r);
  const b = createBike(city, sp.lane, sp.s, w.rng, kind);
  b.lvl = 0; w.bikes.push(b);
  // Spielfigur 60 px seitlich neben dem Rad (quer zur Fahrtrichtung), steht
  w.player.x = b.x - Math.sin(b.angle) * 60; w.player.y = b.y + Math.cos(b.angle) * 60; w.player.inCar = null;
  w.camera.x = b.x; w.camera.y = b.y;
  return { w, b };
}

test('Schuss auf einen Radfahrer: Strahl trifft das Rad, der Fahrer stürzt (Treffer als Person), das Rad bleibt liegen', () => {
  const { w, b } = withBike();
  const ang = Math.atan2(b.y - w.player.y, b.x - w.player.x);
  const r = castRay(w, w.player.x, w.player.y, ang, 400, null, 0);
  assert.equal(r.hit?.type, 'bike', 'Strahl trifft den Radfahrer');
  const peds = w.peds.length;
  w.events.length = 0;
  shoot(w, w.player, PISTOL, ang, () => 0.5, 0);
  assert.equal(b.state, 'lying', 'Rad liegt');
  assert.equal(w.peds.length, peds + 1, 'Fahrer ist jetzt ein Mensch am Boden');
  const rider = w.peds.at(-1);
  assert.ok(rider.state === 'down' || rider.state === 'dead', `Fahrer gestürzt (${rider.state})`);
  assert.ok(rider.hp < 100, 'Treffer zählt');
  assert.ok(w.events.some((e) => e.type === 'bike-down' && e.player), 'Ereignis für die Statistik');
  const S = [createStats(), createStats()];
  trackStep(S, createTracker(), w, w.events, 0);
  assert.equal(S[0].cyclistsDown, 1);
  assert.equal(castRay(w, w.player.x, w.player.y, ang, 400, null, 0).hit?.type !== 'bike', true, 'liegendes Rad fängt keine Kugeln');
});

test('Schlag oder Tritt holt den Radfahrer vom Rad', () => {
  const { w, b } = withBike();
  w.player.x = b.x - Math.cos(b.angle) * 14; w.player.y = b.y - Math.sin(b.angle) * 14; // direkt dahinter
  const ang = Math.atan2(b.y - w.player.y, b.x - w.player.x);
  assert.ok(strike(w, w.player, KICK, ang) >= 1);
  assert.equal(b.state, 'lying');
});

test('Kapern: Y/F neben dem Rad zieht den Fahrer herunter, man fährt selbst (leise, ≈ 26 km/h), absteigen und wieder aufsteigen', () => {
  const { w, b } = withBike();
  b.speed = 30;
  w.player.x = b.x - Math.sin(b.angle) * 20; w.player.y = b.y + Math.cos(b.angle) * 20;
  const peds = w.peds.length;
  updateWorld(w, { ...idle(), enterExit: true }, 1 / 60);
  const car = w.cars.find((c) => c.id === w.player.inCar);
  assert.ok(car && car.kind === 'bicycle' && isBikeKind(car.kind), 'sitzt auf dem Rad');
  assert.ok(!w.bikes.includes(b), 'aus dem Radverkehr genommen');
  assert.equal(w.peds.length, peds + 1, 'der Fahrer steht jetzt daneben');
  assert.ok(w.events.some((e) => e.type === 'carjack' && e.bike), 'als Kapern gezählt');
  assert.notEqual(w.playerCarId, car.id, 'wird nicht zum Auftragsauto');
  // fahren: Vollgas 6 s
  for (let i = 0; i < 360; i++) updateWorld(w, { ...idle(), throttle: 1 }, 1 / 60);
  const v = speedOf(car);
  assert.ok(v > 45 && v <= KINDS.bicycle.top + 1, `Radtempo ${(v * 0.36).toFixed(0)} km/h`);
  // absteigen: Rad bleibt stehen, ohne Fahrer
  for (let i = 0; i < 120; i++) updateWorld(w, { ...idle(), brake: 1 }, 1 / 60);
  updateWorld(w, { ...idle(), enterExit: true }, 1 / 60);
  assert.equal(w.player.inCar, null, 'abgestiegen');
  assert.equal(car.driver, null);
  updateWorld(w, { ...idle(), enterExit: true }, 1 / 60);
  assert.equal(w.player.inCar, car.id, 'wieder aufgestiegen');
});

test('E-Roller wird zum E-Roller; liegendes Rad lässt sich aufheben', () => {
  const { w, b } = withBike('scooter');
  w.player.x = b.x - Math.sin(b.angle) * 20; w.player.y = b.y + Math.cos(b.angle) * 20;
  updateWorld(w, { ...idle(), enterExit: true }, 1 / 60);
  assert.equal(w.cars.find((c) => c.id === w.player.inCar)?.kind, 'escooter');
  const r2 = withBike();
  r2.b.state = 'lying'; r2.b.speed = 0;
  r2.w.player.x = r2.b.x + 15; r2.w.player.y = r2.b.y;
  const peds = r2.w.peds.length;
  updateWorld(r2.w, { ...idle(), enterExit: true }, 1 / 60);
  assert.equal(r2.w.cars.find((c) => c.id === r2.w.player.inCar)?.kind, 'bicycle', 'aufgehoben');
  assert.equal(r2.w.peds.length, peds, 'niemand wird heruntergezogen');
});

test('Maus (Diablo): Klick auf Radfahrer greift an, Doppelklick kapert; liegendes Rad: weit weg hinlaufen, nah aufsteigen', () => {
  const { w, b } = withBike();
  assert.equal(clickIntent(w, b.x, b.y).kind, 'attack');
  assert.equal(clickIntent(w, b.x, b.y, false, { double: true }).kind, 'enter');
  b.state = 'lying';
  w.player.x = b.x + 300; w.player.y = b.y;
  assert.equal(clickIntent(w, b.x, b.y).kind, 'approach');
  w.player.x = b.x + 30;
  assert.equal(clickIntent(w, b.x, b.y).kind, 'enter');
  // Klick aufs liegende Rad daneben: aufsteigen
  updateWorld(w, { ...idle(), clickWorld: { x: b.x, y: b.y }, clickPressed: true }, 1 / 60);
  for (let i = 0; i < 120 && !w.player.inCar; i++) updateWorld(w, idle(), 1 / 60);
  assert.equal(w.cars.find((c) => c.id === w.player.inCar)?.kind, 'bicycle');
});
