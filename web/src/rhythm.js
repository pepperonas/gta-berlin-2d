// Tagesrhythmus der Stadt (rein rechnerisch): wie viel Verkehr und wie viele Menschen zu welcher Uhrzeit an welchem
// Wochentag unterwegs sind, und wie belebt ein Ort ist (gezählte Kfz je Werktag, Einwohnerdichte, Lokale in der Nähe).
// world.js stellt daraus die Zielbevölkerung um die Kamera ein.
import { densityAt } from './map.js';

export const DAYS = ['Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa', 'So'];
export const START_DAY = 4; // ein neues Spiel beginnt am Freitag

// Stützstellen [Minute, Anteil] – stetig interpoliert; alle Kurven treffen sich um Mitternacht im selben Wert, damit der
// Wechsel Werktag ↔ Wochenende keinen Sprung macht (das Nachtleben trägt die Freitag-/Samstagnacht)
const TRAFFIC_WORKDAY = [[0, 0.26], [240, 0.12], [360, 0.55], [450, 1], [570, 0.72], [720, 0.78], [960, 0.95], [1050, 1], [1170, 0.7], [1320, 0.4], [1440, 0.26]];
const TRAFFIC_WEEKEND = [[0, 0.26], [300, 0.14], [480, 0.35], [660, 0.7], [900, 0.8], [1080, 0.75], [1260, 0.5], [1440, 0.26]];
const PEOPLE_WORKDAY = [[0, 0.28], [240, 0.08], [390, 0.35], [480, 0.7], [720, 0.95], [1020, 1], [1200, 0.75], [1320, 0.45], [1440, 0.28]];
const PEOPLE_WEEKEND = [[0, 0.28], [300, 0.1], [540, 0.35], [720, 0.85], [900, 1], [1140, 0.95], [1320, 0.7], [1440, 0.28]];

function curve(tab, m) {
  for (let i = 1; i < tab.length; i++) if (m <= tab[i][0]) { const [a, va] = tab[i - 1], [b, vb] = tab[i]; return va + (vb - va) * (m - a) / (b - a || 1); }
  return tab[tab.length - 1][1];
}
const wrap = (m) => ((m % 1440) + 1440) % 1440;
export const isWeekend = (day) => day === 5 || day === 6;

export function trafficLevel(minutes, day) { return curve(isWeekend(day) ? TRAFFIC_WEEKEND : TRAFFIC_WORKDAY, wrap(minutes)); }
export function peopleLevel(minutes, day) { return curve(isWeekend(day) ? PEOPLE_WEEKEND : PEOPLE_WORKDAY, wrap(minutes)); }

// Nachtleben: 0 tagsüber, abends ansteigend, Freitag-/Samstagnacht voll (die Nacht zählt zum Vortag bis 6 Uhr)
export function nightlife(minutes, day) {
  const m = wrap(minutes), night = m < 360 ? (day + 6) % 7 : day; // Stunden nach Mitternacht gehören zur Nacht davor
  const party = night === 4 || night === 5 ? 1 : night === 3 || night === 6 ? 0.55 : 0.35;
  const t = m < 360 ? (m < 240 ? 1 : 1 - (m - 240) / 120) : m >= 1260 ? 1 : m >= 1110 ? (m - 1110) / 150 : 0;
  return party * t;
}

// Örtlicher Verkehr: mittlere Kfz je Werktag der Straßen im Umkreis (nach Länge gewichtet), 1 ≈ 8 000 Kfz/Tag
export function localTraffic(city, x, y, r = 3000) {
  let sum = 0, len = 0;
  for (const s of city.edgeSegs.query({ x: x - r, y: y - r, w: 2 * r, h: 2 * r }, [])) {
    const e = s.e;
    if (e.cls > 8 || e.junction) continue;
    const L = Math.hypot(s.bx - s.ax, s.by - s.ay);
    sum += (e.dtv ?? 0) * L; len += L;
  }
  if (!len) return 0.5;
  return Math.min(1.8, Math.max(0.3, Math.sqrt(sum / len / 8000)));
}

// Einwohner je Hektar im Umkreis (Mittel der bewohnten Zellen; Straßen selbst liegen außerhalb der Baublöcke)
export function densityNear(city, x, y, r = 1500, step = 640) {
  let sum = 0, n = 0;
  for (let dx = -r; dx <= r; dx += step) for (let dy = -r; dy <= r; dy += step) {
    const v = densityAt(city, x + dx, y + dy);
    if (v > 0) { sum += v; n++; }
  }
  return n ? sum / n : 0;
}

// Örtliche Belebung zu Fuß: Wohndichte (1 ≈ 200 EW/ha) plus Geschäfte und Lokale im Umkreis
export function localPeople(city, x, y) {
  const dens = densityNear(city, x, y) / 200;
  let shops = 0;
  for (const q of city.poiHash.query({ x: x - 2500, y: y - 2500, w: 5000, h: 5000 }, [])) if (q.cat !== 'bus') shops++;
  return Math.min(1.3, Math.max(0.2, 0.2 + dens * 0.45 + Math.min(1, shops / 60) * 0.6));
}

// Bars, Clubs und Spätis im Umkreis (für das Nachtleben)
export function nightSpots(city, x, y, r = 2500) {
  let n = 0;
  for (const q of city.poiHash.query({ x: x - r, y: y - r, w: 2 * r, h: 2 * r }, [])) if (q.cat === 'drink' || q.kind === 'convenience' || q.kind === 'nightclub') n++;
  return n;
}

// Zielbevölkerung um die Kamera. base: Grundwerte (TRAFFIC.cars / .pedestrians)
export function populationTargets(city, x, y, minutes, day, base) {
  const cars = base.cars * trafficLevel(minutes, day) * localTraffic(city, x, y);
  const night = nightlife(minutes, day) * Math.min(1, nightSpots(city, x, y) / 12);
  const peds = base.pedestrians * (peopleLevel(minutes, day) * localPeople(city, x, y) + night * 0.9);
  return { cars: Math.round(Math.min(base.cars * 1.7, Math.max(3, cars))), peds: Math.round(Math.min(base.pedestrians * 1.6, Math.max(4, peds))) };
}

export const dayName = (day) => DAYS[((day % 7) + 7) % 7];
