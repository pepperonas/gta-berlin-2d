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
};
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
