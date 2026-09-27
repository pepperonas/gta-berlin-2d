// Tiere (animals.js), Umgebungsklang (ambience.js) und Zeichnen der neuen Figuren ohne kaputte Koordinaten.
import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { animalSpots, manageAnimals, updateAnimals, ANIMALS } from '../web/src/animals.js';
import { ambienceAt, birdLevel, bellStrikes, AMB } from '../web/src/ambience.js';
import { createCar } from '../web/src/car.js';
import { surfaceAt, inBuilding, T } from '../web/src/map.js';
import { BUILDING_KIND } from '../web/src/citycodes.js';
import { LIFE } from '../web/src/life.js';

const city = realCity();
const giver = city.places.giver;
const run = (w, sec) => { for (let i = 0; i < sec * 60; i++) updateWorld(w, idle(), 1 / 60); };

test('Tierplätze: Tauben auf Plätzen und vor Imbissen (nie im Haus), Enten nur auf dem Wasser, deterministisch', () => {
  const water = city.list('water').filter((f) => f.bbox.w * f.bbox.h > 250000).slice(0, 25);
  const spots = [...animalSpots(city, giver.x, giver.y, 4000), ...water.flatMap((f) => animalSpots(city, f.bbox.x + f.bbox.w / 2, f.bbox.y + f.bbox.h / 2, 3000))];
  const pig = spots.filter((s) => s.kind === 'pigeon'), duck = spots.filter((s) => s.kind === 'duck');
  assert.ok(pig.length > 10 && duck.length > 10, `Tauben ${pig.length}, Enten ${duck.length}`);
  for (const s of pig) assert.ok(!inBuilding(city, s.x, s.y), s.key);
  for (const s of duck) assert.equal(surfaceAt(city, s.x, s.y), T.WATER, s.key);
  assert.deepEqual(animalSpots(city, giver.x, giver.y).map((s) => s.key), animalSpots(city, giver.x, giver.y).map((s) => s.key));
});

test('Tauben fliegen auf, wenn man hingeht, verschwinden außer Sicht und kommen erst wieder, wenn der Platz außer Sicht ist', () => {
  const w = createWorld({ city }); w.mission.state = 'idle';
  run(w, 1 / 60);
  const birds = w.animals.filter((a) => a.kind === 'pigeon');
  assert.ok(birds.length > 5);
  for (const a of w.animals) assert.ok(!inBuilding(city, a.x, a.y));
  const a = birds[0], key = a.key, flock = w.animals.filter((b) => b.key === key);
  // Spielfigur mitten in den Schwarm
  w.player.x = a.x + 5; w.player.y = a.y;
  updateAnimals(w, 1 / 60);
  assert.ok(flock.every((b) => b.state === 'fly' || Math.hypot(b.x - a.x, b.y - a.y) > ANIMALS.scarePerson), 'Schwarm fliegt auf');
  w.player.x = giver.x + 5000; // Spielfigur weg, damit nichts neu scheucht
  for (let i = 0; i < 8 * 60; i++) updateAnimals(w, 1 / 60);
  assert.ok(flock.some((b) => b.z > 20 || b.state === 'peck'), 'im Flug oder woanders gelandet');
  // Kamera weit weg: der Schwarm wird abgebaut; zurück: wieder da (außer Sicht aufgestellt)
  const cam = { ...w.camera };
  w.camera.x += 30000; w._animT = -99; manageAnimals(w);
  assert.equal(w.animals.filter((b) => b.key === key).length, 0);
  Object.assign(w.camera, cam); w._animT = -99; manageAnimals(w);
  assert.ok(w.animals.length > 0, 'Schwärme wieder aufgestellt');
  for (const b of w.animals) assert.ok(Math.abs(b.x - cam.x) >= LIFE.viewHalfX + 60 - 40 || Math.abs(b.y - cam.y) >= LIFE.viewHalfY + 60 - 40, 'Taube im Bild aufgetaucht');
});

test('Schuss oder Hupe scheucht Tauben im weiten Umkreis; Enten schwimmen weg, bleiben aber im Wasser', () => {
  const w = createWorld({ city }); w.mission.state = 'idle';
  run(w, 1 / 60);
  const p = w.animals.find((a) => a.kind === 'pigeon');
  w.player.x = p.x + 5000;
  w.events.push({ type: 'shot', x: p.x + ANIMALS.scareShot - 40, y: p.y });
  updateAnimals(w, 1 / 60);
  assert.equal(p.state, 'fly');
  // Enten: am Wasser aufstellen
  const f = city.list('water').find((x) => x.bbox.w * x.bbox.h > 250000);
  const s = animalSpots(city, f.bbox.x + f.bbox.w / 2, f.bbox.y + f.bbox.h / 2, 3000).find((q) => q.kind === 'duck');
  w.animals = [{ kind: 'duck', key: s.key, x: s.x, y: s.y, z: 0, vx: 0, vy: 0, facing: 0, state: 'swim', t: 0, hx: s.x, hy: s.y, seed: 3, flap: 0 }];
  const d = w.animals[0];
  w.player.x = d.x + 20; w.player.y = d.y;
  updateAnimals(w, 1 / 60);
  assert.equal(d.state, 'flee');
  w.player.x += 5000;
  for (let i = 0; i < 30 * 60; i++) { updateAnimals(w, 1 / 60); assert.equal(surfaceAt(city, d.x, d.y), T.WATER, 'Ente an Land'); }
});

test('Vogelgesang: Morgenchor laut, mittags leiser, nachts still; im Park bei Tag Vögel, nachts nicht', () => {
  assert.equal(birdLevel(6 * 60), 1);
  assert.ok(birdLevel(13 * 60) < 0.6 && birdLevel(13 * 60) > 0);
  assert.equal(birdLevel(2 * 60), 0);
  assert.equal(birdLevel(23 * 60), 0);
  const park = city.list('area').filter((a) => a.kind === 4 && a.bbox.w * a.bbox.h > 4e6)[0]; // große Grünfläche
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.camera.x = park.bbox.x + park.bbox.w / 2; w.camera.y = park.bbox.y + park.bbox.h / 2;
  w.clock = 6 * 60; assert.ok(ambienceAt(w).birds > 0.3);
  w.clock = 2 * 60; assert.equal(ambienceAt(w).birds, 0);
});

test('Umgebung: Gemurmel vor Kneipen am Abend, Verkehr hörbar, Martinshorn in der Nähe mit wechselndem Ton', () => {
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 22 * 60 + 30; w.day = 4;
  run(w, 1 / 60);
  w.camera.x = giver.x; w.camera.y = giver.y;
  const m = ambienceAt(w);
  assert.ok(m.bar > 0.1, `Bar ${m.bar}`);
  for (const k of ['hum', 'traffic', 'birds', 'bar', 'water', 'rumble']) assert.ok(m[k] >= 0 && m[k] <= 1, k);
  const c = createCar({ x: giver.x + 200, y: giver.y, kind: 'police' }); c.siren = true; w.cars.push(c);
  const tones = new Set();
  for (let t = 0; t < 2.4; t += 0.1) { w.time = t; const s = ambienceAt(w).sirens; assert.equal(s.length, 1); tones.add(s[0].high); assert.ok(s[0].gain > 0.8); }
  assert.equal(tones.size, 2, 'tatü-tata');
  c.x += AMB.siren + 100;
  assert.equal(ambienceAt(w).sirens.length, 0, 'zu weit weg');
});

test('Kirchenglocke schlägt zur vollen Stunde, wenn eine Kirche in Hörweite ist', () => {
  const church = city.list('building').find((b) => b.kind === BUILDING_KIND.church);
  assert.ok(church);
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.camera.x = church.cx; w.camera.y = church.cy;
  w.clock = 15 * 60 + 0.1; assert.equal(bellStrikes(w, 14 * 60 + 59.9), 3);
  w.clock = 0.05; assert.equal(bellStrikes(w, 1439.95), 12, 'Mitternacht');
  w.clock = 15 * 60 + 30; assert.equal(bellStrikes(w, 15 * 60 + 29.9), 0, 'keine volle Stunde');
  // ein Haus, in dessen Hörweite keine Kirche steht (aber andere Gebäude)
  const R = AMB.bells, churches = city.list('building').filter((b) => b.kind === BUILDING_KIND.church);
  const far = city.list('building').find((b) => b.kind !== BUILDING_KIND.church && churches.every((c) => Math.abs(c.cx - b.cx) > R + 400 || Math.abs(c.cy - b.cy) > R + 400));
  assert.ok(far, 'Ort ohne Kirche gefunden');
  w.camera.x = far.cx; w.camera.y = far.cy; w.clock = 15 * 60 + 0.1;
  assert.equal(bellStrikes(w, 14 * 60 + 59.9), 0, 'keine Kirche in Hörweite');
});

test('Zeichnen: Nutzfahrzeuge, Blaulicht, Räder, Tauben und Roller ohne ungültige Koordinaten, Blaulicht leuchtet nachts', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  const bad = [];
  const mk = () => new Proxy({ canvas: { width: 1280, height: 720 } }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      return (...a) => { if (a.some((v) => typeof v === 'number' && !Number.isFinite(v))) bad.push(k); };
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  globalThis.OffscreenCanvas = class { constructor(w, h) { this.width = w; this.height = h; } getContext() { return mk(); } };
  const { Renderer } = await import('../web/src/render.js');
  const ctx = mk();
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 23 * 60;
  run(w, 2);
  const p = w.player;
  const kinds = ['truck', 'delivery', 'garbage', 'police', 'ambulance'];
  kinds.forEach((k, i) => { const c = createCar({ x: p.x - 150 + i * 70, y: p.y + 60, kind: k, angle: 1 }); c.blue = true; c.work = true; c.hazard = true; c.driver = 'npc'; w.cars.push(c); });
  const r = new Renderer(ctx);
  const lights = r.collectLights(w, { x: p.x - 700, y: p.y - 400, w: 1400, h: 800 }, { dark: 1, lampsOn: true, windowsLit: 1, ambient: [20, 20, 40], sun: { dx: 0, dy: 0, len: 0 } }, false);
  assert.ok(lights.some((l) => l.rgb === '60,130,255'), 'Blaulicht in der Lichtkarte');
  for (const t of [0, 0.3]) { w.time = t; r.draw(w, 1280, 720, 1); }
  assert.deepEqual(bad, []);
  assert.ok(w.bikes.length > 0 && w.animals.length > 0, 'Räder und Tauben im Bild');
});
