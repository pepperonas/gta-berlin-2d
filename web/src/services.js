// Fahrzeuge mit Aufgabe: Paketwagen und Müllauto halten unterwegs (fleet.js), Blaulichtfahrzeuge fahren Einsätze.
//   Tote auf der Straße → Rettungswagen mit Martinshorn, der am Einsatzort hält und den Toten mitnimmt.
//   Schüsse → Streifenwagen zum Tatort, hält dort mit Blaulicht. Dazu fährt ab und zu ein Einsatz einfach vorbei.
// Einsatzfahrzeuge entstehen außer Sicht und fahren über den Spurgraph zum Ziel (traffic.js setGoal); über Rot fahren
// sie langsam. Nur in der Welt mit Tagesrhythmus (Standardbevölkerung) – Tests mit fester Bevölkerung bleiben ruhig.
import { createCar } from './car.js';
import { placeOnLane, spawnSpot, setGoal, currentSeg, dropClaims, claimNarrow, narrowFree } from './traffic.js';
import { KINDS, nextStopAfter, stopDuration, mayStopOn, EMERGENCY } from './fleet.js';
import { LIFE } from './life.js';

export const EMERG = {
  ambulanceDelay: 12, policeDelay: 9,      // s bis zur Alarmierung
  arrive: 110,                             // px: am Einsatzort
  sceneAmbulance: 14, scenePolice: 18,     // s Halt am Einsatzort
  pickup: 150,                             // px: so weit nimmt der Rettungswagen Tote mit
  giveUp: 150,                             // s ohne Ankunft: Einsatz abbrechen
  policeCooldown: 45,                      // s: nicht bei jedem Schuss ein neuer Streifenwagen
  maxEach: 2,
  passMin: 150, passMax: 330,              // s zwischen vorbeifahrenden Einsätzen
};

const outOfView = (w, x, y, pad = 80) => Math.abs(x - w.camera.x) > LIFE.viewHalfX + pad || Math.abs(y - w.camera.y) > LIFE.viewHalfY + pad;

// Arbeitshalte von Paketwagen und Müllauto (jeden Schritt für KI-Fahrzeuge dieser Arten)
export function updateService(w, c, dt) {
  const ai = c.ai, kind = c.kind;
  if (!ai || (kind !== 'delivery' && kind !== 'garbage')) return;
  if (!(ai.hold > 0)) { c.hazard = false; c.work = false; }
  const v = Math.hypot(c.vx, c.vy);
  ai.odo = (ai.odo ?? 0) + v * dt;
  ai.nextStop ??= nextStopAfter(kind, w.rng());
  if (ai.hold > 0 || ai.odo < ai.nextStop || v > 160) return;
  const lane = ai.segs?.[currentSeg(ai)]?.lane, p = lane?.pts;
  if (!p) return;
  const toEnd = Math.hypot(p[p.length - 2] - c.x, p[p.length - 1] - c.y), fromStart = Math.hypot(p[0] - c.x, p[1] - c.y);
  if (!mayStopOn(kind, lane, toEnd, fromStart)) return;
  ai.hold = stopDuration(kind, w.rng());
  ai.odo = 0; ai.nextStop = nextStopAfter(kind, w.rng());
  if (kind === 'delivery') c.hazard = true; else c.work = true;
}

// Einsatzfahrzeug außer Sicht auf eine Spur setzen und zum Ziel schicken
function dispatch(w, kind, tx, ty, near = { x: tx, y: ty }, minR = 1000, maxR = 2000) {
  for (let t = 0; t < 14; t++) {
    const sp = spawnSpot(w.city, w.rng, near.x, near.y, minR, maxR);
    if (!sp) return null;
    if (!outOfView(w, sp.x, sp.y) || !narrowFree(w, sp.lane) || !w.cars.every((o) => Math.hypot(o.x - sp.x, o.y - sp.y) > 90)) continue;
    const car = createCar({ x: sp.x, y: sp.y, kind, color: KINDS[kind].colors[0] });
    placeOnLane(car, w.city, sp.lane, sp.s, w.rng);
    car.driver = 'npc';
    if (!setGoal(car, w.city, tx, ty)) continue;
    claimNarrow(w, car, sp.lane);
    Object.assign(car.ai, { urgent: true, cruiseK: 1.3 });
    car.siren = true;
    w.cars.push(car);
    return car;
  }
  return null;
}

// Einsätze verwalten: Alarmierung, Anfahrt, Einsatzort, Abfahrt, Abbau
export function manageEmergency(w, dt) {
  if (!w.rhythm) return;
  const E = (w.emerg ??= { incidents: [], passT: EMERG.passMin + w.rng() * (EMERG.passMax - EMERG.passMin), lastPolice: -1e9 });
  // neue Einsätze aus dem Geschehen
  for (const p of w.peds) if (p.state === 'dead' && !p.reported) { p.reported = true; E.incidents.push({ kind: 'ambulance', x: p.x, y: p.y, t: EMERG.ambulanceDelay }); }
  for (const ev of w.events) if (ev.type === 'shot' && !w.player.inside && w.time - E.lastPolice > EMERG.policeCooldown) { // unter Tage: keine Polizei oben
    E.lastPolice = w.time; E.incidents.push({ kind: 'police', x: ev.x ?? w.player.x, y: ev.y ?? w.player.y, t: EMERG.policeDelay });
  }
  // Tote in der Nähe eines schon laufenden Rettungseinsatzes zusammenfassen
  E.incidents = E.incidents.filter((a, i) => !(a.kind === 'ambulance' && !a.car && E.incidents.some((b, j) => j < i && b.kind === 'ambulance' && Math.hypot(a.x - b.x, a.y - b.y) < EMERG.pickup)));
  const active = (k) => w.cars.filter((c) => c.kind === k && c.duty && !c.done).length;
  for (const inc of E.incidents) {
    if (inc.car) continue;
    if ((inc.t -= dt) > 0) continue;
    if (active(inc.kind) >= EMERG.maxEach) { inc.t = 3; continue; }
    const car = dispatch(w, inc.kind, inc.x, inc.y);
    if (!car) { inc.t = 1; continue; }
    inc.car = car; car.duty = { inc, since: w.time, phase: 'drive' };
  }
  // vorbeifahrende Einsätze (Stadtgeräusch): Start außer Sicht, Ziel gegenüber der Kamera
  if ((E.passT -= dt) <= 0) {
    E.passT = EMERG.passMin + w.rng() * (EMERG.passMax - EMERG.passMin);
    const kind = w.rng() < 0.5 ? 'police' : 'ambulance', a = w.rng() * Math.PI * 2, cam = w.camera;
    const sx = cam.x + Math.cos(a) * 1000, sy = cam.y + Math.sin(a) * 1000;
    const car = dispatch(w, kind, 2 * cam.x - sx, 2 * cam.y - sy, { x: sx, y: sy }, 0, 500);
    if (car) car.duty = { pass: true, since: w.time, phase: 'drive', x: 2 * cam.x - sx, y: 2 * cam.y - sy };
  }
  for (const c of w.cars) {
    const d = c.duty;
    if (!d || c.done || !EMERGENCY.has(c.kind)) continue; // Busse haben eigene Dienste (transitlive.js)
    if (c.wrecked || c.driver !== 'npc') { finish(w, c, false); continue; }
    const gx = d.inc?.x ?? d.x, gy = d.inc?.y ?? d.y, dist = Math.hypot(c.x - gx, c.y - gy);
    if (d.phase === 'drive') {
      if (d.pass ? dist < 250 : dist < EMERG.arrive) {
        if (d.pass) { finish(w, c, true); continue; }
        d.phase = 'scene'; c.siren = false; c.blue = true;
        c.ai.hold = c.kind === 'ambulance' ? EMERG.sceneAmbulance : EMERG.scenePolice;
      } else if (w.time - d.since > EMERG.giveUp) finish(w, c, false);
    } else if (d.phase === 'scene' && !(c.ai.hold > 0)) {
      if (c.kind === 'ambulance') { // Tote mitnehmen, mit Sondersignal ins Krankenhaus
        w.peds = w.peds.filter((p) => !(p.state === 'dead' && Math.hypot(p.x - gx, p.y - gy) < EMERG.pickup));
        w.events.push({ type: 'pickup-body', x: gx, y: gy });
      }
      finish(w, c, c.kind === 'ambulance');
    }
  }
  E.incidents = E.incidents.filter((i) => !i.car || !i.car.done);
  // fertige Einsatzfahrzeuge außer Sicht abbauen
  w.cars = w.cars.filter((c) => {
    const gone = c.done && EMERGENCY.has(c.kind) && c.driver === 'npc' && outOfView(w, c.x, c.y, 200);
    if (gone) dropClaims(c, w);
    return !gone;
  });
}

function finish(w, c, sirenOn) {
  c.done = true; c.blue = false; c.siren = sirenOn && c.driver === 'npc';
  if (c.ai) { c.ai.field = null; c.ai.urgent = sirenOn; c.ai.cruiseK = sirenOn ? 1.25 : 1; }
}
