// Lädt die echte Karte einmal pro Testprozess (Dekodieren dauert ~0,5 s).
import { readFileSync } from 'node:fs';
import { decodeCity } from '../../web/src/map.js';

let cached = null;
export function realCityJson() { return JSON.parse(readFileSync(new URL('../../web/data/city.json', import.meta.url))); }
export function realCity() { return (cached ??= decodeCity(realCityJson())); }
