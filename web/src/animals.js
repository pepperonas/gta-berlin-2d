// Tiere der Stadt: Taubenschwärme picken auf Plätzen und vor Imbissen und flattern auf, wenn jemand zu nah kommt,
// ein Auto vorbeirast oder ein Schuss fällt; Enten paddeln in Ufernähe auf Spree, Kanal und Seen und schwimmen weg,
// wenn man am Ufer auf sie zugeht. Orte sind deterministisch (Ort-Hash), Bewegungen laufen über den Welt-Zufall.
// Wie beim Stadtleben (life.js): neue Tiere nur außer Sicht, Aufgeflogene verschwinden, sobald sie aus dem Bild sind.
import { hash01, inBuilding, onRoad, surfaceAt, T } from './map.js';
import { AREA_KIND } from './citycodes.js';
import { pointInRings } from './geom.js';
import { frontOf, LIFE } from './life.js';

export const ANIMALS = {
  radius: 1500, every: 0.5, maxFlocks: 14, maxDucks: 6,
  scarePerson: 55, scareCar: 50, scareShot: 450,
  fly: [95, 140], flyTime: [2.5, 5], climb: 40,   // px/s, s, px/s nach oben
  duckFlee: 60,                                  // px: Spielfigur am Ufer
};
const h = (...n) => hash01(n.reduce((a, b) => a * 31 + Math.round(b), 11));

// Wo gerade Tauben bzw. Enten sein wollen: [{ key, kind, x, y, n }]
export function animalSpots(city, cx, cy, radius = ANIMALS.radius) {
  const out = [], box = { x: cx - radius, y: cy - radius, w: 2 * radius, h: 2 * radius };
  for (const q of city.poiHash.query(box, [])) {
    if (q.cat !== 'food' && q.cat !== 'cafe' && q.cat !== 'ubahn' && q.cat !== 'sbahn') continue;
    if (h(q.x, q.y) > 0.3) continue;
    const f = frontOf(city, q);
    if (f) out.push({ key: `t${Math.round(q.x)},${Math.round(q.y)}`, kind: 'pigeon', x: f.x + f.ux * 25, y: f.y + f.uy * 25, n: 4 + Math.floor(h(q.y, q.x) * 6) });
  }
  for (const f of city.render.query(box, [])) {
    if (f.layer === 'area' && f.kind === AREA_KIND.plaza) {
      const b = f.bbox; if (!b || b.w * b.h < 90000) continue;
      for (let t = 0; t < 10; t++) {
        const x = b.x + h(b.x, t, 1) * b.w, y = b.y + h(b.y, t, 2) * b.h;
        if (pointInRings(x, y, f.rings) && !inBuilding(city, x, y) && !onRoad(city, x, y)) { out.push({ key: `p${Math.round(b.x)},${Math.round(b.y)}`, kind: 'pigeon', x, y, n: 5 + Math.floor(h(b.x, b.y) * 8) }); break; }
      }
    } else if (f.layer === 'water') {
      const b = f.bbox; if (!b || b.w * b.h < 250000) continue;
      // Ufernah: Punkte an den Ringecken, ein paar Meter ins Wasser versetzt
      const ring = f.rings[0], m = ring.length / 2, n = Math.min(4, Math.floor(m / 40) + 1);
      for (let g = 0; g < n; g++) {
        const i = 2 * Math.floor(h(b.x, b.y, g) * m), x0 = ring[i], y0 = ring[i + 1];
        if (Math.abs(x0 - cx) > radius || Math.abs(y0 - cy) > radius) continue;
        for (const d of [60, 90, 130]) {
          let spot = null;
          for (let a = 0; a < 8 && !spot; a++) { const x = x0 + Math.cos(a * 0.785) * d, y = y0 + Math.sin(a * 0.785) * d; if (surfaceAt(city, x, y) === T.WATER) spot = { x, y }; }
          if (spot) { out.push({ key: `d${Math.round(x0)},${Math.round(y0)}`, kind: 'duck', x: spot.x, y: spot.y, n: 2 + Math.floor(h(x0, y0) * 4) }); break; }
        }
      }
    }
  }
  return out;
}

function makeBird(w, s, i) {
  const a = w.rng() * Math.PI * 2, r = s.kind === 'duck' ? 10 + i * 9 : 6 + w.rng() * 22;
  return { kind: s.kind, key: s.key, x: s.x + Math.cos(a) * r, y: s.y + Math.sin(a) * r, z: 0, vx: 0, vy: 0, facing: w.rng() * 6.28,
    state: s.kind === 'duck' ? 'swim' : 'peck', t: w.rng() * 2, hx: s.x, hy: s.y, seed: Math.floor(w.rng() * 1e6), flap: 0 };
}

// Tiere um die Kamera halten (alle 0,5 s): fehlende Schwärme außer Sicht aufstellen, ferne abbauen
export function manageAnimals(w, all = false) {
  if (!w.rhythm) return;
  if (!all && w.time - (w._animT ?? -99) < ANIMALS.every) return;
  w._animT = w.time;
  const cam = w.camera, inView = (x, y) => Math.abs(x - cam.x) < LIFE.viewHalfX + 60 && Math.abs(y - cam.y) < LIFE.viewHalfY + 60;
  w.flocks ??= new Map();
  const spots = animalSpots(w.city, cam.x, cam.y), want = new Set(spots.map((s) => s.key));
  // Abbau: fern, nicht mehr gewünscht, oder aufgeflogen und aus dem Bild
  w.animals = w.animals.filter((a) => {
    const far = Math.hypot(a.x - cam.x, a.y - cam.y) > LIFE.despawn;
    const flown = a.state === 'fly' && !inView(a.x, a.y);
    return !(far || flown || (!want.has(a.key) && !inView(a.x, a.y)));
  });
  for (const [key, f] of w.flocks) if (!want.has(key) && !inView(f.x, f.y)) w.flocks.delete(key);
  let pigeons = 0, ducks = 0;
  for (const f of w.flocks.values()) f.kind === 'duck' ? ducks++ : pigeons++;
  for (const s of spots) {
    if (w.flocks.has(s.key)) continue;
    if (s.kind === 'duck' ? ducks >= ANIMALS.maxDucks : pigeons >= ANIMALS.maxFlocks) continue;
    if (!all && inView(s.x, s.y)) continue;
    w.flocks.set(s.key, { ...s });
    s.kind === 'duck' ? ducks++ : pigeons++;
    for (let i = 0; i < s.n; i++) w.animals.push(makeBird(w, s, i));
  }
  // aufgescheuchte Schwärme kommen erst wieder, wenn ihr Platz außer Sicht ist
  for (const [key, f] of w.flocks) if (!w.animals.some((a) => a.key === key) && !inView(f.x, f.y)) w.flocks.delete(key);
}

function flyOff(w, a, fx, fy) {
  if (a.kind === 'duck') { // Enten schwimmen weg
    const ang = Math.atan2(a.y - fy, a.x - fx) + (w.rng() - 0.5) * 0.8;
    a.state = 'flee'; a.t = 2 + w.rng() * 1.5; a.vx = Math.cos(ang) * 38; a.vy = Math.sin(ang) * 38; a.facing = ang;
    return;
  }
  const ang = Math.atan2(a.y - fy, a.x - fx) + (w.rng() - 0.5) * 1.4, v = ANIMALS.fly[0] + w.rng() * (ANIMALS.fly[1] - ANIMALS.fly[0]);
  a.state = 'fly'; a.t = ANIMALS.flyTime[0] + w.rng() * (ANIMALS.flyTime[1] - ANIMALS.flyTime[0]);
  a.vx = Math.cos(ang) * v; a.vy = Math.sin(ang) * v; a.facing = ang;
}

export function updateAnimals(w, dt) {
  if (!w.animals.length) return;
  const pl = w.player, shots = w.events.filter((e) => e.type === 'shot' || e.type === 'horn');
  const threats = [];
  if (!pl.inCar && !pl.dead) threats.push({ x: pl.x, y: pl.y, r: ANIMALS.scarePerson });
  for (const c of w.cars) if (Math.abs(c.vx) + Math.abs(c.vy) > 60) threats.push({ x: c.x, y: c.y, r: ANIMALS.scareCar + c.hw });
  for (const p of w.peds) if (p.state === 'flee' || p.state === 'walk' && p.style === 'jog' || p.state === 'dog') threats.push({ x: p.x, y: p.y, r: 35 });
  for (const a of w.animals) {
    a.flap += dt;
    if (a.state === 'peck' || a.state === 'swim') {
      const scare = shots.find((e) => Math.hypot(e.x - a.x, e.y - a.y) < ANIMALS.scareShot) ?? threats.find((t) => Math.hypot(t.x - a.x, t.y - a.y) < t.r);
      if (scare) { flyOff(w, a, scare.x, scare.y); continue; }
      if ((a.t -= dt) <= 0) { // picken / paddeln: kleine Hüpfer bzw. Treiben um den Heimatpunkt
        a.t = a.kind === 'duck' ? 1.5 + w.rng() * 3 : 0.4 + w.rng() * 1.6;
        const back = Math.hypot(a.hx - a.x, a.hy - a.y) > (a.kind === 'duck' ? 60 : 30);
        const ang = back ? Math.atan2(a.hy - a.y, a.hx - a.x) : w.rng() * Math.PI * 2, v = a.kind === 'duck' ? 8 + w.rng() * 8 : 14 + w.rng() * 16;
        a.vx = Math.cos(ang) * v; a.vy = Math.sin(ang) * v; a.facing = ang; a.hop = a.kind === 'duck' ? 0 : 0.18;
      }
      if (a.kind === 'pigeon') { if ((a.hop -= dt) <= 0) { a.vx *= 0.8; a.vy *= 0.8; } }
      const nx = a.x + a.vx * dt, ny = a.y + a.vy * dt;
      if (a.kind === 'duck' ? surfaceAt(w.city, nx, ny) === T.WATER : !inBuilding(w.city, nx, ny)) { a.x = nx; a.y = ny; }
      else { a.vx = -a.vx; a.vy = -a.vy; }
    } else if (a.state === 'flee') { // Ente schwimmt weg
      const nx = a.x + a.vx * dt, ny = a.y + a.vy * dt;
      if (surfaceAt(w.city, nx, ny) === T.WATER) { a.x = nx; a.y = ny; } else { a.vx = -a.vx; a.vy = -a.vy; }
      if ((a.t -= dt) <= 0) { a.state = 'swim'; a.t = 1; a.hx = a.x; a.hy = a.y; }
    } else if (a.state === 'fly') {
      a.x += a.vx * dt; a.y += a.vy * dt; a.z = Math.min(160, a.z + ANIMALS.climb * dt);
      if ((a.t -= dt) <= 0) a.state = 'land';
    } else if (a.state === 'land') { // sinkt und landet, wo es geht
      a.x += a.vx * 0.5 * dt; a.y += a.vy * 0.5 * dt; a.z = Math.max(0, a.z - 55 * dt);
      if (a.z === 0) {
        if (inBuilding(w.city, a.x, a.y) || surfaceAt(w.city, a.x, a.y) === T.WATER) { a.state = 'fly'; a.t = 1; }
        else { a.state = 'peck'; a.t = 1; a.hx = a.x; a.hy = a.y; a.vx = a.vy = 0; }
      }
    }
  }
}
