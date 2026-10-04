// Radfahrer und E-Roller: fahren auf dem Spurgraph des Autoverkehrs (Einbahnstraßen und Abbiegeverbote gelten damit
// auch für sie), aber nur auf der rechten Spur, seitlich versetzt – auf dem Radstreifen, wo der Querschnitt einen hat,
// sonst am rechten Fahrbahnrand. Hauptstraßen ohne Radstreifen meiden sie. Die meisten halten bei Rot, manche nicht.
// Dazu abgestellte E-Roller auf dem Gehweg (reine Darstellung, deterministisch je Straße).
import { touch } from './levels.js';
import { buildLaneGraph, turnAngle } from './roadgraph.js';
import { laneOffsets } from './street.js';
import { offsetPolyline, polylineLength, pointAlong } from './geom.js';
import { signalState } from './signals.js';
import { hash01, inBuilding, onRoad } from './map.js';
import { walkable, sidewalkOffset, walkRange, createPed, nearestSpot, knockDown } from './pedestrians.js';

export const BIKE = { r: 6, bike: [45, 62], scooter: [50, 70], accel: 55, brake: 160, look: 34, obey: 0.8, share: 0.15, scooterShare: 0.25 };
let nextId = 1;
export const RIDER_SHIRTS = ['#2d3436', '#0984e3', '#d63031', '#00b894', '#fdcb6e', '#6c5ce7', '#e17055', '#dfe6e9'];
export const riderShirt = (b) => RIDER_SHIRTS[b.seed % RIDER_SHIRTS.length];

const cyclePx = (lane) => (lane.dir === 1 ? lane.edge.cs.right : lane.edge.cs.left).cycle;
// Radtaugliche Spur: rechte Spur, Straße mit Radstreifen oder Nebenstraße (keine Autobahn/Schnellstraße)
export function bikeable(lane) {
  const cls = lane.edge.cls;
  return !lane.removed && !lane.busOnly && lane.k === lane.n - 1 && cls >= 3 && cls <= 8 && (cyclePx(lane) > 0 || cls >= 5);
}

// Fahrlinie des Rades zu einer Spur (Radstreifenmitte bzw. 0,8 m vom rechten Fahrbahnrand), gecacht an der Spur
export function bikePath(city, lane) {
  if (lane._bike) return lane._bike;
  const cs = lane.edge.cs, S = city.scale, lo = laneOffsets(cs, S);
  const side = lane.dir === 1 ? cs.right : cs.left;
  const lf = lane.dir === 1 ? lo.fwd[lane.k] : -lo.bwd[lane.k];
  const target = cs.width / 2 - side.parkW - (side.cycle > 0 ? side.cycle / 2 : 0.8 * S);
  const pts = offsetPolyline(lane.pts, Math.max(0, target - lf));
  return (lane._bike = { pts, len: polylineLength(pts), onCycle: side.cycle > 0 });
}

export function createBike(city, lane, s, rng, kind = 'bike') {
  const [v0, v1] = BIKE[kind];
  const b = { id: nextId++, kind, lane, s, x: 0, y: 0, angle: 0, speed: 0, vmax: v0 + rng() * (v1 - v0), state: 'ride',
    obeys: rng() < BIKE.obey, cross: null, pedal: rng() * 6, seed: Math.floor(rng() * 1e6), t: 0 };
  place(city, b);
  b.speed = b.vmax * 0.8;
  return b;
}
const tmp = { x: 0, y: 0, ux: 1, uy: 0 };
function place(city, b) {
  const p = bikePath(city, b.lane);
  pointAlong(p.pts, b.s, tmp);
  b.x = tmp.x; b.y = tmp.y; b.angle = Math.atan2(tmp.uy, tmp.ux);
}

// Nächste radtaugliche Spur (lieber geradeaus); ohne Alternative notfalls jede
function nextBikeLane(lane, rng) {
  const opts = lane.next.filter(bikeable), pool = opts.length ? opts : lane.next;
  if (!pool.length) return null;
  let total = 0;
  const w = pool.map((m) => { const x = 0.3 + Math.max(0, Math.cos(turnAngle(lane, m))) * 2; total += x; return x; });
  let r = rng() * total;
  for (let i = 0; i < pool.length; i++) if ((r -= w[i]) <= 0) return pool[i];
  return pool[pool.length - 1];
}

// Etwas voraus (Auto, Mensch, anderes Rad, Spielfigur)? Abstand entlang der Fahrtrichtung oder Infinity
function aheadDist(b, world) {
  const c = Math.cos(b.angle), s = Math.sin(b.angle);
  let best = Infinity;
  const test = (x, y, lat, len = 0) => {
    const rx = x - b.x, ry = y - b.y, along = rx * c + ry * s - len;
    if (along <= 0 || along > BIKE.look + len) return;
    if (Math.abs(-rx * s + ry * c) > lat) return;
    best = Math.min(best, along);
  };
  for (const o of world.cars) {
    if (Math.abs(o.x - b.x) > 90 || Math.abs(o.y - b.y) > 90 || !touch(world.city, b, o)) continue;
    const cosA = Math.cos(o.angle - b.angle);
    // stehender Querverkehr (wartet an der Kreuzung – auch auf dieses Rad): vorbeifahren statt gegenseitig warten
    if (Math.abs(cosA) < 0.6 && Math.abs(o.vx) + Math.abs(o.vy) < 20) continue;
    test(o.x, o.y, o.hh + 5, o.hw * Math.abs(cosA));
  }
  for (const p of world.peds) if (p.state !== 'dead' && Math.abs(p.x - b.x) < 45 && Math.abs(p.y - b.y) < 45 && touch(world.city, b, p)) test(p.x, p.y, 9);
  for (const o of world.bikes) if (o !== b && o.state === 'ride' && Math.abs(o.x - b.x) < 45 && Math.abs(o.y - b.y) < 45 && touch(world.city, b, o)) test(o.x, o.y, 8);
  const pl = world.player;
  if (!pl.inCar && touch(world.city, b, pl)) test(pl.x, pl.y, 10);
  return best;
}

export function updateBike(b, world, dt) {
  if (b.state === 'lying') { b.t += dt; return; }
  const city = world.city;
  if (b.lane.removed) { b.state = 'gone'; return; }
  const path = bikePath(city, b.lane);
  let target = b.vmax;
  // Ampel am Spurende
  const rest = path.len - b.s;
  if (!b.cross && b.obeys && rest < 60 && city.signals?.has(b.lane.to)) {
    const light = signalState(city, b.lane.to, b.angle, world.time);
    if (light !== 'green' && rest > 6) target = Math.min(target, Math.max(0, (rest - 10) * 2));
  }
  // Notausgang: wer ohne Ampel über 6 s steht, schiebt sich 2 s lang an allem vorbei (nie ein ewiger Knoten)
  if (b.pushT > 0) b.pushT -= dt;
  else {
    const d = aheadDist(b, world);
    if (d < BIKE.look) target = Math.min(target, Math.max(0, (d - 14) * 2.5));
    b.stuckT = b.speed < 2 && target < 2 && !(rest < 60 && city.signals?.has(b.lane.to)) ? (b.stuckT ?? 0) + dt : 0;
    if (b.stuckT > 6) { b.pushT = 2; b.stuckT = 0; }
  }
  b.speed = target > b.speed ? Math.min(target, b.speed + BIKE.accel * dt) : Math.max(target, b.speed - BIKE.brake * dt);
  if (b.speed < 2 && target < 2) b.speed = 0;
  let move = b.speed * dt;
  b.pedal += move * 0.12;
  if (b.cross) { // Kreuzung queren: gerade zum Anfang der nächsten Fahrlinie
    const cr = b.cross;
    cr.u += move;
    if (cr.u < cr.L) { b.x = cr.x0 + (cr.x1 - cr.x0) * cr.u / cr.L; b.y = cr.y0 + (cr.y1 - cr.y0) * cr.u / cr.L; b.angle = Math.atan2(cr.y1 - cr.y0, cr.x1 - cr.x0); return; }
    move = cr.u - cr.L; b.cross = null; b.s = 0;
  }
  b.s += move;
  if (b.s >= path.len) {
    const next = nextBikeLane(b.lane, world.rng);
    if (!next) { b.state = 'gone'; return; }
    const np = bikePath(city, next), e = path.pts, n = e.length;
    const x0 = e[n - 2], y0 = e[n - 1], x1 = np.pts[0], y1 = np.pts[1], L = Math.hypot(x1 - x0, y1 - y0);
    b.lane = next;
    if (L > 2) { b.cross = { x0, y0, x1, y1, L, u: 0 }; b.x = x0; b.y = y0; return; }
    b.s = 0;
  }
  place(city, b);
}

// Fahrer runter (Auto-Zusammenstoß, Schuss, Schlag): das Rad bleibt liegen, der Fahrer wird ein Passant, der vor dem
// Stoß flieht (knockDown; seit dem Umbau fällt niemand mehr um; fromX/fromY: woher der Stoß kam). Liefert den Fahrer
// (oder null, wenn kein Gehweg in der Nähe ist).
export function dismount(w, b, fromX, fromY, { fall = true } = {}) {
  b.state = 'lying'; b.t = 0; b.speed = 0; b.cross = null;
  const sp = nearestSpot(w.city, b.x, b.y);
  if (!sp) return null;
  const ped = createPed(w.city, sp, w.rng);
  Object.assign(ped, { x: b.x, y: b.y, shirt: riderShirt(b), lvl: b.lvl });
  if (fall) knockDown(ped, fromX, fromY);
  w.peds.push(ped);
  return ped;
}

// Zufälliger Startplatz auf einer radtauglichen Spur im Ring minR…maxR
export function bikeSpawn(city, rng, cx, cy, minR, maxR) {
  const g = buildLaneGraph(city);
  const segs = g.hash.query({ x: cx - maxR, y: cy - maxR, w: 2 * maxR, h: 2 * maxR }, []);
  for (let t = 0; t < 30 && segs.length; t++) {
    const sg = segs[Math.floor(rng() * segs.length)];
    if (!bikeable(sg.lane)) continue;
    const p = bikePath(city, sg.lane), s = rng() * p.len;
    pointAlong(p.pts, s, tmp);
    const d = Math.hypot(tmp.x - cx, tmp.y - cy);
    if (d < minR || d > maxR) continue;
    return { lane: sg.lane, s };
  }
  return null;
}

// Abgestellte E-Roller am Gehweg einer Kante (Darstellung): { x, y, angle, lying, seed }, gecacht an der Kante
export function parkedScooters(city, e) {
  if (e._scoot) return e._scoot;
  const out = [];
  if (walkable(e) && e.cls <= 8) {
    const S = city.scale, [w0, w1] = walkRange(city, e);
    const n = Math.floor((w1 - w0) / (90 * S) + hash01(e.id * 7 + 3) * 1.3);
    for (let k = 0; k < n; k++) {
      const side = hash01(e.id * 13 + k) < 0.5 ? 1 : -1, off = sidewalkOffset(city, e, side);
      if (off === null) continue;
      const s = w0 + hash01(e.id * 31 + k * 7) * (w1 - w0);
      pointAlong(e.pts, s, tmp);
      const o = off + 0.8 * S; // an der Hauswandseite des Gehwegs, nicht mitten im Weg
      const x = tmp.x - tmp.uy * o * side, y = tmp.y + tmp.ux * o * side;
      if (inBuilding(city, x, y) || onRoad(city, x, y)) continue;
      const lying = hash01(e.id * 17 + k) < 0.2;
      out.push({ x, y, angle: Math.atan2(tmp.uy, tmp.ux) + (hash01(e.id + k * 3) - 0.5) * (lying ? 3 : 0.8), lying, seed: e.id * 10 + k });
    }
  }
  return (e._scoot = out);
}
