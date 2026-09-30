// Neue Fahrzeuge (0.43.0): Motorrad und Roller mit Wheelie-/Stoppie-Grenze und Abwurf beim Aufprall, dazu weitere
// Pkw mit eigenem Charakter (Trommelbremsen, keine Fahrhilfen, leichte Hinterachse). Nur Physik und Spiellogik.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createCar, stepCar } from '../web/src/car.js';
import { SPECS, specLine } from '../web/src/carmodels.js';
import { isOpenKind, isMotoKind, KINDS, pickKind } from '../web/src/fleet.js';
import { tractionOf } from '../web/src/traction.js';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { ambienceAt } from '../web/src/ambience.js';

const DT = 1 / 60;
const kmh = (c) => Math.hypot(c.vx, c.vy) * 0.36;
function veh(model, { v0 = 0, esp = true, traction } = {}) {
  const kind = isMotoKind(model) ? model : 'car';
  const c = createCar({ x: 0, y: 0, kind }); if (kind === 'car') c.model = model;
  c.driver = 'player'; c.esp = esp; if (traction) c.traction = traction; c.vx = v0 / 0.36;
  return c;
}
const run = (c, secs, ctl, each) => { Object.assign(c.controls, ctl); for (let i = 0; i < secs * 60; i++) { stepCar(c, DT, null); each?.(c); } return c; };
const brakeDist = (m) => { const c = veh(m, { v0: 100 }); const x0 = c.x; run(c, 8, { brake: 1 }, (q) => { if (q.vx < 0) q.vx = 0; }); return (c.x - x0) / 10; };

test('Motorrad: Wheelie begrenzt den Anzug, Stoppie die Bremse – es bremst länger als ein Auto', () => {
  const s = SPECS.motorcycle, g = 9.81;
  const wheelie = g * s.front * s.wb / s.h, stoppie = g * (1 - s.front) * s.wb / s.h;
  let axMax = 0, axMin = 0, wh = 0, st = 0;
  const c = run(veh('motorcycle'), 3, { throttle: 1 }, (q) => { axMax = Math.max(axMax, q.dyn.ax); wh = Math.max(wh, q.dyn.wheelie); });
  assert.ok(axMax <= wheelie * 1.001, `Anzug ${axMax.toFixed(1)} ≤ Wheelie-Grenze ${wheelie.toFixed(1)} m/s²`);
  assert.ok(wh > 0.5, 'Vorderrad kommt hoch (sichtbar)');
  assert.ok(kmh(c) > 90, `schnell: ${kmh(c).toFixed(0)} km/h nach 3 s`);
  run(veh('motorcycle', { v0: 100 }), 2, { brake: 1 }, (q) => { axMin = Math.min(axMin, q.dyn.ax); st = Math.max(st, q.dyn.stoppie); });
  assert.ok(-axMin <= stoppie * 1.06, `Bremse (plus Fahrtwind) ${(-axMin).toFixed(1)} ≤ Stoppie-Grenze ${stoppie.toFixed(1)} m/s²`);
  assert.ok(st > 0.5, 'Hinterrad wird leicht');
  assert.ok(brakeDist('motorcycle') > brakeDist('limousine') * 1.2, `Motorrad ${brakeDist('motorcycle').toFixed(1)} m, Limousine ${brakeDist('limousine').toFixed(1)} m`);
  // Roller: wenig Leistung, Spitze ≈ 95 km/h; Schräglage in der Kurve
  const r = run(veh('scooter'), 40, { throttle: 1 });
  assert.ok(Math.abs(kmh(r) - SPECS.scooter.vmax) < 6, `Roller ${kmh(r).toFixed(0)} km/h`);
  const k = run(veh('motorcycle', { v0: 60 }), 1.2, { steer: 0.6, throttle: 0.3 });
  assert.ok(Math.abs(k.dyn.lean) > 0.3, `legt sich in die Kurve (${(k.dyn.lean * 57.3).toFixed(0)}°)`);
  assert.equal(specLine(veh('motorcycle')), 'Motorrad · Vierzylinder · Kette · 110 kW');
});

test('Neue Pkw: Oldtimer bremst schlechter, Muscle-Car ohne Fahrhilfen dreht sich unter Gas, Pick-up hat leere Hinterachse', () => {
  assert.ok(brakeDist('oldtimer') > brakeDist('limousine') * 1.3, 'Trommelbremsen');
  // Muscle-Car: Vollgas in der Kurve – ohne ESP (Werk) bricht das Heck aus; der Hot Hatch (ESP, Front) bleibt ruhig
  const slide = (m) => { const c = veh(m, { v0: 45 }); run(c, 1.2, { steer: 0.45, throttle: 0.25 }); let a = 0; run(c, 1.5, { throttle: 1 }, (q) => { a = Math.max(a, Math.abs(q.dyn.alphaR)); }); return a; };
  assert.ok(slide('musclecar') > 0.3, `Muscle-Car quer (${slide('musclecar').toFixed(2)} rad)`);
  assert.ok(slide('hothatch') < 0.15, `Hot Hatch ruhig (${slide('hothatch').toFixed(2)} rad)`);
  // Pick-up auf Schnee: leichte Hinterachse → schlechter vom Fleck als ein gleich starker Allrad
  const snow = tractionOf({ snow: 1 });
  const t50 = (m) => { const c = veh(m, { traction: snow }); for (let t = 0; t < 30; t += DT) { if (kmh(c) >= 50) return t; c.controls.throttle = 1; stepCar(c, DT, null); } return 30; };
  assert.ok(t50('pickup') > t50('rallye'), `Pick-up ${t50('pickup').toFixed(1)} s, Rallye ${t50('rallye').toFixed(1)} s`);
  assert.ok(SPECS.kleinbus.engine === 'rear' && SPECS.roadster.front === 0.5);
});

test('Aufprall mit dem Motorrad wirft den Fahrer ab; Zweiräder: kein Missionsauto, keine Kisten, Stadt ungedämpft', () => {
  const city = realCity();
  const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle';
  const pc = w.cars.find((c) => c.id === w.playerCarId);
  const m = createCar({ x: pc.x, y: pc.y, angle: pc.angle, kind: 'motorcycle', role: 'parked' }); m.driver = null;
  pc.x += 400; w.cars.push(m);
  w.player.x = m.x - Math.sin(m.angle) * 12; w.player.y = m.y + Math.cos(m.angle) * 12;
  updateWorld(w, { ...idle(), enterExit: true }, DT);
  assert.equal(w.player.inCar, m.id, 'aufgestiegen');
  assert.notEqual(w.playerCarId, m.id, 'kein Missionsauto');
  assert.equal(ambienceAt(w).inCar, false, 'offen: nicht gedämpft');
  assert.ok(isOpenKind('motorcycle') && isOpenKind('scooter') && isOpenKind('bicycle') && !isOpenKind('car'));
  // gegen eine Wand: vor dem Motorrad ein fester Pfosten
  const post = { x: m.x + Math.cos(m.angle) * 90, y: m.y + Math.sin(m.angle) * 90, r: 12, layer: 'test' };
  w.solids.insert(post, { x: post.x - 12, y: post.y - 12, w: 24, h: 24 });
  const hp0 = w.player.hp ?? 100;
  for (let i = 0; i < 120 && w.player.inCar; i++) updateWorld(w, { ...idle(), throttle: 1 }, DT);
  assert.equal(w.player.inCar, null, 'abgeworfen');
  assert.ok(m.fallen && m.driver === null, 'Maschine liegt');
  assert.ok(w.player.stun > 0 && (w.player.hp ?? 100) < hp0, 'benommen und verletzt');
  // wieder aufsteigen richtet sie auf
  for (let i = 0; i < 200 && w.player.stun > 0; i++) updateWorld(w, idle(), DT);
  w.player.x = m.x - Math.sin(m.angle) * 12; w.player.y = m.y + Math.cos(m.angle) * 12;
  updateWorld(w, { ...idle(), enterExit: true }, DT);
  assert.equal(w.player.inCar, m.id); assert.equal(m.fallen, false);
  // im Verkehr: Zweiräder tagsüber, nachts kaum
  let day = 0, night = 0;
  for (let i = 0; i < 4000; i++) { const r = i / 4000; if (isMotoKind(pickKind(14 * 60, 2, 6, r))) day++; if (isMotoKind(pickKind(3 * 60, 2, 6, r))) night++; }
  assert.ok(day > night * 3 && day > 60, `Zweiräder tags ${day}, nachts ${night}`);
  assert.ok(KINDS.motorcycle.L < 30 && KINDS.scooter.L < KINDS.motorcycle.L);
});
