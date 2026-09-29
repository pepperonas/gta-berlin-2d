// Vom Spieler geführter Zug: verlässt den Fahrplan (das virtuelle Fahrzeug wird entfernt) und fährt mit trainphysics.js
// entlang seiner Linie. Züge auf derselben Strecke voraus begrenzen den Weg (Zwangsbremsung), Fahrplan-Züge desselben
// Musters dahinter warten (transitlive.js blocked). Türen, Haltestellen, Trinkgeld, Wenden am Linienende.
import { positionAt, pointOn, trainCars, TRAIN } from './transit.js';
import { createDrive, stepDrive, stopInfo, tipFor, maxDecel } from './trainphysics.js';
import { TRAIN_DRIVE } from './config.js';
import { hash01 } from './map.js';
import { RIDE, vehicleState, alightSpot, stationExit, elevated } from './ride.js';
import { obstacleAt } from './transitlive.js';

const trainLen = (mode) => { const k = TRAIN[mode]; return k.cars * k.carL + (k.cars - 1) * k.gap; };
const SAFE = 80; // px Abstand zum Zug voraus

export function takeTrain(w, hit) {
  if (!hit || hit.ref.carId !== undefined || hit.car !== 0 || hit.front > RIDE.cab) return false;
  if (hit.ref.playerTrain) return retakeTrain(w);
  const st = vehicleState(w, hit.ref);
  if (!st || st.mode === 'bus') return false;
  const s = w.transit.tracked.get(hit.ref.pid), v = s.veh.find((x) => x.key === hit.ref.key);
  v.gone = true; // aus dem Fahrplan
  const pos = positionAt(st.p, v.tau);
  w.playerTrain = { pid: st.p.id, s: st.s, v: st.speed, drive: createDrive(st.mode, st.speed), nextStop: pos.stop, served: [], atStop: null, leftT: null, passengers: 20 + Math.floor(hash01(st.p.id * 31 + Math.floor(w.clock)) * 60) };
  w.player.ride = { kind: 'driver', ref: { playerTrain: true }, mode: st.mode, car: 0, lastStop: { ...pointOn(st.p, st.p.stops[Math.max(0, pos.stop - 1)]), name: st.p.stopNames[Math.max(0, pos.stop - 1)], i: Math.max(0, pos.stop - 1), pid: st.p.id }, since: w.time, line: st.p.name, dest: st.p.stopNames[st.p.stopNames.length - 1] };
  w.events.push({ type: 'train-take', mode: st.mode, line: st.p.name, x: w.player.x, y: w.player.y });
  return true;
}

// Den eigenen, stehengelassenen Zug wieder übernehmen (Führerstand vorn)
function retakeTrain(w) {
  const t = w.playerTrain;
  if (!t || w.player.ride) return false;
  const p = w.city.transit.patterns[t.pid], i = t.atStop?.i ?? Math.max(0, t.nextStop - 1), q = pointOn(p, p.stops[i]);
  w.player.ride = { kind: 'driver', ref: { playerTrain: true }, mode: p.mode, car: 0, lastStop: { x: q.x, y: q.y, name: p.stopNames[i], i, pid: p.id }, since: w.time, line: p.name, dest: p.stopNames[p.stopNames.length - 1] };
  t.leftT = null;
  w.events.push({ type: 'train-take', mode: p.mode, line: p.name, x: w.player.x, y: w.player.y });
  return true;
}

// Freier Weg bis zum Heck des nächsten Zugs voraus auf derselben Strecke (gleiches Muster oder Muster, deren Weg hier
// auf ≤ 30 px mit dem eigenen übereinstimmt), minus Sicherheitsabstand
export function trainAhead(w, t) {
  const tr = w.city.transit, p = tr.patterns[t.pid];
  let free = Infinity;
  const look = 2500;
  const probe = pointOn(p, t.s + 200);
  for (const [pid, s] of w.transit?.tracked ?? []) {
    const q = tr.patterns[pid];
    if (q.mode === 'bus' || q.mode !== p.mode) continue;
    for (const v of s.veh) {
      if (v.gone) continue;
      const qs = positionAt(q, v.tau).s, head = pointOn(q, qs);
      if (Math.abs(head.x - probe.x) > look || Math.abs(head.y - probe.y) > look) continue;
      // Heck des anderen Zugs auf den eigenen Weg projizieren: nur gleiche Strecke (Abstand ≤ 30 px) zählt
      const tailS = qs - trainLen(q.mode);
      const tail = pointOn(q, tailS);
      for (let d = 0; d <= look; d += 20) {
        const m = pointOn(p, t.s + d);
        if (Math.hypot(m.x - tail.x, m.y - tail.y) <= 30) { free = Math.min(free, Math.max(0, d - SAFE)); break; }
      }
    }
  }
  return free;
}

export function updatePlayerTrain(w, input, dt) {
  const t = w.playerTrain;
  if (!t) return;
  const tr = w.city.transit, p = tr.patterns[t.pid], k = TRAIN_DRIVE, driving = w.player.ride?.kind === 'driver';
  const endFree = Math.max(0, p.stops[p.stops.length - 1] - t.s);
  const aheadFree = trainAhead(w, t);
  const limit = Math.min(endFree, aheadFree, p.mode === 'tram' ? tramFree(w, p, t.s) : Infinity);
  const inp = driving ? { throttle: input.throttle, brake: input.brake, emergency: !!input.handbrake, limit } : { brake: 1, limit };
  const wasBlocked = t.blocked;
  stepDrive(t.drive, inp, dt);
  t.blocked = aheadFree < 400 && t.drive.v < 5;
  if (t.blocked && !wasBlocked) w.events.push({ type: 'train-blocked' });
  t.v = t.drive.v; t.s += t.v * dt;
  while (t.nextStop < p.stops.length - 1 && t.s > p.stops[t.nextStop] + k.stopZone) t.nextStop++;
  t.atStop = t.drive.stopped ? stopInfo(p, t.s) : null;
  // Türen (E/A): nur im Stand an einer Haltestelle
  if (driving && input.action && !atTerminus(w)) {
    if (t.drive.doors === 'closed' && t.atStop) {
      t.drive.doors = 'open'; t.drive.doorT = 0;
      const tip = t.served.includes(t.atStop.i) ? 0 : tipFor(t.atStop.dist, maxDecel(t.drive));
      t.served.push(t.atStop.i);
      const out = Math.floor(hash01(t.pid * 97 + t.atStop.i + Math.floor(w.clock / 10)) * 12), inn = Math.floor(hash01(t.pid * 53 + t.atStop.i * 7 + Math.floor(w.clock / 10)) * 14);
      t.passengers = Math.max(0, t.passengers - out) + inn;
      w.events.push({ type: 'doors-open', out, inn });
      if (tip > 0) { w.money += Math.round(tip); w.events.push({ type: 'tip', amount: Math.round(tip) }); }
      const q = pointOn(p, p.stops[t.atStop.i]);
      w.player.ride.lastStop = { x: q.x, y: q.y, name: p.stopNames[t.atStop.i], i: t.atStop.i, pid: p.id };
    } else if (t.drive.doors === 'open') { t.drive.doors = 'closed'; w.events.push({ type: 'doors-close' }); }
  }
  if (t.drive.doors === 'open' && t.drive.doorT > k.doorsAuto) { t.drive.doors = 'closed'; w.events.push({ type: 'doors-close' }); }
  // ohne Fahrer: nach 30 s außer Sicht entfernen
  if (!driving) {
    t.leftT = (t.leftT ?? 0) + dt;
    const h = pointOn(p, t.s), cam = w.camera;
    if (t.leftT > 30 && (Math.abs(h.x - cam.x) > 1400 || Math.abs(h.y - cam.y) > 900)) w.playerTrain = null;
  }
}

export function leaveTrain(w) {
  const t = w.playerTrain, st = vehicleState(w, { playerTrain: true });
  if (!t || !st) return false;
  if (st.underground || elevated(w, st, st.cars[0])) { // Tunnel/Hochbahn: nur am Bahnsteig, Ausgang an der Straße
    if (!t.atStop || t.drive.v > 0) { w.notice = { text: st.underground ? 'Nur am Bahnsteig' : 'Aussteigen nur am Bahnhof', t: 1.5 }; return false; }
    const ex = stationExit(w, st.p, t.atStop.i);
    w.player.ride = null; w.player.x = ex.x; w.player.y = ex.y; w.player.lvl = 0;
  } else {
    const spot = alightSpot(w, st, 0);
    if (!spot) { w.notice = { text: 'Kein Platz zum Aussteigen', t: 1.5 }; return false; }
    w.player.ride = null; w.player.x = spot.x; w.player.y = spot.y;
    if (t.v > RIDE.hopOff) w.player.stun = RIDE.stun;
  }
  t.leftT = 0;
  w.events.push({ type: 'alight', hop: t.v > RIDE.hopOff, x: w.player.x, y: w.player.y });
  return true;
}

// Am Endhalt in die Gegenrichtung: Muster derselben Linie, dessen erster Halt ≤ 60 m vom eigenen Endhalt liegt
export function turnAround(w) {
  const t = w.playerTrain;
  if (!t || t.drive.v > 0) return false;
  const tr = w.city.transit, p = tr.patterns[t.pid], n = p.stops.length;
  if (t.s < p.stops[n - 1] - k_zone()) return false;
  const end = pointOn(p, p.stops[n - 1]);
  let best = null, bd = 600;
  for (const q of tr.patterns) {
    if (q.name !== p.name || q.id === p.id || q.mode !== p.mode) continue;
    const a = pointOn(q, q.stops[0]), d = Math.hypot(a.x - end.x, a.y - end.y);
    if (d < bd) { bd = d; best = q; }
  }
  if (!best) { w.notice = { text: 'Hier kann nicht gewendet werden', t: 2 }; return false; }
  Object.assign(t, { pid: best.id, s: best.stops[0], nextStop: 1, served: [0], atStop: { i: 0, dist: 0 } });
  t.drive.v = 0; t.v = 0;
  w.player.ride.line = best.name; w.player.ride.dest = best.stopNames[best.stopNames.length - 1];
  w.events.push({ type: 'turnaround', line: best.name });
  return true;
}
const k_zone = () => TRAIN_DRIVE.stopZone;

// Am Endhalt, Zug steht, Türen zu und der Halt ist bedient: dann heißt E/A „Wenden“ statt „Türen“
export function atTerminus(w) {
  const t = w.playerTrain;
  if (!t) return false;
  const p = w.city.transit.patterns[t.pid], last = p.stops.length - 1;
  return t.drive.v === 0 && t.drive.doors === 'closed' && Math.abs(t.s - p.stops[last]) <= TRAIN_DRIVE.stopZone && t.served.includes(last);
}

// Freier Weg der eigenen Straßenbahn bis zum ersten Hindernis auf dem Gleis (px), Infinity ohne Hindernis.
// obstacleAt meldet alles, dessen Mitte bis zu ~24 px + 0,4 × halbe Länge vom Prüfpunkt liegt – die nahe Kante kann
// also schon an der Spitze sein. Mit weniger Abstand schöbe der Zug das Hindernis (collideRail) im Kriechgang vor sich
// her, statt davor zu stehen.
const OBSTACLE_MARGIN = 40;
export function tramFree(w, p, s) {
  for (const d of [20, 45, 75, 110, 150, 200]) { const q = pointOn(p, s + d); if (obstacleAt(w, q.x, q.y)) return Math.max(0, d - OBSTACLE_MARGIN); }
  return Infinity;
}
