// Spielwelt: verbindet Stadt, Spieler, Autos, Passanten und Mission zu einem Simulationsschritt.
// Enthält kein DOM – Eingaben kommen als abstrakter Zustand (siehe input.js), Ausgaben als Ereignisse.
import { PLAYER, PED, TRAFFIC, CAR, PARKED } from './config.js';
import { clamp, damp } from './math.js';
import { mulberry32 } from './rng.js';
import { circleVsRect, circleVsCircle, circleVsObb, circleVsSegment, obbVsRect, obbVsObb, obbVsSegment, obbBounds } from './collision.js';
import { createCar, stepCar, collideCarWorld, collideCars, speedOf, forwardSpeed, CAR_COLORS, damage } from './car.js';
import { placeOnLane, spawnSpot, driveAi } from './traffic.js';
import { createPed, updatePed, scare, knockDown, nearestSpot, pedSpawnSpot } from './pedestrians.js';
import { createMission, updateMission, resetMission } from './mission.js';
import { insideBorder, inBuilding, locationName, hash01 } from './map.js';
import { parkingStrip } from './street.js';
import { PARK } from './citycodes.js';
import { pointAlong } from './geom.js';
import { buildLaneGraph, nearestLane } from './roadgraph.js';
import { sidewalkPoint } from './pedestrians.js';
import { resolveSave } from './save.js';

// city: dekodierte Karte (map.js decodeCity). cars/pedestrians: Zielbevölkerung um die Kamera.
export function createWorld({ city, seed = 1989, cars = TRAFFIC.cars, pedestrians = TRAFFIC.pedestrians } = {}) {
  if (!city) throw new Error('createWorld braucht eine Karte (city)');
  const rng = mulberry32(seed + 7);
  const w = {
    city, rng, solids: city.solids, cars: [], peds: [], events: [], time: 0,
    player: { x: 0, y: 0, angle: 0, inCar: null, step: 0, stun: 0 },
    playerCarId: null,
    mission: createMission(),
    money: 0, completed: 0, bestTime: null,
    camera: { x: 0, y: 0, zoom: 1 },
    prompt: null, notice: null,
  };

  spawnPlayerAndCar(w);
  const pc = city.parked[0];
  w.cars.push(createCar({ x: pc.x, y: pc.y, angle: pc.angle, color: '#16a085', role: 'parked' }));

  w.carTarget = cars; w.pedTarget = pedestrians;
  w.camera.x = w.player.x; w.camera.y = w.player.y;
  w.focusKey = `world${++worldCount}`;
  w.loading = !city.focus(w.focusKey, w.camera.x, w.camera.y);
  if (!w.loading) populate(w);
  return w;
}
let worldCount = 0;

// Verkehr, Passanten und Parker verwerfen (nach einem Ortswechsel); am neuen Ort baut streamWorld sie neu auf.
export function resetPopulation(w, keepCar = null) {
  w.cars = w.cars.filter((c) => {
    const keep = c === keepCar || c.id === w.playerCarId || c.id === w.player.inCar || c.cargo || c.role === 'parked';
    if (!keep && c.parkKey) w.parkedKeys?.delete(c.parkKey); // Stellplatz wieder frei
    return keep;
  });
  w.peds = [];
  w.populated = false;
}

// Startbevölkerung: im ganzen Umkreis verteilt (auch im Bild), danach nur noch außerhalb der Sicht.
function populate(w) {
  w.populated = true;
  for (let k = 0; k < w.carTarget; k++) spawnTraffic(w, 120, TRAFFIC.spawnMax);
  for (let k = 0; k < w.pedTarget; k++) spawnPed(w, 60, TRAFFIC.spawnMax);
}

// Kacheln um die Kamera nachladen. false = der Stadtteil hier ist noch nicht da (im Browser kommt er asynchron),
// die Welt steht dann still, bis er geladen ist.
export function streamWorld(w) {
  w.loading = !w.city.focus(w.focusKey, w.camera.x, w.camera.y);
  if (!w.loading && !w.populated) populate(w);
  return !w.loading;
}

function spawnPed(w, minR, maxR) {
  const sp = pedSpawnSpot(w.city, w.rng, w.camera.x, w.camera.y, minR, maxR);
  if (!sp) return null;
  const ped = createPed(w.city, sp, w.rng);
  w.peds.push(ped);
  return ped;
}

// Bevölkerung um die Kamera halten: Fernes abbauen, Fehlendes im Ring außerhalb der Sicht erzeugen.
function managePopulation(w) {
  const cam = w.camera, far = TRAFFIC.despawn;
  const keep = (c) => c.id === w.playerCarId || c.id === w.player.inCar || c.cargo || c.role === 'parked' || c.role === 'curb' || c.driver === 'player';
  w.cars = w.cars.filter((c) => keep(c) || Math.hypot(c.x - cam.x, c.y - cam.y) < far);
  w.peds = w.peds.filter((p) => Math.hypot(p.x - cam.x, p.y - cam.y) < far);
  const npc = w.cars.filter((c) => c.driver === 'npc' || (c.driver === null && c.role === 'traffic')).length;
  if (npc < w.carTarget) spawnTraffic(w, TRAFFIC.spawnMin, TRAFFIC.spawnMax);
  if (w.peds.length < w.pedTarget) spawnPed(w, TRAFFIC.spawnMin * 0.8, TRAFFIC.spawnMax);
}

function spawnPlayerAndCar(w) {
  const { places } = w.city;
  w.player.x = places.playerSpawn.x; w.player.y = places.playerSpawn.y;
  w.player.inCar = null; w.player.angle = -Math.PI / 2;
  let car = w.cars.find((c) => c.id === w.playerCarId);
  if (!car) {
    car = createCar({ x: 0, y: 0, color: '#c0392b', role: 'player' });
    w.cars.push(car);
    w.playerCarId = car.id;
  }
  Object.assign(car, {
    x: places.playerCar.x, y: places.playerCar.y, angle: places.playerCar.angle,
    vx: 0, vy: 0, angVel: 0, health: CAR.health, wrecked: false, wreckT: 0, cargo: false, driver: null, ai: null,
  });
  car.controls = { throttle: 0, brake: 0, steer: 0, handbrake: false };
}

function spawnTraffic(w, minR, maxR) {
  for (let tries = 0; tries < 8; tries++) {
    const sp = spawnSpot(w.city, w.rng, w.camera.x, w.camera.y, minR, maxR);
    if (!sp) return null;
    if (!w.cars.every((o) => Math.hypot(o.x - sp.x, o.y - sp.y) > 70)) continue;
    const car = createCar({ x: sp.x, y: sp.y, color: CAR_COLORS[Math.floor(w.rng() * CAR_COLORS.length)] });
    placeOnLane(car, w.city, sp.lane, sp.s, w.rng);
    car.driver = 'npc';
    w.cars.push(car);
    return car;
  }
  return null;
}

// Teleport-Ziel zu einem Kartenpunkt: im Auto auf die nächste Fahrspur (in Fahrtrichtung), zu Fuß auf den nächsten
// Gehweg. null, wenn der Punkt außerhalb des Spielgebiets liegt oder nichts Passendes in der Nähe ist.
// Liegt das Ziel in einem noch nicht geladenen Stadtteil, kommt { pending: true } zurück (später erneut fragen).
export function findTeleportSpot(w, x, y) {
  const city = w.city;
  if (!insideBorder(city, x, y)) return null;
  if (!city.focus('teleport', x, y)) return { pending: true, x, y };
  let spot = null;
  if (playerCar(w)) {
    const hit = nearestLane(buildLaneGraph(city), x, y, undefined, 3000);
    if (hit) {
      const p = hit.lane.pts, i = hit.i;
      spot = { x: hit.x, y: hit.y, angle: Math.atan2(p[i + 3] - p[i + 1], p[i + 2] - p[i]) };
    }
  } else {
    const sp = nearestSpot(city, x, y, 3000);
    if (sp) { const p = sidewalkPoint(city, sp.edge, sp.side, sp.s); spot = { x: p.x, y: p.y, angle: 0 }; }
  }
  if (!spot || !insideBorder(city, spot.x, spot.y) || inBuilding(city, spot.x, spot.y)) return null;
  spot.name = locationName(city, spot.x, spot.y);
  return spot;
}

export function teleportTo(w, spot) {
  const car = playerCar(w), p = w.player;
  if (car) Object.assign(car, { x: spot.x, y: spot.y, angle: spot.angle, vx: 0, vy: 0, angVel: 0 });
  p.x = spot.x; p.y = spot.y;
  w.camera.x = spot.x; w.camera.y = spot.y;
  // Verkehr und Passanten sofort am neuen Ort aufbauen (sonst wäre die Straße einige Sekunden leer).
  resetPopulation(w, car);
  streamWorld(w);
  w.city.release('teleport');
}

// --- Geparkte Autos am Straßenrand -------------------------------------------------------
const SLOT_M = { parallel: 5.6, diagonal: 3.0, perpendicular: 2.6 };

// Stellplätze einer Kante (deterministisch, einmal berechnet): Mitte des Parkstreifens, Autoausrichtung je Aufstellung.
export function parkingSlots(city, e) {
  if (e._slots) return e._slots;
  const S = city.scale, slots = [];
  if (e.inside && e.cls <= 8 && !e.bridge) {
    // StVO § 12: kein Parken bis 5 m vor/nach der Ecke einer Kreuzung (Ecke = halbe Breite der Querstraße vom Knoten)
    const cornerGap = (n) => { const nd = city.nodes.get(n); if (!nd) return 5 * S; let r = 0; for (const k of nd.edges) { const o = city.edges.get(k); if (o && o !== e) r = Math.max(r, o.w / 2); } return nd.edges.length > 2 ? r + 5 * S : 2 * S; };
    const m0 = cornerGap(e.a), m1 = cornerGap(e.b), p = { x: 0, y: 0, ux: 1, uy: 0 };
    const avoid = [city.places.dropoff, city.places.playerCar, city.places.pickup, ...(city.parked ?? [])];
    for (const side of [-1, 1]) {
      const ps = parkingStrip(e.cs, side);
      if ((ps.kind !== PARK.lane && ps.kind !== PARK.half) || ps.depth < 0.9 * S) continue;
      const step = SLOT_M[ps.orient] * S;
      for (let i = 0, s = m0 + step / 2; s < e.len - m1 - step / 2; i++, s += step) {
        if (hash01(e.id * 977 + (side + 1) * 31 + i * 7919) >= PARKED.share) continue;
        pointAlong(e.pts, s, p);
        const x = p.x - p.uy * ps.offset, y = p.y + p.ux * ps.offset;
        if (avoid.some((q) => q && Math.hypot(q.x - x, q.y - y) < 12 * S)) continue;
        let angle = Math.atan2(p.uy, p.ux) + (side < 0 ? Math.PI : 0);          // parallel in Fahrtrichtung der Seite
        if (ps.orient === 'perpendicular') angle += side * Math.PI / 2;
        else if (ps.orient === 'diagonal') angle += side * Math.PI / 4;
        slots.push({ key: `${e.id}:${side}:${i}`, x, y, angle });
      }
    }
  }
  e._slots = slots;
  return slots;
}

function slotFree(w, slot) {
  const probe = { x: slot.x, y: slot.y, angle: slot.angle, hw: CAR.length / 2, hh: CAR.width / 2 };
  for (const s of w.solids.query(obbBounds(probe), [])) {
    const m = s.seg ? obbVsSegment(probe, s) : s.r !== undefined ? circleVsObb(s.x, s.y, s.r, probe) : obbVsRect(probe, s);
    if (m && m.depth > 1) return false;
  }
  return w.cars.every((c) => Math.hypot(c.x - slot.x, c.y - slot.y) > 30);
}

// Parkende Autos im Umkreis der Kamera erzeugen, ferne wieder abbauen (Stellplatz wird dann wieder frei).
function manageParked(w) {
  const cam = w.camera;
  w.parkedKeys ??= new Set();
  w.cars = w.cars.filter((c) => {
    if (c.role !== 'curb' || c.driver === 'player' || c.id === w.playerCarId || c.cargo) return true;
    if (Math.hypot(c.x - cam.x, c.y - cam.y) < PARKED.despawn) return true;
    w.parkedKeys.delete(c.parkKey);
    return false;
  });
  if ((w.parkTick = (w.parkTick ?? 0) + 1) % 20 !== 1) return;
  const R = PARKED.radius, seen = new Set();
  for (const sg of w.city.edgeSegs.query({ x: cam.x - R, y: cam.y - R, w: 2 * R, h: 2 * R }, [])) {
    if (seen.has(sg.e)) continue;
    seen.add(sg.e);
    for (const slot of parkingSlots(w.city, sg.e)) {
      if (w.parkedKeys.has(slot.key) || Math.hypot(slot.x - cam.x, slot.y - cam.y) > R || !slotFree(w, slot)) continue;
      const car = createCar({ x: slot.x, y: slot.y, angle: slot.angle, color: CAR_COLORS[Math.floor(hash01(slot.x * 31 + slot.y) * CAR_COLORS.length)], role: 'curb' });
      car.parkKey = slot.key; car.controls.handbrake = true;
      w.parkedKeys.add(slot.key);
      w.cars.push(car);
    }
  }
}

export function playerCar(w) { return w.cars.find((c) => c.id === w.player.inCar) ?? null; }

// Mission neu starten: Spieler zum Späti, eigenes Auto repariert zurück auf den Parkplatz.
export function restartMission(w) {
  for (const c of w.cars) c.cargo = false;
  resetMission(w.mission);
  const far = Math.hypot(w.camera.x - w.city.places.playerSpawn.x, w.camera.y - w.city.places.playerSpawn.y) > TRAFFIC.despawn;
  spawnPlayerAndCar(w);
  w.camera.x = w.player.x; w.camera.y = w.player.y;
  if (far) { resetPopulation(w); streamWorld(w); }
}

const tmp = [];
function pushCircleOutOfWorld(w, obj, r) {
  const box = { x: obj.x - r - 2, y: obj.y - r - 2, w: 2 * r + 4, h: 2 * r + 4 };
  for (const s of w.solids.query(box, tmp)) {
    const m = s.seg ? circleVsSegment(obj.x, obj.y, r, s) : s.r !== undefined ? circleVsCircle(obj.x, obj.y, r, s.x, s.y, s.r) : circleVsRect(obj.x, obj.y, r, s);
    if (m) { obj.x += m.nx * m.depth; obj.y += m.ny * m.depth; }
  }
}

function spotFree(w, x, y, r, ignoreCar) {
  const box = { x: x - r, y: y - r, w: 2 * r, h: 2 * r };
  for (const s of w.solids.query(box, tmp)) {
    const m = s.seg ? circleVsSegment(x, y, r, s) : s.r !== undefined ? circleVsCircle(x, y, r, s.x, s.y, s.r) : circleVsRect(x, y, r, s);
    if (m) return false;
  }
  return w.cars.every((c) => c === ignoreCar || !circleVsObb(x, y, r, c));
}

function tryEnter(w) {
  const p = w.player;
  let best = null, bd = PLAYER.enterDist;
  for (const c of w.cars) {
    if (c.wrecked) continue;
    const d = Math.hypot(c.x - p.x, c.y - p.y);
    if (d < bd) { bd = d; best = c; }
  }
  if (!best) return false;
  if (best.driver === 'npc') {
    // Fahrer steigt aus und flieht.
    const ped = fleeingDriver(w, best, p.x, p.y, 3.5);
    if (ped) w.peds.push(ped);
    w.events.push({ type: 'carjack', x: best.x, y: best.y });
  }
  best.driver = 'player'; best.ai = null;
  best.controls = { throttle: 0, brake: 0, steer: 0, handbrake: false };
  p.inCar = best.id;
  if (best.role !== 'player' && !w.cars.some((c) => c.id === w.playerCarId && !c.wrecked)) w.playerCarId = best.id;
  w.events.push({ type: 'door', x: best.x, y: best.y });
  return true;
}

function sideSpot(car, side, extra) {
  const rx = -Math.sin(car.angle), ry = Math.cos(car.angle);
  const d = car.hh + PLAYER.radius + extra;
  return { x: car.x + rx * d * side, y: car.y + ry * d * side };
}

function tryExit(w) {
  const car = playerCar(w);
  if (!car) return false;
  const fx = Math.cos(car.angle), fy = Math.sin(car.angle);
  const cands = [sideSpot(car, -1, 3), sideSpot(car, 1, 3),
    { x: car.x - fx * (car.hw + 10), y: car.y - fy * (car.hw + 10) },
    { x: car.x + fx * (car.hw + 10), y: car.y + fy * (car.hw + 10) }];
  const spot = cands.find((s) => spotFree(w, s.x, s.y, PLAYER.radius, car));
  if (!spot) { w.notice = { text: 'Kein Platz zum Aussteigen', t: 1.5 }; return false; }
  car.driver = null;
  car.controls = { throttle: 0, brake: 0, steer: 0, handbrake: speedOf(car) < 60 };
  w.player.inCar = null;
  w.player.x = spot.x; w.player.y = spot.y;
  w.events.push({ type: 'door', x: car.x, y: car.y });
  return true;
}

// Fahrer steigt aus und flieht (wird danach ein normaler Passant).
function fleeingDriver(w, car, fromX, fromY, secs) {
  const sp = nearestSpot(w.city, car.x, car.y);
  if (!sp) return null;
  const ped = createPed(w.city, sp, w.rng);
  const s = sideSpot(car, -1, 14); ped.x = s.x; ped.y = s.y;
  scare(ped, fromX, fromY, secs);
  return ped;
}

function updatePlayerOnFoot(w, input, dt) {
  const p = w.player;
  if (p.stun > 0) { p.stun -= dt; return; }
  let mx = input.moveX, my = input.moveY;
  const mag = Math.min(1, Math.hypot(mx, my));
  if (mag > 0.05) {
    const run = input.sprint || mag > 0.92;
    const speed = (run ? PLAYER.run : PLAYER.walk) * (input.sprint ? 1 : mag);
    const nx = mx / (Math.hypot(mx, my) || 1), ny = my / (Math.hypot(mx, my) || 1);
    p.x += nx * speed * dt; p.y += ny * speed * dt;
    p.angle = Math.atan2(ny, nx);
    p.step += speed * dt;
  }
  pushCircleOutOfWorld(w, p, PLAYER.radius);
  p.x = clamp(p.x, 8, w.city.width - 8); p.y = clamp(p.y, 8, w.city.height - 8);
}

function applyDriverInput(car, input) {
  car.controls.throttle = input.throttle;
  car.controls.brake = input.brake;
  car.controls.steer = input.steer;
  car.controls.handbrake = input.handbrake;
  car.horn = input.horn;
}

// Ein fester Simulationsschritt. input: siehe input.js (abstrakte Aktionen).
export function updateWorld(w, input, dt) {
  w.events.length = 0;
  if (w.pendingSave && !resolveSave(w)) { w.loading = true; return; }
  if (!streamWorld(w)) return;
  w.time += dt;
  if (w.notice && (w.notice.t -= dt) <= 0) w.notice = null;
  const m = w.mission;

  // Briefing/Ergebnis frieren die Welt ein; nur die Mission reagiert auf Eingaben.
  if (m.state === 'briefing' || m.state === 'success' || m.state === 'failed') {
    w.events.push(...updateMission(m, missionCtx(w, input), dt));
    return;
  }

  const p = w.player;
  if (input.enterExit) { if (p.inCar) tryExit(w); else tryEnter(w); }

  const pc = playerCar(w);
  if (pc) {
    if (pc.wrecked) applyDriverInput(pc, { throttle: 0, brake: 0, steer: 0, handbrake: false, horn: false });
    else applyDriverInput(pc, input);
    if (pc.horn && !pc._hornWas) w.events.push({ type: 'horn', x: pc.x, y: pc.y });
    pc._hornWas = pc.horn;
  } else updatePlayerOnFoot(w, input, dt);

  for (const c of w.cars) if (c.driver === 'npc') driveAi(c, w, dt);
  for (const c of w.cars) {
    if (c.driver === null && !c.wrecked && c !== pc) { c.controls.throttle = 0; c.controls.brake = 0; c.controls.steer = 0; c.controls.handbrake = true; }
    // Unberührte geparkte Autos schlafen (spart die Weltkollision für hunderte Autos).
    if (c.role === 'curb' && c.driver === null && !c.wrecked && Math.abs(c.vx) + Math.abs(c.vy) < 2 && Math.abs(c.angVel) < 0.01) { c.vx = c.vy = c.angVel = 0; continue; }
    stepCar(c, dt, w.city);
    collideCarWorld(c, w, w.events);
  }
  for (let i = 0; i < w.cars.length; i++) for (let j = i + 1; j < w.cars.length; j++) {
    const a = w.cars[i], b = w.cars[j];
    if (Math.abs(a.x - b.x) < 60 && Math.abs(a.y - b.y) < 60) collideCars(a, b, w.events);
  }

  if (pc) { p.x = pc.x; p.y = pc.y; p.angle = pc.angle; }

  // Wracks: KI-Fahrer steigt aus und flieht, Wrack verschwindet später außer Sicht.
  for (const c of w.cars) {
    if (!c.wrecked) continue;
    c.wreckT += dt;
    if (c.driver === 'npc') {
      c.driver = null; c.ai = null;
      const ped = fleeingDriver(w, c, c.x, c.y, 3);
      if (ped) w.peds.push(ped);
    }
  }
  const cam = w.camera;
  w.cars = w.cars.filter((c) => !(c.wrecked && c.wreckT > 20 && c.id !== w.playerCarId && c.id !== p.inCar && !c.cargo
      && Math.hypot(c.x - cam.x, c.y - cam.y) > 900));

  // Spieler zu Fuß gegen Autos.
  if (!p.inCar) {
    for (const c of w.cars) {
      const mm = circleVsObb(p.x, p.y, PLAYER.radius, c);
      if (!mm) continue;
      p.x += mm.nx * mm.depth; p.y += mm.ny * mm.depth;
      if (speedOf(c) > 120 && p.stun <= 0) { p.stun = 0.8; w.events.push({ type: 'bump', x: p.x, y: p.y }); }
    }
  }

  // Passanten: Bedrohungen wahrnehmen, gegen Autos prüfen, bewegen.
  const threats = [];
  if (pc && !pc.wrecked && speedOf(pc) > 130) threats.push({ x: pc.x, y: pc.y, vx: pc.vx, vy: pc.vy, r: 90 });
  for (const e of w.events) {
    if (e.type === 'horn' && !e.npc) threats.push({ x: e.x, y: e.y, r: 170, always: true });
    if (e.type === 'crash' && e.strength > 0.25) threats.push({ x: e.x, y: e.y, r: 130, always: true });
  }
  for (const ped of w.peds) {
    if (ped.state !== 'down' && ped.state !== 'flee') {
      for (const t of threats) {
        const d = Math.hypot(ped.x - t.x, ped.y - t.y);
        if (d > t.r) continue;
        const toward = t.always || ((ped.x - t.x) * t.vx + (ped.y - t.y) * t.vy) > 0;
        if (toward) { scare(ped, t.x, t.y); break; }
      }
    }
    for (const c of w.cars) {
      if (ped.state === 'down') break;
      const mm = circleVsObb(ped.x, ped.y, PED.radius, c);
      if (!mm) continue;
      if (speedOf(c) > 55) {
        knockDown(ped, c.x, c.y);
        w.events.push({ type: 'hit', x: ped.x, y: ped.y });
        for (const o of w.peds) if (o !== ped && Math.hypot(o.x - ped.x, o.y - ped.y) < 110) scare(o, ped.x, ped.y);
      } else { ped.x += mm.nx * mm.depth; ped.y += mm.ny * mm.depth; }
    }
    updatePed(ped, w, dt);
  }
  // Überzählige (geflohene Fahrer) wieder abbauen, wenn außer Sicht.
  if (w.peds.length > w.pedTarget + 8) {
    const idx = w.peds.findIndex((q) => q.state === 'walk' && Math.hypot(q.x - cam.x, q.y - cam.y) > 900);
    if (idx >= 0) w.peds.splice(idx, 1);
  }
  managePopulation(w);
  manageParked(w);

  w.events.push(...updateMission(m, missionCtx(w, input), dt));
  if (m.state === 'success') {
    w.money += m.result.reward;
    w.completed += 1;
    if (w.bestTime === null || m.result.time < w.bestTime) { w.bestTime = m.result.time; m.result.newBest = true; }
  }

  updateCamera(w, dt);
}

function missionCtx(w, input) {
  return { places: w.city.places, player: w.player, cars: w.cars, timeLimit: w.city.timeLimit, input };
}

export function updateCamera(w, dt) {
  const cam = w.camera, p = w.player, car = playerCar(w);
  let tx = p.x, ty = p.y, zoom = 1;
  if (car) {
    tx = car.x + car.vx * 0.45; ty = car.y + car.vy * 0.45;
    zoom = 1 - clamp(speedOf(car) / 330, 0, 1) * 0.28;
  }
  cam.x = damp(cam.x, tx, 5, dt);
  cam.y = damp(cam.y, ty, 5, dt);
  cam.zoom = damp(cam.zoom, zoom, 2, dt);
}

export { forwardSpeed, speedOf, damage, obbVsRect, obbVsObb };
