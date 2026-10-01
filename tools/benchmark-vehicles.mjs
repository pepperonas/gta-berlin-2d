// Reproduzierbare Geradeausmessung auf trockener Straße, ESP/ABS an, Vollgas.
// node tools/benchmark-vehicles.mjs [modell ...] > /tmp/vehicle-benchmark.json
import { createCar, stepCar } from '../web/src/car.js';
import { SPECS } from '../web/src/carmodels.js';

const models = process.argv.slice(2);
const dt = 1 / 120, marks = [10, 30, 50, 70, 100, 130, 200];
const results = [];
for (const model of models.length ? models : Object.keys(SPECS)) {
  if (!SPECS[model]) throw new Error(`Unbekanntes Modell: ${model}`);
  const car = createCar({ x: 0, y: 0, kind: SPECS[model].twoWheel ? model : 'car' });
  Object.assign(car, { model, driver: 'player', esp: true, abs: true });
  car.controls.throttle = 1;
  const seconds = {}, acceleration = {};
  let previous = 0, speed = 0;
  for (let i = 1; i <= 120 / dt; i++) {
    stepCar(car, dt, null);
    speed = Math.hypot(car.vx, car.vy) * 0.36;
    if (!Number.isFinite(speed)) throw new Error(`${model}: ungültiger Zustand`);
    for (const mark of marks) if (seconds[mark] === undefined && previous < mark && speed >= mark) {
      seconds[mark] = +((i - 1 + (mark - previous) / (speed - previous)) * dt).toFixed(3);
      acceleration[mark] = +((speed - previous) / 3.6 / dt).toFixed(3);
    }
    previous = speed;
  }
  results.push({ model, seconds, acceleration, topKmh: +speed.toFixed(2) });
}
console.log(JSON.stringify(results, null, 2));
