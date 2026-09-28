// Erleuchtete Fenster (windows.js): jedes Fenster einzeln, in zufälliger Folge, Wohnungen gemeinsam, Büros anders.
import test from 'node:test';
import assert from 'node:assert/strict';
import { windowAt, litWindows, houseFraction, tvFlicker, WIN } from '../web/src/windows.js';
import { lightAt } from '../web/src/daylight.js';
import { BUILDING_KIND } from '../web/src/citycodes.js';
import { realCity } from './helpers/city.js';

const house = (seed, kind = BUILDING_KIND.house) => ({ seed, kind });
// Anteil, ab dem ein Fenster brennt (erster Anteil in 0,005er-Schritten), abends um 21 Uhr
function onAt(seed, face, col, row) {
  for (let f = 0; f <= 1.0001; f += 0.005) if (windowAt(seed, face, col, row, f, 21 * 60)) return Math.round(f * 1000) / 1000;
  return null;
}

test('Fenster gehen einzeln und in zufälliger Folge an, nicht hausweise im Block', () => {
  const cols = 12, rows = 6, th = [];
  for (let c = 0; c < cols; c++) for (let r = 0; r < rows; r++) th.push({ c, r, f: onAt(4711, 3, c, r) });
  const lit = th.filter((x) => x.f !== null);
  assert.ok(lit.length > 50, `fast alle Fenster brennen irgendwann (${lit.length}/72)`);
  const distinct = new Set(lit.map((x) => x.f)).size;
  assert.ok(distinct > lit.length * 0.6, `eigene Zeitpunkte je Fenster: ${distinct} von ${lit.length}`);
  // steigt der Anteil in kleinen Schritten, kommen immer nur wenige Fenster auf einmal dazu
  let worst = 0, prev = 0;
  for (let f = 0; f <= 1; f += 0.02) {
    const n = th.filter((x) => x.f !== null && x.f <= f).length;
    worst = Math.max(worst, n - prev); prev = n;
  }
  assert.ok(worst <= 8, `höchstens 8 von 72 Fenstern je 2-%-Schritt (${worst})`);
  // zufällige Folge: nicht von links nach rechts oder von unten nach oben
  const order = lit.slice().sort((a, b) => a.f - b.f);
  const firstHalfCols = order.slice(0, order.length / 2).map((x) => x.c);
  assert.ok(Math.max(...firstHalfCols) >= cols - 3 && Math.min(...firstHalfCols) <= 2, 'die ersten Lichter über die ganze Fassade verteilt');
});

test('Wohnungen: Fenster einer Wohnung gehen kurz nacheinander an, verschiedene Wohnungen zu verschiedenen Zeiten', () => {
  // Nachbarfenster derselben Etage liegen im Mittel näher beieinander als Fenster verschiedener Etagen
  let same = 0, nSame = 0, other = 0, nOther = 0;
  for (let seed = 1; seed <= 40; seed++) for (let c = 0; c < 10; c++) {
    const a = onAt(seed, 0, c, 1), b = onAt(seed, 0, c + 1, 1), d = onAt(seed, 0, c, 4);
    if (a !== null && b !== null) { same += Math.abs(a - b); nSame++; }
    if (a !== null && d !== null) { other += Math.abs(a - d); nOther++; }
  }
  assert.ok(same / nSame < 0.75 * (other / nOther), `Nachbarn ${(same / nSame).toFixed(3)} vs. andere Etage ${(other / nOther).toFixed(3)}`);
});

test('Fenster: deterministisch, tagsüber dunkel, abends mehr als um 3 Uhr, nachts kurz Licht im Bad', () => {
  const b = house(99);
  const count = (m) => litWindows(b, 1, 200, 100, lightAt(m).windowsLit, m).length;
  assert.equal(count(13 * 60), 0, 'mittags kein Licht');
  assert.ok(count(22 * 60) > count(3 * 60), `22 Uhr ${count(22 * 60)} > 3 Uhr ${count(3 * 60)}`);
  assert.deepEqual(litWindows(b, 1, 200, 100, 0.5, 1300), litWindows(b, 1, 200, 100, 0.5, 1300));
  // Nachts wechseln einzelne Fenster, ohne dass der Tagesgang sich bewegt (je 9 Minuten neu ausgewürfelt)
  let changed = 0;
  for (let seed = 1; seed <= 200; seed++) {
    const at = (m) => litWindows(house(seed), 0, 200, 100, 0.12, m).map((w) => w.x + ',' + w.y).join(';');
    if (at(3 * 60) !== at(3 * 60 + 27)) changed++;
  }
  assert.ok(changed > 20 && changed < 190, `nachts macht hier und da jemand Licht (${changed} von 200 Häusern)`);
});

test('Fenster liegen im Fensterraster der Fassade, Farben aus der Palette, Fernseher flackern', () => {
  const b = house(5), L = 180, H = 90;
  const w = litWindows(b, 2, L, H, 1, 21 * 60);
  assert.ok(w.length > 0);
  for (const q of w) {
    assert.equal((q.x - WIN.x) % WIN.cellW, 0);
    assert.ok(q.x >= WIN.x && q.x + q.w <= L - 4 && q.y >= 0 && q.y + q.h <= H - 2, 'innerhalb der Fassade');
    assert.ok(['warm', 'neutral', 'cool', 'tv', 'dim'].includes(q.type));
  }
  const types = new Set();
  for (let s = 0; s < 60; s++) for (const q of litWindows(house(s), 0, 300, 120, 1, 21 * 60)) types.add(q.type);
  assert.ok(types.size >= 4, `verschiedene Lichtfarben (${[...types]})`);
  const f = [0, 0.3, 0.7, 1.1].map((t) => tvFlicker(3, 4, t));
  assert.ok(Math.max(...f) - Math.min(...f) > 0.1 && Math.min(...f) >= 0.55);
});

test('Büros, Schulen, Hallen: abends noch Licht, nachts fast dunkel; Wohnhäuser nach dem Tagesgang', () => {
  const office = house(1, BUILDING_KIND.public), home = house(1);
  assert.ok(houseFraction(office, 0.6, 3 * 60) < 0.06, 'nachts dunkel');
  assert.ok(houseFraction(office, 0.3, 18 * 60 + 30) > 0.2, 'abends Licht im Büro');
  assert.equal(houseFraction(home, 0.4, 22 * 60), 0.4);
  assert.ok(houseFraction(office, 0, 12 * 60, 1) > houseFraction(office, 0, 12 * 60, 0), 'bei trübem Wetter tagsüber Licht');
});

test('Zeichnen: am Abend gehen die Fenster Minute für Minute einzeln an, nicht in Sprüngen', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  const { Renderer } = await import('../web/src/render.js');
  const { createWorld } = await import('../web/src/world.js');
  const w = createWorld({ city: realCity(), cars: 0, pedestrians: 0 });
  w.forceWeather = 'clear';
  const ctx = new Proxy({ lineWidth: 1 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'getLineDash') return () => [];
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  globalThis.OffscreenCanvas = class { constructor(wd, hg) { this.width = wd; this.height = hg; } getContext() { return ctx; } };
  const r = new Renderer(ctx), counts = [];
  for (let m = 19 * 60; m <= 21 * 60; m += 2) { w.clock = m; r.draw(w, 1280, 720, 1.2); counts.push(r.stats.litWindows); }
  const final = counts[counts.length - 1];
  assert.ok(final > 200, `abends viele erleuchtete Fenster (${final})`);
  let steps = 0, worst = 0;
  for (let i = 1; i < counts.length; i++) { const d = counts[i] - counts[i - 1]; if (d > 0) steps++; worst = Math.max(worst, d); }
  assert.ok(steps > counts.length * 0.6, `fast jede 2. Minute kommen Fenster dazu (${steps}/${counts.length - 1})`);
  assert.ok(worst < final * 0.1, `nie mehr als 10 % auf einmal (${worst} von ${final})`);
});
