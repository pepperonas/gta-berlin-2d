import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorld, updateWorld, playerCar } from '../web/src/world.js';
import { MISSION } from '../web/src/config.js';
import { idle, route, driveTo, followRoute } from './helpers/bot.js';
import { realCity } from './helpers/city.js';

const city = realCity();

const DT = 1 / 60;
function step(w, patch = {}) { updateWorld(w, { ...idle(), ...patch }, DT); }

function walkTo(w, t, maxSecs = 20) {
  for (let i = 0; i < maxSecs * 60; i++) {
    const dx = t.x - w.player.x, dy = t.y - w.player.y, d = Math.hypot(dx, dy);
    if (d < 6) return true;
    step(w, { moveX: dx / d, moveY: dy / d });
  }
  return false;
}

function acceptMission(w) {
  assert.ok(walkTo(w, w.city.places.giver), 'Auftraggeber nicht erreicht');
  step(w);
  assert.equal(w.mission.prompt, 'A: Auftrag annehmen');
  step(w, { action: true });
  assert.equal(w.mission.state, 'briefing');
  step(w); step(w, { action: true });
  assert.equal(w.mission.state, 'toPickup');
}

function enterOwnCar(w) {
  const car = w.cars.find((c) => c.id === w.playerCarId);
  assert.ok(walkTo(w, { x: car.x - 30, y: car.y }), 'Auto nicht erreicht');
  step(w, { enterExit: true });
  assert.equal(w.player.inCar, car.id);
}

// Fährt über Kreuzungen zum Ziel und hält in der Zone an.
function driveRoute(w, target, maxSecs = 600) {
  const pts = [...route(w.city, playerCar(w), target), target], state = {};
  for (let i = 0; i < maxSecs * 60; i++) {
    const inp = idle();
    if (followRoute(w, pts, inp, state)) return true;
    updateWorld(w, inp, DT);
  }
  return false;
}

test('kompletter Missionsablauf: annehmen → abholen → abliefern → Erfolg', () => {
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  acceptMission(w);
  enterOwnCar(w);
  const car = playerCar(w);
  assert.ok(driveRoute(w, w.city.places.pickup), `Lagerhalle nicht erreicht (${w.mission.state}, ${w.mission.timer.toFixed(0)} s übrig)`);
  assert.equal(w.mission.prompt, 'A gedrückt halten: Kisten einladen');
  for (let i = 0; i < (MISSION.loadTime + 0.2) * 60; i++) step(w, { actionHeld: true });
  assert.equal(w.mission.state, 'toDropoff');
  assert.ok(car.cargo);
  assert.ok(driveRoute(w, w.city.places.dropoff), `Abgabeort nicht erreicht (${w.mission.state}, ${w.mission.timer.toFixed(0)} s übrig, ${Math.round(Math.hypot(car.x - w.city.places.dropoff.x, car.y - w.city.places.dropoff.y))} px entfernt)`);
  step(w);
  assert.equal(w.mission.prompt, 'A: Kisten abliefern');
  step(w, { action: true });
  assert.equal(w.mission.state, 'success');
  assert.ok(w.mission.result.reward >= 100);
  assert.ok(w.money > 0 && w.completed === 1 && w.bestTime > 0);
  assert.ok(w.mission.result.time < city.timeLimit, `Zeit ${w.mission.result.time.toFixed(1)} s`);
  console.log(`# Bot-Missionszeit ${w.mission.result.time.toFixed(1)} s von ${city.timeLimit} s, Belohnung ${w.mission.result.reward} €`);
});

test('Einladen scheitert zu Fuß und bei zu hoher Geschwindigkeit', () => {
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  acceptMission(w);
  w.player.x = w.city.places.pickup.x; w.player.y = w.city.places.pickup.y;
  step(w, { actionHeld: true });
  assert.match(w.mission.prompt, /Auto/);
  const car = w.cars.find((c) => c.id === w.playerCarId);
  Object.assign(car, { x: w.city.places.pickup.x, y: w.city.places.pickup.y, angle: 0, vx: 200, vy: 0 });
  w.player.inCar = car.id; car.driver = 'player';
  step(w, { actionHeld: true, throttle: 1 });
  assert.equal(w.mission.prompt, 'Anhalten zum Einladen');
  assert.equal(w.mission.load, 0);
});

test('Zeit läuft ab → gescheitert', () => {
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  acceptMission(w);
  for (let i = 0; i < (city.timeLimit + 1) * 60; i++) step(w);
  assert.equal(w.mission.state, 'failed');
  assert.match(w.mission.result.reason, /Zeit/);
});

test('Wrack mit Ware → gescheitert', () => {
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  acceptMission(w);
  enterOwnCar(w);
  const car = playerCar(w);
  Object.assign(car, { x: w.city.places.pickup.x, y: w.city.places.pickup.y, vx: 0, vy: 0 });
  for (let i = 0; i < (MISSION.loadTime + 0.2) * 60; i++) step(w, { actionHeld: true });
  assert.equal(w.mission.state, 'toDropoff');
  car.health = 0; car.wrecked = true;
  step(w);
  assert.equal(w.mission.state, 'failed');
  assert.match(w.mission.result.reason, /Schrott/);
});

test('Aussteigen und wieder Einsteigen; Zielpfeil zeigt dann auf das Auto mit der Ware', async () => {
  const { missionObjective } = await import('../web/src/mission.js');
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  acceptMission(w);
  enterOwnCar(w);
  const car = playerCar(w);
  Object.assign(car, { x: w.city.places.pickup.x, y: w.city.places.pickup.y, vx: 0, vy: 0 });
  for (let i = 0; i < (MISSION.loadTime + 0.2) * 60; i++) step(w, { actionHeld: true });
  step(w, { enterExit: true });
  assert.equal(w.player.inCar, null);
  const o = missionObjective(w.mission, { places: w.city.places, player: w.player, cars: w.cars });
  assert.match(o.text, /Wagen/);
  assert.equal(o.target.x, car.x);
});
