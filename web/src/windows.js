// Erleuchtete Fenster (rein, ohne Canvas): jedes Fenster einzeln. Die Fassade ist ein Raster aus Fensterzellen
// (WIN, deckungsgleich mit dem Fenstermuster in render.js); je Etage bilden zwei bis vier nebeneinanderliegende Fenster
// eine Wohnung. Ob in einer Wohnung jemand zu Hause ist und wann dort das Licht angeht, bestimmt ein Hash aus Haus,
// Fassade, Etage und Wohnung; die Räume einer Wohnung gehen kurz nacheinander an, nie alle zugleich. Wie viele Fenster
// brennen, gibt der Tagesgang vor (daylight.js windowsLit) – steigt er, gehen einzelne Fenster in zufälliger Folge an,
// sinkt er, gehen sie wieder aus. Nachts macht hier und da jemand kurz Licht (Bad, Küche). Büros, Schulen und Hallen
// haben ihren eigenen Tagesgang: abends noch Licht, nachts fast dunkel.
import { BUILDING_KIND } from './citycodes.js';

export const WIN = { cellW: 14, cellH: 16, x: 4, y: 4, w: 6, h: 8 };
// Lichtfarben: warmes Glühlampenlicht, neutral, kaltweiß (LED/Küche), Fernseher (flackert bläulich), gedimmt (Vorhang)
export const WIN_TYPES = ['warm', 'neutral', 'cool', 'tv', 'dim'];
export const WIN_COLOR = { warm: '#ffcf78', neutral: '#ffecc0', cool: '#e4ecff', tv: '#9dbcff', dim: '#e0975a' };
export const WIN_LIGHT = { warm: '#fff0cc', neutral: '#fff6e0', cool: '#f2f6ff', tv: '#c4d6ff', dim: '#ffcf9a' };

const hash = (...n) => {
  let x = 2166136261;
  for (const v of n) { x ^= v | 0; x = Math.imul(x, 16777619); x ^= x >>> 13; }
  x = Math.imul(x ^ (x >>> 16), 2246822507); x = Math.imul(x ^ (x >>> 13), 3266489909);
  return ((x ^ (x >>> 16)) >>> 0) / 4294967296;
};

const OFFICE = [[0, 0.03], [360, 0.04], [420, 0.35], [1020, 0.45], [1140, 0.3], [1230, 0.1], [1320, 0.04], [1440, 0.03]];
function lerp(tab, m) {
  for (let i = 1; i < tab.length; i++) if (m <= tab[i][0]) { const [m0, a] = tab[i - 1], [m1, b] = tab[i]; return a + (b - a) * (m - m0) / (m1 - m0 || 1); }
  return tab[tab.length - 1][1];
}
export const isWorkplace = (b) => b.kind === BUILDING_KIND.public || b.kind === BUILDING_KIND.industrial || b.kind === BUILDING_KIND.warehouse;
// Anteil brennender Fenster eines Hauses: Wohnhäuser nach dem Tagesgang (frac), Arbeitsstätten nach Bürozeiten;
// trübes Wetter macht tagsüber in manchen Wohnungen Licht (gloom 0…1)
export function houseFraction(b, frac, minutes, gloom = 0) {
  const m = ((minutes % 1440) + 1440) % 1440;
  if (isWorkplace(b)) return lerp(OFFICE, m) * (m > 420 && m < 1140 ? 0.7 + 0.3 * gloom : 1); // tagsüber sieht man es nur bei Trübe
  return frac;
}

// Zustand eines Fensters: null (dunkel) oder { type, curtain } – deterministisch aus Haus, Fassade, Zelle und Uhrzeit
export function windowAt(seed, face, col, row, frac, minutes) {
  const m = ((minutes % 1440) + 1440) % 1440;
  const flatW = 2 + Math.floor(hash(seed, face, row, 1) * 3); // 2–4 Fenster je Wohnung, je Etage anders
  const flat = Math.floor((col + Math.floor(hash(seed, face, row, 2) * flatW)) / flatW);
  const home = hash(seed, face, row, flat, 3);        // wann die Wohnung „dran“ ist (klein = früh an, spät aus)
  const room = hash(seed, face, row, col, 4);         // wann dieser Raum dran ist
  const key = 0.78 * home + 0.22 * room;
  let on = key < frac;
  // nachts: kurz Licht in einem einzelnen Raum (Bad, Küche) – je 9 Minuten neu ausgewürfelt
  if (!on && frac > 0 && (m < 330 || m > 1380)) on = hash(seed, face, row, col, Math.floor(minutes / 9), 5) < 0.012;
  if (!on) return null;
  const k = hash(seed, face, row, flat, 6), evening = m > 1080 || m < 120;
  const type = k < 0.58 ? 'warm' : k < 0.78 ? 'neutral' : k < 0.87 ? 'cool' : k < 0.95 ? (evening ? 'tv' : 'warm') : 'dim';
  return { type, curtain: hash(seed, face, row, col, 7) < 0.22 };
}

// Fenster einer Fassade (Länge L, Höhe H in px) → [{ x, y, w, h, type }] nur die brennenden, in Fassaden-Koordinaten
// (x entlang der Wand ab der Ecke, y von unten nach oben wie das Muster in render.js)
export function litWindows(b, face, L, H, frac, minutes, out = []) {
  out.length = 0;
  if (frac <= 0 && !(minutes % 1440 < 330 || minutes % 1440 > 1380)) return out;
  const seed = b.seed | 0;
  for (let col = 0; WIN.cellW * col + WIN.x + WIN.w <= L - 4; col++) {
    for (let row = 0; WIN.cellH * row + WIN.y + WIN.h <= H - 2; row++) {
      const s = windowAt(seed, face, col, row, frac, minutes);
      if (!s) continue;
      const h = s.curtain ? WIN.h * 0.55 : WIN.h;
      out.push({ x: WIN.cellW * col + WIN.x, y: WIN.cellH * row + WIN.y + (WIN.h - h), w: WIN.w, h, type: s.type });
    }
  }
  return out;
}

// Fernseher-Flackern: Helligkeit 0,55…1, je Fenster eigener Rhythmus (t in Sekunden)
export const tvFlicker = (x, y, t) => 0.55 + 0.45 * Math.abs(Math.sin(t * 3.1 + x * 0.7) * Math.sin(t * 7.3 + y * 1.3));
