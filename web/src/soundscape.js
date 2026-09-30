// Fahrzeug- und Schrittklang (rein rechnerisch, testbar): Drehzahl mit Gängen je Fahrzeugart, Reifen auf Belag/Nässe/
// Schnee, Quietschen beim Rutschen, Fahrtwind, die nächsten fremden Autos als eigene Stimmen (Entfernung, Richtung,
// Dopplereffekt) und Schritte zu Fuß. audio.js macht daraus Klang; hier wird nichts gezeichnet und nichts gewürfelt.
import { speedOf } from './car.js';
import { surfaceAt, T } from './map.js';
import { carModel } from './carmodels.js';

// Motoren: Leerlauf/Abregeldrehzahl, Zylinder (Zündfrequenz = U/min / 60 · Zylinder / 2), Gänge als Geschwindigkeit
// (px/s) je 1000 U/min, Diesel nagelt. 10 px = 1 m, CAR.maxSpeed 330 px/s ≈ 119 km/h.
export const ENGINES = {
  car: { idle: 800, red: 6400, cyl: 4, gears: [14, 24, 35, 46, 57, 68], diesel: false },
  police: { idle: 750, red: 6200, cyl: 6, gears: [16, 27, 39, 51, 63, 76], diesel: false },
  ambulance: { idle: 700, red: 4200, cyl: 4, gears: [18, 30, 44, 58, 72, 84], diesel: true },
  delivery: { idle: 750, red: 4300, cyl: 4, gears: [17, 29, 43, 57, 70, 82], diesel: true },
  truck: { idle: 600, red: 2600, cyl: 6, gears: [18, 30, 45, 62, 80, 100, 120], diesel: true },
  garbage: { idle: 600, red: 2400, cyl: 6, gears: [16, 27, 40, 55, 72, 90], diesel: true },
  bus: { idle: 600, red: 2500, cyl: 6, gears: [22, 38, 56, 78, 100, 122], diesel: true },
};
// Motoren der Pkw-Modelle (carmodels.js), hörbar im selbst gefahrenen Auto: Drei-/Vier-/Sechs-/Achtzylinder, Diesel,
// Zweitakter (zündet jede Umdrehung: strokes 2), Elektromotor (kein Zünden, nur Summen: electric, ein Gang)
export const MODEL_ENGINES = {
  zweitakter: { idle: 950, red: 4500, cyl: 2, strokes: 2, gears: [17, 32, 48, 66], diesel: false },
  kleinwagen: { idle: 850, red: 6200, cyl: 3, gears: [14, 25, 38, 55, 77], diesel: false },
  kompakt: { idle: 800, red: 6500, cyl: 4, gears: [14, 24, 35, 47, 63, 90], diesel: false },
  limousine: { idle: 750, red: 6500, cyl: 6, gears: [15, 25, 37, 50, 64, 82, 105], diesel: false },
  taxi: { idle: 750, red: 4500, cyl: 4, gears: [18, 30, 45, 60, 78, 100, 130], diesel: true },
  kombi: { idle: 800, red: 6500, cyl: 5, gears: [15, 25, 37, 50, 66, 99], diesel: false },
  transporter: { idle: 750, red: 4300, cyl: 4, gears: [17, 29, 43, 57, 75, 104], diesel: true },
  gelaende: { idle: 700, red: 4600, cyl: 6, gears: [16, 27, 40, 54, 70, 90, 115], diesel: true },
  elektro: { idle: 0, red: 16000, cyl: 0, gears: [38], diesel: false, electric: true },
  sportwagen: { idle: 950, red: 8500, cyl: 8, gears: [14, 23, 33, 45, 58, 74, 98], diesel: false },
  heckcoupe: { idle: 900, red: 7800, cyl: 6, gears: [15, 25, 36, 49, 63, 80, 104], diesel: false },
};
export const engineOf = (kind) => ENGINES[kind] ?? ENGINES.car;
// Motor eines Autos: im selbst gefahrenen Pkw (mit Fahrdynamik) oder fest gewählten Modell nach Modell, sonst nach Art
export const engineFor = (car) => ((car.dyn || car.model) && MODEL_ENGINES[carModel(car)]) || engineOf(car.kind);
const clamp01 = (v) => Math.max(0, Math.min(1, v));
export const SOUND_SPEED = 3430; // px/s (343 m/s)

// Motorzustand eines Autos fortschreiben: st = { rpm, gear, shiftT } (wird verändert und zurückgegeben).
// Hochschalten kurz vor der Abregeldrehzahl (bei wenig Gas früher), zurück, wenn die Drehzahl unter ~40 % fällt;
// beim Schalten sackt die Drehzahl ab und die Last setzt kurz aus (shiftT > 0).
export function stepEngine(st, car, dt) {
  const E = engineFor(car), v = speedOf(car), ctl = car.controls ?? {}, thr = car.wrecked ? 0 : clamp01(ctl.throttle ?? 0);
  st.gear ??= 1; st.rpm ??= E.idle; st.shiftT = Math.max(0, (st.shiftT ?? 0) - dt);
  const fwd = (car.vx * Math.cos(car.angle) + car.vy * Math.sin(car.angle));
  const rev = fwd < -5;
  const up = E.red * (0.62 + 0.3 * thr), down = E.red * 0.38;
  const rpmIn = (g) => (v / E.gears[g - 1]) * 1000;
  if (!rev) {
    while (st.gear < E.gears.length && rpmIn(st.gear) > up) { st.gear++; st.shiftT = 0.18; }
    while (st.gear > 1 && rpmIn(st.gear) < down) { st.gear--; st.shiftT = 0.12; }
  } else st.gear = 1;
  // Kupplung schleift unten herum: im Ersten nie unter Leerlauf, mit Gas schon im Stand hochdrehen
  const wheel = rpmIn(st.gear) * (rev ? 1.3 : 1), slip = (car.spin ? 0.35 : 0) + (st.gear === 1 ? thr * 0.3 : 0);
  let target = E.electric ? wheel : Math.max(E.idle + thr * E.red * 0.12, wheel + slip * E.red * 0.5);
  if (car.wrecked) target = 0;
  target = Math.min(E.red, target);
  const k = st.shiftT > 0 ? 0.25 : 1; // beim Schalten nicht mitziehen
  st.rpm += (target - st.rpm) * Math.min(1, dt * (target > st.rpm ? 7 : 5) * k);
  st.load = st.shiftT > 0 ? 0 : thr;
  st.fire = (st.rpm / 60) * (E.cyl / (E.strokes === 2 ? 1 : 2)); // Zündfrequenz in Hz (Zweitakter: jede Umdrehung)
  st.norm = clamp01((st.rpm - E.idle) / (E.red - E.idle));
  st.diesel = E.diesel;
  st.electric = !!E.electric; st.twoStroke = E.strokes === 2;
  return st;
}

// Reifen und Fahrtwind: roll (Abrollgeräusch 0..1), cobble (Pflaster rumpelt), wet (Zischen auf nasser Straße),
// snow (Knirschen), skid (Quietschen: Querrutschen, Vollbremsung, durchdrehende Räder), wind (Fahrtwind ∝ v²)
export function tireState(world, car) {
  const v = speedOf(car), vn = clamp01(v / 330);
  const c = Math.cos(car.angle), s = Math.sin(car.angle), lat = Math.abs(-car.vx * s + car.vy * c);
  const surf = world.city ? surfaceAt(world.city, car.x, car.y, car.lvl ?? null) : T.ROAD;
  const snow = clamp01((world.snow ?? 0) * 1.4), wet = clamp01((world.wet ?? 0) - snow * 0.5);
  const brake = car.controls?.brake > 0.6 && v > 90 ? 0.6 : 0, hand = car.controls?.handbrake && v > 60 ? 0.8 : 0;
  // Auf Schnee und Nässe quietscht nichts, das Rutschen rauscht nur
  const grip = 1 - Math.max(snow, wet * 0.8);
  const skid = clamp01(Math.max((lat - 40) / 120, brake, hand, car.spin ? 0.7 : 0, car.dyn ? car.skid ?? 0 : 0)) * (v > 15 || car.spin ? 1 : 0); // Fahrdynamik: Schräglauf/Blockieren
  return {
    roll: vn, cobble: surf === T.COBBLE ? vn : 0, offroad: surf === T.GRASS ? vn : 0,
    wet: wet * clamp01(v / 120), snow: snow * clamp01(v / 60), skid: skid * grip, slide: skid * (1 - grip),
    wind: vn * vn, splash: (car.aqua ?? 0) > 0 ? 1 : 0,
  };
}

// Die nächsten fremden Fahrzeuge als Stimmen: [{ id, kind, gain, pan, rate, fire, diesel }] – lauteste zuerst.
// rate = Dopplerfaktor (> 1 kommt näher), fire = Zündfrequenz aus einer groben Drehzahl nach Tempo und Gang.
export function carVoices(world, listener, n = 4, R = 500, exclude = null) {
  const out = [];
  for (const c of world.cars) {
    if (c.id === exclude || c.wrecked || !c.driver) continue;
    const dx = c.x - listener.x, dy = c.y - listener.y, d = Math.hypot(dx, dy);
    if (d >= R) continue;
    const E = engineFor(c), v = speedOf(c);
    const gear = E.gears.findIndex((g) => v / g * 1000 < E.red * 0.7);
    const rpm = Math.max(E.idle, v / E.gears[gear < 0 ? E.gears.length - 1 : gear] * 1000 + (c.controls?.throttle ?? 0) * 400);
    // Relativgeschwindigkeit entlang der Sichtlinie (positiv = nähert sich)
    const lvx = listener.vx ?? 0, lvy = listener.vy ?? 0, ux = dx / (d || 1), uy = dy / (d || 1);
    const vr = -((c.vx - lvx) * ux + (c.vy - lvy) * uy);
    const big = E.diesel && (c.kind === 'truck' || c.kind === 'bus' || c.kind === 'garbage') ? 1.6 : 1;
    const gain = (1 - d / R) ** 2 * (0.35 + 0.65 * clamp01(v / 200 + (c.controls?.throttle ?? 0) * 0.4)) * big;
    out.push({ id: c.id, kind: c.kind ?? 'car', d, gain, pan: Math.max(-1, Math.min(1, dx / 300)), rate: SOUND_SPEED / (SOUND_SPEED - Math.max(-1500, Math.min(1500, vr))), fire: (rpm / 60) * (E.cyl / 2), diesel: E.diesel, tire: clamp01(v / 330) });
  }
  return out.sort((a, b) => b.gain - a.gain).slice(0, n);
}

// Schritte: Schrittlänge nach Tempo (Gehen 0,75 m, Joggen 1,1 m, Sprint 1,6 m); liefert die Anzahl der Schritte
// zwischen zwei Ständen des Wegzählers p.step (px)
export function strideOf(speed) { return speed < 20 ? 7.5 : speed < 45 ? 11 : 16; }
export function stepsBetween(prev, now, speed) {
  if (!(speed > 0) || now <= prev) return 0;
  const L = strideOf(speed);
  return Math.floor(now / L) - Math.floor(prev / L);
}

// Wie ein Schritt klingt: 'snow' knirscht, 'wet' platscht, 'grass' raschelt dumpf, sonst 'hard' (Gehweg, Pflaster)
export function footstepKind(world, x, y) {
  if ((world.snow ?? 0) > 0.25) return 'snow';
  const surf = world.city ? surfaceAt(world.city, x, y, world.player?.lvl ?? null) : T.SIDEWALK;
  if (surf === T.GRASS) return 'grass';
  if ((world.wet ?? 0) > 0.35) return 'wet';
  return 'hard';
}
