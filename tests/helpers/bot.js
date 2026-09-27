// Autopilot für Tests: steuert den Spieler über abstrakte Eingaben durch die komplette Mission.
import { playerCar, speedOf } from '../../web/src/world.js';
import { NV, NH, neighbor, DIRS } from '../../web/src/traffic.js';
import { wrapAngle, clamp } from '../../web/src/math.js';

export const idle = () => ({ moveX: 0, moveY: 0, steer: 0, throttle: 0, brake: 0, handbrake: false, sprint: false, horn: false,
  action: false, actionHeld: false, enterExit: false, pause: false, mapToggle: false,
  menuUp: false, menuDown: false, menuLeft: false, menuRight: false, confirm: false, back: false });

function center(city, i, j) { return { x: city.vRoads[i] + city.roadW / 2, y: city.hRoads[j] + city.roadW / 2 }; }

function nearestIntersection(city, x, y) {
  let best = null;
  for (let i = 0; i < NV; i++) for (let j = 0; j < NH; j++) {
    const c = center(city, i, j), d = Math.hypot(c.x - x, c.y - y);
    if (!best || d < best.d) best = { i, j, d };
  }
  return best;
}

// Route über Kreuzungsmitten (BFS), danach direkt zum Ziel.
export function route(city, from, to) {
  const a = nearestIntersection(city, from.x, from.y), b = nearestIntersection(city, to.x, to.y);
  const key = (i, j) => i * 100 + j;
  const prev = new Map([[key(a.i, a.j), null]]);
  const q = [[a.i, a.j]];
  while (q.length) {
    const [i, j] = q.shift();
    if (i === b.i && j === b.j) break;
    for (const d of Object.keys(DIRS)) {
      const n = neighbor(i, j, d);
      if (n && !prev.has(key(...n))) { prev.set(key(...n), [i, j]); q.push(n); }
    }
  }
  const pts = [];
  for (let cur = [b.i, b.j]; cur; cur = prev.get(key(...cur))) pts.unshift(center(city, ...cur));
  return pts;
}

export function driveTo(w, target, input, opts = {}) {
  const car = playerCar(w);
  const dx = target.x - car.x, dy = target.y - car.y, d = Math.hypot(dx, dy);
  const diff = wrapAngle(Math.atan2(dy, dx) - car.angle);
  const sp = speedOf(car);
  input.steer = clamp(diff * 2.5, -1, 1);
  const want = opts.stop ? clamp((d - 6) * 1.2, 0, 200) : Math.abs(diff) > 0.7 ? 70 : 230;
  if (sp < want) { input.throttle = 1; input.brake = 0; } else { input.throttle = 0; input.brake = sp - want > 30 ? 1 : 0.3; }
  if (Math.abs(diff) > 1.8 && d < 60) { input.throttle = 0; input.brake = 1; input.steer = -input.steer; } // rückwärts rangieren
  return d;
}
