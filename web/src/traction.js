// Wetter auf der Straße (rein): wie nass, verschneit und glatt eine Stelle ist, ob dort eine Pfütze steht, wie stark eine
// Böe schiebt – und was daraus für Bremsen, Anfahren, Seitenhalt und Lenkung folgt. world.js schreibt das Ergebnis je
// Schritt ans Auto (car.traction); car.js, traffic.js und trainphysics.js lesen nur die Faktoren. Überdachte Stellen
// (Durchfahrt, Boden unter einer Brücke) sind trocken, Brücken frieren zuerst. Kein world.rng: Aquaplaning-Richtung aus
// dem Pfützen-Hash, Böen aus weather.js gustAt(world.time).
import { edgePuddles } from './wetfx.js';
import { gustAt } from './weather.js';
import { segDist2 } from './geom.js';
import { hash01 } from './map.js';

export const TRACTION = {
  wet: { brake: 0.77, accel: 0.85, lat: 0.82, steer: 0.95 },
  snow: { brake: 0.5, accel: 0.55, lat: 0.55, steer: 0.8 },
  ice: { brake: 0.33, accel: 0.4, lat: 0.35, steer: 0.65 },
  bridgeIce: 1.5, puddleWet: 0.3,
  rail: { wet: 0.75, ice: 0.6 },
};
export const DRY = Object.freeze({ brake: 1, accel: 1, lat: 1, steer: 1 });
export const AQUA = { speed: 70 / 0.36, time: 0.35, lat: 0.15, steer: 0.2, brake: 0.3, yaw: 0.6 };
export const GUST = { push: 45, threshold: 0.9, bridge: 1.6, warn: 8 };
const KEYS = ['brake', 'accel', 'lat', 'steer'];
const q = [];
const clamp01 = (v) => Math.min(1, Math.max(0, v ?? 0));
// Zwischen 1 (trocken) und dem Tabellenwert f nach Stärke mischen; die Ränder exakt (trocken = genau 1)
const mix = (amt, f) => { const a = clamp01(amt); return a === 0 ? 1 : a === 1 ? f : 1 - (1 - f) * a; };

// Nächste Fahrbahn derselben Ebene unter (x, y) und ob etwas darüber liegt (höhere Ebene oder Durchfahrt)
function under(world, x, y, lvl) {
  let covered = false, near = null, nd = Infinity;
  for (const s of world.city.edgeSegs.query({ x: x - 40, y: y - 40, w: 80, h: 80 }, q)) {
    const e = s.e;
    if (e.junction) continue;
    const d2 = segDist2(x, y, s.ax, s.ay, s.bx, s.by), half = e.w / 2;
    if (d2 > half * half) continue;
    const el = e.lvl ?? 0;
    if (el > lvl || (e.passage && el === lvl)) covered = true;
    if (el === lvl && d2 < nd) { nd = d2; near = e; }
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
  const a = GUST.push * storm * g * (lvl >= 1 ? GUST.bridge : 1) / ((car.hw * car.hh) / (21 * 10));
  return { ax: (wx.wind.x / wl) * a, ay: (wx.wind.y / wl) * a };
}

// Gierimpuls beim Aufschwimmen: Richtung und Stärke aus dem Pfützen-Hash (±AQUA.yaw rad/s)
export const aquaYaw = (p) => (hash01(Math.round(p.x) * 73856 + Math.round(p.y) * 19349) * 2 - 1) * AQUA.yaw;
