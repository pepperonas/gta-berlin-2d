import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorld } from '../web/src/world.js';
import { makeSave, validateSave, writeSave, readSave, applySave, memoryStorage, SAVE_KEY } from '../web/src/save.js';

test('Speichern und Laden stellt Geld, Fortschritt und Autoposition wieder her', () => {
  const w = createWorld({ cars: 0, pedestrians: 0 });
  w.money = 1234; w.completed = 2; w.bestTime = 51.5;
  const car = w.cars.find((c) => c.id === w.playerCarId);
  car.x = 1000; car.y = 1100; car.health = 60;
  const st = memoryStorage();
  assert.ok(writeSave(st, w));
  const s = readSave(st);
  assert.equal(s.money, 1234);
  const w2 = createWorld({ cars: 0, pedestrians: 0 });
  applySave(w2, s);
  const car2 = w2.cars.find((c) => c.id === w2.playerCarId);
  assert.deepEqual([w2.money, w2.completed, w2.bestTime, car2.x, car2.y, car2.health], [1234, 2, 51.5, 1000, 1100, 60]);
  assert.ok(Math.hypot(w2.player.x - 1000, w2.player.y - 1100) < 40);
});

test('kaputte oder fremde Spielstände werden verworfen', () => {
  const st = memoryStorage();
  assert.equal(readSave(st), null);
  st.setItem(SAVE_KEY, '{kaputt');
  assert.equal(readSave(st), null);
  const good = makeSave(createWorld({ cars: 0, pedestrians: 0 }));
  assert.ok(validateSave(good));
  assert.equal(validateSave({ ...good, version: 99 }), null);
  assert.equal(validateSave({ ...good, money: -5 }), null);
  assert.equal(validateSave({ ...good, player: { x: 1e9, y: 0 } }), null);
  assert.equal(validateSave({ ...good, car: { x: 'a' } }).car, null);
});

test('volles/gesperrtes Storage meldet Fehler statt zu werfen', () => {
  const st = { getItem: () => null, setItem: () => { throw new Error('QuotaExceeded'); } };
  assert.equal(writeSave(st, createWorld({ cars: 0, pedestrians: 0 })), false);
});
