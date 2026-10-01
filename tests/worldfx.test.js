import test from 'node:test';
import assert from 'node:assert/strict';
import { WorldEffects, tireEffect } from '../web/src/worldfx.js';
import { FX_BUDGET, filmMood } from '../web/src/visualstyle.js';
import { T, surfaceAt } from '../web/src/map.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { idleInput } from '../web/src/idle.js';
import { realCity } from './helpers/city.js';

const quiet = () => ({ player: { x: 20, y: 30, swimming: false }, cars: [], camera: { x: 0, y: 0 } });

test('Reifeneffekte folgen Untergrund, Nässe und Drift', () => {
  assert.equal(tireEffect(T.ROAD, 0, 0, false), null);
  assert.equal(tireEffect(T.ROAD, 0, 0, true), 'smoke');
  assert.equal(tireEffect(T.GRASS, 0, 0, true), 'dust');
  assert.equal(tireEffect(T.ROAD, 0.8, 0, true), 'spray');
  assert.equal(tireEffect(T.ROAD, 0.8, 0.5, true), 'snow');
});

test('Effekte bleiben begrenzt, reduzieren sich auf niedrig und laufen vollständig aus', () => {
  const fx = new WorldEffects(), w = quiet();
  fx.handleEvents(Array.from({ length: 500 }, () => ({ type: 'footSplash', x: 20, y: 30, surface: T.WATER })));
  assert.ok(fx.particles.length <= FX_BUDGET.high.particles);
  assert.ok(fx.rings.length <= FX_BUDGET.high.rings);
  fx.update(w, 1 / 60, 'low');
  assert.ok(fx.particles.length <= FX_BUDGET.low.particles);
  assert.ok(fx.rings.length <= FX_BUDGET.low.rings);
  for (let i = 0; i < 120; i++) fx.update(w, 1 / 60, 'low');
  assert.equal(fx.particles.length, 0); assert.equal(fx.rings.length, 0);
});

test('Schwimmwellen sind bei 30/60/120 Hz gleich häufig; Sprung erzeugt keine Schwimmspur', () => {
  const counts = [];
  for (const hz of [30, 60, 120]) {
    const fx = new WorldEffects(), w = quiet(); w.player.swimming = true; w.player.moveSpeed = 18;
    for (let i = 0; i < hz; i++) fx.update(w, 1 / hz, 'high');
    counts.push(fx.rings.length);
  }
  assert.deepEqual(counts, [4, 4, 4]);
  const fx = new WorldEffects(), w = quiet(); w.player.swimming = true; w.player.jumpZ = 0.5;
  for (let i = 0; i < 120; i++) fx.update(w, 1 / 60, 'high');
  assert.equal(fx.rings.length, 0);
});

test('Lichtstimmung begrenzt den Leuchthof auf Dämmerung und Nacht', () => {
  assert.equal(filmMood({ dark: 0, elevation: 1 }).bloom, 0);
  assert.ok(filmMood({ dark: 1, elevation: -1 }).bloom > 0);
  assert.ok(filmMood({ dark: 0.2, elevation: 0.15 }).warmth > filmMood({ dark: 0, elevation: 1 }).warmth);
});

test('Sprung und Landung senden je ein Ereignis, auch bei einem Treffer in der Luft', () => {
  const w = createWorld({ city: realCity(), cars: 0, pedestrians: 0 });
  w.cars = []; const all = [];
  updateWorld(w, { ...idleInput, jumpPressed: true }, 1 / 60); all.push(...w.events);
  w.player.stun = 1;
  for (let i = 0; i < 75; i++) { updateWorld(w, idleInput, 1 / 60); all.push(...w.events); }
  assert.equal(all.filter(e => e.type === 'footJump').length, 1);
  assert.equal(all.filter(e => e.type === 'footLand').length, 1);
  assert.equal(w.player.jumpZ, 0);
});

test('Wassereintritt spritzt einmal; die Schwimmpose beginnt erst nach der Landung', () => {
  const city = realCity(); let spot;
  outer: for (const water of city.list('water')) {
    const b = water.bbox;
    for (let i = 1; i < 10; i++) for (let j = 1; j < 10; j++) {
      const x = b.x + b.w * i / 10, y = b.y + b.h * j / 10;
      if ([[0,0],[-15,0],[15,0],[0,15],[0,-15]].every(([dx,dy]) => surfaceAt(city,x+dx,y+dy) === T.WATER)) { spot = { x, y }; break outer; }
    }
  }
  assert.ok(spot, 'Wasserfläche in der echten Karte gefunden');
  const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.cars = [];
  Object.assign(w.player, spot, { jumpZ: 0.5, jumpV: -1, swimming: false, lvl: 0 }); Object.assign(w.camera, spot);
  const all = [];
  updateWorld(w, idleInput, 1 / 60); all.push(...w.events); assert.equal(w.player.swimming, false);
  for (let i = 0; i < 80; i++) { updateWorld(w, idleInput, 1 / 60); all.push(...w.events); }
  assert.equal(w.player.swimming, true);
  assert.equal(all.filter(e => e.type === 'footSplash').length, 1);
});

test('Rastercache begrenzt Speicher, aktualisiert Lichtstempel und verwirft älteste Flächen', async () => {
  const { RasterCache } = await import('../web/src/rastercache.js');
  let painted = 0; const cache = new RasterCache({ maxPixels: 200, maxEntryPixels: 100, create: () => ({}) });
  const paint = () => painted++;
  const a = cache.get('a', 'day', 10, 10, paint);
  assert.equal(cache.get('a', 'day', 10, 10, paint), a); assert.equal(painted, 1);
  cache.get('b', 'day', 10, 10, paint); cache.get('a', 'night', 10, 10, paint); cache.get('c', 'day', 10, 10, paint);
  assert.equal(cache.items.has('b'), false); assert.equal(cache.pixels, 200);
  assert.equal(cache.get('big', 'day', 20, 20, paint), null); assert.equal(cache.pixels, 200);
  cache.get('a', 'night', 5, 5, paint); assert.equal(cache.pixels, 125);
});
