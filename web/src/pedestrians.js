// Passanten: gehen auf dem Gehweg links und rechts der echten Straßen, bleiben mal stehen, biegen an
// Kreuzungen ab oder überqueren die Straße, fliehen vor rasenden Autos, Hupen und Unfällen und stehen
// nach einem Anfahren wieder auf.
import { PED } from './config.js';
import { circleVsRect, circleVsCircle, circleVsSegment } from './collision.js';
import { pointAlong, projectOnPolyline } from './geom.js';
import { inBuilding, onRoad, nearestEdge } from './map.js';

const SHIRTS = ['#e74c3c', '#3498db', '#2ecc71', '#9b59b6', '#f39c12', '#1abc9c', '#ecf0f1', '#34495e', '#d35400', '#e84393'];
const SKIN = ['#f2d0b1', '#e0ac69', '#c68642', '#8d5524', '#f5d6c6'];
let nextId = 1;

export const walkable = (e) => e.inside && e.cls >= 3 && e.cls <= 10 && e.cls !== 9 && e.len > 20;

// Abstand der Gehweg-Laufspur von der Straßenmitte, so dass sie nicht in Häusern liegt (je Seite gecacht).
export function sidewalkOffset(city, e, side) {
  e.sw ??= {};
  if (e.sw[side] !== undefined) return e.sw[side];
  const S = city.scale, half = e.w / 2, p = { x: 0, y: 0, ux: 1, uy: 0 };
  let off = half + 0.5 * S;
  for (let o = half + 2.2 * S; o >= half + 0.5 * S; o -= 0.4 * S) {
    let ok = true;
    for (const f of [0.2, 0.5, 0.8]) {
      pointAlong(e.pts, e.len * f, p);
      const x = p.x - p.uy * o * side, y = p.y + p.ux * o * side;
      if (inBuilding(city, x, y)) { ok = false; break; }
    }
    if (ok) { off = o; break; }
  }
  e.sw[side] = off;
  return off;
}

const tmpP = { x: 0, y: 0, ux: 1, uy: 0 };
export function sidewalkPoint(city, e, side, s, out = {}) {
  pointAlong(e.pts, s, tmpP);
  const o = sidewalkOffset(city, e, side);
  out.x = tmpP.x - tmpP.uy * o * side;
  out.y = tmpP.y + tmpP.ux * o * side;
  return out;
}

export function createPed(city, spot, rng) {
  const p = sidewalkPoint(city, spot.edge, spot.side, spot.s);
  return {
    id: nextId++, x: p.x, y: p.y, facing: 0,
    edge: spot.edge, side: spot.side, s: spot.s, dirSign: rng() < 0.5 ? -1 : 1,
    speed: PED.walk * (0.8 + rng() * 0.45),
    state: 'walk', t: 0, nextIdle: 4 + rng() * 10,
    shirt: SHIRTS[Math.floor(rng() * SHIRTS.length)],
    skin: SKIN[Math.floor(rng() * SKIN.length)],
    threat: null, target: null, step: 0,
  };
}

// Nächster Gehweg-Platz zu einer Position.
export function nearestSpot(city, x, y, radius = 600) {
  const n = nearestEdge(city, x, y, radius, walkable) ?? nearestEdge(city, x, y, radius * 6, walkable);
  if (!n) return null;
  const side = ((x - n.x) * -n.uy + (y - n.y) * n.ux) >= 0 ? 1 : -1;
  return { edge: n.e, side, s: Math.min(Math.max(n.s, 1), n.e.len - 1) };
}

// Zufälliger Gehweg-Platz im Ring minR…maxR um (cx, cy).
export function pedSpawnSpot(city, rng, cx, cy, minR, maxR) {
  const segs = city.edgeSegs.query({ x: cx - maxR, y: cy - maxR, w: 2 * maxR, h: 2 * maxR }, []);
  for (let tries = 0; tries < 30 && segs.length; tries++) {
    const sg = segs[Math.floor(rng() * segs.length)];
    if (!walkable(sg.e)) continue;
    const t = rng(), x = sg.ax + (sg.bx - sg.ax) * t, y = sg.ay + (sg.by - sg.ay) * t;
    const d = Math.hypot(x - cx, y - cy);
    if (d < minR || d > maxR) continue;
    const pr = projectOnPolyline(sg.e.pts, x, y);
    return { edge: sg.e, side: rng() < 0.5 ? 1 : -1, s: Math.min(Math.max(pr.s, 1), sg.e.len - 1) };
  }
  return null;
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
    const m = r.seg ? circleVsSegment(ped.x, ped.y, PED.radius, r)
      : r.r !== undefined ? circleVsCircle(ped.x, ped.y, PED.radius, r.x, r.y, r.r) : circleVsRect(ped.x, ped.y, PED.radius, r);
    if (m) { ped.x += m.nx * m.depth; ped.y += m.ny * m.depth; }
  }
}

// Am Ende einer Kante: nächste Kante am Knoten wählen, meist auf derselben Straßenseite bleiben.
function nextLeg(ped, world) {
  const city = world.city, rng = world.rng, e = ped.edge;
  const node = ped.dirSign > 0 ? e.b : e.a;
  const opts = city.nodes[node].edges.map((k) => city.edges[k]).filter((o) => walkable(o) && o !== e);
  const next = opts.length ? opts[Math.floor(rng() * opts.length)] : e;
  const dirSign = next === e ? -ped.dirSign : next.a === node ? 1 : -1;
  const inset = Math.min(2.5 * city.scale, next.len / 2);
  const s = dirSign > 0 ? inset : next.len - inset;
  const here = { x: ped.x, y: ped.y };
  const a = sidewalkPoint(city, next, 1, s), b = sidewalkPoint(city, next, -1, s);
  const da = Math.hypot(a.x - here.x, a.y - here.y), db = Math.hypot(b.x - here.x, b.y - here.y);
  let side = da < db ? 1 : -1;
  if (next === e || rng() < 0.2) side = -side; // Straße überqueren
  const p = side === 1 ? a : b;
  ped.state = 'cross';
  ped.target = { edge: next, side, s, dirSign, x: p.x, y: p.y };
}

// Liegt der nächste Schritt auf einer Fahrbahn und nähert sich ein fahrendes Auto?
function carComing(world, ped, next) {
  if (!onRoad(world.city, next.x, next.y)) return false;
  return world.cars.some((c) => Math.hypot(c.x - ped.x, c.y - ped.y) < 120 && Math.hypot(c.vx, c.vy) > 25);
}

export function updatePed(ped, world, dt) {
  const rng = world.rng, city = world.city;
  const prevX = ped.x, prevY = ped.y;
  switch (ped.state) {
    case 'walk': {
      ped.nextIdle -= dt;
      if (ped.nextIdle <= 0) { ped.state = 'idle'; ped.t = 1 + rng() * 2.5; ped.nextIdle = 6 + rng() * 12; break; }
      // Vor dem Spieler kurz warten.
      const ahead = sidewalkPoint(city, ped.edge, ped.side, ped.s + ped.dirSign * 14);
      const pl = world.player;
      if (!pl.inCar && Math.hypot(pl.x - ahead.x, pl.y - ahead.y) < 11) break;
      if (carComing(world, ped, ahead)) break; // Gehweg quert hier eine Fahrbahn (Einmündung)
      ped.s += ped.dirSign * ped.speed * dt;
      if (ped.s <= 0 || ped.s >= ped.edge.len) { ped.s = Math.max(0, Math.min(ped.edge.len, ped.s)); nextLeg(ped, world); break; }
      sidewalkPoint(city, ped.edge, ped.side, ped.s, ped);
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
      // Vor fahrenden Autos am Straßenrand warten (höchstens 6 s).
      if (d > v && (ped.wait ?? 0) < 6 && carComing(world, ped, { x: ped.x + dx / d * 12, y: ped.y + dy / d * 12 })) { ped.wait = (ped.wait ?? 0) + dt; break; }
      ped.wait = 0;
      if (d <= v || d > 3000) {
        Object.assign(ped, { edge: tg.edge, side: tg.side, s: tg.s, dirSign: tg.dirSign, state: 'walk', target: null });
        if (d > 3000) sidewalkPoint(city, ped.edge, ped.side, ped.s, ped);
      } else { ped.x += dx / d * v; ped.y += dy / d * v; }
      break;
    }
    case 'flee': {
      ped.t -= dt;
      const dx = ped.x - ped.threat.x, dy = ped.y - ped.threat.y, d = Math.hypot(dx, dy) || 1;
      moveWithCollision(ped, dx / d * PED.run * dt, dy / d * PED.run * dt, world);
      if (ped.t <= 0) {
        const sp = nearestSpot(city, ped.x, ped.y);
        if (sp) { Object.assign(ped, { edge: sp.edge, side: sp.side, s: sp.s }); ped.state = 'return'; }
        else ped.state = 'idle';
      }
      break;
    }
    case 'return': {
      const p = sidewalkPoint(city, ped.edge, ped.side, ped.s);
      const dx = p.x - ped.x, dy = p.y - ped.y, d = Math.hypot(dx, dy);
      const v = ped.speed * 1.2 * dt;
      if (d <= Math.max(v, 2)) { ped.state = 'walk'; ped.x = p.x; ped.y = p.y; }
      else if (d > 1500) { ped.x = p.x; ped.y = p.y; ped.state = 'walk'; }
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

