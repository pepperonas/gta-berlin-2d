import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { carModel, carColor, CAR_MODELS, TAXI_COLOR, carSprite, spriteCacheSize, SPRITE_CACHE_MAX } from '../web/src/vehicles.js';
import { createCar } from '../web/src/car.js';
import { personLook } from '../web/src/assets.js';
import { buildLaneGraph, turnAngle } from '../web/src/roadgraph.js';
import { placeOnLane, blinkFor, BLINK_AHEAD } from '../web/src/traffic.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { idle } from './helpers/bot.js';

test('Automodelle: je Auto fest, alle elf kommen vor, Spielerauto ist eine Limousine, Taxis sind elfenbein', () => {
  const cars = Array.from({ length: 1500 }, () => createCar({ x: 0, y: 0, color: '#2e86de' }));
  const seen = new Set(cars.map(carModel));
  for (const m of CAR_MODELS) assert.ok(seen.has(m), `Modell ${m} kommt vor`);
  assert.deepEqual(cars.map(carModel), cars.map(carModel), 'deterministisch');
  const taxis = cars.filter((c) => carModel(c) === 'taxi');
  assert.ok(taxis.length > 50 && taxis.length < 180, `${taxis.length} Taxis von 1500`);
  assert.ok(taxis.every((c) => carColor(c) === TAXI_COLOR));
  assert.equal(carModel(createCar({ x: 0, y: 0, role: 'player' })), 'limousine');
  const w = { ...cars[0], wrecked: true };
  assert.notEqual(carColor(w), cars[0].color, 'Wrack ist ausgebrannt');
});

test('Blinker: vor dem Rechtsabbiegen rechts, vor dem Linksabbiegen links, geradeaus und weit vorher aus', () => {
  const city = realCity(), g = buildLaneGraph(city);
  const pick = (want) => [...g.lanes].find((l) => l.len > 400 && l.next.length === 1 && !l.narrow && (want === 0 ? Math.abs(turnAngle(l, l.next[0])) < 0.2 : want * turnAngle(l, l.next[0]) > 0.9));
  // Auto auf die Spur setzen und einen Fahrschritt laufen lassen (die KI verlängert ihre Route erst beim Fahren)
  const blinkAt = (lane, s) => {
    const w = createWorld({ city, cars: 0, pedestrians: 0 });
    const car = createCar({ x: 0, y: 0 }); car.driver = 'npc';
    placeOnLane(car, city, lane, s, w.rng);
    w.cars.push(car); w.camera.x = car.x; w.camera.y = car.y;
    updateWorld(w, idle(), 1 / 60);
    assert.equal(car.ai.blink, blinkFor(car.ai, car), 'Anzeigefeld folgt blinkFor');
    return car.ai.blink;
  };
  for (const want of [1, -1, 0]) {
    const lane = pick(want);
    assert.ok(lane, `Spur mit ${want} gefunden`);
    assert.equal(blinkAt(lane, lane.len - 60), want, `kurz vor dem Ende: ${want}`);
    assert.equal(blinkAt(lane, Math.max(10, lane.len - BLINK_AHEAD - 150)), 0, 'weit vor der Kreuzung noch nicht');
  }
});

test('Auto-Sprites: Cache bleibt begrenzt; ohne Canvas null (flacher Rückfall)', () => {
  const saved = globalThis.OffscreenCanvas;
  delete globalThis.OffscreenCanvas;
  assert.equal(carSprite('kombi', '#123456', 42, 20, false), null, 'ohne Canvas kein Sprite');
  globalThis.OffscreenCanvas = class {
    constructor(w, h) { this.width = w; this.height = h; }
    getContext() { return new Proxy({}, { get: (t, k) => (k === 'createLinearGradient' ? () => ({ addColorStop() {} }) : () => {}) }); }
  };
  for (let i = 0; i < SPRITE_CACHE_MAX * 2; i++) {
    carSprite(CAR_MODELS[i % CAR_MODELS.length], `#${(i * 7919 % 0xffffff).toString(16).padStart(6, '0')}`, 42, 20, i % 3 === 0);
    assert.ok(spriteCacheSize() <= SPRITE_CACHE_MAX, `Cache wächst über ${SPRITE_CACHE_MAX}`);
  }
  const a = carSprite('taxi', TAXI_COLOR, 42, 20, false);
  assert.ok(a, 'mit Canvas ein Sprite');
  assert.equal(carSprite('taxi', TAXI_COLOR, 42, 20, false), a, 'wiederverwendet');
  globalThis.OffscreenCanvas = saved;
});

test('Passanten: Aussehen je Person fest und vielfältig', () => {
  const peds = Array.from({ length: 200 }, (_, i) => ({ id: i + 1 }));
  const looks = peds.map((p) => personLook(p));
  assert.deepEqual(peds.map((p) => personLook({ id: p.id })), looks.map((l) => l), 'deterministisch');
  assert.ok(new Set(looks.map((l) => l.pants)).size >= 5, 'verschiedene Hosen');
  assert.ok(new Set(looks.map((l) => l.hair)).size >= 5, 'verschiedene Haare');
  const bags = looks.filter((l) => l.bag).length;
  assert.ok(bags > 20 && bags < 100, `${bags} von 200 mit Tasche`);
});
