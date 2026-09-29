// Zug führen (rein): nur Tempo entlang der Linie. Anfahren mit abnehmender Zugkraft über 40 % der Höchstgeschwindigkeit,
// Bremse und Notbremse konstant, Ausrollen langsam; vor einem Hindernis/Endhalt (limit = freier Weg in px) bremst der Zug
// selbsttätig so, dass er davor steht. Bei offenen Türen steht er. Nie rückwärts.
import { TRAIN_DRIVE } from './config.js';

export const brakeDistance = (v, a) => (v * v) / (2 * a);

export function createDrive(mode, v = 0) { return { mode, v, doors: 'closed', doorT: 0, decel: [], stopped: v <= 0 }; }

export function stepDrive(d, input, dt) {
  if (dt <= 0) return d; // kein Zeitschritt: nichts zu integrieren (sonst Division durch 0 im Bremsprotokoll)
  const k = TRAIN_DRIVE[d.mode], v0 = d.v;
  let a;
  if (d.doors !== 'closed') { d.v = 0; d.doorT += dt; return d; }
  if (input.emergency) a = -k.emergency;
  else if (input.brake > 0) a = -k.brake * input.brake;
  else if (input.throttle > 0) a = k.acc * input.throttle * (d.v < k.vmax * 0.4 ? 1 : Math.max(0, (k.vmax - d.v) / (k.vmax * 0.6)));
  else a = -TRAIN_DRIVE.roll;
  let v = Math.max(0, Math.min(k.vmax, d.v + a * dt));
  // Zwangsbremsung: nie weiter als limit (Hindernis voraus, Endhalt)
  const lim = input.limit ?? Infinity;
  if (lim < Infinity) {
    const vAllowed = Math.sqrt(Math.max(0, 2 * k.emergency * Math.max(0, lim - v * dt)));
    if (v > vAllowed) v = Math.max(0, Math.min(v, vAllowed));
    if (lim <= 1) v = 0;
  }
  d.v = v;
  // Auf physikalisch Erreichbares gedeckelt: der schließende „lim<=1 → v=0"-Frame kann in einem Schritt einen
  // Rest-v auf 0 zwingen, was rechnerisch weit über der Notbremsverzögerung läge – ohne Deckel würde maxDecel()
  // das für 8 s als harte Bremsung werten, obwohl der Zug sauber im Rahmen der Notbremsung ausgerollt ist.
  const decel = Math.min(k.emergency, Math.max(0, (v0 - v) / dt));
  d.decel.push([dt, decel]);
  let tsum = 0; for (let i = d.decel.length - 1; i >= 0; i--) { tsum += d.decel[i][0]; if (tsum > 8) { d.decel.splice(0, i); break; } }
  d.stopped = v < TRAIN_DRIVE.stillV;
  return d;
}

export const maxDecel = (d) => d.decel.reduce((m, [, x]) => Math.max(m, x), 0);

export function stopInfo(p, s) {
  let best = null;
  p.stops.forEach((st, i) => { const dist = s - st; if (Math.abs(dist) <= TRAIN_DRIVE.stopZone && (!best || Math.abs(dist) <= Math.abs(best.dist))) best = { i, dist }; }); // Gleichstand: der spätere (doppelter Endhalt im Fahrplan)
  return best;
}

export function tipFor(dist, maxDec) {
  const z = TRAIN_DRIVE.stopZone, e = TRAIN_DRIVE.stopExact;
  const place = Math.abs(dist) <= e ? 1 : Math.max(0, 1 - (Math.abs(dist) - e) / (z - e));
  const gentle = maxDec <= TRAIN_DRIVE.gentle ? 1 : Math.max(0, 1 - (maxDec - TRAIN_DRIVE.gentle) / TRAIN_DRIVE.gentle);
  return Math.round(TRAIN_DRIVE.tipMax * place * gentle * 100) / 100;
}
