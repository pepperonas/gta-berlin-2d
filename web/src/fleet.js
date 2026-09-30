// Fahrzeugarten jenseits des Pkw (rein rechnerisch): Maße, Motorleistung, wann und wo sie unterwegs sind, und ihr
// Arbeitsrhythmus – Paketwagen halten in zweiter Reihe, das Müllauto der BSR fährt morgens von Tonne zu Tonne.
// Blaulichtfahrzeuge (Polizei, Rettungswagen) entstehen nicht zufällig, sondern über Einsätze (emergency.js).
import { isWeekend } from './rhythm.js';

// L × W in px (10 px = 1 m); power skaliert Beschleunigung und Höchsttempo (car.js)
export const KINDS = {
  car: { L: 42, W: 20, power: 1 },
  truck: { L: 76, W: 24, power: 0.7, colors: ['#e9e6df', '#35495e', '#a93226', '#1e6f5c'] },         // 7,5-t-Kastenwagen
  delivery: { L: 52, W: 21, power: 0.85, colors: ['#ffcc00', '#5b3a1e', '#f4f4f2', '#1d3a8a'] },    // Paketdienste
  garbage: { L: 86, W: 25, power: 0.6, colors: ['#f07d00'] },                                        // BSR-Orange
  police: { L: 48, W: 20, power: 1.1, colors: ['#e8ecef'] },                                         // Berliner Polizei (silber-blau)
  ambulance: { L: 60, W: 22, power: 1, colors: ['#f5f5f2'] },                                        // RTW der Feuerwehr
  bus: { L: 120, W: 25, power: 0.75, colors: ['#f0cf1f'] },                                          // Linienbus (12 m), aus dem Fahrplan
  // gekaperte Räder und E-Roller (bikes.js): fahren mit der Autophysik, aber klein, leise und langsam (top in px/s)
  bicycle: { L: 18, W: 7, power: 0.45, top: 72, accel: 0.55, bike: true },                         // ≈ 26 km/h
  escooter: { L: 14, W: 6, power: 0.4, top: 56, accel: 0.5, bike: true },                          // ≈ 20 km/h (Grenze)
  // motorisierte Zweiräder: im Verkehr mit der einfachen Physik, selbst gefahren mit Fahrdynamik (Wheelie, Stoppie)
  motorcycle: { L: 22, W: 8, power: 1.15, moto: true, colors: ['#b3261e', '#1d1f24', '#e8e6e1', '#2d5da8', '#f0a202'] },
  scooter: { L: 18, W: 7, power: 0.6, moto: true, colors: ['#8fc1b5', '#e9e4d6', '#c0392b', '#3d3f45', '#f2c14e'] },
};
export const isBikeKind = (kind) => !!KINDS[kind]?.bike;
export const isMotoKind = (kind) => !!KINDS[kind]?.moto;
// offenes Zweirad (Rad, E-Roller, Motorrad, Roller): keine Kabine – Stadt ungedämpft, keine Kisten, kein Missionsauto
export const isOpenKind = (kind) => isBikeKind(kind) || isMotoKind(kind);
export const EMERGENCY = new Set(['police', 'ambulance']);
export const kindOf = (car) => car.kind ?? 'car';
export const sizeOf = (kind) => KINDS[kind] ?? KINDS.car;

const inHours = (m, a, b) => m >= a && m < b;

// Fahrzeugart für einen neuen Verkehrsteilnehmer auf einer Straße der Klasse cls. r ∈ [0,1).
export function pickKind(minutes, day, cls, r) {
  const m = ((minutes % 1440) + 1440) % 1440, we = isWeekend(day), sun = day === 6;
  let p = 0;
  const truck = inHours(m, 300, 1260) ? (cls <= 4 ? (we ? 0.03 : 0.1) : cls <= 6 ? (we ? 0.01 : 0.04) : 0) : 0.02 * (cls <= 3);
  if ((p += truck) > r) return 'truck';
  const delivery = !sun && inHours(m, 480, 1170) && cls >= 4 ? (we ? 0.04 : 0.08) : 0;
  if ((p += delivery) > r) return 'delivery';
  const garbage = !we && inHours(m, 360, 720) && cls >= 5 && cls <= 8 ? 0.05 : 0;
  if ((p += garbage) > r) return 'garbage';
  // Zweiräder: tagsüber, am Wochenende mehr (Ausflug), nachts kaum
  const moto = inHours(m, 420, 1260) ? (we ? 0.05 : 0.03) : 0.005;
  if ((p += moto * 0.55) > r) return 'motorcycle';
  if ((p += moto * 0.45) > r) return 'scooter';
  return 'car';
}

// Arbeitshalte je Art: Abstand bis zum nächsten Halt (px Fahrstrecke) und Haltedauer (s). r1, r2 ∈ [0,1).
export function nextStopAfter(kind, r) {
  if (kind === 'delivery') return 1500 + r * 3500;   // alle 150–500 m ein Paket
  if (kind === 'garbage') return 350 + r * 450;      // alle 35–80 m Tonnen
  return Infinity;
}
export function stopDuration(kind, r) {
  if (kind === 'delivery') return 12 + r * 16;
  if (kind === 'garbage') return 6 + r * 4;
  return 0;
}

// Darf hier gehalten werden? Nicht auf Hauptstraßen (Lieferwagen) und nicht nahe der Kreuzung.
export function mayStopOn(kind, lane, distToEnd, distFromStart) {
  if (!lane || distToEnd < 250 || distFromStart < 120) return false;
  const cls = lane.edge.cls;
  if (kind === 'garbage') return cls >= 5 && cls <= 8;
  if (kind === 'delivery') return cls >= 4;
  return false;
}

// Berliner Martinshorn (Folgetonhorn): zwei Töne im Wechsel, je 0,6 s. true = hoher Ton.
export const SIREN = { low: 440, high: 585, period: 1.2 };
export const sirenHigh = (t) => (t % SIREN.period) >= SIREN.period / 2;
