import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorld } from '../web/src/world.js';
import { realCity } from './helpers/city.js';
import { makeSave, validateSave, writeSave, readSave, applySave, memoryStorage, SAVE_KEY } from '../web/src/save.js';

test('Speichern und Laden stellt Geld, Fortschritt und Autoposition wieder her', () => {
  const w = createWorld({ city: realCity(), cars: 0, pedestrians: 0 });
  w.money = 1234; w.completed = 2; w.bestTime = 51.5;
  const car = w.cars.find((c) => c.id === w.playerCarId);
  const d = w.city.places.dropoff;
  car.x = d.x; car.y = d.y; car.health = 60;
  const st = memoryStorage();
  assert.ok(writeSave(st, w));
  const s = readSave(st);
  assert.equal(s.money, 1234);
  const w2 = createWorld({ city: realCity(), cars: 0, pedestrians: 0 });
  applySave(w2, s);
  const car2 = w2.cars.find((c) => c.id === w2.playerCarId);
  assert.deepEqual([w2.money, w2.completed, w2.bestTime, car2.x, car2.y, car2.health], [1234, 2, 51.5, d.x, d.y, 60]);
  assert.ok(Math.hypot(w2.player.x - d.x, w2.player.y - d.y) < 40);
});

test('Position außerhalb des Gebiets oder in einem Haus wird beim Laden ignoriert', () => {
  const w = createWorld({ city: realCity(), cars: 0, pedestrians: 0 });
  const s = validateSave({ ...makeSave(w), player: { x: 5, y: 5 }, car: { x: 5, y: 5, angle: 0, health: 80 } });
  applySave(w, s);
  const sp = w.city.places.playerSpawn, pc = w.city.places.playerCar;
  assert.deepEqual([w.player.x, w.player.y], [sp.x, sp.y]);
  const car = w.cars.find((c) => c.id === w.playerCarId);
  assert.deepEqual([car.x, car.y], [pc.x, pc.y]);
});

test('kaputte oder fremde Spielstände werden verworfen', () => {
  const st = memoryStorage();
  assert.equal(readSave(st), null);
  st.setItem(SAVE_KEY, '{kaputt');
  assert.equal(readSave(st), null);
  const good = makeSave(createWorld({ city: realCity(), cars: 0, pedestrians: 0 }));
  assert.ok(validateSave(good));
  assert.equal(validateSave({ ...good, version: 99 }), null);
  assert.equal(validateSave({ ...good, version: 1 }), null, 'Stände der alten Rasterstadt');
  assert.equal(validateSave({ ...good, money: -5 }), null);
  assert.equal(validateSave({ ...good, player: { x: 1e9, y: 0 } }), null);
  assert.equal(validateSave({ ...good, car: { x: 'a' } }).car, null);
});

test('volles/gesperrtes Storage meldet Fehler statt zu werfen', () => {
  const st = { getItem: () => null, setItem: () => { throw new Error('QuotaExceeded'); } };
  assert.equal(writeSave(st, createWorld({ city: realCity(), cars: 0, pedestrians: 0 })), false);
});
