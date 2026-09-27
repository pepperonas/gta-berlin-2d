// Passanten: laufen den Gehweg-Ring ihres Blocks ab, bleiben mal stehen, überqueren an Ecken die
// Straße, fliehen vor rasenden Autos, Hupen und Unfällen und stehen nach einem Anfahren wieder auf.
import { PED, TILE, ROAD_W } from './config.js';
import { circleVsRect, circleVsCircle } from './collision.js';

const SHIRTS = ['#e74c3c', '#3498db', '#2ecc71', '#9b59b6', '#f39c12', '#1abc9c', '#ecf0f1', '#34495e', '#d35400', '#e84393'];
const SKIN = ['#f2d0b1', '#e0ac69', '#c68642', '#8d5524', '#f5d6c6'];
let nextId = 1;

export function perimeter(ring) { return 2 * (ring.w + ring.h); }

export function pointOnRing(ring, s) {
  const P = perimeter(ring);
  s = ((s % P) + P) % P;
  if (s < ring.w) return { x: ring.x + s, y: ring.y };
  s -= ring.w;
  if (s < ring.h) return { x: ring.x + ring.w, y: ring.y + s };
  s -= ring.h;
  if (s < ring.w) return { x: ring.x + ring.w - s, y: ring.y + ring.h };
  s -= ring.w;
  return { x: ring.x, y: ring.y + ring.h - s };
}

export function nearestOnRing(ring, x, y) {
  const P = perimeter(ring);
  let best = { s: 0, d: Infinity };
  for (let s = 0; s < P; s += 4) {
    const p = pointOnRing(ring, s);
    const d = (p.x - x) ** 2 + (p.y - y) ** 2;
    if (d < best.d) best = { s, d };
  }
  return best.s;
}

export function createPed(block, rng) {
  const s = rng() * perimeter(block.ring);
  const p = pointOnRing(block.ring, s);
  return {
    id: nextId++, x: p.x, y: p.y, facing: 0,
    block, s, dirSign: rng() < 0.5 ? -1 : 1,
    speed: PED.walk * (0.8 + rng() * 0.45),
    state: 'walk', t: 0, nextIdle: 4 + rng() * 10,
    shirt: SHIRTS[Math.floor(rng() * SHIRTS.length)],
    skin: SKIN[Math.floor(rng() * SKIN.length)],
    threat: null, target: null, step: 0,
  };
}

export function scare(ped, fromX, fromY, duration = 2.5) {
  if (ped.state === 'down') return;
  ped.state = 'flee';
  ped.t = duration;
  ped.threat = { x: fromX, y: fromY };
}

export function knockDown(ped, fromX, fromY) {
  ped.state = 'down';
  ped.t = 3;
  ped.threat = { x: fromX, y: fromY };
}

const tmp = [];
function moveWithCollision(ped, dx, dy, world) {
  ped.x += dx; ped.y += dy;
  const box = { x: ped.x - 10, y: ped.y - 10, w: 20, h: 20 };
  for (const r of world.solids.query(box, tmp)) {
    const m = r.r !== undefined ? circleVsCircle(ped.x, ped.y, PED.radius, r.x, r.y, r.r) : circleVsRect(ped.x, ped.y, PED.radius, r);
    if (m) { ped.x += m.nx * m.depth; ped.y += m.ny * m.depth; }
  }
}

function blockAt(world, x, y) {
  let best = null, bd = Infinity;
  for (const b of world.city.blocks) {
    const cx = b.x + b.w / 2, cy = b.y + b.h / 2;
    const d = (cx - x) ** 2 + (cy - y) ** 2;
    if (d < bd) { bd = d; best = b; }
  }
  return best;
}

// Ecke des Rings → Nachbarblock über die Straße (falls vorhanden).
function crossingFrom(ped, world) {
  const r = ped.block.ring, gap = ROAD_W * TILE + TILE;
  const p = pointOnRing(r, ped.s);
  const corners = [[r.x, r.y], [r.x + r.w, r.y], [r.x + r.w, r.y + r.h], [r.x, r.y + r.h]];
  const k = corners.findIndex(([cx, cy]) => Math.abs(cx - p.x) < 3 && Math.abs(cy - p.y) < 3);
  if (k < 0) return null;
  const [cx, cy] = corners[k];
  const options = [];
  const horiz = k === 0 || k === 3 ? -gap : gap;
  const vert = k === 0 || k === 1 ? -gap : gap;
  options.push({ x: cx + horiz, y: cy }, { x: cx, y: cy + vert });
  const pick = options[Math.floor(world.rng() * 2)];
  const target = world.city.blocks.find((b) => pick.x >= b.ring.x - 1 && pick.x <= b.ring.x + b.ring.w + 1 && pick.y >= b.ring.y - 1 && pick.y <= b.ring.y + b.ring.h + 1);
  return target ? { block: target, x: pick.x, y: pick.y } : null;
}

export function updatePed(ped, world, dt) {
  const rng = world.rng;
  const prevX = ped.x, prevY = ped.y;
  switch (ped.state) {
    case 'walk': {
      ped.nextIdle -= dt;
      if (ped.nextIdle <= 0) { ped.state = 'idle'; ped.t = 1 + rng() * 2.5; ped.nextIdle = 6 + rng() * 12; break; }
      // Vor dem Spieler oder anderen Passanten kurz warten.
      const ahead = pointOnRing(ped.block.ring, ped.s + ped.dirSign * 14);
      const pl = world.player;
      if (!pl.inCar && Math.hypot(pl.x - ahead.x, pl.y - ahead.y) < 11) break;
      const P = perimeter(ped.block.ring);
      const before = ((ped.s % P) + P) % P;
      ped.s += ped.dirSign * ped.speed * dt;
      const p = pointOnRing(ped.block.ring, ped.s);
      ped.x = p.x; ped.y = p.y;
      // An Ecken manchmal die Straße überqueren.
      const after = ((ped.s % P) + P) % P;
      const corners = [0, ped.block.ring.w, ped.block.ring.w + ped.block.ring.h, 2 * ped.block.ring.w + ped.block.ring.h];
      for (const cs of corners) {
        const passed = ped.dirSign > 0 ? before < cs && after >= cs : before > cs && after <= cs;
        if (passed && rng() < 0.25) {
          ped.s = cs;
          const cr = crossingFrom(ped, world);
          if (cr) { ped.state = 'cross'; ped.target = cr; }
        }
      }
      break;
    }
    case 'idle':
      ped.t -= dt;
      if (ped.t <= 0) { ped.state = 'walk'; if (rng() < 0.3) ped.dirSign *= -1; }
      break;
    case 'cross': {
      const tg = ped.target;
      const dx = tg.x - ped.x, dy = tg.y - ped.y, d = Math.hypot(dx, dy);
      const v = ped.speed * 1.35 * dt;
      if (d <= v) {
        ped.block = tg.block; ped.s = nearestOnRing(tg.block.ring, tg.x, tg.y);
        ped.state = 'walk'; ped.target = null;
      } else { ped.x += dx / d * v; ped.y += dy / d * v; }
      break;
    }
    case 'flee': {
      ped.t -= dt;
      const dx = ped.x - ped.threat.x, dy = ped.y - ped.threat.y, d = Math.hypot(dx, dy) || 1;
      moveWithCollision(ped, dx / d * PED.run * dt, dy / d * PED.run * dt, world);
      if (ped.t <= 0) { ped.state = 'return'; ped.block = blockAt(world, ped.x, ped.y); ped.s = nearestOnRing(ped.block.ring, ped.x, ped.y); }
      break;
    }
    case 'return': {
      const p = pointOnRing(ped.block.ring, ped.s);
      const dx = p.x - ped.x, dy = p.y - ped.y, d = Math.hypot(dx, dy);
      const v = ped.speed * 1.2 * dt;
      if (d <= Math.max(v, 2)) { ped.state = 'walk'; ped.x = p.x; ped.y = p.y; }
      else moveWithCollision(ped, dx / d * v, dy / d * v, world);
      break;
    }
    case 'down':
      ped.t -= dt;
      if (ped.t <= 0) scare(ped, ped.threat.x, ped.threat.y, 2);
      break;
  }
  const mx = ped.x - prevX, my = ped.y - prevY;
  if (mx * mx + my * my > 0.01) { ped.facing = Math.atan2(my, mx); ped.step += Math.hypot(mx, my); }
}
