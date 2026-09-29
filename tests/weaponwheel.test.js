// Rechte Maustaste und Waffenrad (weaponwheel.js, gezeichnet in hud.js)
import test from 'node:test';
import assert from 'node:assert/strict';
import { wheelSlot, slotDir, createRightButton, drawWeaponIcon, WHEEL, easeTimeScale } from '../web/src/weaponwheel.js';
import { WEAPONS } from '../web/src/combat.js';

test('Waffenrad: Richtung wählt das Segment (0 oben, im Uhrzeigersinn), Mitte ist Totzone', () => {
  const n = 6;
  assert.equal(wheelSlot(0, -100, n), 0, 'oben');
  assert.equal(wheelSlot(100, -58, n), 1, 'rechts oben');
  assert.equal(wheelSlot(100, 58, n), 2, 'rechts unten');
  assert.equal(wheelSlot(0, 100, n), 3, 'unten');
  assert.equal(wheelSlot(-100, 58, n), 4);
  assert.equal(wheelSlot(-100, -58, n), 5);
  assert.equal(wheelSlot(3, -5, n), -1, 'Totzone');
  assert.equal(wheelSlot(0, -(WHEEL.dead + 1), n), 0, 'knapp außerhalb der Totzone');
  // jede Segmentmitte führt zurück auf ihr Segment; Grenzen liegen genau zwischen zwei Mitten
  for (let i = 0; i < n; i++) { const d = slotDir(i, n); assert.equal(wheelSlot(d.x * 100, d.y * 100, n), i); }
  const b = slotDir(0.49, n), c = slotDir(0.51, n);
  assert.equal(wheelSlot(b.x * 100, b.y * 100, n), 0); assert.equal(wheelSlot(c.x * 100, c.y * 100, n), 1);
  assert.equal(wheelSlot(10, 10, 0), -1, 'ohne Waffen');
});

test('Rechte Maustaste: tippen = ein-/aussteigen, halten = Rad auf, Maus wählt, loslassen nimmt die Waffe', () => {
  const rb = createRightButton();
  // Tippen
  rb.press(10, 500, 300);
  assert.deepEqual(rb.tick(10.1, true, 0, 6), {});
  assert.deepEqual(rb.release(10.12), { tap: true, enterExit: true });
  assert.equal(rb.open, false);
  // Halten öffnet das Rad, die Anzeige startet bei der aktuellen Waffe
  rb.press(20, 500, 300);
  assert.deepEqual(rb.tick(20 + WHEEL.hold - 0.01, true, 3, 6), {}, 'noch zu kurz');
  assert.deepEqual(rb.tick(20 + WHEEL.hold, true, 3, 6), { opened: true });
  assert.equal(rb.state.hover, 3);
  rb.move(500, 300 - 150); // nach oben
  assert.equal(rb.state.hover, 0);
  rb.move(500 + 150, 300 + 90); // rechts unten
  assert.equal(rb.state.hover, 2);
  rb.move(502, 301); // zurück in die Mitte: Wahl bleibt
  assert.equal(rb.state.hover, 2);
  assert.deepEqual(rb.release(21), { pick: 2, closed: true });
  assert.equal(rb.open, false);
  // lange gehalten, aber im Auto (Rad kann nicht auf): kein Aussteigen beim Loslassen
  rb.press(30, 0, 0);
  rb.tick(31, false, 0, 6);
  assert.deepEqual(rb.release(31), {});
  // Rad offen, dann steigt man ein oder wird umgehauen: es schließt ohne Wahl
  rb.press(40, 0, 0); rb.tick(41, true, 1, 6);
  assert.equal(rb.open, true);
  assert.deepEqual(rb.tick(41.1, false, 1, 6), { closed: true });
  assert.deepEqual(rb.release(41.2), {});
  // Maus bei geschlossenem Rad ändert nichts
  const h = rb.state.hover; rb.move(0, -500); assert.equal(rb.state.hover, h);
});

test('Rad: gewählt ist, worauf der echte Zeiger zeigt – ab der Radmitte, schon nach wenigen Punkten, auch weit draußen', () => {
  const rb = createRightButton();
  rb.press(0, 400, 400); rb.tick(1, true, 0, 6);
  assert.deepEqual([rb.state.cx, rb.state.cy], [400, 400], 'Rad öffnet am Zeiger');
  rb.move(400 + 14, 400 - 9); assert.equal(rb.state.hover, 1, 'ein kleiner Ruck nach rechts oben reicht (17 Punkte)');
  rb.move(400 + 900, 400 + 520); assert.equal(rb.state.hover, 2, 'weit draußen zählt die Richtung');
  rb.move(400 - 60, 400 - 35); assert.equal(rb.state.hover, 5, 'direkt hinüber, ohne Umweg über die Mitte');
  rb.move(401, 399); assert.equal(rb.state.hover, 5, 'Totzone: Wahl bleibt');
  // place: am Bildrand hereingerückte Mitte – die Wahl richtet sich nach der neuen Mitte
  rb.place(600, 300); assert.equal(rb.state.hover, 4, 'Zeiger liegt jetzt links unten der Mitte');
  rb.move(600, 150); assert.equal(rb.state.hover, 0);
  // Bewegung vor dem Öffnen: das Rad öffnet dort, wo der Zeiger dann ist; Anzeige startet bei der aktuellen Waffe
  const r2 = createRightButton();
  r2.press(0, 100, 100); r2.move(400, 100); r2.tick(1, true, 3, 6);
  assert.deepEqual([r2.state.cx, r2.state.cy, r2.state.hover], [400, 100, 3]);
  r2.move(400, 100 - 60); assert.equal(r2.state.hover, 0, 'ab dem Öffnen gezählt');
});

test('Rad: Mausrad dreht weiter, Zifferntaste wählt und schließt, Abbrechen ohne Wahl, Controller-Stick, verpasstes Loslassen', () => {
  const rb = createRightButton();
  rb.press(0, 0, 0); rb.tick(1, true, 4, 6);
  rb.nudge(1); assert.equal(rb.state.hover, 5); rb.nudge(1); assert.equal(rb.state.hover, 0, 'rundum');
  rb.nudge(-3); assert.equal(rb.state.hover, 5, 'eine Raste = ein Feld, egal wie groß der Ausschlag');
  assert.deepEqual(rb.choose(1), { pick: 1, closed: true }); assert.equal(rb.open, false);
  assert.deepEqual(rb.release(2), {}, 'danach nichts mehr');
  assert.deepEqual(rb.choose(2), {}, 'geschlossen: Ziffern normal');
  rb.press(3, 0, 0); rb.tick(4, true, 2, 6);
  assert.deepEqual(rb.cancel(), { closed: true, cancelled: true });
  assert.deepEqual(rb.release(4.1), {}, 'abgebrochen: Loslassen wählt nicht');
  // Controller: Stick mit Totzone, losgelassener Stick behält die Wahl
  const pad = createRightButton();
  pad.press(0); pad.tick(1, true, 0, 6);
  pad.aim(0.2, 0.1); assert.equal(pad.state.hover, 0, 'Totzone');
  pad.aim(0, 1); assert.equal(pad.state.hover, 3, 'unten');
  pad.aim(-0.9, -0.5); assert.equal(pad.state.hover, 5);
  pad.aim(0, 0); assert.equal(pad.state.hover, 5, 'Stick los: Wahl bleibt');
  assert.deepEqual(pad.release(2), { pick: 5, closed: true });
  pad.press(3); assert.deepEqual(pad.release(3.1), { tap: true, enterExit: true }, 'LB getippt');
  // verpasstes Loslassen (außerhalb des Fensters): sync entscheidet wie release
  const lost = createRightButton();
  lost.press(0, 0, 0); lost.tick(1, true, 1, 6); lost.move(0, 80);
  assert.deepEqual(lost.sync(1.2, true), {}, 'noch gedrückt');
  assert.deepEqual(lost.sync(1.3, false), { pick: 3, closed: true });
  assert.equal(lost.down, false); assert.deepEqual(lost.sync(1.4, false), {});
  const tap = createRightButton(); tap.press(0, 0, 0);
  assert.deepEqual(tap.sync(0.1, false), { tap: true, enterExit: true }, 'kurz: getippt');
});

test('Zeitlupe blendet weich ein und aus und kommt genau am Ziel an', () => {
  let s = 1; const seq = [];
  for (let i = 0; i < 45; i++) { s = easeTimeScale(s, true, 1 / 60); seq.push(s); }
  assert.ok(seq[0] < 1 && seq[0] > 0.8, 'nicht schlagartig');
  assert.ok(seq[11] < 0.45, 'nach 0,2 s fast da (spürbar, aber nicht träge)');
  for (let i = 1; i < seq.length; i++) assert.ok(seq[i] <= seq[i - 1], 'monoton');
  assert.equal(seq.at(-1), WHEEL.slow, 'kommt an');
  for (let i = 0; i < 45; i++) s = easeTimeScale(s, false, 1 / 60);
  assert.equal(s, 1);
  assert.equal(easeTimeScale(1, true, 1), WHEEL.slow, 'großer Zeitschritt überschießt nicht');
});

test('Waffensymbole: jede Waffe hat ein eigenes Symbol, gültige Koordinaten, Zustand des Canvas unverändert', () => {
  const calls = [];
  let depth = 0;
  const ctx = new Proxy({}, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'save') return () => depth++;
      if (k === 'restore') return () => depth--;
      return (...a) => { calls.push([k, ...a]); if (a.some((v) => typeof v === 'number' && !Number.isFinite(v))) throw new Error(`NaN in ${k}`); };
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const shapes = new Set();
  for (const w of WEAPONS) {
    calls.length = 0;
    drawWeaponIcon(ctx, w.id, 100, 100, 50, '#fff');
    assert.equal(depth, 0, `${w.id}: save/restore ausgeglichen`);
    assert.ok(calls.some(([k]) => k === 'fill'), `${w.id}: gezeichnet`);
    shapes.add(JSON.stringify(calls.filter(([k]) => k !== 'save' && k !== 'restore')));
  }
  assert.equal(shapes.size, WEAPONS.length, 'lauter verschiedene Symbole');
});

test('Waffenrad im HUD: ein Segment je Waffe mit Symbol, das gezeigte hervorgehoben, Munition in der Mitte', async () => {
  const { Hud } = await import('../web/src/hud.js');
  const texts = [], fills = [];
  const mk = () => new Proxy({ canvas: { width: 1920, height: 1080 } }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'measureText') return (s) => ({ width: String(s).length * 8 });
      if (k === 'createLinearGradient' || k === 'createRadialGradient' || k === 'createPattern') return () => ({ addColorStop() {} });
      if (k === 'fillText') return (s) => texts.push(String(s));
      if (k === 'fill') return () => fills.push(t.fillStyle);
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const hud = new Hud(mk());
  hud.begin(1920, 1080);
  const p = { weapon: 0, mag: WEAPONS.map((w) => w.mag ?? 0) };
  p.mag[3] = 5;
  hud.drawWeaponWheel(p, 3);
  assert.equal(hud.counts.wheelIcons, WEAPONS.length);
  assert.ok(texts.includes('PISTOLE') && texts.includes(`5 / ${WEAPONS[3].mag}`), `Mitte: ${texts.join(' | ')}`);
  assert.ok(fills.includes('rgba(255,211,61,0.92)'), 'gezeigtes Segment gelb');
  assert.equal(hud.layout.wheel.hover, 3);
  texts.length = 0; hud.drawWeaponWheel(p, 1);
  assert.ok(texts.includes('Nahkampf'), 'Nahkampfwaffe ohne Munition');
  // Munition je Segment, Tasten 1–6, Zeiger, Hinweis je Gerät
  texts.length = 0; hud.counts = {}; hud.drawWeaponWheel(p, 3, { vx: 40, vy: -30, age: 1 });
  assert.ok(texts.includes(`5/${WEAPONS[3].mag}`), 'Munition am Segment');
  for (let i = 1; i <= WEAPONS.length; i++) assert.ok(texts.includes(String(i)), `Taste ${i}`);
  assert.ok(!hud.counts.wheelPointer, 'Maus: kein Ersatzzeiger – der echte Mauszeiger zeigt');
  assert.ok(texts.some((t) => t.includes('Esc bricht ab')));
  hud.drawWeaponWheel(p, 3, { age: 1, cx: 400, cy: 300 });
  assert.deepEqual([hud.layout.wheel.cx, hud.layout.wheel.cy], [400, 300], 'Maus: Rad am Zeiger');
  hud.counts = {}; hud.drawWeaponWheel(p, 3, { vx: 40, vy: -30, age: 1, pad: true });
  assert.ok(hud.counts.wheelPointer, 'Controller: Stickrichtung gezeichnet');
  assert.deepEqual([hud.layout.wheel.cx, hud.layout.wheel.cy], [hud.vw / 2, hud.vh / 2], 'Controller: Bildmitte');
  texts.length = 0; hud.counts = {}; hud.drawWeaponWheel(p, 3, { age: 0, pad: true });
  assert.ok(!hud.counts.wheelPointer, 'ohne Ausschlag kein Zeiger');
  assert.ok(texts.some((t) => t.includes('LB loslassen')) && !texts.includes('1'), 'Controller: eigener Hinweis, keine Zifferntasten');
});
