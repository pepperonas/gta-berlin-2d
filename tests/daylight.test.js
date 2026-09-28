import test from 'node:test';
import assert from 'node:assert/strict';
import { lightAt, formatClock, parseClock, SHADOW_MAX } from '../web/src/daylight.js';
import { addBuildingShadow, casterBox, shadowOffset, treeShadowGeom, addTrunkShadow, crownShadowSprite, Lighting } from '../web/src/lighting.js';
import { CLOCK } from '../web/src/config.js';
import { realCity } from './helpers/city.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { makeSave, validateSave, applySave, memoryStorage, writeSave, readSave } from '../web/src/save.js';
import { idle } from './helpers/bot.js';

test('Tageslicht: Mittag hell mit kurzem Schatten, Mitternacht dunkel mit Laternen', () => {
  const noon = lightAt(13 * 60), night = lightAt(0);
  assert.equal(noon.dark, 0);
  assert.ok(noon.sun.len < 0.8, `Mittagsschatten ${noon.sun.len}`);
  assert.equal(noon.sun.strength, 1);
  assert.equal(noon.lampsOn, false);
  assert.ok(night.dark > 0.7, `Nacht dunkel (${night.dark})`);
  assert.equal(night.sun.strength, 0, 'nachts kein Sonnenschatten');
  assert.equal(night.lampsOn, true);
  assert.ok(night.ambient[2] > night.ambient[0], 'Nachtlicht ist bläulich');
  assert.ok(night.windowsLit > 0.2 && noon.windowsLit === 0);
});

test('Tageslicht: morgens und abends lange Schatten in entgegengesetzte Richtungen, mittags nach Norden', () => {
  const am = lightAt(7 * 60).sun, pm = lightAt(19 * 60).sun, noon = lightAt(13 * 60).sun;
  assert.ok(am.len > 1.5 && pm.len > 1.5, 'lange Schatten bei tiefer Sonne');
  assert.ok(am.len <= SHADOW_MAX && pm.len <= SHADOW_MAX);
  assert.ok(am.dx < -0.5 && pm.dx > 0.5, `morgens nach Westen (${am.dx}), abends nach Osten (${pm.dx})`);
  assert.ok(noon.dy < -0.9, 'Mittagssonne im Süden → Schatten nach Norden (oben)');
  // Sonne im Osten morgens → Schatten nach Westen; y wächst nach unten (Süden)
});

test('Tageslicht: stetig über 24 h (keine Sprünge von Minute zu Minute)', () => {
  let prev = lightAt(0);
  for (let m = 1; m <= 1440; m++) {
    const l = lightAt(m);
    for (let k = 0; k < 3; k++) assert.ok(Math.abs(l.ambient[k] - prev.ambient[k]) < 0.02, `Licht springt um ${m} min`);
    assert.ok(Math.abs(l.dark - prev.dark) < 0.03, `Dunkelheit springt um ${m} min`);
    const s = (x) => [x.sun.dx * x.sun.len * x.sun.strength, x.sun.dy * x.sun.len * x.sun.strength];
    const [a, b] = s(l), [c, d] = s(prev);
    assert.ok(Math.hypot(a - c, b - d) < 0.25, `Schatten springt um ${m} min`);
    assert.ok(l.windowsLit >= 0 && l.windowsLit <= 1);
    prev = l;
  }
});

test('Uhrzeit formatieren und lesen', () => {
  assert.equal(formatClock(0), '00:00');
  assert.equal(formatClock(965.7), '16:05');
  assert.equal(formatClock(1440 + 61), '01:01');
  assert.equal(parseClock('21:30'), 1290);
  assert.equal(parseClock('7:05'), 425);
  for (const bad of ['25:00', '12:60', 'abc', '', null]) assert.equal(parseClock(bad), null);
});

test('Spieluhr: 1 Spielminute je Sekunde, springt über Mitternacht, beginnt am Nachmittag', () => {
  const w = createWorld({ city: realCity(), cars: 0, pedestrians: 0 });
  assert.equal(w.clock, CLOCK.start);
  assert.ok(CLOCK.start >= 14 * 60 && CLOCK.start <= 17 * 60);
  const c0 = w.clock;
  for (let i = 0; i < 120; i++) updateWorld(w, idle(), 1 / 60);
  assert.ok(Math.abs(w.clock - c0 - 2 * CLOCK.minutesPerSecond) < 1e-6, `nach 2 s: ${w.clock - c0} min`);
  w.clock = 1439.5;
  for (let i = 0; i < 60; i++) updateWorld(w, idle(), 1 / 60);
  assert.ok(w.clock >= 0 && w.clock < 1, `nach Mitternacht ${w.clock}`);
});

test('Spielstand merkt sich die Uhrzeit; alte Stände ohne Uhr starten zur Startzeit', () => {
  const city = realCity();
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.clock = 1300.4;
  const st = memoryStorage();
  writeSave(st, w);
  const s = readSave(st);
  assert.equal(s.clock, 1300);
  const w2 = createWorld({ city, cars: 0, pedestrians: 0 });
  applySave(w2, s);
  assert.equal(w2.clock, 1300);
  const old = makeSave(w); delete old.clock;
  const v = validateSave(old);
  assert.ok(v, 'alter Stand bleibt gültig');
  const w3 = createWorld({ city, cars: 0, pedestrians: 0 });
  applySave(w3, v);
  assert.equal(w3.clock, CLOCK.start);
  const bad = makeSave(w); bad.clock = 99999;
  assert.equal(validateSave(bad).clock, null, 'unsinnige Uhrzeit wird verworfen');
});

// Pfad-Aufzeichner: sammelt die Teilpfade (Vierecke) von addBuildingShadow
function recorder() {
  const polys = []; let cur = null;
  return {
    polys,
    moveTo(x, y) { cur = [[x, y]]; polys.push(cur); },
    lineTo(x, y) { cur.push([x, y]); },
    closePath() {},
  };
}
const area = (p) => { let a = 0; for (let i = 0; i < p.length; i++) { const [x0, y0] = p[i], [x1, y1] = p[(i + 1) % p.length]; a += x0 * y1 - x1 * y0; } return a / 2; };

test('Hausschatten: alle Wandvierecke gleich orientiert (sonst löschen sich Überlappungen bei „nonzero“ aus)', () => {
  // L-förmiges Haus mit Innenhof, beide Ringrichtungen
  const outer = [0, 0, 100, 0, 100, 60, 50, 60, 50, 100, 0, 100];
  const hole = [10, 10, 10, 40, 40, 40, 40, 10];
  for (const rings of [[outer, hole], [outer.slice().reverse().flatMap((_, i, a) => i % 2 ? [] : [a[i + 1], a[i]]), hole]]) {
    for (const [ox, oy] of [[30, -20], [-15, 40], [0, 25]]) {
      const g = recorder();
      addBuildingShadow(g, { rings }, ox, oy);
      assert.ok(g.polys.length >= 5, 'je (nicht zur Sonne parallele) Wand ein Viereck');
      for (const p of g.polys) assert.ok(area(p) > 0, `Viereck mit Fläche ${area(p)} falsch orientiert`);
    }
  }
});

test('Schatten werfende Häuser außerhalb des Bildes werden gefunden (Suchfenster gegen die Sonne verlängert)', () => {
  const v = { x: 0, y: 0, w: 1000, h: 600 };
  const sun = lightAt(19 * 60).sun; // Abendsonne im Westen → Schatten nach Osten
  const box = casterBox(v, sun);
  const [ox] = shadowOffset(sun, 400);
  assert.ok(ox > 0);
  assert.ok(box.x <= -ox + 1, 'Fenster reicht nach Westen, woher die Schatten kommen');
  assert.ok(box.x + box.w >= v.w, 'deckt das Bild ab');
});

test('Baumschatten: Stamm vom Fuß bis in die Krone, Krone quer so breit wie der Baum, bei tiefer Sonne gestreckt', () => {
  const tree = { x: 1000, y: 2000, size: 40, r: 3, genus: 'Tilia', seed: 7 };
  const low = lightAt(19 * 60 + 30).sun, high = lightAt(13 * 60).sun;
  const a = treeShadowGeom(tree, low), b = treeShadowGeom(tree, high);
  for (const s of [a, b]) {
    assert.ok(Math.abs(Math.hypot(s.ux, s.uy) - 1) < 1e-9);
    assert.ok(s.mid > s.top && s.top > 0, 'Kronenmitte höher als der Kronenansatz');
    assert.ok(Math.abs(s.cx - (tree.x + s.ux * s.mid)) < 1e-9 && Math.abs(s.cy - (tree.y + s.uy * s.mid)) < 1e-9, 'Krone liegt auf der Stammlinie');
    assert.ok(s.mid - s.major < s.top, 'Kronenschatten beginnt vor dem Kronenansatz: Stamm und Krone hängen zusammen');
    assert.equal(s.minor, tree.size * 0.95, 'quer so breit wie die Krone');
    assert.ok(s.w0 >= tree.r && s.w1 < s.w0, 'Stamm am Fuß so dick wie der Baum, nach oben dünner');
  }
  assert.ok(a.mid > 2 * b.mid, `Abendschatten länger (${a.mid.toFixed(0)} gegen ${b.mid.toFixed(0)})`);
  assert.ok(a.major / a.minor > 1.3 && b.major / b.minor < 1.2, 'tiefe Sonne streckt die Krone, Mittagssonne kaum');
  assert.ok(a.major / a.minor <= 3, 'Streckung begrenzt');
  // Richtung wie die Hausschatten
  const [ox, oy] = shadowOffset(low, 100);
  assert.ok((a.ux * ox + a.uy * oy) / Math.hypot(ox, oy) > 0.999);
  // Stamm: geschlossenes Viereck, der Fußpunkt liegt darin
  const pts = [];
  const g = { moveTo: (x, y) => pts.push([x, y]), lineTo: (x, y) => pts.push([x, y]), closePath: () => pts.push('close') };
  addTrunkShadow(g, a);
  assert.equal(pts.length, 5); assert.equal(pts[4], 'close');
  const mid = [(pts[0][0] + pts[3][0]) / 2, (pts[0][1] + pts[3][1]) / 2];
  assert.ok(Math.hypot(mid[0] - tree.x, mid[1] - tree.y) < 1e-9, 'beginnt am Stammfuß');
  assert.ok(Math.hypot(pts[0][0] - pts[3][0], pts[0][1] - pts[3][1]) > Math.hypot(pts[1][0] - pts[2][0], pts[1][1] - pts[2][1]), 'verjüngt');
});

test('Baumschatten zeichnen: ohne Canvas (Node) eine gestreckte Scheibe statt des Bilds, keine ungültigen Zahlen', () => {
  assert.equal(crownShadowSprite('Tilia', 0), null, 'ohne Canvas kein Bild');
  const calls = [];
  const g = new Proxy({}, { get(t, k) { if (k in t) return t[k]; return (...a) => { if (a.some((v) => typeof v === 'number' && !Number.isFinite(v))) throw new Error(`NaN in ${k}`); calls.push(k); }; }, set(t, k, v) { t[k] = v; return true; } });
  const L = new Lighting();
  L.shadow = { width: 800, height: 600, getContext: () => g };
  const ctx = new Proxy({}, { get: (t, k) => (k in t ? t[k] : () => {}), set(t, k, v) { t[k] = v; return true; } });
  const trees = [{ x: 0, y: 0, size: 30, r: 2, genus: 'Betula', seed: 1 }, { x: 100, y: 50, size: 50, r: 4, genus: 'Nadel', seed: 2 }];
  L.drawShadows(ctx, 800, 600, [1, 0, 0], lightAt(18 * 60).sun, [], trees);
  assert.equal(calls.filter((k) => k === 'arc').length, 2, 'je Baum eine Krone');
  assert.equal(calls.filter((k) => k === 'transform').length, 2, 'je Baum gestreckt');
  assert.ok(calls.filter((k) => k === 'closePath').length >= 2, 'je Baum ein Stamm');
});
