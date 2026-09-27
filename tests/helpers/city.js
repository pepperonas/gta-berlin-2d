// Echte Karte (web/data/berlin/) für Tests: Kacheln werden synchron von der Platte geladen.
// realCity() hält das frühere Kerngebiet (Ortsteile Kreuzberg und Neukölln) fest geladen – einmal pro Testprozess;
// Welten laden ringsum wie im Spiel nach (city.focus).
import { readFileSync } from 'node:fs';
import { openCity } from '../../web/src/map.js';

const dir = new URL('../../web/data/berlin/', import.meta.url);
export const realIndex = () => JSON.parse(readFileSync(new URL('index.json', dir)));
export const realOverview = () => JSON.parse(readFileSync(new URL('overview.json', dir)));
export const tileLoader = () => (k) => JSON.parse(readFileSync(new URL(`tiles/${k}.json`, dir)));
export const openRealCity = () => openCity(realIndex(), tileLoader());

// Hüllrechteck von Ortsteilen (px)
export function districtBox(city, names) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const d of city.districts) if (names.includes(d.name)) for (const r of d.rings) for (let i = 0; i < r.length; i += 2) {
    x0 = Math.min(x0, r[i]); x1 = Math.max(x1, r[i]); y0 = Math.min(y0, r[i + 1]); y1 = Math.max(y1, r[i + 1]);
  }
  return [x0, y0, x1, y1];
}

let cached = null;
export function realCity() {
  if (cached) return cached;
  cached = openRealCity();
  cached.loadArea(...districtBox(cached, ['Kreuzberg', 'Neukölln']), { pin: true });
  return cached;
}
