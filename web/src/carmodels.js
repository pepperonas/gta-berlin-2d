// Pkw-Modelle und ihre Technik (rein rechnerisch): Antrieb, Motorlage, Masse, Gewichtsverteilung, Schwerpunkthöhe,
// Radstand, Spur, Leistung, Höchsttempo, Reifenhaftung. Die Fahrphysik des gefahrenen Autos (dynamics.js) liest
// daraus alles; Zeichnen (vehicles.js), Motorklang (soundscape.js) und HUD zeigen es. Das Modell eines Autos folgt
// aus seiner Nummer (Hash, kein world.rng) oder steht fest (car.model, z. B. vom Befehl „auto sportwagen“).
// Keine echten Marken: Modellnamen sind erfunden (Berliner Orte), die Bauart steht in label.

// Pkw-Modelle im Verkehr (Häufigkeit ≈ Berliner Straßenbild) – alle in derselben Kollisionsbox 4,2 × 2,0 m
export const CAR_MODELS = ['kleinwagen', 'kompakt', 'limousine', 'kombi', 'transporter', 'taxi', 'elektro', 'gelaende', 'sportwagen', 'heckcoupe', 'zweitakter',
  'hothatch', 'roadster', 'musclecar', 'oldtimer', 'pickup', 'kleinbus', 'rallye',
  'supersport', 'gtcoupe', 'leichtbau', 'elektrosport', 'sprinter', 'hochdach', 'powerkombi', 'familienkombi', 'business', 'sportlimo',
  'luxus', 'coupe', 'leichtcoupe', 'gklasse', 'defender', 'niva', 'kompaktsuv', 'sportsuv', 'grosssuv'];
// Anteile (Summe 1): Alltagsautos häufig, Sportwagen und Exoten selten; neue Modelle (0.45.0) nach echten Vorbildern
const SHARE = { kleinwagen: 0.11, kompakt: 0.09, limousine: 0.1, kombi: 0.071, transporter: 0.06, taxi: 0.07, elektro: 0.05, gelaende: 0.025, sportwagen: 0.01, heckcoupe: 0.01, zweitakter: 0.01,
  hothatch: 0.015, roadster: 0.015, musclecar: 0.01, oldtimer: 0.01, pickup: 0.01, kleinbus: 0.01, rallye: 0.005,
  supersport: 0.004, gtcoupe: 0.006, leichtbau: 0.006, elektrosport: 0.008, sprinter: 0.03, hochdach: 0.03, powerkombi: 0.008, familienkombi: 0.04, business: 0.04, sportlimo: 0.01,
  luxus: 0.015, coupe: 0.01, leichtcoupe: 0.008, gklasse: 0.008, defender: 0.008, niva: 0.006, kompaktsuv: 0.05, sportsuv: 0.012, grosssuv: 0.02 };
export const SPECIAL_MODELS = ['truck', 'delivery', 'garbage', 'police', 'ambulance', 'bus'];
export const MOTO_MODELS = ['motorcycle', 'scooter'];

// drive: fwd (Front), rwd (Heck), awd (Allrad, front = Anteil vorn); engine: front | mid | rear | floor (Akku im Boden)
// mass kg, front = Gewichtsanteil Vorderachse, h = Schwerpunkthöhe m, wb = Radstand m, track = Spur m, kW, vmax km/h,
// mu = Reifenhaftung (trocken), yaw = Faktor der Gierträgheit (Mittelmotor klein, Heckmotor/Überhänge groß),
// vLow = bis zu diesem Tempo (m/s) begrenzt das Drehmoment statt der Leistung, bias = Bremskraftanteil vorn,
// brakeK = Bremsanlage relativ zu heute (Trommelbremsen < 1),
// twoWheel = Zweirad: Wheelie/Stoppie begrenzen Anzug und Bremse (Vorder-/Hinterrad hebt ab), keine Querlast (es legt sich)
const S = (label, drive, engine, mass, front, h, wb, track, kW, vmax, mu, o = {}) =>
  ({ label, drive, engine, mass, front, h, wb, track, kW, vmax, mu, yaw: 1, vLow: 7, bias: 0.68, awdFront: 0.5, steerMax: 0.62, ...o });
export const SPECS = {
  zweitakter: S('Zweitakter', 'fwd', 'front', 650, 0.6, 0.56, 2.02, 1.2, 19, 107, 0.82, { yaw: 1.05, vLow: 5, twoStroke: true }),
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
  // Muscle-Car: großer V8 vorn, schwere Nase, weiche Reifen, schwache Bremsen – quertreibt am Gas
  musclecar: S('Muscle-Car', 'rwd', 'front', 1650, 0.56, 0.52, 2.74, 1.52, 330, 250, 0.95, { yaw: 1.1, vLow: 8, brakeK: 0.85 }),
  // Oldtimer-Limousine (60er): Trommelbremsen, Diagonalreifen, wenig Leistung
  oldtimer: S('Oldtimer', 'rwd', 'front', 1300, 0.53, 0.58, 2.75, 1.45, 60, 150, 0.75, { yaw: 1.1, brakeK: 0.7, steerMax: 0.58 }),
  // Pick-up: Motor vorn, leere Ladefläche → wenig Last auf der angetriebenen Hinterachse, hoher Schwerpunkt
  pickup: S('Pick-up', 'rwd', 'front', 2150, 0.6, 0.8, 3.1, 1.65, 150, 180, 0.9, { yaw: 1.1, diesel: true }),
  // Kleinbus mit Heckmotor (Bulli-Bauart): schwaches Heck-Boxermotörchen, hoch, windempfindlich
  kleinbus: S('Kleinbus', 'rwd', 'rear', 1500, 0.44, 0.9, 2.4, 1.4, 55, 115, 0.85, { yaw: 1.15, brakeK: 0.8 }),
  // Rallye-Kompakt: Allrad mit Hecktendenz, leicht, bissige Reifen
  rallye: S('Rallye-Kompakt', 'awd', 'front', 1280, 0.58, 0.5, 2.52, 1.55, 190, 230, 1.08, { yaw: 0.9, awdFront: 0.35 }),
  // Neue Modelle (0.45.0), Werte nach echten Vorbildern (Leergewicht + Fahrer, Achslast, Radstand, Leistung, Spitze)
  // V10-Mittelmotor mit Allrad (Vorbild: italienischer Supersportler, 640 PS)
  supersport: S('Supersportwagen', 'awd', 'mid', 1422, 0.43, 0.44, 2.62, 1.67, 470, 325, 1.2, { yaw: 0.78, bias: 0.6, awdFront: 0.3 }),
  // Front-Mittelmotor-V8, Transaxle, lange Haube (Vorbild: deutsches GT-Coupé)
  gtcoupe: S('GT-Coupé', 'rwd', 'front', 1615, 0.47, 0.46, 2.63, 1.63, 390, 318, 1.15, { yaw: 0.9, bias: 0.62 }),
  // leichter Mittelmotor-Zweisitzer (Vorbild: französisches Leichtbau-Coupé)
  leichtbau: S('Mittelmotor-Leichtbau', 'rwd', 'mid', 1100, 0.44, 0.44, 2.42, 1.55, 185, 250, 1.1, { yaw: 0.75, bias: 0.62 }),
  // zwei E-Motoren, Akku tief im Boden (Vorbild: deutsche E-Sportlimousine, 760 PS)
  elektrosport: S('Elektro-Sportlimousine', 'awd', 'floor', 2300, 0.48, 0.45, 2.9, 1.66, 560, 260, 1.12, { yaw: 0.9, vLow: 3.5, awdFront: 0.4, electric: true, bias: 0.62 }),
  // Hochdach-Kastenwagen, Heckantrieb (Vorbild: Stuttgarter Großraumtransporter)
  sprinter: S('Großraumtransporter', 'rwd', 'front', 2700, 0.52, 1.05, 3.66, 1.72, 120, 160, 0.88, { yaw: 1.15, steerMax: 0.56, diesel: true }),
  // Hochdachkombi (Vorbild: Wolfsburger Stadtlieferwagen)
  hochdach: S('Hochdachkombi', 'fwd', 'front', 1650, 0.6, 0.75, 2.75, 1.56, 90, 180, 0.92, { yaw: 1.05, diesel: true }),
  // V8-Biturbo-Kombi mit Allrad (Vorbild: Ingolstädter Power-Kombi, 600 PS)
  powerkombi: S('Power-Kombi', 'awd', 'front', 2075, 0.57, 0.52, 2.93, 1.67, 441, 280, 1.12, { awdFront: 0.4, yaw: 1.0 }),
  // Diesel-Kombi der Mittelklasse (Vorbild: Wolfsburger Mittelklasse-Variant)
  familienkombi: S('Familienkombi', 'fwd', 'front', 1650, 0.6, 0.55, 2.79, 1.58, 147, 225, 1.0, { diesel: true }),
  // Reihensechser, fast 50:50 (Vorbild: Münchner obere Mittelklasse)
  business: S('Businesslimousine', 'rwd', 'front', 1800, 0.51, 0.52, 2.98, 1.6, 250, 250, 1.04, {}),
  // Hochleistungs-Reihensechser, Heckantrieb (Vorbild: Münchner Sportlimousine, 510 PS)
  sportlimo: S('Sportlimousine', 'rwd', 'front', 1730, 0.52, 0.5, 2.86, 1.62, 375, 290, 1.12, { yaw: 0.95, vLow: 8 }),
  // lange Oberklasse, V8, schwer und ruhig (Vorbild: Stuttgarter Oberklasse)
  luxus: S('Luxuslimousine', 'rwd', 'front', 2100, 0.52, 0.54, 3.1, 1.65, 320, 250, 1.02, { yaw: 1.1 }),
  // kurzes Reihensechser-Coupé (Vorbild: Münchner Kompakt-Sportcoupé, 460 PS)
  coupe: S('Sportcoupé', 'rwd', 'front', 1700, 0.53, 0.48, 2.75, 1.6, 338, 285, 1.1, { yaw: 0.92 }),
  // Boxer-Vierzylinder, tief, wenig Grip – Drift-Liebling (Vorbild: japanisches Leichtbau-Coupé)
  leichtcoupe: S('Leichtes Coupé', 'rwd', 'front', 1275, 0.53, 0.46, 2.575, 1.54, 172, 226, 1.0, { yaw: 0.88 }),
  // kantiger V8-Geländewagen mit Leiterrahmen, hoher Schwerpunkt (Vorbild: Grazer Geländewagen)
  gklasse: S('Geländewagen (Kastenform)', 'awd', 'front', 2560, 0.5, 0.95, 2.89, 1.64, 310, 210, 0.92, { yaw: 1.1, steerMax: 0.58 }),
  // Sechszylinder-Diesel, lange Federwege (Vorbild: britischer Geländewagen)
  defender: S('Expeditions-Geländewagen', 'awd', 'front', 2350, 0.5, 0.88, 3.02, 1.7, 221, 191, 0.9, { yaw: 1.1, diesel: true }),
  // permanenter Allrad, 83 PS, keine Fahrhilfen (Vorbild: russischer Kompakt-Geländewagen)
  niva: S('Kleiner Geländewagen', 'awd', 'front', 1285, 0.55, 0.75, 2.2, 1.43, 61, 142, 0.85, { yaw: 1.05, brakeK: 0.85, steerMax: 0.6 }),
  // Kompakt-SUV mit Frontantrieb (Vorbild: Wolfsburger Kompakt-SUV)
  kompaktsuv: S('Kompakt-SUV', 'fwd', 'front', 1600, 0.6, 0.66, 2.68, 1.58, 110, 200, 0.98, {}),
  // sportliches V6-SUV mit hecklastigem Allrad (Vorbild: Stuttgarter Sport-SUV)
  sportsuv: S('Sport-SUV', 'awd', 'front', 2050, 0.53, 0.66, 2.9, 1.66, 250, 245, 1.05, { awdFront: 0.4 }),
  // großes Reihensechser-SUV (Vorbild: Münchner Oberklasse-SUV)
  grosssuv: S('Großes SUV', 'awd', 'front', 2200, 0.52, 0.7, 2.98, 1.68, 250, 243, 1.02, { awdFront: 0.4, yaw: 1.05 }),
  // Motorräder (Zweiräder; Masse inkl. Fahrer, Spur ohne Bedeutung): Wheelie begrenzt den Anzug, Stoppie die Bremse
  motorcycle: S('Motorrad', 'rwd', 'mid', 280, 0.5, 0.66, 1.45, 1, 110, 240, 1.1, { twoWheel: true, yaw: 1, bias: 0.75, steerMax: 0.5, vLow: 6 }),
  scooter: S('Motorroller', 'rwd', 'rear', 215, 0.42, 0.6, 1.35, 1, 11, 95, 1, { twoWheel: true, yaw: 1, bias: 0.6, steerMax: 0.55, vLow: 5 }),
  // Nutz- und Einsatzfahrzeuge (fleet.js-Arten), damit auch ein gekaperter LKW oder Bus echt fährt
  police: S('Streifenwagen', 'rwd', 'front', 1750, 0.53, 0.56, 2.94, 1.58, 190, 240, 1.02),
  ambulance: S('Rettungswagen', 'rwd', 'front', 4200, 0.45, 1.15, 3.66, 1.75, 140, 140, 0.85, { yaw: 1.15, steerMax: 0.55, diesel: true }),
  delivery: S('Paketwagen', 'fwd', 'front', 2900, 0.55, 1.0, 3.3, 1.7, 105, 145, 0.88, { yaw: 1.1, steerMax: 0.58, diesel: true }),
  truck: S('Lkw 7,5 t', 'rwd', 'front', 7500, 0.45, 1.4, 4.2, 1.95, 150, 100, 0.8, { yaw: 1.2, vLow: 4, steerMax: 0.55, diesel: true }),
  garbage: S('Müllwagen', 'rwd', 'front', 18000, 0.38, 1.6, 4.6, 2.05, 240, 85, 0.75, { yaw: 1.2, vLow: 3, steerMax: 0.55, diesel: true }),
  bus: S('Linienbus', 'rwd', 'rear', 12500, 0.35, 1.25, 6.0, 2.1, 220, 90, 0.78, { yaw: 1.25, vLow: 3, steerMax: 0.6, diesel: true }),
};

export const DRIVE_LABEL = { fwd: 'FWD', rwd: 'RWD', awd: 'AWD' };
export const ENGINE_LABEL = { front: 'Frontmotor', mid: 'Mittelmotor', rear: 'Heckmotor', floor: 'Elektromotoren' };
const MOTO_LINE = { motorcycle: 'Motorrad · Vierzylinder · Kette', scooter: 'Motorroller · Einzylinder · Automatik' };

// Modellnamen (erfunden, Berliner Orte – keine echten Marken): „Hersteller“ und Typ, wie sie beim Einsteigen erscheinen
export const NAMES = {
  kleinwagen: ['Havel', 'Piccolo'], kompakt: ['Spree', 'Ronda'], limousine: ['Teltower', 'T6'], taxi: ['Teltower', 'T6 Droschke'],
  kombi: ['Märker', 'Allwetter'], transporter: ['Spree', 'Kasten'], elektro: ['Voltwerk', 'E-Terra'], gelaende: ['Grunewald', 'Keiler'],
  sportwagen: ['Oberbaum', 'Furia'], heckcoupe: ['Wannsee', 'Boxer 6'], zweitakter: ['Lausitz', 'Kolibri'], hothatch: ['Spree', 'Ronda Sport'],
  roadster: ['Köpenick', 'Spyder'], musclecar: ['Tempelhof', 'Stier V8'], oldtimer: ['Adlershof', 'Kanzler'], pickup: ['Grunewald', 'Kipper'],
  kleinbus: ['Havel', 'Kiezbus'], rallye: ['Märker', 'Schotter'], police: ['Teltower', 'T6 Streife'], ambulance: ['Spree', 'Kasten RTW'],
  delivery: ['Spree', 'Kasten Paket'], truck: ['Oberlausitz', 'L75'], garbage: ['Kiezwerk', 'Presse 26'], bus: ['Kiezwerk', 'Stadtbus 12'],
  motorcycle: ['Tegel', 'Blitz 1000'], scooter: ['Kiezflitzer', '125'],
  supersport: ['Oberbaum', 'Tempesta'], gtcoupe: ['Tempelhof', 'GT 40'], leichtbau: ['Adlershof', 'Flèche'], elektrosport: ['Voltwerk', 'Blitz GT'],
  sprinter: ['Spree', 'Großraum 316'], hochdach: ['Havel', 'Kasten Hoch'], powerkombi: ['Märker', 'RS Avant'], familienkombi: ['Teltower', 'T4 Tourer'],
  business: ['Teltower', 'T5'], sportlimo: ['Teltower', 'T3 RS'], luxus: ['Grunewald', 'Senator'], coupe: ['Wannsee', 'Coupé 240'],
  leichtcoupe: ['Lausitz', 'Hachi'], gklasse: ['Grunewald', 'Kommandant'], defender: ['Grunewald', 'Förster 110'], niva: ['Lausitz', 'Taiga'],
  kompaktsuv: ['Spree', 'Tiga'], sportsuv: ['Oberbaum', 'Cayo'], grosssuv: ['Teltower', 'TX7'],
};
// Leistung in PS (1 kW = 1,36 PS)
export const psOf = (spec) => Math.round(spec.kW * 1.36);

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
// { make: 'Oberbaum', type: 'Furia', full: 'Oberbaum Furia' }
export function vehicleName(car) {
  const [make, type] = NAMES[carModel(car)] ?? NAMES.limousine;
  return { make, type, full: `${make} ${type}` };
}
// „Sportwagen · Mittelmotor · RWD · 435 PS“
export function specLine(car) {
  const s = specOf(car);
  if (s.twoWheel) return `${MOTO_LINE[carModel(car)] ?? s.label} · ${psOf(s)} PS`;
  return `${s.label} · ${ENGINE_LABEL[s.engine]} · ${DRIVE_LABEL[s.drive]} · ${psOf(s)} PS`;
}
