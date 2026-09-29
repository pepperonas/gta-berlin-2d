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
  const bridge = city.list('edge').find((e) => e.bridge && (e.lvl ?? 0) >= 1 && e.len > 400); // Mitte weit weg von den Anschlüssen
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

import { stepCar } from '../web/src/car.js';
import { updateWorld } from '../web/src/world.js';
import { idle } from './helpers/bot.js';
import { AQUA } from '../web/src/traction.js';

function puddleRun(fps, kmh) {
  const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle'; w.wet = 1; w.forceWeather = 'rain';
  const e = city.list('edge').find((x) => !x.bridge && (x.lvl ?? 0) === 0 && x.cls <= 6 && edgePuddles(city, x).length && edgePuddles(city, x)[0].rx > 12);
  const pd = edgePuddles(city, e)[0];
  const car = w.cars.find((c) => c.id === w.playerCarId);
  w.player.inCar = car.id; car.driver = 'player';
  const v = kmh / 0.36; car.angle = pd.a; car.x = pd.x - Math.cos(pd.a) * v * 0.3; car.y = pd.y - Math.sin(pd.a) * v * 0.3; car.lvl = 0;
  car.vx = Math.cos(pd.a) * v; car.vy = Math.sin(pd.a) * v; car.angVel = 0;
  w.camera.x = car.x; w.camera.y = car.y;
  const events = [];
  for (let i = 0; i < fps; i++) { updateWorld(w, { ...idle(), throttle: 1 }, 1 / fps); events.push(...w.events.filter((x) => x.type === 'aquaplane')); if (car.aqua > 0) car.sawAqua = true; }
  return { events, car, pd };
}

test('Aquaplaning in einer echten Pfütze: nur über 70 km/h, ein Ereignis, gleich bei 30 und 60 fps', () => {
  const slow = puddleRun(60, 50), fast = puddleRun(60, 90), fast30 = puddleRun(30, 90);
  assert.equal(slow.events.length, 0, 'langsam: kein Aquaplaning');
  assert.equal(fast.events.length, 1, 'schnell: genau ein Ereignis');
  assert.equal(fast30.events.length, 1, '30 fps: ebenso');
  assert.ok(fast.events[0].player, 'Spielerauto markiert');
  assert.equal(puddleRun(60, 90).car.angle.toFixed(6), fast.car.angle.toFixed(6), 'deterministisch');
});

test('Böen in der Welt: Pkw versetzt 0,5–1 m, auf der Brücke mehr, geparkte Autos bewegen sich nie', () => {
  const offset = (lvl) => {
    const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle'; w.forceWeather = 'storm';
    w.weather = weatherAt(w.seed, 0, 600, 'storm');
    // Wind quer zur Fahrtrichtung: Auto fährt senkrecht zum Wind
    const a = Math.atan2(w.weather.wind.y, w.weather.wind.x) + Math.PI / 2;
    // stärkste Böe der ersten 10 min suchen, dort 3 s messen
    let best = 0, bt = 0; for (let t = 0; t < 600; t += 0.1) { const g = gustPushProbe(w, t); if (g > best) { best = g; bt = t; } }
    const c = createCar({ x: 0, y: 0 }); c.angle = a; c.vx = Math.cos(a) * 139; c.vy = Math.sin(a) * 139; c.lvl = lvl; c.driver = 'player';
    let side = 0;
    for (let t = bt - 1.5; t < bt + 1.5; t += 1 / 60) {
      w.time = t;
      const push = gustPush(w, c, lvl);
      if (push) { c.vx += push.ax / 60; c.vy += push.ay / 60; }
      c.controls.throttle = 0.35; stepCar(c, 1 / 60, null);
    }
    side = Math.abs(-c.x * Math.sin(a) + c.y * Math.cos(a));
    return side / 10; // m
  };
  const ground = offset(0), onBridge = offset(1);
  assert.ok(ground >= 0.5 && ground <= 1, `Versatz ${ground.toFixed(2)} m`);
  assert.ok(onBridge > ground * 1.4 && onBridge <= 2, `Brücke ${onBridge.toFixed(2)} m`);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.forceWeather = 'storm';
  for (let i = 0; i < 60; i++) updateWorld(w, idle(), 1 / 60);
  const parked = w.cars.filter((c) => c.role === 'curb' && c.driver === null).slice(0, 10).map((c) => ({ c, x: c.x, y: c.y }));
  assert.ok(parked.length > 3, 'geparkte Autos im Bild');
  for (let i = 0; i < 20 * 60; i++) updateWorld(w, idle(), 1 / 60);
  for (const p of parked) assert.ok(Math.hypot(p.c.x - p.x, p.c.y - p.y) < 0.01, 'geparktes Auto bewegt sich nicht');
});
const gustPushProbe = (w, t) => { w.time = t; const p = gustPush(w, Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0 }), 0); return p ? Math.hypot(p.ax, p.ay) : 0; };

test('Warnschild: Vorrang Aquaplaning > Glätte > Schnee > Sturm > Nässe; zu Fuß keins', async () => {
  const { roadWarning } = await import('../web/src/traction.js');
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  const car = w.cars.find((c) => c.id === w.playerCarId);
  car.x = city.places.playerCar.x; car.y = city.places.playerCar.y; car.lvl = 0; // nicht überdacht
  w.player.inCar = null;
  w.wet = 1; assert.equal(roadWarning(w), null, 'zu Fuß: nichts');
  w.player.inCar = car.id; car.driver = 'player';
  w.wet = 0; w.snow = 0; w.ice = 0; w.weather = weatherAt(1, 0, 600, 'clear'); assert.equal(roadWarning(w), null);
  w.wet = 1; assert.equal(roadWarning(w), 'Nässe');
  w.snow = 0.5; assert.equal(roadWarning(w), 'Schnee');
  w.ice = 0.5; assert.equal(roadWarning(w), 'Glätte');
  car.aqua = 0.2; assert.equal(roadWarning(w), 'Aquaplaning!');
});

test('Überdacht nur, was wirklich darüber liegt: vor einem Brückenanfang ist die Straße nass, nicht trocken', () => {
  const w = world(); w.wet = 1; w.ice = 1;
  let checked = 0, wrong = 0;
  for (const b of city.list('edge')) {
    if (!b.bridge || (b.lvl ?? 0) < 1) continue;
    for (const g of city.list('edge')) {
      if (g.bridge || (g.lvl ?? 0) !== 0 || g.junction) continue;
      const shared = [g.a, g.b].find((n) => n === b.a || n === b.b);
      if (shared === undefined) continue;
      // 3 m vor dem gemeinsamen Knoten auf der Bodenstraße
      const atA = shared === g.a, p = g.pts, n = p.length;
      const [x0, y0, x1, y1] = atA ? [p[0], p[1], p[2], p[3]] : [p[n - 2], p[n - 1], p[n - 4], p[n - 3]];
      const L = Math.hypot(x1 - x0, y1 - y0); if (L < 40) continue;
      const x = x0 + (x1 - x0) / L * 30, y = y0 + (y1 - y0) / L * 30;
      checked++; if (roadCondition(w, x, y, 0).covered) wrong++;
      if (checked >= 60) break;
    }
    if (checked >= 60) break;
  }
  assert.ok(checked >= 20, `${checked} Brückenanfänge geprüft`);
  assert.equal(wrong, 0, `${wrong} von ${checked} Stellen vor einer Brücke fälschlich überdacht`);
});

test('Warnschild als Zugführer: im Tunnel keins (Schienen trocken wie in trainAdhesion), oben nach Wetter', async () => {
  const { roadWarning } = await import('../web/src/traction.js');
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  const road = city.list('edge').find((e) => !e.bridge && !e.passage && (e.lvl ?? 0) === 0 && e.cls <= 6 && e.len > 200);
  const k = (road.pts.length >> 1) & ~1;
  w.player.x = road.pts[k]; w.player.y = road.pts[k + 1]; w.player.inCar = null;
  w.wet = 1; w.snow = 0; w.ice = 0; w.weather = weatherAt(1, 0, 600, 'rain');
  w.player.ride = { kind: 'driver', mode: 'tram', underground: false };
  assert.equal(roadWarning(w), 'Nässe');
  w.player.ride = { kind: 'driver', mode: 'ubahn', underground: true };
  assert.equal(roadWarning(w), null, 'im Tunnel');
});

test('Aquaplaning versetzt die Fahrtrichtung spürbar (Gieren, solange das Auto schwimmt)', async () => {
  const { aquaYaw } = await import('../web/src/traction.js');
  const r = puddleRun(60, 90), yaw = aquaYaw(r.pd);
  assert.ok(Math.abs(yaw) > 0.1, `Pfütze mit spürbarem Gieren (${yaw.toFixed(2)} rad/s)`);
  const dev = Math.abs(Math.atan2(Math.sin(r.car.angle - r.pd.a), Math.cos(r.car.angle - r.pd.a)));
  assert.ok(dev >= 0.5 * Math.abs(yaw) * AQUA.time, `Abweichung ${(dev * 180 / Math.PI).toFixed(1)}° (Soll ≥ ${(0.5 * Math.abs(yaw) * AQUA.time * 180 / Math.PI).toFixed(1)}°)`);
});
