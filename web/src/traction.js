// Wetter auf der Straße (rein): wie nass, verschneit und glatt eine Stelle ist, ob dort eine Pfütze steht, wie stark eine
// Böe schiebt – und was daraus für Bremsen, Anfahren, Seitenhalt und Lenkung folgt. world.js schreibt das Ergebnis je
// Schritt ans Auto (car.traction); car.js, traffic.js und trainphysics.js lesen nur die Faktoren. Überdachte Stellen
// (Durchfahrt, Boden unter einer Brücke) sind trocken, Brücken frieren zuerst. Kein world.rng: Aquaplaning-Richtung aus
// dem Pfützen-Hash, Böen aus weather.js gustAt(world.time).
import { edgePuddles } from './wetfx.js';
import { gustAt } from './weather.js';
import { segDist2 } from './geom.js';
import { hash01, nearestEdge } from './map.js';
import { railAt } from './tunnel.js';

export const TRACTION = {
  wet: { brake: 0.77, accel: 0.85, lat: 0.82, steer: 0.95 },
  snow: { brake: 0.5, accel: 0.55, lat: 0.55, steer: 0.8 },
  ice: { brake: 0.33, accel: 0.4, lat: 0.35, steer: 0.65 },
  bridgeIce: 1.5, puddleWet: 0.3,
  rail: { wet: 0.75, ice: 0.6 },
};
export const DRY = Object.freeze({ brake: 1, accel: 1, lat: 1, steer: 1 });
export const AQUA = { speed: 70 / 0.36, time: 0.35, lat: 0.15, steer: 0.2, brake: 0.3, yaw: 0.6 };
// dynamic: Anteil für das selbst gefahrene Auto (dynamics.js) – dort giert eine Böe das Auto auch (echte Reifen), ein
// kleinerer Stoß gibt denselben Versatz wie im Verkehr: ein spürbarer Schubs, kein Wegdriften
export const GUST = { push: 45, threshold: 0.9, bridge: 1.6, warn: 8, dynamic: 0.35 };
const KEYS = ['brake', 'accel', 'lat', 'steer'];
const q = [], qr = [];
const clamp01 = (v) => Math.min(1, Math.max(0, v ?? 0));
// Zwischen 1 (trocken) und dem Tabellenwert f nach Stärke mischen; die Ränder exakt (trocken = genau 1)
const mix = (amt, f) => { const a = clamp01(amt); return a === 0 ? 1 : a === 1 ? f : 1 - (1 - f) * a; };

// Nächste Fahrbahn derselben Ebene unter (x, y) und ob etwas darüber liegt: eine höhere Fahrbahn, die wirklich darüber
// hinweggeht, oder eine Durchfahrt durch ein Haus
const over = [], nodes = new Set();
function under(world, x, y, lvl) {
  let covered = false, near = null, nd = Infinity;
  over.length = 0; nodes.clear();
  for (const s of world.city.edgeSegs.query({ x: x - 40, y: y - 40, w: 80, h: 80 }, q)) {
    const e = s.e;
    if (e.junction) continue;
    const el = e.lvl ?? 0;
    if (el === lvl) { nodes.add(e.a); nodes.add(e.b); } // Anschlüsse der eigenen Ebene ringsum
    const d2 = segDist2(x, y, s.ax, s.ay, s.bx, s.by), half = e.w / 2;
    if (d2 > half * half) continue;
    if (el > lvl) over.push(e);
    if (e.passage && el === lvl) covered = true;
    if (el === lvl && d2 < nd) { nd = d2; near = e; }
  }
  // eine höhere Fahrbahn, die hier an die eigene Ebene anschließt, ist ein Brückenanfang (auch die Gegenfahrbahn einer
  // zweibahnigen Brücke), kein Dach; eine Überführung hat ihre Anschlüsse weit weg
  for (const e of over) if (!nodes.has(e.a) && !nodes.has(e.b)) { covered = true; break; }
  // Hochbahn/Bahnbrücke darüber (Gleise stehen nicht in edgeSegs): Gleis höherer Ebene höchstens 1,5 m entfernt
  if (!covered) for (const f of world.city.render.query({ x: x - 20, y: y - 20, w: 40, h: 40 }, qr)) {
    if (f.layer !== 'rail' || (f.lvl ?? 0) <= lvl) continue;
    const p = f.pts, n = p.length;
    // am Anfang/Ende des Gleiszugs (Rampe, Brückenkopf) ist noch nichts darüber
    if (Math.hypot(x - p[0], y - p[1]) < 60 || Math.hypot(x - p[n - 2], y - p[n - 1]) < 60) continue;
    for (let i = 0; i + 3 < n; i += 2) if (segDist2(x, y, p[i], p[i + 1], p[i + 2], p[i + 3]) < 15 * 15) { covered = true; break; }
    if (covered) break;
  }
  return { covered, near };
}

function inPuddle(city, e, x, y) {
  for (const p of edgePuddles(city, e)) {
    const dx = x - p.x, dy = y - p.y, c = Math.cos(p.a), s = Math.sin(p.a);
    const lx = (dx * c + dy * s) / p.rx, ly = (-dx * s + dy * c) / p.ry;
    if (lx * lx + ly * ly <= 1) return p;
  }
  return null;
}

export function puddleAt(world, x, y, lvl = 0) {
  if ((world.wet ?? 0) <= TRACTION.puddleWet) return null;
  const u = under(world, x, y, lvl);
  return u.covered || !u.near ? null : inPuddle(world.city, u.near, x, y);
}

export function roadCondition(world, x, y, lvl = 0) {
  const u = under(world, x, y, lvl), bridge = lvl >= 1;
  if (u.covered) return { wet: 0, snow: 0, ice: 0, puddle: null, covered: true, bridge };
  const wet = clamp01(world.wet), snow = clamp01(world.snow), ice = Math.min(1, clamp01(world.ice) * (bridge ? TRACTION.bridgeIce : 1));
  const puddle = wet > TRACTION.puddleWet && u.near ? inPuddle(world.city, u.near, x, y) : null;
  return { wet, snow, ice, puddle, covered: false, bridge };
}

export function tractionOf(c) {
  const out = {};
  for (const k of KEYS) out[k] = Math.max(TRACTION.ice[k], mix(c.wet, TRACTION.wet[k]) * mix(c.snow, TRACTION.snow[k]) * mix(c.ice, TRACTION.ice[k]));
  return out;
}

export function adhesionOf(c) {
  return Math.max(TRACTION.rail.ice, mix(c.wet, TRACTION.rail.wet) * mix(c.ice, TRACTION.rail.ice));
}

export function gustPush(world, car, lvl = 0) {
  const wx = world.weather, storm = wx?.storm ?? 0;
  if (storm <= 0 || !wx.wind || (car.role === 'curb' && car.driver === null) || Math.hypot(car.vx, car.vy) < 5) return null;
  const g = gustAt(wx, world.time) - GUST.threshold;
  if (g <= 0) return null;
  const wl = Math.hypot(wx.wind.x, wx.wind.y) || 1;
  const a = GUST.push * storm * g * (lvl >= 1 ? GUST.bridge : 1) / ((car.hw * car.hh) / (21 * 10)) * (car.driver === 'player' && !car.top ? GUST.dynamic : 1);
  return { ax: (wx.wind.x / wl) * a, ay: (wx.wind.y / wl) * a };
}

// Gierimpuls beim Aufschwimmen: Richtung und Stärke aus dem Pfützen-Hash (±AQUA.yaw rad/s)
export const aquaYaw = (p) => (hash01(Math.round(p.x) * 73856 + Math.round(p.y) * 19349) * 2 - 1) * AQUA.yaw;

// Ebene an der Spitze eines Zugs: Straßenbahn = Ebene der Fahrbahn darunter (Brücken!), S-/U-Bahn = Ebene des Gleises
export function spotLevel(world, mode, x, y) {
  if (mode === 'tram') return nearestEdge(world.city, x, y, 25)?.e.lvl ?? 0;
  return railAt(world.city, x, y)?.lvl ?? 0;
}

// Warnschild im HUD für das Fahrzeug des Spielers (Auto oder geführter Zug); zu Fuß und als Fahrgast keins
export function roadWarning(world) {
  const p = world.player, car = p.inCar ? world.cars.find((c) => c.id === p.inCar) : null;
  if (!car && p.ride?.kind !== 'driver') return null;
  if (car?.aqua > 0) return 'Aquaplaning!';
  if (!car && p.ride.underground) return null; // im Tunnel trocken (wie trainAdhesion)
  const at = car ?? p, lvl = car ? car.lvl ?? 0 : spotLevel(world, p.ride.mode, p.x, p.y), c = roadCondition(world, at.x, at.y, lvl);
  if (c.ice > 0.2) return 'Glätte';
  if (c.snow > 0.2) return 'Schnee';
  const g = car ? gustPush(world, car, lvl) : null;
  if (g && Math.hypot(g.ax, g.ay) > GUST.warn) return 'Sturm';
  if (c.wet > 0.3) return 'Nässe';
  return null;
}
