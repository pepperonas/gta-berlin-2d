// Gangbild (rein, nur Darstellung): aus der zurückgelegten Strecke (p.step), der Blickrichtung und der Zeit eine Pose.
// Der Zustand je Figur (a = p._anim) lebt nur fürs Zeichnen – die Simulation liest ihn nie.
//
// Takt: Schritte je Sekunde steigen mit dem Tempo und sind gedeckelt (Menschen machen längere, nicht beliebig schnellere
// Schritte). Ausschlag blendet beim Anlaufen/Anhalten weich ein und aus, Rennen blendet in die Laufpose über
// (Vorlage, angewinkelte Arme, lange Schritte, Flugphase). Im Stand: Atmen und Gewichtsverlagerung.

export const GAIT = {
  still: 4,          // px/s: darunter steht man
  runFrom: 95,       // px/s: ab hier Laufpose (Spieler rennt 155, geht 80; Jogger ~85–100)
  cadence0: 1.05,    // Doppelschritte/s bei langsamem Gehen
  cadenceK: 1 / 70,  // mehr je px/s
  cadenceMax: 2.9,
  ampRate: 7,        // 1/s: wie schnell der Ausschlag folgt
  turnRate: 16,      // rad/s: gezeichnete Drehung höchstens so schnell
  maxDt: 0.1,
};

export function createAnim(p) { const f = p.facing ?? p.angle ?? 0; return { t: null, step: p.step ?? 0, v: 0, phase: 0, amp: 0, run: 0, face: f, move: p.angle ?? f }; }

const wrap = (a) => Math.atan2(Math.sin(a), Math.cos(a));

// Zustand nachführen (Zeit t in s). facing = Blick/Oberkörper, move = Laufrichtung (Beine; beim Spieler getrennt,
// weil er zielt, während er läuft). Liefert a.
export function stepAnim(a, p, t, facing, move = facing) {
  if (a.t === null || t < a.t) { a.t = t; a.step = p.step ?? 0; a.face = facing; a.move = move; return a; }
  const dt = Math.min(GAIT.maxDt, t - a.t);
  a.t = t;
  if (dt <= 0) return a;
  const ds = Math.abs((p.step ?? 0) - a.step);
  a.step = p.step ?? 0;
  const v = Math.min(400, ds / dt);
  a.v += (v - a.v) * Math.min(1, dt * 10);
  const moving = a.v > GAIT.still;
  const cad = moving ? Math.min(GAIT.cadenceMax, GAIT.cadence0 + a.v * GAIT.cadenceK) : 0;
  a.phase = (a.phase + 2 * Math.PI * cad * dt) % (2 * Math.PI * 64);
  const ampT = moving ? Math.min(1, 0.35 + a.v / 60) : 0;
  a.amp += (ampT - a.amp) * Math.min(1, dt * GAIT.ampRate);
  const runT = a.v > GAIT.runFrom ? 1 : 0;
  a.run += (runT - a.run) * Math.min(1, dt * 6);
  // Beim Anhalten schrumpft der Ausschlag gegen 0: die Füße kommen nebeneinander, statt mitten im Schritt einzufrieren
  const mx = GAIT.turnRate * dt, turn = (from, to) => wrap(from + Math.max(-mx, Math.min(mx, wrap(to - from))));
  a.face = turn(a.face, facing);
  if (moving) a.move = turn(a.move ?? facing, move);
  return a;
}

// Pose aus dem Zustand. style: { stoop (0…1, gebeugt), cane, stroller, carry } verändern Schrittlänge und Arme.
// Ergebnis in lokalen Koordinaten (+x vorn, +y rechts), Einheiten px bei Figurgröße 1:
//  footL/footR: { x, lift } (Fuß vor/zurück, Anheben 0…1), handL/handR: { x, y }, twist (rad, Schultern gegen Hüfte),
//  bob (Skalierung Oberkörper), lean (px nach vorn), sway (px seitlich, Gewichtsverlagerung), breath
export function gaitPose(a, time = 0, style = {}, seed = 0) {
  const amp = a.amp, run = a.run, ph = a.phase, s = Math.sin(ph), c = Math.cos(ph);
  const stoop = style.stoop ?? 0;
  const stride = amp * (4.6 + run * 3) * (1 - stoop * 0.35) * (style.stroller ? 0.8 : 1);
  // Fuß vorn/hinten gegengleich; angehoben wird der Fuß in der Schwungphase (vorwärts), beim Rennen beide kurz (Flug)
  const liftL = Math.max(0, c) * amp * (0.6 + run * 0.4), liftR = Math.max(0, -c) * amp * (0.6 + run * 0.4);
  const footL = { x: s * stride, lift: liftL }, footR = { x: -s * stride, lift: liftR };
  // Arme: gegengleich zu den Beinen; beim Rennen weiter und angewinkelt (Hände näher am Körper)
  const armSwing = amp * (3.2 + run * 2.2) * (style.carry ? 0.45 : 1);
  const inward = run * 1.2;
  let handL = { x: -s * armSwing + run * 1.2, y: -(5.2 - inward) }, handR = { x: s * armSwing + run * 1.2, y: 5.2 - inward };
  if (style.stroller) { handL = { x: 6.2, y: -2.6 }; handR = { x: 6.2, y: 2.6 }; }
  else if (style.cane) handR = { x: 3.2 + s * amp * 1.4, y: 5.4 };
  const idle = 1 - Math.min(1, amp * 3);
  const breath = Math.sin(time * 1.7 + seed) * 0.025 * idle;
  const sway = Math.sin(time * 0.45 + seed * 1.3) * 0.5 * idle + s * amp * 0.35;
  return {
    footL, footR, handL, handR,
    twist: s * amp * (0.1 + run * 0.08),
    bob: 1 + Math.abs(c) * amp * (0.025 + run * 0.035),
    lean: run * 1.4 + stoop * 1.6,
    sway, breath,
  };
}

// Beine relativ zum Oberkörper: Drehwinkel der Hüfte und ob rückwärts gegangen wird (Laufrichtung mehr als 100° von der
// Blickrichtung weg). Die Hüfte dreht höchstens ±80°, darüber wird der Schritt rückwärts gesetzt.
export function legFrame(a) {
  let d = wrap((a.move ?? a.face) - a.face), back = false;
  if (Math.abs(d) > Math.PI * 0.56) { back = true; d = wrap(d - Math.PI); }
  return { rot: Math.max(-1.4, Math.min(1.4, d)), back };
}
