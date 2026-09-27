import test from 'node:test';
import assert from 'node:assert/strict';
import { radialDeadzone, fromHostReading, readPad, readKeys, merge, InputState, BTN } from '../web/src/input.js';

test('Totzone: kleine Ausschläge = 0, Vollausschlag bleibt 1', () => {
  assert.deepEqual(radialDeadzone(0.1, 0.1), [0, 0]);
  const [x] = radialDeadzone(1, 0);
  assert.ok(Math.abs(x - 1) < 1e-9);
});

test('Lesedaten der Xbox-Hülle werden zum Standard-Gamepad (Y-Achse gedreht)', () => {
  const gp = fromHostReading({ a: true, y: true, lt: 0.8, rt: 0.1, lx: 0, ly: 1, menu: true });
  assert.equal(gp.buttons[BTN.A].pressed, true);
  assert.equal(gp.buttons[BTN.Y].pressed, true);
  assert.equal(gp.buttons[BTN.MENU].pressed, true);
  assert.equal(gp.buttons[BTN.LT].pressed, true);
  assert.equal(gp.buttons[BTN.RT].pressed, false);
  assert.equal(gp.axes[1], -1); // Windows.Gaming.Input: +Y = oben; Web: +Y = unten
  const r = readPad(gp);
  assert.ok(r.a && r.menu && r.ly < -0.9 && r.lt === 0.8);
});

test('Flanken: A löst action genau einmal aus, actionHeld solange gehalten', () => {
  const s = new InputState();
  const base = merge({}, {});
  let o = s.frame({ ...base, a: true }, 1 / 60);
  assert.ok(o.action && o.actionHeld && o.confirm);
  o = s.frame({ ...base, a: true }, 1 / 60);
  assert.ok(!o.action && o.actionHeld);
});

test('Menü-Wiederholung beim Halten des Steuerkreuzes', () => {
  const s = new InputState();
  const base = merge({}, {});
  let count = 0;
  for (let i = 0; i < 60; i++) if (s.frame({ ...base, down: true }, 1 / 60).menuDown) count++;
  assert.ok(count >= 4 && count <= 8, `count=${count}`);
});

test('Tastatur und Gamepad werden zusammengeführt (stärkerer Ausschlag gewinnt)', () => {
  const k = readKeys(new Set(['KeyW']));
  const m = merge(k, { lx: 0.4, ly: 0, rt: 0.3 });
  assert.equal(m.rt, 1);
  assert.equal(m.lx, 0.4);
  assert.equal(m.up, true);
});
