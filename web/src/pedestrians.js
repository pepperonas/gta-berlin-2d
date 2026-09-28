// Passanten: gehen auf dem Gehweg links und rechts der echten Straßen, bleiben mal stehen, biegen an
// Kreuzungen ab oder überqueren die Straße, fliehen vor rasenden Autos, Hupen und Unfällen und stehen
// nach einem Anfahren wieder auf.
import { touch } from './levels.js';
import { blocks } from './car.js';
import { PED } from './config.js';
import { circleVsRect, circleVsCircle, circleVsSegment } from './collision.js';
import { pointAlong, projectOnPolyline } from './geom.js';
import { inBuilding, onRoad, nearestEdge } from './map.js';
import { updateFight } from './combat.js';

const SHIRTS = ['#e74c3c', '#3498db', '#2ecc71', '#9b59b6', '#f39c12', '#1abc9c', '#ecf0f1', '#34495e', '#d35400', '#e84393'];
const SKIN = ['#f2d0b1', '#e0ac69', '#c68642', '#8d5524', '#f5d6c6'];
let nextId = 1;

export const walkable = (e) => e.inside && e.cls >= 3 && e.cls <= 10 && e.cls !== 9 && e.len > 20;

// Gehwegabschnitt einer Kante: von Ecke zu Ecke (an Kreuzungen endet der Gehweg am Rand der Querstraße).
export function walkRange(city, e) {
  if (e._walk) return e._walk;
  const corner = (n) => {
    const nd = city.nodes.get(n);
    if (!nd || nd.edges.length < 3) return 1 * city.scale;
    let r = 0; for (const k of nd.edges) { const o = city.edges.get(k); if (o && o !== e) r = Math.max(r, o.w / 2); }
    return r + 1 * city.scale;
  };
  let a = corner(e.a), b = e.len - corner(e.b);
  if (b - a < 2 * city.scale) { const m = e.len / 2; a = m - city.scale; b = m + city.scale; }
  return (e._walk = [a, b]);
}

// Abstand der Gehweg-Laufspur von der Straßenmitte, so dass sie nicht in Häusern liegt (je Seite gecacht).
export function sidewalkOffset(city, e, side) {
  e.sw ??= {};
  if (e.sw[side] !== undefined) return e.sw[side];
  const S = city.scale, half = e.w / 2, p = { x: 0, y: 0, ux: 1, uy: 0 };
  let off = null; // null = auf dieser Seite gibt es keinen freien Gehweg (überbaut, Arkade)
  const [w0, w1] = walkRange(city, e);
  const n = Math.max(3, Math.ceil((w1 - w0) / (1.5 * S))); // alle 1,5 m prüfen (schmale Vorsprünge)
  for (let o = half + 2.2 * S; o >= half + 0.5 * S; o -= 0.4 * S) {
    let ok = true;
    for (let k = 0; k <= n; k++) {
      pointAlong(e.pts, w0 + (w1 - w0) * k / n, p);
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
  const o = sidewalkOffset(city, e, side) ?? e.w / 2 + 0.5 * city.scale;
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
  let side = ((x - n.x) * -n.uy + (y - n.y) * n.ux) >= 0 ? 1 : -1;
  if (sidewalkOffset(city, n.e, side) === null) side = -side;
  const [w0, w1] = walkRange(city, n.e);
  return { edge: n.e, side, s: Math.min(Math.max(n.s, w0), w1) };
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
    let side = rng() < 0.5 ? 1 : -1;
    if (sidewalkOffset(city, sg.e, side) === null) side = -side;
    if (sidewalkOffset(city, sg.e, side) === null) continue;
    const [w0, w1] = walkRange(city, sg.e);
    return { edge: sg.e, side, s: Math.min(Math.max(pr.s, w0), w1) };
  }
  return null;
}

export function scare(ped, fromX, fromY, duration = 2.5) {
  if (ped.state === 'down' || ped.state === 'dead') return;
  ped.state = 'flee';
  ped.t = duration;
  ped.threat = { x: fromX, y: fromY };
}

export function knockDown(ped, fromX, fromY) {
  if (ped.state === 'dead') return;
  ped.state = 'down';
  ped.t = 3;
  ped.threat = { x: fromX, y: fromY };
}

const tmp = [];
function moveWithCollision(ped, dx, dy, world) {
  ped.x += dx; ped.y += dy;
  const box = { x: ped.x - 10, y: ped.y - 10, w: 20, h: 20 };
  for (const r of world.solids.query(box, tmp)) {
    if (!blocks(world, r, ped.lvl)) continue;
    const m = r.seg ? circleVsSegment(ped.x, ped.y, PED.radius, r)
      : r.r !== undefined ? circleVsCircle(ped.x, ped.y, PED.radius, r.x, r.y, r.r) : circleVsRect(ped.x, ped.y, PED.radius, r);
    if (m) { ped.x += m.nx * m.depth; ped.y += m.ny * m.depth; }
  }
}

// Am Ende einer Kante: nächste Kante am Knoten wählen, meist auf derselben Straßenseite bleiben.
function nextLeg(ped, world) {
  const city = world.city, rng = world.rng, e = ped.edge;
  const node = ped.dirSign > 0 ? e.b : e.a;
  const hasWalk = (o) => sidewalkOffset(city, o, 1) !== null || sidewalkOffset(city, o, -1) !== null;
  const opts = (city.nodes.get(node)?.edges ?? []).map((k) => city.edges.get(k)).filter((o) => o && walkable(o) && o !== e && hasWalk(o));
  const next = opts.length ? opts[Math.floor(rng() * opts.length)] : e;
  const dirSign = next === e ? -ped.dirSign : next.a === node ? 1 : -1;
  const [w0, w1] = walkRange(city, next);
  const s = dirSign > 0 ? w0 : w1;
  const here = { x: ped.x, y: ped.y };
  const a = sidewalkPoint(city, next, 1, s), b = sidewalkPoint(city, next, -1, s);
  const da = Math.hypot(a.x - here.x, a.y - here.y), db = Math.hypot(b.x - here.x, b.y - here.y);
  let side = da < db ? 1 : -1;
  const sameSide = side;
  if (next === e || rng() < 0.2) side = -side; // Straße überqueren
  if (sidewalkOffset(city, next, side) === null) side = -side;
  // Liegt auf der neuen Straße in der Nähe ein Zebrastreifen oder eine Ampelquerung, dort hinübergehen.
  if (side !== sameSide && sidewalkOffset(city, next, sameSide) !== null) {
    const z = next.crossings?.find((c) => Math.abs(c.s - s) < 30 * city.scale);
    if (z) {
      ped.state = 'cross';
      const t = sidewalkPoint(city, next, side, z.s);
      ped.target = { edge: next, side, s: z.s, dirSign, x: t.x, y: t.y, via: sidewalkPoint(city, next, sameSide, z.s) };
      return;
    }
  }
  const p = side === 1 ? a : b;
  // Um die Ecke über den Schnittpunkt der beiden Gehweglinien gehen, nicht quer durch das Eckhaus.
  const tA = pointAlong(e.pts, Math.min(Math.max(ped.s, 0), e.len), {}), tB = pointAlong(next.pts, s, {});
  const ax = tA.ux * ped.dirSign, ay = tA.uy * ped.dirSign, bx = tB.ux * dirSign, by = tB.uy * dirSign;
  const den = ax * by - ay * bx;
  let via = null;
  if (Math.abs(den) > 0.25) { // Richtungswechsel: Ecke berechnen
    const t = ((p.x - here.x) * by - (p.y - here.y) * bx) / den;
    if (t > 0 && t < 40 * city.scale) via = { x: here.x + ax * t, y: here.y + ay * t };
  }
  ped.state = 'cross';
  ped.target = { edge: next, side, s, dirSign, x: p.x, y: p.y, via };
}

// Liegt der nächste Schritt auf einer Fahrbahn und nähert sich ein fahrendes Auto?
function carComing(world, ped, next) {
  if (!onRoad(world.city, next.x, next.y)) return false;
  return world.cars.some((c) => Math.hypot(c.x - ped.x, c.y - ped.y) < 120 && Math.hypot(c.vx, c.vy) > 25 && touch(world.city, ped, c));
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
      const [w0, w1] = walkRange(city, ped.edge);
      if (ped.s <= w0 || ped.s >= w1) { ped.s = Math.max(w0, Math.min(w1, ped.s)); nextLeg(ped, world); break; }
      sidewalkPoint(city, ped.edge, ped.side, ped.s, ped);
      break;
    }
    case 'idle':
      ped.t -= dt;
      if (ped.t <= 0) { ped.state = 'walk'; if (rng() < 0.3) ped.dirSign *= -1; }
      break;
    case 'cross': {
      const tg = ped.target;
      if (tg.via && Math.hypot(tg.via.x - ped.x, tg.via.y - ped.y) < 3) tg.via = null;
      const aim = tg.via ?? tg;
      const dx = aim.x - ped.x, dy = aim.y - ped.y, d = Math.hypot(dx, dy);
      const v = ped.speed * 1.35 * dt;
      // Vor fahrenden Autos am Straßenrand warten (höchstens 6 s).
      if (d > v && (ped.wait ?? 0) < 6 && carComing(world, ped, { x: ped.x + dx / d * 12, y: ped.y + dy / d * 12 })) { ped.wait = (ped.wait ?? 0) + dt; break; }
      ped.wait = 0;
      if (!tg.via && (d <= v || d > 3000 || (ped.crossT = (ped.crossT ?? 0) + dt) > 20)) {
        Object.assign(ped, { edge: tg.edge, side: tg.side, s: tg.s, dirSign: tg.dirSign, state: 'walk', target: null, crossT: 0 });
        sidewalkPoint(city, ped.edge, ped.side, ped.s, ped);
      } else if (d > 0) moveWithCollision(ped, dx / d * Math.min(v, d), dy / d * Math.min(v, d), world);
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
      // kommt nicht voran (z. B. ein Auto steht auf dem Rückweg): nach 4 s einfach weitergehen
      ped.returnT = (ped.returnT ?? 0) + dt;
      if (d < (ped.returnBest ?? Infinity) - 5) { ped.returnBest = d; ped.returnT = 0; }
      if (ped.returnT > 4) { ped.x = p.x; ped.y = p.y; ped.state = 'walk'; ped.returnT = 0; ped.returnBest = undefined; }
      else if (d <= Math.max(v, 2)) { ped.state = 'walk'; ped.x = p.x; ped.y = p.y; ped.returnBest = undefined; }
      else if (d > 1500) { ped.x = p.x; ped.y = p.y; ped.state = 'walk'; }
      else moveWithCollision(ped, dx / d * v, dy / d * v, world);
      break;
    }
    case 'dead':
      ped.deadT = (ped.deadT ?? 0) + dt;
      return;
    case 'hang': { // an seinem Platz: kleine Bewegungen, Blick wandert (life.js)
      const hg = ped.hang;
      ped.x = hg.x; ped.y = hg.y;
      ped.hangT = (ped.hangT ?? 0) + dt;
      const k = ped.id * 1.7;
      ped.facing = hg.face + Math.sin(ped.hangT * 0.6 + k) * (hg.act === 'queue' || hg.act === 'wait' ? 0.25 : 0.45);
      if (hg.act === 'queue' || hg.act === 'wait' || hg.act === 'music') ped.step = Math.sin(ped.hangT * 1.5 + k) * 3; // Tippeln
      return;
    }
    case 'fight':
      if (!updateFight(ped, world, dt, moveWithCollision)) { ped.state = 'idle'; scare(ped, world.player.x, world.player.y, 2); }
      break;
    case 'down':
      ped.t -= dt;
      // aufstehen und weglaufen (erst den Zustand verlassen – scare() ignoriert Liegende)
      if (ped.t <= 0) {
        ped.state = 'idle';
        if (ped.angry) { ped.state = 'fight'; ped.fightT = 0; ped.hitCd = 0.5; ped.angry = false; } // Gegenwehr
        else scare(ped, ped.threat.x, ped.threat.y, 2);
      }
      break;
  }
  const mx = ped.x - prevX, my = ped.y - prevY;
  if (mx * mx + my * my > 0.01) { ped.facing = Math.atan2(my, mx); ped.step += Math.hypot(mx, my); }
}

