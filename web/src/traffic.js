// Verkehr: Rechtsverkehr auf dem Straßenraster. KI-Fahrer folgen Wegpunkten durch Kreuzungen,
// bremsen vor Hindernissen, lösen Blockaden und fahren sich frei, wenn sie stecken.
import { TILE, COLS, ROWS } from './config.js';
import { clamp, wrapAngle } from './math.js';
import { forwardSpeed } from './car.js';

export const DIRS = { N: [0, -1], S: [0, 1], E: [1, 0], W: [-1, 0] };
export const OPP = { N: 'S', S: 'N', E: 'W', W: 'E' };
const ANGLE = { E: 0, S: Math.PI / 2, W: Math.PI, N: -Math.PI / 2 };
const NV = COLS + 1, NH = ROWS + 1; // Anzahl senkrechter / waagerechter Straßen

// Spurmitte: Südwärts links (West-Hälfte), nordwärts rechts; ostwärts unten, westwärts oben.
export function laneCoord(city, dir, idx) {
  if (dir === 'S') return city.vRoads[idx] + TILE;
  if (dir === 'N') return city.vRoads[idx] + 3 * TILE;
  if (dir === 'E') return city.hRoads[idx] + 3 * TILE;
  return city.hRoads[idx] + TILE;
}

export function box(city, i, j) {
  return { x: city.vRoads[i], y: city.hRoads[j], w: city.roadW, h: city.roadW };
}

export function entryPoint(city, dir, i, j) {
  const b = box(city, i, j);
  if (dir === 'S') return { x: laneCoord(city, 'S', i), y: b.y };
  if (dir === 'N') return { x: laneCoord(city, 'N', i), y: b.y + b.h };
  if (dir === 'E') return { x: b.x, y: laneCoord(city, 'E', j) };
  return { x: b.x + b.w, y: laneCoord(city, 'W', j) };
}

export function exitPoint(city, dir, i, j) {
  const b = box(city, i, j);
  if (dir === 'S') return { x: laneCoord(city, 'S', i), y: b.y + b.h };
  if (dir === 'N') return { x: laneCoord(city, 'N', i), y: b.y };
  if (dir === 'E') return { x: b.x + b.w, y: laneCoord(city, 'E', j) };
  return { x: b.x, y: laneCoord(city, 'W', j) };
}

export function neighbor(i, j, dir) {
  const [dx, dy] = DIRS[dir];
  const ni = i + dx, nj = j + dy;
  return ni >= 0 && nj >= 0 && ni < NV && nj < NH ? [ni, nj] : null;
}

export function allowedExits(i, j, din) {
  const out = Object.keys(DIRS).filter((d) => d !== OPP[din] && neighbor(i, j, d));
  return out.length ? out : [OPP[din]];
}

// Wegpunkte durch die Kreuzung (i,j): von der Einfahrt in din zur Ausfahrt in dout.
export function turnPath(city, din, dout, i, j) {
  const a = entryPoint(city, din, i, j), b = exitPoint(city, dout, i, j);
  if (din === dout) return [{ ...b, slow: false }];
  const corner = (din === 'N' || din === 'S') ? { x: a.x, y: b.y } : { x: b.x, y: a.y };
  const pts = [];
  for (const t of [0.35, 0.7, 1]) {
    const u = 1 - t;
    pts.push({ x: u * u * a.x + 2 * u * t * corner.x + t * t * b.x, y: u * u * a.y + 2 * u * t * corner.y + t * t * b.y, slow: true });
  }
  return pts;
}

function chooseExit(i, j, din, rng) {
  const opts = allowedExits(i, j, din);
  if (opts.includes(din) && rng() < 0.5) return din;
  return opts[Math.floor(rng() * opts.length)];
}

// Setzt ein Auto auf ein zufälliges (oder vorgegebenes) Straßensegment.
export function placeOnSegment(car, city, rng, seg) {
  let i, j, dir, n;
  for (let tries = 0; tries < 50; tries++) {
    i = seg?.i ?? Math.floor(rng() * NV);
    j = seg?.j ?? Math.floor(rng() * NH);
    dir = seg?.dir ?? ['N', 'S', 'E', 'W'][Math.floor(rng() * 4)];
    n = neighbor(i, j, dir);
    if (n) break;
  }
  const a = exitPoint(city, dir, i, j), b = entryPoint(city, dir, n[0], n[1]);
  const t = seg?.t ?? (0.2 + rng() * 0.6);
  car.x = a.x + (b.x - a.x) * t;
  car.y = a.y + (b.y - a.y) * t;
  car.angle = ANGLE[dir];
  car.vx = car.vy = car.angVel = 0;
  initAi(car, city, dir, n[0], n[1], rng);
}

function initAi(car, city, dir, ti, tj, rng) {
  car.ai = {
    dir, ti, tj,
    nextTurn: chooseExit(ti, tj, dir, rng),
    waypoints: [entryPoint(city, dir, ti, tj)],
    cruise: 125 + rng() * 45,
    blockedT: 0, ignoreT: 0, stuckT: 0, reverseT: 0, hornT: 0,
  };
}

// Nächstes Straßensegment zum Auto finden (nach Unfall / Abdrängen).
export function replan(car, city, rng) {
  let best = null;
  for (let i = 0; i < NV; i++) for (let j = 0; j < NH; j++) for (const dir of ['N', 'S', 'E', 'W']) {
    const n = neighbor(i, j, dir);
    if (!n) continue;
    const a = exitPoint(city, dir, i, j), b = entryPoint(city, dir, n[0], n[1]);
    const len = Math.hypot(b.x - a.x, b.y - a.y);
    const t = clamp(((car.x - a.x) * (b.x - a.x) + (car.y - a.y) * (b.y - a.y)) / (len * len), 0, 1);
    const px = a.x + (b.x - a.x) * t, py = a.y + (b.y - a.y) * t;
    const d = Math.hypot(car.x - px, car.y - py) + Math.abs(wrapAngle(ANGLE[dir] - car.angle)) * 40;
    if (!best || d < best.d) best = { d, dir, n };
  }
  car.ai.dir = best.dir; car.ai.ti = best.n[0]; car.ai.tj = best.n[1];
  car.ai.nextTurn = chooseExit(best.n[0], best.n[1], best.dir, rng);
  car.ai.waypoints = [entryPoint(city, best.dir, best.n[0], best.n[1])];
}

// Hindernisabstand vor dem Auto (Kegel), getrennt nach KI-Autos und „ehrlichen“ Hindernissen.
function obstacleAhead(car, world) {
  const c = Math.cos(car.angle), s = Math.sin(car.angle);
  let dCar = Infinity, dOther = Infinity, playerBlock = false;
  const check = (ox, oy, lat, isAiCar, isPlayer) => {
    const rx = ox - car.x, ry = oy - car.y;
    const along = rx * c + ry * s;
    if (along <= 0 || along > 110) return;
    const side = Math.abs(-rx * s + ry * c);
    if (side > lat) return;
    if (isAiCar) dCar = Math.min(dCar, along);
    else { dOther = Math.min(dOther, along); if (isPlayer) playerBlock = true; }
  };
  for (const o of world.cars) {
    if (o === car) continue;
    check(o.x, o.y, 22, o.driver === 'npc' && !o.wrecked, o.driver === 'player');
  }
  for (const p of world.peds) if (p.state !== 'gone') check(p.x, p.y, 16, false, false);
  const pl = world.player;
  if (!pl.inCar) check(pl.x, pl.y, 17, false, true);
  return { dCar, dOther, playerBlock };
}

export function driveAi(car, world, dt) {
  const ai = car.ai, city = world.city, ctl = car.controls;
  if (!ai || car.wrecked) return;
  if (!ai.waypoints.length || !ai.waypoints[0]) replan(car, city, world.rng);
  const vf = forwardSpeed(car);

  // Wegpunkt erreicht → nächsten planen.
  let wp = ai.waypoints[0];
  if (Math.hypot(wp.x - car.x, wp.y - car.y) < 16) {
    ai.waypoints.shift();
    if (!ai.waypoints.length) {
      const dout = ai.nextTurn;
      ai.waypoints = turnPath(city, ai.dir, dout, ai.ti, ai.tj);
      const n = neighbor(ai.ti, ai.tj, dout);
      ai.dir = dout; ai.ti = n[0]; ai.tj = n[1];
      ai.nextTurn = chooseExit(ai.ti, ai.tj, ai.dir, world.rng);
      ai.waypoints.push(entryPoint(city, ai.dir, ai.ti, ai.tj));
    }
    wp = ai.waypoints[0];
  }
  const dWp = Math.hypot(wp.x - car.x, wp.y - car.y);
  if (dWp > 320) { replan(car, city, world.rng); return; }
  // Spurhalten: auf geraden Stücken einen Punkt auf der Spurmitte ~70 px voraus ansteuern.
  let aim = wp;
  if (!wp.slow) {
    const [ux, uy] = DIRS[ai.dir];
    const look = Math.min(70, dWp);
    aim = ux !== 0
      ? { x: car.x + ux * look, y: laneCoord(city, ai.dir, ai.tj) }
      : { x: laneCoord(city, ai.dir, ai.ti), y: car.y + uy * look };
  }
  const dx = aim.x - car.x, dy = aim.y - car.y;

  if (ai.reverseT > 0) {
    ai.reverseT -= dt;
    ctl.throttle = 0; ctl.brake = 1; ctl.handbrake = false;
    ctl.steer = -clamp(wrapAngle(Math.atan2(dy, dx) - car.angle) * 2, -1, 1);
    return;
  }

  const diff = wrapAngle(Math.atan2(dy, dx) - car.angle);
  ctl.steer = clamp(diff * 2.4, -1, 1);

  let target = ai.cruise;
  if (wp.slow) target = 70;
  else if (ai.nextTurn !== ai.dir && ai.waypoints.length === 1 && dWp < 110) target = 75;
  if (Math.abs(diff) > 0.6) target = Math.min(target, 60);

  const { dCar, dOther, playerBlock } = obstacleAhead(car, world);
  ai.ignoreT = Math.max(0, ai.ignoreT - dt);
  const d = Math.min(dOther, ai.ignoreT > 0 ? Infinity : dCar);
  if (d < 110) target = Math.min(target, Math.max(0, (d - 40) * 2.2));

  if (target < 1 && Math.abs(vf) < 6) {
    ai.blockedT += dt;
    if (dCar < dOther && ai.blockedT > 2.5) { ai.ignoreT = 1.3; ai.blockedT = 0; }
    if (playerBlock && ai.blockedT > 1.5 && ai.hornT <= 0) {
      world.events.push({ type: 'horn', x: car.x, y: car.y, npc: true });
      ai.hornT = 3;
    }
  } else ai.blockedT = 0;
  ai.hornT -= dt;

  if (vf < target - 8) { ctl.throttle = clamp((target - vf) / 60, 0.25, 1); ctl.brake = 0; }
  else if (vf > target + 8) { ctl.throttle = 0; ctl.brake = clamp((vf - target) / 70, 0.25, 1); }
  else { ctl.throttle = 0.15; ctl.brake = 0; }
  ctl.handbrake = false;

  // Festgefahren (Gas, aber keine Bewegung, kein Hindernis) → kurz zurücksetzen.
  if (ctl.throttle > 0.3 && Math.abs(vf) < 8) {
    ai.stuckT += dt;
    if (ai.stuckT > 1.8) { ai.reverseT = 1.0; ai.stuckT = 0; }
  } else ai.stuckT = 0;
}

export { NV, NH, ANGLE };
