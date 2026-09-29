import test from 'node:test';
import assert from 'node:assert/strict';
import { roadCondition, tractionOf, gustPush, adhesionOf, puddleAt, TRACTION, DRY, GUST } from '../web/src/traction.js';
import { edgePuddles } from '../web/src/wetfx.js';
import { createWorld } from '../web/src/world.js';
import { createCar } from '../web/src/car.js';
import { weatherAt } from '../web/src/weather.js';
import { realCity } from './helpers/city.js';

const city = realCity();
const world = () => { const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.wet = 0; w.snow = 0; w.ice = 0; return w; };
const mid = (e) => { const k = (e.pts.length >> 1) & ~1; return { x: e.pts[k], y: e.pts[k + 1] }; };

test('Haftung: trocken exakt 1, Tabellenwerte je Zustand, gemischt mit Untergrenze', () => {
  assert.deepEqual(tractionOf({ wet: 0, snow: 0, ice: 0 }), DRY);
  assert.deepEqual(tractionOf({ wet: 1, snow: 0, ice: 0 }), TRACTION.wet);
  assert.deepEqual(tractionOf({ wet: 0, snow: 1, ice: 0 }), TRACTION.snow);
  assert.deepEqual(tractionOf({ wet: 0, snow: 0, ice: 1 }), TRACTION.ice);
  const half = tractionOf({ wet: 0.5, snow: 0, ice: 0 });
  assert.ok(Math.abs(half.brake - (1 - 0.5 * (1 - TRACTION.wet.brake))) < 1e-9, 'linear nach Stärke');
  const worst = tractionOf({ wet: 1, snow: 1, ice: 1 });
  for (const k of ['brake', 'accel', 'lat', 'steer']) assert.equal(worst[k], TRACTION.ice[k], `${k}: Untergrenze Glätte`);
});

test('Straßenzustand: Weltwerte, Brücke glatter, überdacht (Durchfahrt, unter der Brücke) trocken', () => {
  const w = world(); w.wet = 1; w.snow = 0.4; w.ice = 0.5;
  const road = city.list('edge').find((e) => !e.bridge && !e.passage && (e.lvl ?? 0) === 0 && e.cls <= 6 && e.len > 200);
  const r = roadCondition(w, mid(road).x, mid(road).y, 0);
  assert.equal(r.wet, 1); assert.equal(r.snow, 0.4); assert.equal(r.ice, 0.5); assert.equal(r.covered, false);
  const bridge = city.list('edge').find((e) => e.bridge && (e.lvl ?? 0) >= 1 && e.len > 60);
  const b = mid(bridge);
  assert.equal(roadCondition(w, b.x, b.y, bridge.lvl).ice, 0.75, 'Brücke ×1,5');
  const under = roadCondition(w, b.x, b.y, 0);
  assert.equal(under.covered, true, 'unter der Brücke überdacht');
  assert.equal(under.wet + under.snow + under.ice, 0, 'unter der Brücke trocken');
  const passage = city.list('edge').find((e) => e.passage && e.len > 20);
  assert.ok(passage, 'Durchfahrt im Testgebiet');
  const p = mid(passage);
  assert.equal(roadCondition(w, p.x, p.y, passage.lvl ?? 0).covered, true, 'Durchfahrt überdacht');
});

test('Pfütze: nur in der gezeichneten Ellipse und nur bei Nässe', () => {
  const w = world();
  const e = city.list('edge').find((x) => !x.bridge && (x.lvl ?? 0) === 0 && edgePuddles(city, x).length);
  const pd = edgePuddles(city, e)[0];
  w.wet = 0.2; assert.equal(roadCondition(w, pd.x, pd.y, 0).puddle, null, 'zu trocken');
  w.wet = 1; assert.ok(roadCondition(w, pd.x, pd.y, 0).puddle, 'in der Pfütze');
  assert.deepEqual(puddleAt(w, pd.x, pd.y, 0), pd);
  const off = pd.rx + 30;
  assert.equal(puddleAt(w, pd.x - Math.sin(pd.a) * off, pd.y + Math.cos(pd.a) * off, 0), null, 'daneben');
});

test('Böen: quer zur Windrichtung, Brücke ×1,6, schwere Fahrzeuge weniger, stehende und geparkte gar nicht', () => {
  const w = world(); w.weather = weatherAt(1, 0, 600, 'storm'); w.time = 0;
  // Zeitpunkt einer kräftigen Böe
  let t = 0; while (!gustPush(w, Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0 }), 0)) { t += 0.1; w.time = t; }
  const car = Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0 });
  const g = gustPush(w, car, 0), gb = gustPush(w, car, 1);
  const wl = Math.hypot(w.weather.wind.x, w.weather.wind.y);
  assert.ok(Math.abs(g.ax / Math.hypot(g.ax, g.ay) - w.weather.wind.x / wl) < 1e-9, 'Windrichtung');
  assert.ok(Math.abs(Math.hypot(gb.ax, gb.ay) / Math.hypot(g.ax, g.ay) - GUST.bridge) < 1e-9, 'Brücke');
  const van = Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0, hw: 32, hh: 13 });
  assert.ok(Math.hypot(...Object.values(gustPush(w, van, 0))) < Math.hypot(g.ax, g.ay), 'schwerer: weniger');
  assert.equal(gustPush(w, Object.assign(createCar({ x: 0, y: 0 }), { vx: 2, vy: 0 }), 0), null, 'steht');
  const parked = Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0, role: 'curb', driver: null });
  assert.equal(gustPush(w, parked, 0), null, 'geparkt');
  w.weather = weatherAt(1, 0, 600, 'clear');
  assert.equal(gustPush(w, car, 0), null, 'kein Sturm');
});

test('Schienenhaftung: trocken 1, nass 0,75, Frost 0,6, nie darunter', () => {
  assert.equal(adhesionOf({ wet: 0, snow: 0, ice: 0 }), 1);
  assert.equal(adhesionOf({ wet: 1, snow: 0, ice: 0 }), TRACTION.rail.wet);
  assert.equal(adhesionOf({ wet: 1, snow: 1, ice: 1 }), TRACTION.rail.ice);
});
