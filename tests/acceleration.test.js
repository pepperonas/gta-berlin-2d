import test from 'node:test';
import assert from 'node:assert/strict';
import { createCar, stepCar } from '../web/src/car.js';
import { SPECS, CAR_MODELS } from '../web/src/carmodels.js';
import { DYN, driveEnvelope, stepDynamics } from '../web/src/dynamics.js';
import { DRY, tractionOf } from '../web/src/traction.js';

const kmh = c => Math.hypot(c.vx, c.vy) * 0.36;
const make = (model, esp = true) => Object.assign(createCar({ x: 0, y: 0 }), { model, driver: 'player', esp, abs: true });
function sprint(model, { dt = 1 / 120, esp = true, traction = DRY, steer = 0 } = {}) {
  const c = make(model, esp), times = {}, acceleration = {};
  c.traction = traction; Object.assign(c.controls, { throttle: 1, steer });
  let prev = 0;
  for (let i = 1; i <= 40 / dt; i++) {
    stepCar(c, dt, null);
    const v = kmh(c);
    assert.ok([c.x, c.y, c.vx, c.vy, c.angle, c.angVel].every(Number.isFinite), `${model}: endlicher Zustand`);
    for (const mark of [10, 30, 50, 70, 100, 130]) if (!times[mark] && prev < mark && v >= mark) {
      times[mark] = (i - 1 + (mark - prev) / (v - prev)) * dt;
      acceleration[mark] = (v - prev) / 3.6 / dt;
    }
    prev = v;
  }
  return { c, times, acceleration };
}

test('Stadt-Antritt spürbar stärker als die dokumentierte Ausgangsmessung', () => {
  const before = {
    kleinwagen: [2.469, 3.999], kompakt: [2.357, 3.380], limousine: [1.835, 2.584],
    hothatch: [2.136, 3.012], musclecar: [2.190, 3.076], transporter: [2.867, 4.645],
  };
  for (const [model, [fifty, seventy]] of Object.entries(before)) {
    const { times } = sprint(model);
    assert.ok(times[50] < fifty * 0.86, `${model}: 0–50 ${times[50]}`);
    assert.ok(times[70] < seventy * 0.89, `${model}: 0–70 ${times[70]}`);
  }
});

test('Alle 45 Modelle: Antriebskraft fällt mit Tempo, Übergänge ohne Schubstufe', () => {
  for (const [model, spec] of Object.entries(SPECS)) {
    let prev = driveEnvelope(spec, 0).force;
    for (let v = 0.05; v <= spec.vmax / 3.6; v += 0.05) {
      const { force, traction } = driveEnvelope(spec, v);
      assert.ok(force > 0 && Number.isFinite(force), model);
      assert.ok(force <= prev + 1e-8, `${model}: Kraft steigt bei ${v * 3.6} km/h`);
      assert.ok(force / prev > 0.97, `${model}: abrupter Kraftsprung`);
      assert.ok(traction >= 1 && traction <= 1 + DYN.launch.gain, model);
      prev = force;
    }
    assert.equal(driveEnvelope(spec, 90 / 3.6).traction, 1, `${model}: Anfahrhilfe endet`);
  }
});

test('Alle Modelle: tatsächlicher Schub bei 70 kleiner als bei 30 km/h, keine künstliche Spätbeschleunigung', () => {
  for (const model of Object.keys(SPECS)) {
    const { c, acceleration } = sprint(model);
    assert.ok(acceleration[70] < acceleration[30], `${model}: ${JSON.stringify(acceleration)}`);
    assert.ok(kmh(c) <= SPECS[model].vmax + 1, `${model}: überschreitet Höchsttempo`);
    assert.ok(Math.abs(c.y) < 1e-8 && Math.abs(c.angle) < 1e-8, `${model}: zieht seitlich weg`);
  }
});

test('Anfahrhilfe bewahrt extreme Sportwagen, Zweiräder, Schnee, Eis und Drift', () => {
  for (const model of ['sportwagen', 'supersport', 'rallye', 'elektrosport', 'motorcycle', 'scooter']) {
    assert.equal(driveEnvelope(SPECS[model], 0).traction, 1, model);
  }
  const s = SPECS.musclecar;
  assert.equal(driveEnvelope(s, 0, 0.4).traction, 1, 'auf Eis kein zusätzlicher Grip');
  assert.equal(driveEnvelope(s, 0, 1, 0.5).traction, 1, 'stark eingelenkt kein zusätzlicher Grip');
  for (const esp of [true, false]) {
    const dry = sprint('musclecar', { esp }), wet = sprint('musclecar', { esp, traction: tractionOf({ wet: 1 }) });
    const snow = sprint('musclecar', { esp, traction: tractionOf({ snow: 1 }) });
    assert.ok(dry.times[50] < wet.times[50] && wet.times[50] < snow.times[50], 'Wetter bleibt spürbar');
  }
});

test('Antritt bleibt bei 30/60/120 Hz konsistent und mit wenig Gas dosierbar', () => {
  for (const model of ['kleinwagen', 'musclecar', 'elektro', 'truck']) {
    const ref = sprint(model);
    for (const dt of [1 / 30, 1 / 60]) {
      const run = sprint(model, { dt });
      assert.ok(Math.abs(run.times[70] - ref.times[70]) < 0.035, `${model}: dt ${dt}`);
    }
    let prev = 0;
    for (const throttle of [0.1, 0.25, 0.5, 1]) {
      const c = make(model); c.controls.throttle = throttle;
      for (let i = 0; i < 60; i++) stepCar(c, 1 / 60, null);
      assert.ok(kmh(c) >= prev, `${model}: Gaspedal nicht monoton`);
      prev = kmh(c);
    }
  }
});

test('Alle Pkw: Vollgas-Anfahren mit Einschlag bleibt mit ESP auch nass und auf Schnee kontrollierbar', () => {
  for (const model of CAR_MODELS) for (const weather of [{}, { wet: 1 }, { snow: 1 }]) for (const steer of [-0.25, 0.25]) {
    const c = make(model); c.traction = tractionOf(weather);
    Object.assign(c.controls, { throttle: 1, steer });
    for (let i = 0; i < 300; i++) {
      stepCar(c, 1 / 60, null);
      assert.ok(Number.isFinite(c.vx) && Number.isFinite(c.vy), model);
      assert.ok(Math.abs(c.dyn.alphaR) < 0.15, `${model} ${JSON.stringify(weather)}: Heck bricht beim Start aus`);
    }
  }
});

test('Aquaplaning und schlechter Untergrund: kein NaN, kein Anfahrbonus, keine erhöhte Spitze', () => {
  for (const model of Object.keys(SPECS)) {
    const c = make(model); c.aqua = 0.3; c.vx = 80 / 0.36;
    Object.assign(c.controls, { throttle: 1, steer: 0.2 });
    for (let i = 0; i < 60; i++) stepCar(c, 1 / 60, null);
    assert.ok([c.vx, c.vy, c.angle].every(Number.isFinite), model);
  }
  const c = make('musclecar'); c.controls.throttle = 1;
  for (let i = 0; i < 60 * 60; i++) stepDynamics(c, 1 / 60, { grip: 0.55, drag: 3.2, top: 0.5 }, DRY, c.controls);
  assert.ok(kmh(c) < SPECS.musclecar.vmax * 0.5 + 1);
});
