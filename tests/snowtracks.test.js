import test from 'node:test';
import assert from 'node:assert/strict';
import { createTrails, recordTrails, trailAlpha, visibleTrails, TRAILS } from '../web/src/snowtracks.js';

const car = (id, x, y, angle = 0) => ({ id, x, y, angle, hw: 9, hh: 20, vx: 1, vy: 0 });

function drive(tr, c, from, to, t0 = 0, depth = 0.6) {
  let t = t0;
  for (let x = from; x <= to; x += 4) { c.x = x; recordTrails(tr, [c], t, depth); t += 0.05; }
  return t;
}

test('Schneespuren: ein fahrendes Auto hinterlässt zwei parallele Reifenspuren', () => {
  const tr = createTrails(), c = car(1, 0, 100);
  drive(tr, c, 0, 200);
  const segs = visibleTrails(tr, { x: -50, y: 0, w: 400, h: 300 }, 10, 0.6, 0);
  const len = segs.reduce((acc, q) => acc + Math.hypot(q.bx - q.ax, q.by - q.ay), 0);
  assert.ok(len >= 2 * 180, `Spurlänge ${len.toFixed(0)} px`);
  const ys = [...new Set(segs.map((s) => Math.round(s.ay)))].sort((a, b) => a - b);
  assert.equal(ys.length, 2, 'zwei Spurlinien');
  assert.ok(Math.abs(ys[1] - ys[0] - 2 * (c.hw - TRAILS.inset)) < 1.5, 'Spurweite = Wagenbreite');
  for (const s of segs) assert.ok(s.ax <= 200 && s.bx >= -20 && Math.abs(s.ay - s.by) < 0.01, 'entlang der Fahrt');
});

test('Schneespuren: ohne Schneedecke keine Spuren; stehende Autos und Sprünge (Teleport) ziehen keine Linie', () => {
  const tr = createTrails(), c = car(1, 0, 100);
  drive(tr, c, 0, 200, 0, 0.01);
  assert.equal(tr.n, 0, 'kein Schnee');
  const t = drive(tr, c, 0, 40);
  const n = tr.n;
  recordTrails(tr, [c], t + 1, 0.6); recordTrails(tr, [c], t + 2, 0.6);
  assert.equal(tr.n, n, 'Stillstand zeichnet nichts');
  c.x = 5000; recordTrails(tr, [c], t + 3, 0.6);
  assert.equal(tr.n, n, 'Sprung zieht keine Linie');
  c.x = 5010; recordTrails(tr, [c], t + 4, 0.6);
  assert.ok(tr.n > n, 'danach geht es normal weiter');
});

test('Schneespuren verblassen mit der Zeit, bei Schneefall schneller; Ringpuffer hat eine Obergrenze', () => {
  assert.ok(trailAlpha(0, 0.6, 0) > trailAlpha(60, 0.6, 0), 'älter = blasser');
  assert.ok(trailAlpha(60, 0.6, 1) < trailAlpha(60, 0.6, 0), 'Neuschnee deckt zu');
  assert.equal(trailAlpha(TRAILS.life + 1, 0.6, 0), 0, 'nach der Lebenszeit weg');
  assert.equal(trailAlpha(0, 0.02, 0), 0, 'ohne Schneedecke unsichtbar');
  const tr = createTrails(), c = car(1, 0, 0);
  drive(tr, c, 0, TRAILS.max * TRAILS.step * 2);
  assert.equal(tr.n, TRAILS.max, 'begrenzt');
});

test('Schneespuren: nur Stücke im Sichtfeld, die noch sichtbar sind', () => {
  const tr = createTrails(), c = car(1, 0, 100);
  const t = drive(tr, c, 0, 400);
  const all = visibleTrails(tr, { x: -50, y: 0, w: 600, h: 300 }, t, 0.6, 0).length;
  const part = visibleTrails(tr, { x: 300, y: 0, w: 300, h: 300 }, t, 0.6, 0).length;
  assert.ok(part > 0 && part < all / 2, `${part} von ${all}`);
  assert.equal(visibleTrails(tr, { x: -50, y: 0, w: 600, h: 300 }, t + TRAILS.life + 5, 0.6, 0).length, 0, 'alte weg');
});
