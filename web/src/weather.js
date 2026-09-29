// Wetter (rein rechnerisch): für jeden Tag in Blöcken zu 3 Stunden ein Wetterbild aus einem Hash von Welt-Samen und
// Tagnummer – gleiche Welt, gleiches Wetter, ganz ohne Zufallszahlen der Simulation. Zwischen zwei Blöcken wird
// 45 Spielminuten lang überblendet. Nebel gibt es nur am frühen Morgen. Die Nässe des Bodens folgt dem Regen träge
// (updateWet in world.js): schnell nass, langsam trocken.
import { hash01 } from './map.js';

export const WEATHER_KINDS = ['clear', 'cloudy', 'overcast', 'rain', 'heavyrain', 'storm', 'thunder', 'fog', 'densefog', 'snow', 'heavysnow'];
export const WX_LABEL = {
  clear: 'sonnig', cloudy: 'wolkig', overcast: 'bedeckt', rain: 'Regen', heavyrain: 'Starkregen', storm: 'Sturm',
  thunder: 'Gewitter', fog: 'Nebel', densefog: 'dichter Nebel', snow: 'Schnee', heavysnow: 'Schneesturm',
};
export const WX_ICON = { clear: '☀', cloudy: '⛅', overcast: '☁', rain: '🌧', heavyrain: '🌧', storm: '💨', thunder: '⛈', fog: '≋', densefog: '≋', snow: '🌨', heavysnow: '❄' };
// Werte je Wetterbild: Bewölkung, Regen (1 = Landregen, 1,6 = Starkregen), Nebel (1 = Nebel, 1,7 = dicht),
// Schneefall (0,45 leicht … 1 stark), Sturm (0…1, Böen) und Gewitter (0…1, Blitzhäufigkeit)
const P = (cloud, rain = 0, fog = 0, snow = 0, storm = 0, thunder = 0) => ({ cloud, rain, fog, snow, storm, thunder });
export const PARAMS = {
  clear: P(0.08), cloudy: P(0.45), overcast: P(0.85), rain: P(0.95, 1), heavyrain: P(1, 1.6, 0.15),
  storm: P(0.9, 0.7, 0, 0, 1), thunder: P(1, 1.4, 0.1, 0, 0.6, 1), fog: P(0.6, 0, 1), densefog: P(0.7, 0, 1.7),
  snow: P(0.9, 0, 0.1, 0.45), heavysnow: P(1, 0, 0.45, 1, 0.5),
};
const KEYS = ['cloud', 'rain', 'fog', 'snow', 'storm', 'thunder'];
export const BLOCK = 180, BLEND = 45;
export const WET = { rise: 1 / 40, dry: 1 / 600 }; // je Sekunde: nass in ~40 s Starkregen, trocken in ~10 min
// Schneedecke je Sekunde: bei starkem Schneefall in ~100 s geschlossen, taut ohne Schneefall in ~25 min, Regen taut schnell
export const SNOW = { rise: 1 / 100, melt: 1 / 1500, rainMelt: 1 / 150 };

// Wetterlage eines Tages: meist wechselhaft-normal, manchmal unbeständig (Sturm, Gewitter), manchmal Winter (Schnee)
export function dayType(seed, dayCount) {
  const r = hash01((seed * 104729 + dayCount * 7121 + 91) | 0);
  return r < 0.14 ? 'winter' : r < 0.28 ? 'unsettled' : 'normal';
}

// Wetterbild eines Blocks (Tag dayCount, Block 0…7)
export function blockKind(seed, dayCount, block) {
  const r = hash01((seed * 7919 + dayCount * 131 + block * 17 + 3) | 0);
  const r2 = hash01((seed * 613 + dayCount * 29 + block * 101 + 11) | 0);
  const morning = block === 1 || block === 2; // 3–9 Uhr
  const type = dayType(seed, dayCount);
  if (morning && r < (type === 'winter' ? 0.2 : 0.12)) return r2 < 0.35 ? 'densefog' : 'fog';
  const p = morning ? r - 0.12 : r;
  if (type === 'winter') return p < 0.22 ? 'overcast' : p < 0.36 ? 'cloudy' : p < 0.74 ? 'snow' : 'heavysnow';
  if (type === 'unsettled') {
    const warm = block >= 4 && block <= 6; // Gewitter am Nachmittag und Abend
    return p < 0.16 ? 'clear' : p < 0.34 ? 'cloudy' : p < 0.48 ? 'overcast' : p < 0.62 ? 'rain' : p < 0.72 ? 'heavyrain' : p < 0.86 || !warm ? 'storm' : 'thunder';
  }
  return p < 0.34 ? 'clear' : p < 0.62 ? 'cloudy' : p < 0.8 ? 'overcast' : p < 0.93 ? 'rain' : 'heavyrain';
}

// Wetter zur Uhrzeit: { kind, cloud, rain, fog, snow, storm, thunder, wind:{x,y} } – stetig über Blockgrenzen und Mitternacht
export function weatherAt(seed, dayCount, minutes, force = null) {
  if (force) return { kind: force, ...PARAMS[force], wind: windAt(seed, dayCount, PARAMS[force].storm) };
  const m = ((minutes % 1440) + 1440) % 1440, b = Math.floor(m / BLOCK), into = m - b * BLOCK;
  const k0 = blockKind(seed, dayCount, b);
  const nb = (b + 1) % 8, nd = nb === 0 ? dayCount + 1 : dayCount, k1 = blockKind(seed, nd, nb);
  const u = into < BLOCK - BLEND ? 0 : (into - (BLOCK - BLEND)) / BLEND, s = u * u * (3 - 2 * u);
  const a = PARAMS[k0], c = PARAMS[k1], out = { kind: s < 0.5 ? k0 : k1 };
  for (const k of KEYS) out[k] = a[k] + (c[k] - a[k]) * s;
  out.wind = windAt(seed, dayCount, out.storm);
  return out;
}

function windAt(seed, dayCount, storm = 0) {
  const a = hash01(seed * 31 + dayCount * 7 + 1) * Math.PI * 2, v = 12 + hash01(seed + dayCount * 13) * 30 + storm * 190; // px/s Wolkenzug
  return { x: Math.cos(a) * v, y: Math.sin(a) * v };
}

// Böen: Faktor auf den Wind (1 = mittlerer Wind), bei Sturm heftig und unregelmäßig – aus der Spielzeit, ohne Zufall
export function gustAt(wx, t) {
  const s = wx?.storm ?? 0;
  if (s <= 0) return 1;
  const g = 0.5 * Math.sin(t * 0.37) + 0.3 * Math.sin(t * 1.13 + 1.7) + 0.2 * Math.sin(t * 2.71 + 0.4);
  return Math.max(0.2, 1 + s * (0.55 * g + 0.25 * Math.max(0, Math.sin(t * 0.21 + 2)) ** 6));
}

// --- Gewitter: Blitze aus Zeitfenstern (je 2,4 s eines, Wahrscheinlichkeit nach Gewitterstärke) --------------------
export const STRIKE = { slot: 2.4, chance: 0.2, soundSpeed: 343 * 10 }; // Schall in px/s (10 px = 1 m)
// Blitz im Fenster i oder null: { t0 (Zeitpunkt), dx, dy (Einschlag relativ zur Kamera, px), dist (px), near, seed }
export function strikeInSlot(seed, i, thunder) {
  if (thunder <= 0.02) return null;
  if (hash01((seed * 48271 + i * 7867 + 5) | 0) > STRIKE.chance * thunder) return null;
  const hx = (k) => hash01((seed * 1597 + i * 389 + k * 7919) | 0);
  const near = hx(1) < 0.3, a = hx(2) * Math.PI * 2;
  const dist = near ? 900 + hx(3) * 4000 : 10000 + hx(3) * 60000; // nah: 90–490 m, fern: 1–7 km
  return { t0: (i + hx(4) * 0.7) * STRIKE.slot, dx: Math.cos(a) * dist, dy: Math.sin(a) * dist, dist, near, seed: (seed * 31 + i) | 0 };
}

// Helligkeit eines Blitzes age Sekunden nach dem Einschlag: Vorentladung, Hauptblitz, zwei Nachblitze, abklingend
export function flashAt(age) {
  if (age < 0 || age > 0.9) return 0;
  let f = 0;
  for (const [t0, a, k] of [[0, 0.35, 40], [0.06, 1, 14], [0.21, 0.7, 16], [0.37, 0.45, 18]]) if (age >= t0) f = Math.max(f, a * Math.exp(-(age - t0) * k));
  return f;
}

// Blitze, die zur Zeit t leuchten (höchstens die letzten 3 Fenster), mit Helligkeit
export function strikesAt(seed, t, thunder) {
  const out = [], i1 = Math.floor(t / STRIKE.slot);
  for (let i = i1 - 2; i <= i1; i++) {
    const s = strikeInSlot(seed, i, thunder);
    if (!s) continue;
    const f = flashAt(t - s.t0);
    if (f > 0.005) out.push({ ...s, age: t - s.t0, flash: f * (s.near ? 1 : 0.45) });
  }
  return out;
}

// Donner, der im Zeitraum (t0, t1] ankommt: [{ loud 0…1, near, dist }] (Schall braucht bis zu 20 s)
export function thunderBetween(seed, t0, t1, thunder) {
  const out = [];
  if (!(t1 > t0) || thunder <= 0.02) return out;
  const maxDelay = 72000 / STRIKE.soundSpeed;
  for (let i = Math.floor((t0 - maxDelay) / STRIKE.slot) - 1; i <= Math.floor(t1 / STRIKE.slot); i++) {
    const s = strikeInSlot(seed, i, thunder);
    if (!s) continue;
    const at = s.t0 + s.dist / STRIKE.soundSpeed;
    if (at > t0 && at <= t1) out.push({ loud: Math.min(1, 1.6 / Math.sqrt(s.dist / 1000)), near: s.near, dist: s.dist });
  }
  return out;
}

// Schneedecke einen Schritt weiter (0 = keine, 1 = geschlossen und tief)
export function stepSnow(snow, wx, dt) {
  if ((wx?.snow ?? 0) > 0.05) return Math.min(1, snow + SNOW.rise * wx.snow * dt);
  return Math.max(0, snow - (SNOW.melt + SNOW.rainMelt * Math.min(1, wx?.rain ?? 0)) * dt);
}

// Nässe des Bodens einen Schritt weiter
export function stepWet(wet, rain, dt) {
  return rain > 0.05 ? Math.min(1, wet + WET.rise * rain * dt) : Math.max(0, wet - WET.dry * dt);
}

// Temperatur (rein): Tageskurve mit Tiefstwert um 5 Uhr und Höchstwert um 15 Uhr, Spanne nach Tagestyp, je Tag ±2 °C aus
// dem Samen. Stetig über Mitternacht (die Nacht läuft vom Höchstwert des Tages zum Tiefstwert des Folgetags).
// Erzwungener Schnee (Konsole) höchstens +1 °C, damit Glätte und Schnee zusammenpassen.
const TEMP = { winter: [-6, 3], unsettled: [4, 14], normal: [8, 22] }, T_LOW = 300, T_HIGH = 900;
function tempRange(seed, d) {
  const [lo, hi] = TEMP[dayType(seed, d)], sh = (hash01((seed * 7907 + d * 3571 + 17) | 0) - 0.5) * 4;
  return [lo + sh, hi + sh];
}
export function temperatureAt(seed, dayCount, minutes, force = null) {
  const m = ((minutes % 1440) + 1440) % 1440, ease = (u) => 0.5 - 0.5 * Math.cos(Math.PI * u);
  let v;
  if (m >= T_LOW && m <= T_HIGH) { const [lo, hi] = tempRange(seed, dayCount); v = lo + (hi - lo) * ease((m - T_LOW) / (T_HIGH - T_LOW)); }
  else {
    const d = m > T_HIGH ? dayCount : dayCount - 1, u = ((m > T_HIGH ? m : m + 1440) - T_HIGH) / (1440 - T_HIGH + T_LOW);
    const hi = tempRange(seed, d)[1], lo = tempRange(seed, d + 1)[0];
    v = hi + (lo - hi) * ease(u);
  }
  return force === 'snow' || force === 'heavysnow' ? Math.min(v, 1) : v;
}

// Glätte des Bodens je Sekunde: überfrierende Nässe (voll in ~10 Spielminuten), taut über 0 °C (~20 Spielminuten)
export const ICE = { rise: 1 / 10, melt: 1 / 20 };
export function stepIce(ice, wet, tempC, dt) {
  if (tempC > 0) return Math.max(0, ice - ICE.melt * dt);
  return wet > 0.1 ? Math.min(1, ice + ICE.rise * dt) : ice;
}

// Licht an das Wetter anpassen: Wolken nehmen der Sonne die Schatten und dämpfen das Umgebungslicht, Regen und Nebel
// machen den Tag grau (die Lichtkarte kommt dann schon am Tag mit – Scheinwerfer an).
export function weatherLight(L, wx, cover = 0) {
  const cloud = wx.cloud, rain = Math.min(1.6, wx.rain), fog = Math.min(1.7, wx.fog), snowing = wx.snow ?? 0;
  // Schneedecke wirft das Himmelslicht zurück: die Szene bleibt hell, auch unter dichten Wolken
  const dim = (0.22 * cloud + 0.1 * rain + 0.08 * fog + 0.08 * snowing) * (1 - 0.45 * cover);
  const ambient = L.ambient.map((v, i) => v * (1 - dim) * (i === 2 ? 1 : 1 - 0.04 * cloud));
  // Trübe (Regen, Nebel, Schneefall, dichte Wolken): tagsüber machen manche Leute Licht
  const gloom = Math.min(1, 0.55 * Math.min(1, rain) + 0.25 * Math.max(0, rain - 1) + 0.45 * Math.min(1, fog) + 0.5 * snowing + 0.25 * Math.max(0, cloud - 0.8) / 0.2);
  return {
    ...L,
    gloom,
    windowsLit: Math.max(L.windowsLit, gloom > 0.3 ? 0.3 * gloom : 0),
    sun: { ...L.sun, strength: L.sun.strength * (1 - 0.85 * cloud) },
    ambient,
    // nur Regen, Nebel und Schneefall machen den Tag so trüb, dass die Lichtkarte (Scheinwerfer) kommt – bloße Wolken
    // nicht. Eine Schneedecke hellt die Nacht auf (sie wirft das Licht der Stadt zurück).
    dark: Math.min(1, Math.max(0, Math.max(L.dark, 0.3 * Math.min(1, rain) + 0.12 * Math.max(0, rain - 1) + 0.25 * Math.min(1, fog) + 0.1 * Math.max(0, fog - 1) + 0.2 * snowing) - 0.18 * cover * L.dark - 0.08 * cover)),
    lampsOn: L.lampsOn || (rain > 0.6 || fog > 0.6 || snowing > 0.6) && L.dark > 0.15,
  };
}

// Regen/Nebel lassen weniger Menschen raus, Räder noch weniger
export const peopleFactor = (wx) => Math.max(0.15, 1 - 0.45 * wx.rain - 0.15 * Math.min(1, wx.fog) - 0.35 * (wx.snow ?? 0) - 0.3 * (wx.storm ?? 0));
export const bikeFactor = (wx) => Math.max(0, 1 - 0.8 * wx.rain - 0.3 * Math.min(1, wx.fog) - 0.9 * (wx.snow ?? 0) - 0.5 * (wx.storm ?? 0));
// Schirm: je Person fest (aus der Nummer), bei stärkerem Regen mehr
export const hasUmbrella = (id, rain) => rain > 0.2 && hash01(id * 5 + 2) < Math.min(1, rain) * 0.85;
