// Wetter (rein rechnerisch): für jeden Tag in Blöcken zu 3 Stunden ein Wetterbild aus einem Hash von Welt-Samen und
// Tagnummer – gleiche Welt, gleiches Wetter, ganz ohne Zufallszahlen der Simulation. Zwischen zwei Blöcken wird
// 45 Spielminuten lang überblendet. Nebel gibt es nur am frühen Morgen. Die Nässe des Bodens folgt dem Regen träge
// (updateWet in world.js): schnell nass, langsam trocken.
import { hash01 } from './map.js';

export const WEATHER_KINDS = ['clear', 'cloudy', 'overcast', 'rain', 'fog'];
export const WX_LABEL = { clear: 'sonnig', cloudy: 'wolkig', overcast: 'bedeckt', rain: 'Regen', fog: 'Nebel' };
export const WX_ICON = { clear: '☀', cloudy: '⛅', overcast: '☁', rain: '🌧', fog: '≋' };
// Werte je Wetterbild: Bewölkung, Regen, Nebel
const PARAMS = { clear: [0.08, 0, 0], cloudy: [0.45, 0, 0], overcast: [0.85, 0, 0], rain: [0.95, 1, 0], fog: [0.6, 0, 1] };
export const BLOCK = 180, BLEND = 45;
export const WET = { rise: 1 / 40, dry: 1 / 600 }; // je Sekunde: nass in ~40 s Starkregen, trocken in ~10 min

// Wetterbild eines Blocks (Tag dayCount, Block 0…7)
export function blockKind(seed, dayCount, block) {
  const r = hash01((seed * 7919 + dayCount * 131 + block * 17 + 3) | 0);
  const morning = block === 1 || block === 2; // 3–9 Uhr
  if (morning && r < 0.12) return 'fog';
  const p = morning ? r - 0.12 : r;
  return p < 0.34 ? 'clear' : p < 0.62 ? 'cloudy' : p < 0.8 ? 'overcast' : 'rain';
}

// Wetter zur Uhrzeit: { kind, cloud, rain, fog, wind:{x,y} } – stetig über Blockgrenzen und Mitternacht
export function weatherAt(seed, dayCount, minutes, force = null) {
  if (force) { const [cloud, rain, fog] = PARAMS[force]; return { kind: force, cloud, rain, fog, wind: windAt(seed, dayCount) }; }
  const m = ((minutes % 1440) + 1440) % 1440, b = Math.floor(m / BLOCK), into = m - b * BLOCK;
  const k0 = blockKind(seed, dayCount, b);
  const nb = (b + 1) % 8, nd = nb === 0 ? dayCount + 1 : dayCount, k1 = blockKind(seed, nd, nb);
  const u = into < BLOCK - BLEND ? 0 : (into - (BLOCK - BLEND)) / BLEND, s = u * u * (3 - 2 * u);
  const a = PARAMS[k0], c = PARAMS[k1];
  const mix = (i) => a[i] + (c[i] - a[i]) * s;
  return { kind: s < 0.5 ? k0 : k1, cloud: mix(0), rain: mix(1), fog: mix(2), wind: windAt(seed, dayCount) };
}

function windAt(seed, dayCount) {
  const a = hash01(seed * 31 + dayCount * 7 + 1) * Math.PI * 2, v = 12 + hash01(seed + dayCount * 13) * 30; // px/s Wolkenzug
  return { x: Math.cos(a) * v, y: Math.sin(a) * v };
}

// Nässe des Bodens einen Schritt weiter
export function stepWet(wet, rain, dt) {
  return rain > 0.05 ? Math.min(1, wet + WET.rise * rain * dt) : Math.max(0, wet - WET.dry * dt);
}

// Licht an das Wetter anpassen: Wolken nehmen der Sonne die Schatten und dämpfen das Umgebungslicht, Regen und Nebel
// machen den Tag grau (die Lichtkarte kommt dann schon am Tag mit – Scheinwerfer an).
export function weatherLight(L, wx) {
  const cloud = wx.cloud, dim = 0.22 * cloud + 0.12 * wx.rain + 0.1 * wx.fog;
  const ambient = L.ambient.map((v, i) => v * (1 - dim) * (i === 2 ? 1 : 1 - 0.04 * cloud));
  return {
    ...L,
    sun: { ...L.sun, strength: L.sun.strength * (1 - 0.85 * cloud) },
    ambient,
    // nur Regen und Nebel machen den Tag so trüb, dass die Lichtkarte (Scheinwerfer) kommt – bloße Wolken nicht
    dark: Math.min(1, Math.max(L.dark, 0.3 * wx.rain + 0.25 * wx.fog)),
    lampsOn: L.lampsOn || (wx.rain > 0.6 || wx.fog > 0.6) && L.dark > 0.08,
  };
}

// Regen/Nebel lassen weniger Menschen raus, Räder noch weniger
export const peopleFactor = (wx) => 1 - 0.45 * wx.rain - 0.15 * wx.fog;
export const bikeFactor = (wx) => 1 - 0.8 * wx.rain - 0.3 * wx.fog;
// Schirm: je Person fest (aus der Nummer), bei stärkerem Regen mehr
export const hasUmbrella = (id, rain) => rain > 0.2 && hash01(id * 5 + 2) < rain * 0.85;
