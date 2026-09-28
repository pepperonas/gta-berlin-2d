// Rechte Maustaste und Waffenrad (weaponwheel.js, gezeichnet in hud.js)
import test from 'node:test';
import assert from 'node:assert/strict';
import { wheelSlot, slotDir, createRightButton, drawWeaponIcon, WHEEL } from '../web/src/weaponwheel.js';
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
  assert.deepEqual(rb.release(10.12), { enterExit: true });
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
});
