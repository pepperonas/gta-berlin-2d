// Tageslicht aus der Spieluhr (Minuten seit Mitternacht, 0…1440). Rein rechnerisch, ohne Canvas:
// render.js holt sich daraus Schattenrichtung und -länge, Umgebungslicht (für die Lichtkarte) und ob Laternen brennen.
// Sonnenstand grob für Berlin im Sommer (Sommerzeit): Aufgang 5:30, Mittag 13:00, Untergang 20:30.

export const SUNRISE = 330, SUNSET = 1230;
const MAX_ELEV = 58 * Math.PI / 180;   // Mittagshöhe um die Sommersonnenwende
const AZ_RISE = 50, AZ_SET = 310;      // Azimut (Grad ab Nord im Uhrzeigersinn) bei Auf-/Untergang
export const SHADOW_MAX = 2.4;         // längster Schatten = 2,4 × Höhe (tiefe Sonne; länger deckt ganze Straßen zu)

// Umgebungslicht (Faktor je Farbkanal, 1 = volles Tageslicht) über den Tag; zyklisch linear interpoliert.
const AMBIENT = [
  [0, [0.30, 0.35, 0.52]],
  [270, [0.30, 0.35, 0.52]],
  [330, [0.62, 0.52, 0.58]],   // Morgendämmerung
  [390, [0.95, 0.82, 0.72]],
  [480, [1, 1, 1]],
  [1080, [1, 1, 1]],
  [1170, [1, 0.88, 0.70]],     // goldene Stunde
  [1230, [0.78, 0.58, 0.62]],  // Sonnenuntergang
  [1290, [0.42, 0.42, 0.60]],  // blaue Stunde
  [1350, [0.30, 0.35, 0.52]],
  [1440, [0.30, 0.35, 0.52]],
];

// Anteil erleuchteter Fenster
const WINDOWS = [[0, 0.35], [180, 0.12], [330, 0.1], [420, 0], [1140, 0], [1260, 0.55], [1380, 0.6], [1440, 0.35]];

export const wrapMinutes = (m) => ((m % 1440) + 1440) % 1440;

function lerpTable(tab, m) {
  for (let i = 1; i < tab.length; i++) {
    if (m <= tab[i][0]) {
      const [m0, a] = tab[i - 1], [m1, b] = tab[i], u = (m - m0) / (m1 - m0 || 1);
      return Array.isArray(a) ? a.map((x, k) => x + (b[k] - x) * u) : a + (b - a) * u;
    }
  }
  return tab[tab.length - 1][1];
}

const smooth = (a, b, x) => { const u = Math.min(1, Math.max(0, (x - a) / (b - a))); return u * u * (3 - 2 * u); };

export function lightAt(minutes) {
  const m = wrapMinutes(minutes);
  const d = (m - SUNRISE) / (SUNSET - SUNRISE);             // 0 Aufgang … 1 Untergang
  const elev = Math.sin(Math.PI * d) * MAX_ELEV;              // < 0 nachts
  const az = (AZ_RISE + (AZ_SET - AZ_RISE) * Math.min(1, Math.max(0, d))) * Math.PI / 180;
  // Schatten zeigt von der Sonne weg; Karte: Norden oben, y nach unten.
  const len = Math.min(SHADOW_MAX, 1 / Math.tan(Math.max(elev, 0.02)));
  const strength = smooth(0, 7 * Math.PI / 180, elev);       // Schatten verblassen zum Horizont hin
  const ambient = lerpTable(AMBIENT, m);
  const lum = 0.3 * ambient[0] + 0.55 * ambient[1] + 0.15 * ambient[2];
  const dark = Math.min(1, Math.max(0, (1 - lum) / 0.62));
  return {
    minutes: m,
    elevation: elev,
    sun: { dx: -Math.sin(az), dy: Math.cos(az), len, strength },
    ambient,                                                   // Multiplikator je Kanal
    dark,                                                      // 0 Tag … 1 tiefe Nacht
    lampsOn: m >= 1215 || m < 345,                             // Laternen: 20:15 bis 5:45
    windowsLit: lerpTable(WINDOWS, m),
  };
}

export function formatClock(minutes) {
  const m = Math.floor(wrapMinutes(minutes));
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
}

// "21:30" → 1290 (für ?uhr=…); ungültig → null
export function parseClock(s) {
  const r = /^(\d{1,2}):(\d{2})$/.exec(String(s ?? '').trim());
  if (!r || +r[1] > 23 || +r[2] > 59) return null;
  return +r[1] * 60 + +r[2];
}
