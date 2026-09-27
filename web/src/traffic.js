// Verkehr: Rechtsverkehr auf dem echten Straßennetz. KI-Fahrer folgen ihrer Spur (Pure Pursuit),
// wählen an Kreuzungen die nächste Spur, bremsen vor Kurven und Hindernissen, lösen Blockaden,
// fahren sich frei und suchen nach einem Unfall die nächste passende Spur.
import { clamp, wrapAngle } from './math.js';
import { forwardSpeed } from './car.js';
import { buildLaneGraph, chooseNext, connector, turnAngle, nearestLane } from './roadgraph.js';

const LOOKAHEAD = 420; // so viel Route (px) hält die KI im Voraus

// Kurventempo aus dem Abbiegewinkel: geradeaus unbegrenzt, rechtwinklig ~ 55 px/s.
function turnSpeed(angle) {
  const a = Math.abs(angle);
  if (a < 0.25) return Infinity;
  return clamp(150 - a * 70, 38, 140);
}

function appendLane(ai, lane, fromS = 0) {
  const p = lane.pts;
  let acc = 0;
  for (let i = 0; i < p.length; i += 2) {
    if (i > 0) acc += Math.hypot(p[i] - p[i - 2], p[i + 1] - p[i - 1]);
    if (acc < fromS && i < p.length - 2) continue;
    ai.route.push(p[i], p[i + 1]); ai.cap.push(lane.cruise);
  }
  ai.lane = lane;
}

function extendRoute(ai, rng) {
  const next = chooseNext(ai.lane, rng);
  if (!next) return false;
  const v = Math.min(turnSpeed(turnAngle(ai.lane, next)), next.cruise, ai.lane.cruise);
  const con = connector(ai.lane, next);
  if (ai.cap.length) ai.cap[ai.cap.length - 1] = Math.min(ai.cap[ai.cap.length - 1], v);
  for (let i = 0; i < con.length; i += 2) { ai.route.push(con[i], con[i + 1]); ai.cap.push(v); }
  appendLane(ai, next);
  return true;
}

function remaining(ai, car) {
  const r = ai.route;
  let L = Math.hypot(r[2 * ai.i + 2] - car.x, r[2 * ai.i + 3] - car.y);
  for (let k = ai.i + 1; k < r.length / 2 - 1 && L < LOOKAHEAD; k++) L += Math.hypot(r[2 * k + 2] - r[2 * k], r[2 * k + 3] - r[2 * k + 1]);
  return L;
}

export function initAi(car, lane, s, rng) {
  car.ai = {
    route: [], cap: [], i: 0, lane: null,
    cruiseK: 0.85 + rng() * 0.3,
    blockedT: 0, ignoreT: 0, stuckT: 0, reverseT: 0, hornT: 0,
  };
  appendLane(car.ai, lane, s);
  if (car.ai.route.length < 4) extendRoute(car.ai, rng);
}

// Setzt ein Auto auf eine Spur (Bogenlänge s) und richtet es aus.
export function placeOnLane(car, city, lane, s, rng) {
  const p = lane.pts;
  let acc = 0;
  for (let i = 0; i < p.length - 2; i += 2) {
    const L = Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]);
    if (acc + L >= s || i === p.length - 4) {
      const t = L ? clamp((s - acc) / L, 0, 1) : 0;
      car.x = p[i] + (p[i + 2] - p[i]) * t; car.y = p[i + 1] + (p[i + 3] - p[i + 1]) * t;
      car.angle = Math.atan2(p[i + 3] - p[i + 1], p[i + 2] - p[i]);
      break;
    }
    acc += L;
  }
  car.vx = car.vy = car.angVel = 0;
  initAi(car, lane, s, rng);
}

// Zufällige Spur mit Abstand minR…maxR zu (cx, cy).
export function spawnSpot(city, rng, cx, cy, minR, maxR) {
  const g = buildLaneGraph(city);
  const segs = g.hash.query({ x: cx - maxR, y: cy - maxR, w: 2 * maxR, h: 2 * maxR }, []);
  for (let tries = 0; tries < 40 && segs.length; tries++) {
    const sg = segs[Math.floor(rng() * segs.length)];
    const t = rng();
    const x = sg.ax + (sg.bx - sg.ax) * t, y = sg.ay + (sg.by - sg.ay) * t;
    const d = Math.hypot(x - cx, y - cy);
    if (d < minR || d > maxR) continue;
    let s = 0;
    const p = sg.lane.pts;
    for (let i = 0; i < sg.i; i += 2) s += Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]);
    s += Math.hypot(x - sg.ax, y - sg.ay);
    return { lane: sg.lane, s, x, y };
  }
  return null;
}

// Nächste Spur zum Auto finden (nach Unfall / Abdrängen / Übernahme durch die KI).
export function replan(car, city, rng) {
  const g = buildLaneGraph(city);
  const hit = nearestLane(g, car.x, car.y, car.angle, 600) ?? nearestLane(g, car.x, car.y, car.angle, 3000);
  if (!hit) { car.ai = null; return; }
  let s = 0;
  const p = hit.lane.pts;
  for (let i = 0; i < hit.i; i += 2) s += Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]);
  s += Math.hypot(hit.x - p[hit.i], hit.y - p[hit.i + 1]) + 30;
  const keep = car.ai;
  initAi(car, hit.lane, s, rng);
  if (keep) Object.assign(car.ai, { cruiseK: keep.cruiseK });
}

// Hindernisabstand vor dem Auto (Kegel), getrennt nach KI-Autos und „ehrlichen“ Hindernissen.
function obstacleAhead(car, world) {
  const c = Math.cos(car.angle), s = Math.sin(car.angle);
  let dCar = Infinity, dOther = Infinity, playerBlock = false;
  const check = (ox, oy, lat, isAiCar, isPlayer) => {
    const rx = ox - car.x, ry = oy - car.y;
    if (rx * rx + ry * ry > 12100) return;
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
  let ai = car.ai;
  const city = world.city, ctl = car.controls;
  if (car.wrecked) return;
  if (!ai || ai.route.length < 4) { replan(car, city, world.rng); ai = car.ai; if (!ai) return; }
  const r = ai.route;

  // Fortschritt auf der Route: zum Segment weiterschalten, das vor dem Auto liegt.
  let t = 0, px = car.x, py = car.y;
  for (;;) {
    const ax = r[2 * ai.i], ay = r[2 * ai.i + 1], bx = r[2 * ai.i + 2], by = r[2 * ai.i + 3];
    if (bx === undefined) break;
    const dx = bx - ax, dy = by - ay, L2 = dx * dx + dy * dy || 1;
    t = ((car.x - ax) * dx + (car.y - ay) * dy) / L2;
    px = ax + dx * clamp(t, 0, 1); py = ay + dy * clamp(t, 0, 1);
    if (t > 1 || Math.hypot(bx - car.x, by - car.y) < 10) { ai.i++; if (2 * ai.i + 3 >= r.length) { if (!extendRoute(ai, world.rng)) break; } continue; }
    break;
  }
  if (ai.i > 24) { r.splice(0, 2 * (ai.i - 2)); ai.cap.splice(0, ai.i - 2); ai.i = 2; }
  while (remaining(ai, car) < LOOKAHEAD) if (!extendRoute(ai, world.rng)) break;
  if (Math.hypot(px - car.x, py - car.y) > 260) { replan(car, city, world.rng); return; }

  // Zielpunkt voraus auf der Route (Pure Pursuit), plus Tempolimit aus Kurven voraus.
  const vf = forwardSpeed(car);
  const look = clamp(Math.abs(vf) * 0.35, 36, 90);
  let aimX = px, aimY = py, acc = 0, found = false;
  let target = (ai.cap[ai.i] ?? 100) * ai.cruiseK;
  let x0 = px, y0 = py;
  for (let k = ai.i + 1; k < r.length / 2; k++) {
    const x1 = r[2 * k], y1 = r[2 * k + 1], L = Math.hypot(x1 - x0, y1 - y0);
    if (!found && acc + L >= look) { const u = (look - acc) / (L || 1); aimX = x0 + (x1 - x0) * u; aimY = y0 + (y1 - y0) * u; found = true; }
    acc += L;
    const cap = ai.cap[k];
    if (acc < 220 && cap < target) target = Math.min(target, Math.sqrt(cap * cap + 2 * 260 * Math.max(0, acc - 20)));
    x0 = x1; y0 = y1;
    if (acc > 240 && found) break;
  }
  if (!found) { aimX = x0; aimY = y0; }
  const dx = aimX - car.x, dy = aimY - car.y;

  if (ai.reverseT > 0) {
    ai.reverseT -= dt;
    ctl.throttle = 0; ctl.brake = 1; ctl.handbrake = false;
    ctl.steer = -clamp(wrapAngle(Math.atan2(dy, dx) - car.angle) * 2, -1, 1);
    return;
  }

  const diff = wrapAngle(Math.atan2(dy, dx) - car.angle);
  ctl.steer = clamp(diff * 2.4, -1, 1);
  if (Math.abs(diff) > 0.6) target = Math.min(target, 55);

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
