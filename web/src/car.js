// Arcade-Fahrphysik: Längs-/Querzerlegung der Geschwindigkeit, Quergrip, geschwindigkeitsabhängige Lenkung.
import { CAR, KNOCK } from './config.js';
import { clamp, sign } from './math.js';
import { obbVsRect, obbVsObb, circleVsObb, obbVsSegment, obbBounds } from './collision.js';
import { T, surfaceAt } from './map.js';
import { sizeOf } from './fleet.js';
import { DRY, AQUA } from './traction.js';

let nextId = 1;
export const CAR_COLORS = ['#c0392b', '#2e86de', '#f1c40f', '#27ae60', '#ecf0f1', '#8e44ad', '#34495e', '#e67e22', '#16a085', '#7f8c8d'];

// kind: Fahrzeugart (fleet.js KINDS) – bestimmt Maße und Motorleistung; ohne Angabe ein Pkw
export function createCar({ x, y, angle = 0, color = '#c0392b', role = 'traffic', kind = 'car' }) {
  const k = sizeOf(kind);
  return {
    id: nextId++, x, y, angle, vx: 0, vy: 0, angVel: 0, kind, power: k.power,
    hw: k.L / 2, hh: k.W / 2,
    health: CAR.health, wrecked: false, wreckT: 0,
    driver: null, // 'player' | 'npc' | null
    ai: null, role, color,
    controls: { throttle: 0, brake: 0, steer: 0, handbrake: false },
    cargo: false, horn: false, skid: 0, lastHit: 0,
  };
}

export const forwardSpeed = (c) => c.vx * Math.cos(c.angle) + c.vy * Math.sin(c.angle);
export const speedOf = (c) => Math.hypot(c.vx, c.vy);

const SURFACE = {
  [T.ROAD]: { drag: 1, grip: 1, top: 1 },
  [T.PLAZA]: { drag: 1, grip: 1, top: 1 },
  [T.COBBLE]: { drag: 1.25, grip: 0.85, top: 0.92 }, // Kopfsteinpflaster
  [T.SIDEWALK]: { drag: 1.3, grip: 0.95, top: 0.9 },
  [T.GRASS]: { drag: 3.2, grip: 0.55, top: 0.5 },
  [T.WATER]: { drag: 4, grip: 0.4, top: 0.3 },
  [T.BUILDING]: { drag: 1, grip: 1, top: 1 },
};

export function stepCar(car, dt, city) {
  const ctl = car.wrecked ? { throttle: 0, brake: 0, steer: 0, handbrake: true } : car.controls;
  const surf = SURFACE[city ? surfaceAt(city, car.x, car.y, car.lvl ?? null) : T.ROAD] ?? SURFACE[T.ROAD];
  let c = Math.cos(car.angle), s = Math.sin(car.angle);
  let vf = car.vx * c + car.vy * s;
  let vr = -car.vx * s + car.vy * c;
  // Wetter (traction.js, von world.js je Schritt gesetzt; fehlt = trocken), beim Aquaplaning fast kein Halt
  const tr = car.traction ?? DRY, aq = (car.aqua ?? 0) > 0;
  const kBrake = tr.brake * (aq ? AQUA.brake : 1), kLat = tr.lat * (aq ? AQUA.lat : 1), kSteer = tr.steer * (aq ? AQUA.steer : 1);
  car.spin = 0;

  const pw = car.power ?? 1, top = CAR.maxSpeed * surf.top * (0.55 + 0.45 * pw);
  if (ctl.throttle > 0 && vf < top) {
    const t = vf > 0 ? 1 - (vf / top) * 0.55 : 1.4; // aus dem Rückwärtsrollen kräftiger
    vf += CAR.accel * pw * ctl.throttle * t * tr.accel * dt;
    car.spin = ctl.throttle > 0.8 && tr.accel < 0.7 && vf < 150 ? 1 : 0; // Räder drehen durch (nur Darstellung)
  }
  if (ctl.brake > 0) {
    if (vf > 5) vf = Math.max(0, vf - CAR.brake * ctl.brake * kBrake * dt);
    else if (vf > -CAR.maxReverse) vf -= CAR.accel * 0.6 * ctl.brake * dt;
  }
  if (ctl.handbrake) vf -= sign(vf) * Math.min(Math.abs(vf), CAR.handbrake * kBrake * dt);
  vf -= vf * CAR.drag * surf.drag * dt;
  if (ctl.throttle === 0 && ctl.brake === 0 && Math.abs(vf) < 4) vf = 0;

  const grip = (ctl.handbrake ? CAR.handbrakeGrip : CAR.grip * surf.grip) * kLat;
  car.skid = Math.abs(vr) > 70 ? Math.min(1, Math.abs(vr) / 200) : 0;
  vr *= Math.exp(-grip * dt);

  const av = Math.abs(vf);
  const speedFactor = clamp(av / 80, 0, 1) * (1 - 0.45 * clamp(av / CAR.maxSpeed, 0, 1));
  const target = ctl.steer * CAR.steerRate * speedFactor * sign(vf) * (ctl.handbrake ? 1.35 : 1) * kSteer + (aq ? car.aquaYaw ?? 0 : 0); // Aquaplaning: das Auto giert
  car.angVel += (target - car.angVel) * Math.min(1, 12 * dt);
  car.angle += car.angVel * dt;

  c = Math.cos(car.angle); s = Math.sin(car.angle);
  car.vx = vf * c - vr * s;
  car.vy = vf * s + vr * c;
  car.x += car.vx * dt;
  car.y += car.vy * dt;
  if (car.aqua > 0) car.aqua = Math.max(0, car.aqua - dt);
}

function applyImpact(car, nx, ny, events) {
  const vn = car.vx * nx + car.vy * ny;
  if (vn >= 0) return 0;
  car.vx -= (1 + CAR.restitution) * vn * nx;
  car.vy -= (1 + CAR.restitution) * vn * ny;
  car.vx *= 0.85; car.vy *= 0.85;
  car.angVel *= 0.5;
  const impact = -vn;
  damage(car, impact, events);
  return impact;
}

export function damage(car, impact, events) {
  if (impact > CAR.damageThreshold) {
    car.health = Math.max(0, car.health - (impact - CAR.damageThreshold) * CAR.damageFactor);
    if (events) events.push({ type: 'crash', x: car.x, y: car.y, strength: clamp(impact / 300, 0, 1), carId: car.id });
    if (car.health <= 0 && !car.wrecked) {
      car.wrecked = true;
      if (events) events.push({ type: 'wreck', x: car.x, y: car.y, carId: car.id });
    }
  }
}

const tmp = [];
// Auto gegen statische Welt (Gebäude, Wasser, Kisten, Bäume, Weltrand).
export function collideCarWorld(car, world, events) {
  for (let pass = 0; pass < 3; pass++) {
    let hit = false;
    const box = obbBounds(car);
    for (const r of world.solids.query(box, tmp)) {
      if (!blocks(world, r, car.lvl)) continue;
      const m = r.seg ? obbVsSegment(car, r) : r.r !== undefined ? invert(circleVsObb(r.x, r.y, r.r, car)) : obbVsRect(car, r);
      if (!m) continue;
      if (r.layer === 'barrier' && -(car.vx * m.nx + car.vy * m.ny) > KNOCK.speed) { knockOver(world, r, car, events); continue; }
      car.x += m.nx * m.depth; car.y += m.ny * m.depth;
      applyImpact(car, m.nx, m.ny, events);
      hit = true;
    }
    if (!hit) break;
  }
}

// Sperrt ein festes Hindernis ein Objekt der Ebene lvl? Die Stadtgrenze immer; Häuser, Bäume, Kisten und Poller stehen
// am Boden und sperren Boden und Unterführung, nicht die Brücke darüber; Wände (Ufer, Gleisrand, Geländer, Zaun) nur
// ihre eigene Ebene. Umgefahrene Poller sperren nichts mehr.
export function blocks(world, s, lvl = 0) {
  if (isDown(world, s)) return false;
  if (s.kind === 'border') return true;
  const L = lvl ?? 0;
  if (s.seg && s.kind === 'wall') return (s.lvl ?? 0) === L;
  return L <= 0;
}

// Umgefahrene Poller/Schranken: je Welt gemerkt (über den Schlüssel, damit sie auch nach dem Nachladen der Kachel liegen)
export const isDown = (world, s) => s.layer === 'barrier' && world.knocked?.has(s.key);
function knockOver(world, r, car, events) {
  (world.knocked ??= new Map()).set(r.key, Math.atan2(car.vy, car.vx));
  car.vx *= KNOCK.slow; car.vy *= KNOCK.slow;
  car.health = Math.max(0, car.health - KNOCK.damage);
  if (events) events.push({ type: 'knock', x: r.x, y: r.y, carId: car.id });
}

function invert(m) { return m ? { nx: -m.nx, ny: -m.ny, depth: m.depth } : null; }

export function collideCars(a, b, events) {
  const m = obbVsObb(a, b);
  if (!m) return;
  a.x += m.nx * m.depth / 2; a.y += m.ny * m.depth / 2;
  b.x -= m.nx * m.depth / 2; b.y -= m.ny * m.depth / 2;
  const vn = (a.vx - b.vx) * m.nx + (a.vy - b.vy) * m.ny;
  if (vn >= 0) return;
  const j = -(1 + CAR.restitution) * vn / 2;
  a.vx += j * m.nx; a.vy += j * m.ny;
  b.vx -= j * m.nx; b.vy -= j * m.ny;
  damage(a, -vn * 0.8, events);
  damage(b, -vn * 0.8, events);
}
