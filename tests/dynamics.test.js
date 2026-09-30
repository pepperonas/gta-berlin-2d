// Fahrdynamik (dynamics.js): Antrieb, Motorlage, Schwerpunkt und Fahrhilfen müssen sich so auswirken, wie es die Physik
// verlangt. Vergleiche ändern jeweils nur eine Größe (Varianten derselben Daten), damit genau diese den Unterschied macht.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createCar, stepCar } from '../web/src/car.js';
import { SPECS, CAR_MODELS, specLine, carModel } from '../web/src/carmodels.js';
import { geometry } from '../web/src/dynamics.js';
import { bodyShift } from '../web/src/vehicles.js';
import { tractionOf } from '../web/src/traction.js';

const DT = 1 / 60;
const kmh = (c) => Math.hypot(c.vx, c.vy) * 0.36;
// Variante eines Modells (nur für diesen Test registriert)
let nv = 0;
function variant(base, over) { const k = `test${nv++}`; SPECS[k] = { ...SPECS[base], ...over }; return k; }
function car(model, { esp = true, traction, v0 = 0, angle = 0 } = {}) {
  const c = createCar({ x: 0, y: 0, angle }); c.model = model; c.driver = 'player'; c.esp = esp;
  if (traction) c.traction = traction;
  c.vx = v0 / 0.36 * Math.cos(angle); c.vy = v0 / 0.36 * Math.sin(angle);
  return c;
}
const run = (c, secs, ctl, each) => { Object.assign(c.controls, ctl); for (let i = 0; i < secs * 60; i++) { stepCar(c, DT, null); each?.(c, i); } return c; };
const timeTo = (c, target, ctl = { throttle: 1 }, max = 60) => { Object.assign(c.controls, ctl); for (let t = 0; t < max; t += DT) { if (kmh(c) >= target) return t; stepCar(c, DT, null); } return Infinity; };

test('Spielgefühl: Anzug etwas kräftiger als echt (Reihenfolge wie im Datenblatt), echtes Höchsttempo, Bremsen wie im Actionfilm', () => {
  const range = { kleinwagen: [6.5, 11], kompakt: [5, 8.5], limousine: [3.8, 6.5], elektro: [2.5, 4.5], sportwagen: [2.3, 4.5], zweitakter: [15, 27] };
  for (const [m, [a, b]] of Object.entries(range)) {
    const t = timeTo(car(m), 100);
    assert.ok(t >= a && t <= b, `${m}: 0–100 in ${t.toFixed(1)} s`);
  }
  for (const m of CAR_MODELS) {
    const c = run(car(m), 90, { throttle: 1 });
    assert.ok(Math.abs(kmh(c) - SPECS[m].vmax) / SPECS[m].vmax < 0.06, `${m}: Spitze ${kmh(c).toFixed(0)} statt ${SPECS[m].vmax}`);
    const b = car(m, { v0: 100 }); const x0 = b.x;
    run(b, 6, { throttle: 0, brake: 1 }, (q) => { if (q.vx < 0) q.vx = 0; });
    const d = (b.x - x0) / 10;
    assert.ok(d > 18 && d < 36 / (SPECS[m].brakeK ?? 1), `${m}: Bremsweg ${d.toFixed(1)} m (echt ≈ 36–45 m; alte Trommelbremsen länger)`);
  }
});

test('Antrieb beim Anfahren auf Schnee: Allrad vor Heck vor Front (Last wandert nach hinten), Front dreht ohne ASR durch', () => {
  const snow = tractionOf({ snow: 1 });
  const fwd = variant('limousine', { drive: 'fwd' }), awd = variant('limousine', { drive: 'awd', awdFront: 0.45 });
  const t = (m, esp) => timeTo(car(m, { traction: snow, esp }), 50);
  assert.ok(t(awd, true) < t('limousine', true) && t('limousine', true) < t(fwd, true), `Allrad ${t(awd, true).toFixed(2)} Heck ${t('limousine', true).toFixed(2)} Front ${t(fwd, true).toFixed(2)} s`);
  const c = car(fwd, { traction: snow, esp: false }); let spin = 0;
  run(c, 1, { throttle: 1 }, (q) => { spin += q.spin; });
  assert.ok(spin > 30, 'ohne ASR drehen die Vorderräder durch');
  const e = car(fwd, { traction: snow, esp: true }); spin = 0;
  run(e, 1, { throttle: 1 }, (q) => { spin += q.spin; });
  assert.equal(spin, 0, 'ASR verhindert Durchdrehen');
  assert.ok(e.dyn.esp, 'ESP-Leuchte');
});

// Kreisfahrt bei 45 km/h, dann Vollgas ohne ESP: wie ändern sich Schräglauf vorn/hinten und Gierrate?
function powerOn(model) {
  const c = car(model, { v0: 45, esp: false });
  run(c, 1.5, { steer: 0.45, throttle: 0.25 });
  const w0 = c.angVel;
  let us = 0, wMax = 0, turned = 0; const a0 = c.angle;
  run(c, 1.5, { throttle: 1 }, (q) => { us += q.dyn.understeer; wMax = Math.max(wMax, Math.abs(q.angVel)); });
  turned = Math.abs(c.angle - a0);
  return { us: us / 90, w0, wMax, turned };
}

test('Vollgas in der Kurve (ohne ESP): Heckantrieb übersteuert, Frontantrieb untersteuert, Allrad dazwischen', () => {
  const base = variant('limousine', { kW: 250 });
  const fwd = variant(base, { drive: 'fwd' }), awd = variant(base, { drive: 'awd', awdFront: 0.4 });
  const r = powerOn(base), f = powerOn(fwd), a = powerOn(awd);
  assert.ok(r.us < -0.02, `Heck: Heck rutscht mehr (${r.us.toFixed(3)})`);
  assert.ok(f.us > 0.01, `Front: Front schiebt (${f.us.toFixed(3)})`);
  assert.ok(a.us > r.us && a.us < f.us, `Allrad dazwischen (${a.us.toFixed(3)})`);
  assert.ok(r.wMax > r.w0 * 1.8, 'Heck: das Auto dreht ein (Gierrate steigt stark)');
  assert.ok(f.turned < r.turned, 'Front: weniger Richtungsänderung als Heck');
});

test('ESP fängt den Heckantrieb ab: gleiche Kurve, Vollgas – mit ESP stabil, ohne dreht er sich', () => {
  const on = car('sportwagen', { v0: 45, esp: true }), off = car('sportwagen', { v0: 45, esp: false });
  let alphaOn = 0, alphaOff = 0, flagged = false;
  for (const [c, set] of [[on, (v) => { alphaOn = Math.max(alphaOn, v); }], [off, (v) => { alphaOff = Math.max(alphaOff, v); }]]) {
    run(c, 1.5, { steer: 0.45, throttle: 0.25 });
    run(c, 2, { throttle: 1 }, (q) => { set(Math.abs(q.dyn.alphaR)); if (q === on && q.dyn.esp) flagged = true; });
  }
  assert.ok(alphaOn < 0.2, `mit ESP höchstens leicht quer (${alphaOn.toFixed(2)} rad)`);
  assert.ok(alphaOff > 0.35, `ohne ESP bricht das Heck aus (${alphaOff.toFixed(2)} rad)`);
  assert.ok(flagged, 'ESP hat eingegriffen');
  assert.ok(SPECS.zweitakter.noAids, 'der Oldtimer hat kein ESP');
});

test('Motorlage: Mittelmotor lenkt spontaner ein als Heckmotor (Gierträgheit), Heckmotor hat mehr Traktion', () => {
  const mid = variant('sportwagen', {}), rear = variant('sportwagen', { front: 0.38, yaw: 1.18 }), front = variant('sportwagen', { front: 0.55, yaw: 1.1 });
  assert.ok(geometry(SPECS[mid]).Iz < geometry(SPECS[rear]).Iz && geometry(SPECS[mid]).Iz < geometry(SPECS[front]).Iz);
  const rise = (m) => { // Zeit bis 90 % der stationären Gierrate nach einem Lenkimpuls bei 60 km/h
    const c = car(m, { v0: 60 }); const ws = [];
    run(c, 2, { steer: 0.3, throttle: 0.35 }, (q) => ws.push(q.angVel));
    const end = ws.at(-1); return ws.findIndex((w) => w >= end * 0.9) / 60;
  };
  assert.ok(rise(mid) < rise(rear), `Mittelmotor ${rise(mid).toFixed(2)} s, Heckmotor ${rise(rear).toFixed(2)} s`);
  // Traktion: Heckantrieb mit Heckmotor (mehr Last hinten) zieht auf Schnee besser an als mit Frontmotor
  const snow = tractionOf({ snow: 1 });
  const t = (m) => timeTo(car(m, { traction: snow }), 40);
  assert.ok(t(rear) < t(front), `Heckmotor ${t(rear).toFixed(2)} s, Frontmotor ${t(front).toFixed(2)} s`);
});

test('Hoher Schwerpunkt: weniger Kurvengrip, stärkeres Nicken beim Bremsen', () => {
  const low = variant('limousine', { h: 0.45 }), high = variant('limousine', { h: 0.95 });
  const skid = (m) => { const c = car(m, { v0: 60 }); let ay = 0; run(c, 4, { steer: 1 }, (q) => { q.controls.throttle = kmh(q) < 60 ? 0.6 : 0; ay = Math.max(ay, Math.abs(q.dyn.ay)); }); return ay; };
  assert.ok(skid(high) < skid(low) * 0.97, `Querbeschleunigung hoch ${skid(high).toFixed(2)} < tief ${skid(low).toFixed(2)}`);
  const dive = (m) => { const c = car(m, { v0: 80 }); run(c, 0.6, { brake: 1 }); return bodyShift(c)[0]; };
  assert.ok(dive(high) > dive(low) * 1.8 && dive(low) > 0, `Karosserie taucht vorn ein: ${dive(low).toFixed(2)} / ${dive(high).toFixed(2)} px`);
  const c = car('limousine', { v0: 50 }); run(c, 0.8, { steer: 1 });
  assert.ok(bodyShift(c)[1] < 0, 'Rechtskurve drückt die Karosserie nach links (außen)');
});

test('Handbremse: blockierte Hinterräder – das Heck kommt; Bremsen auf Eis dreimal so lang; Stillstand bleibt stehen', () => {
  const turn = (hb) => { const c = car('kompakt', { v0: 50 }); const a0 = c.angle; run(c, 1, { steer: 0.5, handbrake: hb }); return { d: Math.abs(c.angle - a0), a: Math.abs(c.dyn.alphaR) }; };
  const h = turn(true), n = turn(false);
  assert.ok(h.a > 0.3 && h.d > n.d, `Heck rutscht (${h.a.toFixed(2)} rad), dreht weiter (${h.d.toFixed(2)} vs ${n.d.toFixed(2)})`);
  const hb = car('kompakt', { v0: 50 }); run(hb, 1, { handbrake: true });
  assert.ok(kmh(hb) < 40 && kmh(hb) > 25, `Handbremse geradeaus verzögert nur hinten (${kmh(hb).toFixed(1)} km/h nach 1 s)`);
  const stop = (tr) => { const c = car('limousine', { v0: 50, traction: tr }); const x0 = c.x; run(c, 12, { brake: 1 }, (q) => { if (q.vx < 0) q.vx = 0; }); return c.x - x0; };
  const r = stop(tractionOf({ ice: 1 })) / stop(undefined);
  assert.ok(r > 2.3 && r < 4, `Eis ${r.toFixed(2)}× so lang`);
  const s = car('limousine'); run(s, 3, {});
  assert.equal(Math.hypot(s.vx, s.vy), 0, 'ohne Eingabe bleibt es stehen');
  const back = car('limousine'); run(back, 3, { brake: 1 });
  assert.ok(back.vx < 0 && kmh(back) <= 31, `rückwärts höchstens 30 km/h (${kmh(back).toFixed(0)})`);
  const rb = car('limousine'); run(rb, 1.5, { brake: 1 }); const a0 = rb.angle; run(rb, 1, { brake: 1, steer: 1 });
  assert.ok(rb.angle < a0, 'rückwärts lenkt spiegelverkehrt');
});

test('Nur das gefahrene Auto: Verkehr bleibt bei der Arcade-Physik, alles deterministisch, Modelle beschrieben', () => {
  const a = createCar({ x: 0, y: 0 }); a.driver = 'npc'; run(a, 2, { throttle: 1, steer: 0.3 });
  assert.equal(a.dyn, undefined, 'KI-Autos ohne Fahrdynamik');
  const p = () => { const c = car('heckcoupe', { v0: 40, esp: false }); run(c, 3, { steer: 0.5, throttle: 1 }); return [c.x, c.y, c.angle]; };
  assert.deepEqual(p(), p(), 'deterministisch');
  for (const m of CAR_MODELS) {
    const s = SPECS[m];
    assert.ok(['fwd', 'rwd', 'awd'].includes(s.drive) && s.front > 0.3 && s.front < 0.7 && s.h > 0.3 && s.wb <= 4.2, m);
  }
  const c = createCar({ x: 0, y: 0 }); c.model = 'sportwagen';
  assert.equal(specLine(c), 'Sportwagen · Mittelmotor · RWD · 435 PS');
  assert.equal(carModel(createCar({ x: 0, y: 0, kind: 'bus' })), 'bus');
  for (const k of Object.keys(SPECS)) if (k.startsWith('test')) delete SPECS[k];
});

test('Spielspaß: Handbremse dreht das Auto um die Ecke und lässt den Schwung, enger Wendekreis beim Rangieren', () => {
  const c = car('limousine', { v0: 50 }); const a0 = c.angle;
  run(c, 1.2, { steer: 1, handbrake: true });
  assert.ok(Math.abs(c.angle - a0) > 1, `dreht sich ${(Math.abs(c.angle - a0) * 57.3).toFixed(0)}°`);
  assert.ok(kmh(c) > 20, `Schwung bleibt (${kmh(c).toFixed(0)} km/h)`);
  // Wendekreis: 10 km/h, volle Lenkung – Radius aus Tempo und Gierrate
  const r = car('limousine', { v0: 10 }); run(r, 2, { steer: 1, throttle: 0.15 });
  const R = Math.hypot(r.vx, r.vy) / 10 / Math.abs(r.angVel);
  assert.ok(R < 5, `Wendekreis-Radius ${R.toFixed(1)} m`);
});

test('Fehlerfälle: Gas beim Rückwärtsrollen bremst, Wiese begrenzt das Tempo, nach dem Aussteigen kein Fahrzustand', async () => {
  const c = car('limousine'); c.vx = -30; // rollt mit 3 m/s rückwärts
  run(c, 1.2, { throttle: 1 });
  assert.ok(c.vx > -2, `hält an und fährt vorwärts (${(c.vx / 10).toFixed(2)} m/s)`);
  const { stepDynamics } = await import('../web/src/dynamics.js');
  const g = car('limousine'); for (let i = 0; i < 40 * 60; i++) stepDynamics(g, DT, { grip: 0.55, drag: 3.2, top: 0.5 }, { brake: 1, accel: 1, lat: 1, steer: 1 }, { throttle: 1, brake: 0, steer: 0, handbrake: false });
  assert.ok(kmh(g) <= SPECS.limousine.vmax * 0.5 + 1, `Wiese: höchstens halbe Spitze (${kmh(g).toFixed(0)} km/h)`);
  const { createWorld, updateWorld } = await import('../web/src/world.js');
  const { realCity } = await import('./helpers/city.js');
  const { idle } = await import('./helpers/bot.js');
  const w = createWorld({ city: realCity(), cars: 0, pedestrians: 0 }); w.mission.state = 'idle';
  const pc = w.cars.find((q) => q.id === w.playerCarId);
  w.player.x = pc.x - Math.sin(pc.angle) * 22; w.player.y = pc.y + Math.cos(pc.angle) * 22;
  updateWorld(w, { ...idle(), enterExit: true }, DT);
  assert.equal(w.player.inCar, pc.id);
  for (let i = 0; i < 60; i++) updateWorld(w, { ...idle(), throttle: 1, steer: 0.4 }, DT);
  for (let i = 0; i < 30; i++) updateWorld(w, { ...idle(), brake: 1 }, DT);
  assert.ok(pc.dyn, 'fährt mit Fahrdynamik');
  for (let i = 0; i < 120 && w.player.inCar; i++) updateWorld(w, { ...idle(), brake: 1, enterExit: i % 20 === 19 }, DT);
  assert.equal(w.player.inCar, null, 'ausgestiegen');
  assert.equal(pc.dyn, null, 'Fahrzustand gelöscht');
  assert.deepEqual(bodyShift(pc), [0, 0], 'keine schiefe Karosserie');
});
