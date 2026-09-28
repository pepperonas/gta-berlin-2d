// Wetter (weather.js), Nässe, Licht und Bevölkerung bei Wetter, Darstellung (wetfx.js) und Klang.
import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld, resetPopulation } from '../web/src/world.js';
import { createCar, stepCar } from '../web/src/car.js';
import { weatherAt, blockKind, stepWet, weatherLight, hasUmbrella, peopleFactor, WET } from '../web/src/weather.js';
import { lightAt } from '../web/src/daylight.js';
import { edgePuddles, cloudShadows, rainDrops, neonText, neonOn } from '../web/src/wetfx.js';
import { facadeLight } from '../web/src/render.js';
import { ambienceAt } from '../web/src/ambience.js';
import { makeSave, validateSave, applySave } from '../web/src/save.js';
import { onRoad, inBuilding } from '../web/src/map.js';

const city = realCity();
const run = (w, sec) => { for (let i = 0; i < sec * 60; i++) updateWorld(w, idle(), 1 / 60); };

test('Wetter: deterministisch, stetig über Blockgrenzen und Mitternacht, Nebel nur am Morgen, alle Wetterbilder kommen vor', () => {
  assert.deepEqual(weatherAt(7, 3, 600), weatherAt(7, 3, 600));
  const seen = {};
  for (let d = 0; d < 60; d++) for (let b = 0; b < 8; b++) {
    const k = blockKind(1989, d, b);
    seen[k] = (seen[k] ?? 0) + 1;
    if (k === 'fog') assert.ok(b === 1 || b === 2, `Nebel im Block ${b}`);
  }
  for (const k of ['clear', 'cloudy', 'overcast', 'rain', 'fog']) assert.ok(seen[k] > 0, k);
  assert.ok(seen.rain / 480 > 0.08 && seen.rain / 480 < 0.3, `Regenanteil ${seen.rain / 480}`);
  // stetig: minütlich über 10 Tage, auch über Mitternacht (Tagnummer läuft mit)
  let prev = weatherAt(1989, 0, 0), worst = 0;
  for (let t = 1; t < 10 * 1440; t++) {
    const w = weatherAt(1989, Math.floor(t / 1440), t % 1440);
    for (const k of ['cloud', 'rain', 'fog']) worst = Math.max(worst, Math.abs(w[k] - prev[k]));
    prev = w;
  }
  assert.ok(worst < 0.06, `größter Sprung ${worst}`);
  assert.equal(weatherAt(1, 1, 1, 'rain').rain, 1, 'erzwungenes Wetter');
});

test('Nässe: schnell nass, langsam trocken, bleibt zwischen 0 und 1', () => {
  let w = 0;
  for (let i = 0; i < 60; i++) w = stepWet(w, 1, 1);
  assert.equal(w, 1);
  const t10 = (() => { let x = 1, s = 0; while (x > 0) { x = stepWet(x, 0, 1); s++; } return s; })();
  assert.ok(t10 > 300 && Math.abs(t10 - 1 / WET.dry) < 2, `trocken nach ${t10} s`);
  assert.equal(stepWet(0.5, 0, 1e6), 0);
});

test('Licht: Wolken nehmen die Schatten, lösen tags aber keine Lichtkarte aus; Regen und Nebel schon; die Nacht bleibt Nacht', () => {
  const noon = lightAt(13 * 60), night = lightAt(1);
  const cloudy = weatherLight(noon, weatherAt(0, 0, 0, 'overcast')), rain = weatherLight(noon, weatherAt(0, 0, 0, 'rain'));
  assert.ok(cloudy.sun.strength < noon.sun.strength * 0.3);
  assert.equal(cloudy.dark, noon.dark, 'bedeckt: keine Lichtkarte am Tag');
  assert.ok(rain.dark >= 0.3, 'Regen: Scheinwerfer an');
  assert.ok(cloudy.ambient[1] < noon.ambient[1]);
  assert.equal(weatherLight(night, weatherAt(0, 0, 0, 'clear')).dark, night.dark);
});

test('Welt: Wetter nur mit Tagesrhythmus, Tagnummer läuft und steht im Spielstand, Regen macht nass und leerer', () => {
  // ein Samen, dessen natürliches Wetter beim Start nicht klar ist: die feste Welt muss trotzdem klar sein
  let seed = 1; while (weatherAt(seed, 0, 16 * 60).kind === 'clear') seed++;
  const fixed = createWorld({ city, seed, cars: 0, pedestrians: 0 });
  assert.equal(fixed.weather.kind, 'clear', 'gleich beim Erzeugen');
  run(fixed, 1);
  assert.equal(fixed.weather.kind, 'clear');
  const natural = createWorld({ city, seed });
  assert.notEqual(natural.weather.kind, 'clear', 'mit Rhythmus gilt das echte Wetter');
  const w = createWorld({ city }); w.mission.state = 'idle';
  w.clock = 1439.9; run(w, 0.5);
  assert.equal(w.dayCount, 1);
  const s = validateSave(JSON.parse(JSON.stringify(makeSave(w))));
  const w2 = createWorld({ city }); applySave(w2, s);
  assert.equal(w2.dayCount, 1);
  w.clock = 5 * 60; w.day = 1; // früher Dienstagmorgen: Zielzahl weit unter der Obergrenze
  w.forceWeather = 'clear'; resetPopulation(w); run(w, 1);
  const dry = w.pedTarget;
  w.forceWeather = 'rain'; resetPopulation(w); run(w, 3);
  assert.ok(w.wet > 0.05, 'Regen macht nass');
  assert.ok(w.pedTarget < dry, `bei Regen weniger Menschen (${w.pedTarget} < ${dry})`);
  assert.ok(peopleFactor(weatherAt(0, 0, 0, 'rain')) < peopleFactor(weatherAt(0, 0, 0, 'clear')));
});

test('Nasse Fahrbahn: ein Auto rutscht in der Kurve weiter', () => {
  const slide = (wet) => {
    // Auto fährt geradeaus und bekommt einen Stoß zur Seite: wie viel Querfahrt bleibt nach 0,2 s?
    const c = createCar({ x: 0, y: 0 }); c.wet = wet;
    c.vx = 250; c.vy = 150;
    for (let i = 0; i < 12; i++) stepCar(c, 1 / 60, null);
    return Math.abs(-c.vx * Math.sin(c.angle) + c.vy * Math.cos(c.angle));
  };
  assert.ok(slide(1) > slide(0) * 1.05, `${slide(1)} vs ${slide(0)}`);
});

test('Schirme: bei Trockenheit keine, bei Regen viele, je Person fest', () => {
  let n = 0;
  for (let id = 1; id <= 200; id++) { assert.equal(hasUmbrella(id, 0), false); assert.equal(hasUmbrella(id, 0.15), false, 'Nieselregen: noch kein Schirm'); if (hasUmbrella(id, 1)) n++; assert.equal(hasUmbrella(id, 1), hasUmbrella(id, 1)); }
  assert.ok(n > 120 && n < 200, `${n} von 200`);
});

test('Pfützen liegen auf der eigenen Fahrbahn, Wolken ziehen mit dem Wind, Regen dichter bei stärkerem Regen', () => {
  let n = 0;
  for (const e of city.list('edge')) {
    const p = edgePuddles(city, e);
    for (const q of p) { n++; assert.ok(onRoad(city, q.x, q.y) && !inBuilding(city, q.x, q.y), `Pfütze an ${e.name}`); }
    delete e._puddles; assert.deepEqual(edgePuddles(city, e), p);
  }
  assert.ok(n > 1000, `Pfützen ${n}`);
  const v = { x: 0, y: 0, w: 4000, h: 3000 };
  const wx = { ...weatherAt(0, 0, 0, 'cloudy'), wind: { x: 30, y: 0 } };
  const a = cloudShadows(v, wx, 0), b = cloudShadows(v, wx, 10);
  assert.ok(a.length > 0);
  assert.ok(b.some((c) => a.some((d) => Math.abs(c.x - d.x - 300) < 1e-6 && c.y === d.y)), 'Wolken wandern 30 px/s');
  assert.ok(cloudShadows(v, { ...wx, cloud: 0.08 }, 0).length < cloudShadows(v, { ...wx, cloud: 0.95 }, 0).length);
  assert.equal(rainDrops(v, { ...wx, rain: 0 }, 1, 400).length, 0);
  assert.ok(rainDrops(v, { ...wx, rain: 1 }, 1, 400).length > rainDrops(v, { ...wx, rain: 0.3 }, 1, 400).length);
});

test('Leuchtreklame: Späti, Döner, Club, Kneipe; Läden ohne; nur manche flackern', () => {
  assert.equal(neonText({ cat: 'shop', kind: 'convenience', x: 1, y: 2 }), 'SPÄTI');
  assert.equal(neonText({ cat: 'food', kind: 'fast_food', name: 'Kebab Haus', x: 1, y: 2 }), 'DÖNER');
  assert.equal(neonText({ cat: 'food', kind: 'fast_food', name: 'Pizza Roma', x: 1, y: 2 }), 'PIZZA');
  assert.equal(neonText({ cat: 'drink', kind: 'nightclub', x: 1, y: 2 }), 'CLUB');
  assert.equal(neonText({ cat: 'shop', kind: 'clothes', x: 1, y: 2 }), null);
  assert.equal(neonText({ cat: 'hotel', x: 1, y: 2 }), 'HOTEL');
  let flicker = 0;
  for (let i = 0; i < 200; i++) { const q = { x: i * 13, y: i * 7 }; let off = 0; for (let t = 0; t < 5; t += 0.11) if (!neonOn(q, t)) off++; if (off) flicker++; }
  assert.ok(flicker > 5 && flicker < 60, `${flicker} flackernde von 200`);
});

test('Fassaden: der Sonne zugewandte Seite hell, abgewandte dunkel, ohne Sonne neutral', () => {
  const sun = { dx: 0, dy: 1, strength: 1 }; // Schatten nach Süden ⇒ Sonne im Norden
  assert.ok(facadeLight(0, -1, sun) > 0.9, 'Nordwand beschienen');
  assert.ok(facadeLight(0, 1, sun) < -0.9, 'Südwand im Schatten');
  assert.equal(facadeLight(1, 0, { ...sun, strength: 0 }), -0);
});

test('Klang: Regen rauscht, Vögel schweigen im Regen', () => {
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.clock = 6 * 60;
  w.weather = weatherAt(0, 0, 0, 'rain');
  const m = ambienceAt(w);
  assert.equal(m.rain, 1); assert.equal(m.birds, 0);
  w.weather = weatherAt(0, 0, 0, 'clear');
  assert.equal(ambienceAt(w).rain, 0);
});

test('Zeichnen bei Regen in der Nacht und bei Nebel: gültige Koordinaten, Leuchtreklame, Widerschein auf nassem Asphalt', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  const bad = [], texts = [];
  const mk = () => new Proxy({ canvas: { width: 1280, height: 720 } }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'fillText') return (s) => texts.push(s);
      return (...a) => { if (a.some((v) => typeof v === 'number' && !Number.isFinite(v))) bad.push(k); };
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  globalThis.OffscreenCanvas = class { constructor(w, h) { this.width = w; this.height = h; } getContext() { return mk(); } };
  const { Renderer } = await import('../web/src/render.js');
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 22 * 60; w.forceWeather = 'rain'; w.wet = 1;
  resetPopulation(w); run(w, 1);
  const r = new Renderer(mk());
  r.draw(w, 1280, 720, 1);
  assert.deepEqual(bad, []);
  assert.ok(r.stats.drops > 300 && r.stats.puddles > 0, `Tropfen ${r.stats.drops}, Pfützen ${r.stats.puddles}`);
  assert.ok(r._neon.length > 0 && texts.some((s) => r._neon.some((n) => n.text === s)), 'Leuchtreklame gezeichnet');
  const lights = r.collectLights(w, { x: w.camera.x - 700, y: w.camera.y - 400, w: 1400, h: 800 }, r.light, false);
  const refl = lights.filter((l, i) => lights.some((o, j) => j < i && o.rgb === l.rgb && o.x === l.x && l.y > o.y));
  assert.ok(refl.length > 3, `Widerscheine ${refl.length}`);
  w.forceWeather = 'fog'; w.clock = 7 * 60; run(w, 0.1);
  r.draw(w, 1280, 720, 1);
  assert.equal(r.stats.fog, true); assert.equal(r.stats.drops, 0);
  assert.deepEqual(bad, []);
});
