// ÖPNV in der Welt: Fahrplan-Fahrzeuge fortschreiben (transit.js), Straßenbahnen halten vor Hindernissen und klingeln,
// Busse werden nahe der Kamera zu echten KI-Fahrzeugen, die ihre Halte der Reihe nach anfahren und dort halten
// (Wartende steigen ein). Straßenbahnwagen sind für Verkehr und Spieler feste, fahrende Hindernisse.
// Nur in der Welt mit Tagesrhythmus und wenn die Fahrplandaten geladen sind (city.transit).
import { stepTransit, positionAt, pointOn, trainCars, TRANSIT, BUS } from './transit.js';
import { createCar, speedOf, damage } from './car.js';
import { placeOnLane, dropClaims, projectNear } from './traffic.js';
import { buildLaneGraph, nearestLane } from './roadgraph.js';
import { obbVsObb, circleVsObb } from './collision.js';
import { LIFE } from './life.js';

export const BUS_COLOR = '#f0cf1f';
const outOfView = (w, x, y, pad = 80) => Math.abs(x - w.camera.x) > LIFE.viewHalfX + pad || Math.abs(y - w.camera.y) > LIFE.viewHalfY + pad;

// Straßenbahn vor einem Hindernis auf dem Gleis? (Spitze plus 2–8 m voraus, seitlich 2,2 m)
function tramBlocked(w, p, v) {
  const pos = positionAt(p, v.tau);
  if (pos.dwelling) return false;
  const head = pos.s;
  const cam = w.camera;
  const hp = pointOn(p, head);
  if (Math.abs(hp.x - cam.x) > 2500 || Math.abs(hp.y - cam.y) > 2500) return false; // weit weg: keine Hindernisse
  for (const d of [20, 45, 75]) {
    const q = pointOn(p, head + d);
    const hit = (x, y, r) => Math.hypot(x - q.x, y - q.y) < r;
    // !ride: defensiv – in der jetzigen Schrittfolge nicht beobachtbar (Fahrgast sitzt hinter der Spitze, updateRide läuft nach updateTransit)
    if (!w.player.inCar && !w.player.ride && !w.player.dead && hit(w.player.x, w.player.y, 22)) return true;
    for (const c of w.cars) if (hit(c.x, c.y, 24 + c.hw * 0.4)) return true;
    for (const b of w.bikes ?? []) if (b.state === 'ride' && hit(b.x, b.y, 18)) return true;
    for (const ped of w.peds) if (ped.state !== 'dead' && ped.state !== 'hang' && hit(ped.x, ped.y, 16)) return true;
  }
  return false;
}

// Bus als KI-Fahrzeug auf die Straße setzen: auf die Spur, die dicht am Linienweg liegt und in seine Richtung zeigt
// (nicht die Querstraße an der Kreuzung); danach folgt er dem Linienweg (traffic.js followLine).
function materializeBus(w, p, v) {
  const pos = positionAt(p, v.tau), pt = pointOn(p, pos.s);
  const hit = nearestLane(buildLaneGraph(w.city), pt.x, pt.y, pt.angle, 250, true);
  if (!hit || Math.hypot(hit.x - pt.x, hit.y - pt.y) > 60) return null;
  const lp0 = hit.lane.pts, li = Math.min(hit.i, lp0.length - 4);
  const ldx = lp0[li + 2] - lp0[li], ldy = lp0[li + 3] - lp0[li + 1], ll = Math.hypot(ldx, ldy) || 1;
  if ((ldx * Math.cos(pt.angle) + ldy * Math.sin(pt.angle)) / ll < 0.8) return null;
  if (w.cars.some((c) => Math.hypot(c.x - hit.x, c.y - hit.y) < 130)) return null;
  let s = 0;
  const lp = hit.lane.pts;
  for (let i = 0; i < hit.i; i += 2) s += Math.hypot(lp[i + 2] - lp[i], lp[i + 3] - lp[i + 1]);
  s += Math.hypot(hit.x - lp[hit.i], hit.y - lp[hit.i + 1]);
  const car = createCar({ x: hit.x, y: hit.y, kind: 'bus', color: BUS_COLOR });
  car.driver = 'npc';
  placeOnLane(car, w.city, hit.lane, s, w.rng);
  car.ai.follow = { pts: p.shape.pts, cum: p.shape.cum, s: pos.s };
  car.line = p.name;
  car.duty = { bus: true, pid: p.id, stop: Math.max(1, pos.stop), since: w.time, veh: v.key, s: pos.s, off: 0 };
  v.live = car.id;
  w.cars.push(car);
  return car;
}

export function updateTransit(w, dt) {
  const tr = w.city.transit;
  w.railObs = [];
  if (!tr || !w.rhythm) return;
  const st = (w.transit ??= { tracked: new Map(), seed: w.seed ?? 0 });
  stepTransit(st, tr, w.camera, w.clock, w.day, dt, w.time, (p, v) => p.mode === 'tram' && tramBlocked(w, p, v) && (v.blockedT = (v.blockedT ?? 0) + dt) >= 0);
  const cam = w.camera;
  // Busse in der Nähe auf die Straße holen, fertige/ferne wieder abbauen
  for (const [id, s] of st.tracked) {
    const p = tr.patterns[id];
    if (p.mode === 'tram') {
      for (const v of s.veh) {
        if ((v.blockedT ?? 0) > 2.5 && !v.rang) { v.rang = true; const q = pointOn(p, positionAt(p, v.tau).s); w.events.push({ type: 'tram-bell', x: q.x, y: q.y }); }
        if (v.blockedT && !tramBlocked(w, p, v)) { v.blockedT = 0; v.rang = false; }
      }
      continue;
    }
    if (p.mode !== 'bus') continue;
    for (const v of s.veh) {
      if (v.live || v.gone) continue;
      const q = pointOn(p, positionAt(p, v.tau).s);
      const d = Math.hypot(q.x - cam.x, q.y - cam.y);
      if (d < TRANSIT.busLive && (outOfView(w, q.x, q.y) || !w._transitPopulated) && positionAt(p, v.tau).s < p.stops[p.stops.length - 1] - 200) materializeBus(w, p, v);
    }
  }
  w._transitPopulated = true;
  for (const c of w.cars) if (c.duty?.bus && !c.done) updateBus(w, c, tr.patterns[c.duty.pid], dt);
  w.cars = w.cars.filter((c) => {
    if (!c.duty?.bus) return true;
    const far = Math.hypot(c.x - cam.x, c.y - cam.y) > 2400, gone = (c.done || c.wrecked || c.driver !== 'npc') && outOfView(w, c.x, c.y, 200);
    if (far || gone) {
      if (c.driver === 'npc') dropClaims(c, w);
      const s = st.tracked.get(c.duty.pid);
      const v = s?.veh.find((x) => x.key === c.duty.veh);
      if (v) v.gone = true;
      return c.driver !== 'npc' ? true : false; // gekaperter Bus bleibt dem Spieler
    }
    return true;
  });
  // Straßenbahnwagen als Hindernisse (nahe der Kamera)
  for (const [id, s] of st.tracked) {
    const p = tr.patterns[id];
    if (p.mode !== 'tram') continue;
    for (const v of s.veh) {
      const pos = positionAt(p, v.tau);
      const head = pointOn(p, pos.s);
      if (Math.abs(head.x - cam.x) > 2200 || Math.abs(head.y - cam.y) > 2200) continue;
      const moving = !pos.dwelling && !(v.blockedT > 0);
      const spd = moving ? (p.stops[pos.stop] - p.stops[Math.max(0, pos.stop - 1)]) / Math.max(1, p.off[pos.stop] - p.off[Math.max(0, pos.stop - 1)] - p.dwell) : 0;
      for (const c of trainCars(p, pos.s)) w.railObs.push({ x: c.x, y: c.y, angle: c.angle, hw: c.L / 2, hh: c.W / 2, vx: Math.cos(c.angle) * spd, vy: Math.sin(c.angle) * spd, tram: true });
    }
  }
  collideRail(w);
}

// Bus: folgt dem Linienweg; erreicht er die Lage eines Halts, hält er dort (Wartende steigen ein). Nach dem letzten
// Halt oder wenn er den Weg verloren hat (> 15 m daneben, 8 s lang), ist er fertig und fährt außer Sicht davon.
function updateBus(w, c, p, dt) {
  const d = c.duty;
  if (c.driver !== 'npc' || !c.ai || !p) return;
  const q = projectNear(p.shape.pts, p.shape.cum, c.x, c.y, d.s - 100, 800);
  if (q.d < 150) d.s = Math.max(d.s, q.s);
  d.off = q.d > 150 ? (d.off ?? 0) + dt : 0;
  if (d.off > 8) { c.done = true; c.ai.follow = null; return; }
  if (d.boarding) {
    if (c.ai.hold > 0) return;
    d.boarding = false; d.stop++;
    if (d.stop >= p.stops.length) { c.done = true; c.ai.follow = null; }
    return;
  }
  // kein Vorankommen auf dem Weg (verwinkelte Kreuzung, Stau): nach 25 s die Linie aufgeben und davonfahren
  if (d.s > (d.bestS ?? -1) + 20) { d.bestS = d.s; d.progressT = w.time; }
  else if (w.time - (d.progressT ?? w.time) > 25) { c.done = true; c.ai.follow = null; return; }
  while (d.stop < p.stops.length && d.s > p.stops[d.stop] + 60) d.stop++; // verpasste Halte (z. B. beim Aufsetzen dahinter)
  if (d.stop >= p.stops.length) { c.done = true; c.ai.follow = null; return; }
  if (d.s >= p.stops[d.stop] - 45 && q.d < 150) {
    c.ai.hold = p.dwell; d.boarding = true;
    const sp = pointOn(p, p.stops[d.stop]);
    const before = w.peds.length;
    w.peds = w.peds.filter((x) => !(x.state === 'hang' && (x.hang.act === 'wait' || x.hang.act === 'queue') && Math.hypot(x.x - sp.x, x.y - sp.y) < 140));
    if (w.peds.length < before) w.events.push({ type: 'bus-board', x: sp.x, y: sp.y, n: before - w.peds.length });
    w.events.push({ type: 'bus-stop', x: sp.x, y: sp.y, line: p.name, stop: p.stopNames[d.stop] });
  }
}

// Straßenbahnwagen sind unbeweglich: Autos und Spielfigur werden herausgeschoben, schnelle Autos nehmen Schaden
function collideRail(w) {
  for (const o of w.railObs) {
    for (const c of w.cars) {
      if (Math.abs(c.x - o.x) > o.hw + c.hw || Math.abs(c.y - o.y) > o.hw + c.hw) continue;
      const m = obbVsObb(c, o);
      if (!m) continue;
      c.x += m.nx * m.depth; c.y += m.ny * m.depth;
      const vn = (c.vx - o.vx) * m.nx + (c.vy - o.vy) * m.ny;
      if (vn < 0) {
        c.vx -= 1.3 * vn * m.nx; c.vy -= 1.3 * vn * m.ny;
        if (-vn > 60) { damage(c, -vn); w.events.push({ type: 'crash', x: c.x, y: c.y, strength: Math.min(1, -vn / 300) }); }
      }
    }
    const pl = w.player;
    if (!pl.inCar && !pl.ride) { // !ride: defensiv – updateRide setzt den Fahrgast ohnehin nach updateTransit in den Wagen
      const m = circleVsObb(pl.x, pl.y, 7, o);
      if (m) { pl.x += m.nx * m.depth; pl.y += m.ny * m.depth; }
    }
  }
}

// Sichtbare Züge/Bahnen im Rechteck v: [{ p, cars, mode, line }] – U- und S-Bahn nur, wo ihr Gleis oberirdisch liegt
export function transitVisible(w, v, railAt) {
  const tr = w.city.transit, st = w.transit, out = [];
  if (!tr || !st) return out;
  const pad = 400;
  for (const [id, s] of st.tracked) {
    const p = tr.patterns[id];
    if (p.mode === 'bus') continue;
    for (const veh of s.veh) {
      const pos = positionAt(p, veh.tau), head = pointOn(p, pos.s);
      const len = p.mode === 'tram' ? 320 : 1100;
      if (head.x < v.x - pad - len || head.x > v.x + v.w + pad + len || head.y < v.y - pad - len || head.y > v.y + v.h + pad + len) continue;
      let cars = trainCars(p, pos.s);
      if (p.mode !== 'tram') cars = cars.filter((c) => railAt(c.x, c.y));
      if (cars.length) out.push({ p, cars, mode: p.mode, line: p.name, lit: !pos.dwelling });
    }
  }
  return out;
}
