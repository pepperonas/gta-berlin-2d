// Pkw-Modelle und ihre Technik (rein rechnerisch): Antrieb, Motorlage, Masse, Gewichtsverteilung, Schwerpunkthöhe,
// Radstand, Spur, Leistung, Höchsttempo, Reifenhaftung. Die Fahrphysik des gefahrenen Autos (dynamics.js) liest
// daraus alles; Zeichnen (vehicles.js), Motorklang (soundscape.js) und HUD zeigen es. Das Modell eines Autos folgt
// aus seiner Nummer (Hash, kein world.rng) oder steht fest (car.model, z. B. vom Befehl „auto sportwagen“).
// Keine Marken: Namen beschreiben die Bauart.

// Pkw-Modelle im Verkehr (Häufigkeit ≈ Berliner Straßenbild) – alle in derselben Kollisionsbox 4,2 × 2,0 m
export const CAR_MODELS = ['kleinwagen', 'kompakt', 'limousine', 'kombi', 'transporter', 'taxi', 'elektro', 'gelaende', 'sportwagen', 'heckcoupe', 'zweitakter',
  'hothatch', 'roadster', 'musclecar', 'oldtimer', 'pickup', 'kleinbus', 'rallye'];
const SHARE = { kleinwagen: 0.18, kompakt: 0.16, limousine: 0.13, kombi: 0.12, transporter: 0.09, taxi: 0.07, elektro: 0.07, gelaende: 0.04, sportwagen: 0.015, heckcoupe: 0.015, zweitakter: 0.01,
  hothatch: 0.02, roadster: 0.015, musclecar: 0.01, oldtimer: 0.01, pickup: 0.015, kleinbus: 0.015, rallye: 0.005 };
export const SPECIAL_MODELS = ['truck', 'delivery', 'garbage', 'police', 'ambulance', 'bus'];
export const MOTO_MODELS = ['motorcycle', 'scooter'];

// drive: fwd (Front), rwd (Heck), awd (Allrad, front = Anteil vorn); engine: front | mid | rear | floor (Akku im Boden)
// mass kg, front = Gewichtsanteil Vorderachse, h = Schwerpunkthöhe m, wb = Radstand m, track = Spur m, kW, vmax km/h,
// mu = Reifenhaftung (trocken), yaw = Faktor der Gierträgheit (Mittelmotor klein, Heckmotor/Überhänge groß),
// vLow = bis zu diesem Tempo (m/s) begrenzt das Drehmoment statt der Leistung, bias = Bremskraftanteil vorn,
// noAids = ohne ASR/ESP (Oldtimer), brakeK = Bremsanlage relativ zu heute (Trommelbremsen < 1),
// twoWheel = Zweirad: Wheelie/Stoppie begrenzen Anzug und Bremse (Vorder-/Hinterrad hebt ab), keine Querlast (es legt sich)
const S = (label, drive, engine, mass, front, h, wb, track, kW, vmax, mu, o = {}) =>
  ({ label, drive, engine, mass, front, h, wb, track, kW, vmax, mu, yaw: 1, vLow: 7, bias: 0.68, awdFront: 0.5, steerMax: 0.62, ...o });
export const SPECS = {
  zweitakter: S('Zweitakter', 'fwd', 'front', 650, 0.6, 0.56, 2.02, 1.2, 19, 107, 0.82, { yaw: 1.05, vLow: 5, twoStroke: true, noAids: true }),
  kleinwagen: S('Kleinwagen', 'fwd', 'front', 1100, 0.63, 0.54, 2.47, 1.45, 60, 170, 0.95),
  kompakt: S('Kompaktwagen', 'fwd', 'front', 1350, 0.61, 0.55, 2.63, 1.55, 110, 210, 1),
  limousine: S('Limousine', 'rwd', 'front', 1600, 0.52, 0.53, 2.85, 1.58, 180, 240, 1.02),
  taxi: S('Taxi', 'rwd', 'front', 1750, 0.53, 0.56, 2.94, 1.58, 120, 210, 0.98, { diesel: true }),
  kombi: S('Kombi', 'awd', 'front', 1750, 0.58, 0.57, 2.82, 1.58, 170, 230, 1, { awdFront: 0.4 }),
  transporter: S('Transporter', 'fwd', 'front', 2100, 0.6, 0.85, 3.0, 1.7, 100, 160, 0.9, { yaw: 1.1, diesel: true }),
  elektro: S('Elektro-SUV', 'awd', 'floor', 2250, 0.49, 0.55, 2.9, 1.65, 300, 220, 1, { yaw: 0.95, vLow: 3.5, awdFront: 0.45, electric: true }),
  gelaende: S('Geländewagen', 'awd', 'front', 2300, 0.52, 0.85, 2.85, 1.65, 190, 190, 0.92, { yaw: 1.05, diesel: true }),
  sportwagen: S('Sportwagen', 'rwd', 'mid', 1400, 0.42, 0.45, 2.6, 1.6, 320, 300, 1.15, { yaw: 0.78, bias: 0.6 }),
  heckcoupe: S('Heckmotor-Coupé', 'rwd', 'rear', 1450, 0.38, 0.48, 2.45, 1.55, 290, 290, 1.1, { yaw: 1.18, bias: 0.58 }),
  // Hot Hatch: starker Fronttriebler, Vorderräder am Limit (Antrieb frisst Seitenhalt → schiebt unter Last)
  hothatch: S('Hot Hatch', 'fwd', 'front', 1380, 0.62, 0.5, 2.62, 1.56, 221, 250, 1.08, { yaw: 0.95 }),
  // Roadster: leicht, Frontmittelmotor, 50:50, tiefer Schwerpunkt, wenig Leistung – spielerisch, verzeiht viel
  roadster: S('Roadster', 'rwd', 'front', 1050, 0.5, 0.45, 2.31, 1.5, 97, 205, 1.05, { yaw: 0.85, open: true }),
  // Muscle-Car: großer V8 vorn, schwere Nase, weiche Reifen, schwache Bremsen, keine Fahrhilfen – quertreibt am Gas
  musclecar: S('Muscle-Car', 'rwd', 'front', 1650, 0.56, 0.52, 2.74, 1.52, 330, 250, 0.95, { yaw: 1.1, vLow: 8, noAids: true, brakeK: 0.85 }),
  // Oldtimer-Limousine (60er): Trommelbremsen, Diagonalreifen, kein ESP, wenig Leistung
  oldtimer: S('Oldtimer', 'rwd', 'front', 1300, 0.53, 0.58, 2.75, 1.45, 60, 150, 0.75, { yaw: 1.1, noAids: true, brakeK: 0.7, steerMax: 0.58 }),
  // Pick-up: Motor vorn, leere Ladefläche → wenig Last auf der angetriebenen Hinterachse, hoher Schwerpunkt
  pickup: S('Pick-up', 'rwd', 'front', 2150, 0.6, 0.8, 3.1, 1.65, 150, 180, 0.9, { yaw: 1.1, diesel: true }),
  // Kleinbus mit Heckmotor (Bulli-Bauart): schwaches Heck-Boxermotörchen, hoch, windempfindlich
  kleinbus: S('Kleinbus', 'rwd', 'rear', 1500, 0.44, 0.9, 2.4, 1.4, 55, 115, 0.85, { yaw: 1.15, noAids: true, brakeK: 0.8 }),
  // Rallye-Kompakt: Allrad mit Hecktendenz, leicht, bissige Reifen
  rallye: S('Rallye-Kompakt', 'awd', 'front', 1280, 0.58, 0.5, 2.52, 1.55, 190, 230, 1.08, { yaw: 0.9, awdFront: 0.35 }),
  // Motorräder (Zweiräder; Masse inkl. Fahrer, Spur ohne Bedeutung): Wheelie begrenzt den Anzug, Stoppie die Bremse
  motorcycle: S('Motorrad', 'rwd', 'mid', 280, 0.5, 0.66, 1.45, 1, 110, 240, 1.1, { twoWheel: true, yaw: 1, bias: 0.75, steerMax: 0.5, vLow: 6 }),
  scooter: S('Motorroller', 'rwd', 'rear', 215, 0.42, 0.6, 1.35, 1, 11, 95, 1, { twoWheel: true, yaw: 1, bias: 0.6, steerMax: 0.55, vLow: 5, noAids: true }),
  // Nutz- und Einsatzfahrzeuge (fleet.js-Arten), damit auch ein gekaperter LKW oder Bus echt fährt
  police: S('Streifenwagen', 'rwd', 'front', 1750, 0.53, 0.56, 2.94, 1.58, 190, 240, 1.02),
  ambulance: S('Rettungswagen', 'rwd', 'front', 4200, 0.45, 1.15, 3.66, 1.75, 140, 140, 0.85, { yaw: 1.15, steerMax: 0.55, diesel: true }),
  delivery: S('Paketwagen', 'fwd', 'front', 2900, 0.55, 1.0, 3.3, 1.7, 105, 145, 0.88, { yaw: 1.1, steerMax: 0.58, diesel: true }),
  truck: S('Lkw 7,5 t', 'rwd', 'front', 7500, 0.45, 1.4, 4.2, 1.95, 150, 100, 0.8, { yaw: 1.2, vLow: 4, steerMax: 0.55, diesel: true }),
  garbage: S('Müllwagen', 'rwd', 'front', 18000, 0.38, 1.6, 4.6, 2.05, 240, 85, 0.75, { yaw: 1.2, vLow: 3, steerMax: 0.55, diesel: true }),
  bus: S('Linienbus', 'rwd', 'rear', 12500, 0.35, 1.25, 6.0, 2.1, 220, 90, 0.78, { yaw: 1.25, vLow: 3, steerMax: 0.6, diesel: true }),
};

export const DRIVE_LABEL = { fwd: 'Frontantrieb', rwd: 'Heckantrieb', awd: 'Allrad' };
export const ENGINE_LABEL = { front: 'Frontmotor', mid: 'Mittelmotor', rear: 'Heckmotor', floor: 'Elektromotoren' };
const MOTO_LINE = { motorcycle: 'Motorrad · Vierzylinder · Kette', scooter: 'Motorroller · Einzylinder · Automatik' };

const h01 = (n) => { const x = Math.sin(n * 91.345 + 12.9898) * 43758.5453; return x - Math.floor(x); };

// Modell eines Autos (fest je Nummer), Sonderfahrzeuge nach ihrer Art, Spielerauto eine Limousine
export function carModel(car) {
  if (car.model) return car.model;
  if (car.kind && car.kind !== 'car') return car.kind; // LKW, Paketwagen, Müllauto, Einsatzfahrzeuge (fleet.js)
  if (car.role === 'player') return 'limousine';
  let r = h01(car.id);
  for (const m of CAR_MODELS) { if ((r -= SHARE[m]) < 0) return m; }
  return 'limousine';
}
export const specOf = (car) => SPECS[carModel(car)] ?? SPECS.limousine;
// „Sportwagen · Mittelmotor · Heckantrieb · 320 kW“
export function specLine(car) {
  const s = specOf(car);
  if (s.twoWheel) return `${MOTO_LINE[carModel(car)] ?? s.label} · ${s.kW} kW`;
  return `${s.label} · ${ENGINE_LABEL[s.engine]} · ${DRIVE_LABEL[s.drive]} · ${s.kW} kW`;
}
