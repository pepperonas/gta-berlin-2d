// Pkw-Modelle und ihre Technik (rein rechnerisch): Antrieb, Motorlage, Masse, Gewichtsverteilung, Schwerpunkthöhe,
// Radstand, Spur, Leistung, Höchsttempo, Reifenhaftung. Die Fahrphysik des gefahrenen Autos (dynamics.js) liest
// daraus alles; Zeichnen (vehicles.js), Motorklang (soundscape.js) und HUD zeigen es. Das Modell eines Autos folgt
// aus seiner Nummer (Hash, kein world.rng) oder steht fest (car.model, z. B. vom Befehl „auto sportwagen“).
// Keine Marken: Namen beschreiben die Bauart.

// Pkw-Modelle im Verkehr (Häufigkeit ≈ Berliner Straßenbild) – alle in derselben Kollisionsbox 4,2 × 2,0 m
export const CAR_MODELS = ['kleinwagen', 'kompakt', 'limousine', 'kombi', 'transporter', 'taxi', 'elektro', 'gelaende', 'sportwagen', 'heckcoupe', 'zweitakter'];
const SHARE = { kleinwagen: 0.2, kompakt: 0.18, limousine: 0.14, kombi: 0.14, transporter: 0.1, taxi: 0.07, elektro: 0.07, gelaende: 0.05, sportwagen: 0.02, heckcoupe: 0.02, zweitakter: 0.01 };
export const SPECIAL_MODELS = ['truck', 'delivery', 'garbage', 'police', 'ambulance', 'bus'];

// drive: fwd (Front), rwd (Heck), awd (Allrad, front = Anteil vorn); engine: front | mid | rear | floor (Akku im Boden)
// mass kg, front = Gewichtsanteil Vorderachse, h = Schwerpunkthöhe m, wb = Radstand m, track = Spur m, kW, vmax km/h,
// mu = Reifenhaftung (trocken), yaw = Faktor der Gierträgheit (Mittelmotor klein, Heckmotor/Überhänge groß),
// vLow = bis zu diesem Tempo (m/s) begrenzt das Drehmoment statt der Leistung, bias = Bremskraftanteil vorn,
// noAids = ohne ASR/ESP (Oldtimer)
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
  return `${s.label} · ${ENGINE_LABEL[s.engine]} · ${DRIVE_LABEL[s.drive]} · ${s.kW} kW`;
}
