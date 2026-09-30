// Dynamische Auflösung (perf.js): herunter, wenn die Bilder zu lange dauern, wieder hinauf, wenn Luft ist; Hysterese,
// Pausen (Tab im Hintergrund) zählen nicht.
import test from 'node:test';
import assert from 'node:assert/strict';
import { RES, stepResolution } from '../web/src/perf.js';

const feed = (level, ms, n = RES.window) => { const st = { gaps: [] }; for (let i = 0; i < n; i++) level = stepResolution(st, level, ms); return level; };

test('Dynamische Auflösung: langsam → herunter (bis zur letzten Stufe), schnell → hinauf, dazwischen bleibt sie', () => {
  assert.equal(feed(0, 16.7), 0, '60 Bilder/s: volle Auflösung');
  assert.equal(feed(0, 25), 1, 'unter 48 Bilder/s: eine Stufe herunter');
  assert.equal(feed(feed(1, 25), 25), RES.steps.length - 1, 'bis zur letzten Stufe');
  assert.equal(feed(RES.steps.length - 1, 30), RES.steps.length - 1, 'nicht tiefer');
  assert.equal(feed(2, 12), 1, 'schnell: wieder hinauf');
  assert.equal(feed(1, 18), 1, 'zwischen den Schwellen: bleibt (keine Pendelei)');
  assert.equal(feed(0, 16.7, RES.window - 1), 0, 'erst ein volles Fenster entscheidet');
  // Lücken (Tab im Hintergrund, Laden) zählen nicht
  const st = { gaps: [] }; let lv = 0;
  for (let i = 0; i < RES.window * 3; i++) lv = stepResolution(st, lv, i % 2 ? 2000 : 16);
  assert.equal(lv, 0);
  assert.ok(RES.steps.every((s, i) => !i || s < RES.steps[i - 1]) && RES.steps[0] === 1);
});
