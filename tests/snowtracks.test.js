import test from 'node:test';
import assert from 'node:assert/strict';
import { createTrails, recordTrails, trailAlpha, visibleTrails, wheels, TRAILS } from '../web/src/snowtracks.js';
import { createCar } from '../web/src/car.js';

// wie in car.js: hw = halbe Länge, hh = halbe Breite
const car = (id, x, y, angle = 0) => ({ id, x, y, angle, hw: 21, hh: 10, vx: 1, vy: 0 });

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
  assert.ok(Math.abs(ys[1] - ys[0] - 2 * (c.hh - TRAILS.inset)) < 1.5, 'Spurweite = Wagenbreite (nicht -länge)');
  for (const s of segs) assert.ok(s.ax <= 200 + c.hw && s.bx >= -c.hw && Math.abs(s.ay - s.by) < 0.01, 'entlang der Fahrt');
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

test('Schneespuren: Räder eines echten Autos liegen unter der Karosserie; Zweirad eine Spur; Kurve trennt vorn und hinten; Brücke zeichnet nichts', () => {
  const c = createCar({ x: 0, y: 0, angle: 0, kind: 'car' }), w = wheels(c);
  assert.equal(w.length, 8);
  for (let k = 0; k < 8; k += 2) {
    assert.ok(Math.abs(w[k]) < c.hw && Math.abs(w[k + 1]) < c.hh, `Rad ${k / 2} innerhalb ${c.hw}×${c.hh}: ${w[k].toFixed(1)}, ${w[k + 1].toFixed(1)}`);
    assert.ok(Math.abs(w[k + 1]) > c.hh * 0.6, 'außen an der Flanke');
  }
  assert.ok(w[0] < 0 && w[4] > 0, 'hinten und vorn');
  const m = createCar({ x: 0, y: 0, angle: 0, kind: 'motorcycle' }), mw = wheels(m);
  assert.equal(mw.length, 4); assert.ok(Math.abs(mw[1]) < 0.01 && Math.abs(mw[3]) < 0.01, 'eine Linie in der Mitte');
  // Drift auf der Kreisbahn (Auto steht 0,35 rad quer): Vorder- und Hinterräder ziehen getrennte Spuren
  const tr = createTrails(), q = car(1, 0, 0);
  for (let i = 0; i <= 60; i++) { const a = i * 0.03; q.x = Math.sin(a) * 300; q.y = 300 - Math.cos(a) * 300; q.angle = a + 0.35; recordTrails(tr, [q], i * 0.05, 0.6); }
  const rad = new Set();
  for (const s of visibleTrails(tr, { x: -500, y: -500, w: 1500, h: 1500 }, 3, 0.6, 0)) rad.add(Math.round(Math.hypot(s.bx, s.by - 300)));
  assert.ok(rad.size >= 4, `mehrere Spurradien: ${[...rad].join(', ')}`);
  // auf der Brücke: keine Spur am Boden
  const tb = createTrails(), b = car(2, 0, 0); b.lvl = 1;
  drive(tb, b, 0, 200);
  assert.equal(tb.n, 0);
});
