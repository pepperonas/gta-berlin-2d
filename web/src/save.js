// Spielstand: ein Speicherplatz in localStorage (in der Xbox-Hülle: WebView2-Profil im App-Datenordner).
// storage ist injizierbar (Tests nutzen eine Map-Attrappe).
import { playerCar, resetPopulation } from './world.js';
import { insideBorder, inBuilding } from './map.js';

const MAX_XY = 1e7; // grobe Plausibilität; ob die Position in der Stadt liegt, prüft applySave

export const SAVE_KEY = 'gta-berlin.save';
export const SAVE_VERSION = 2; // 2 = echte Karte (Kreuzberg + Nord-Neukölln); Stände der Rasterstadt werden verworfen

export function makeSave(w, now = Date.now()) {
  const car = w.cars.find((c) => c.id === w.playerCarId);
  const pos = w.player.ride ? w.player.ride.lastStop : playerCar(w) ?? w.player; // Fahrgast: an der letzten Haltestelle zu Fuß
  return {
    version: SAVE_VERSION, savedAt: now,
    money: w.money, completed: w.completed, bestTime: w.bestTime, clock: Math.round(w.clock), day: w.day, dayCount: w.dayCount ?? 0,
    wet: Math.round((w.wet ?? 0) * 100) / 100, snow: Math.round((w.snow ?? 0) * 100) / 100,
    player: { x: Math.round(pos.x), y: Math.round(pos.y) },
    car: car && !car.wrecked ? { x: Math.round(car.x), y: Math.round(car.y), angle: car.angle, health: car.health } : null,
  };
}

const num = (v, lo, hi) => typeof v === 'number' && Number.isFinite(v) && v >= lo && v <= hi;

export function validateSave(s) {
  if (!s || typeof s !== 'object' || s.version !== SAVE_VERSION) return null;
  if (!num(s.money, 0, 1e9) || !num(s.completed, 0, 1e6)) return null;
  if (s.bestTime !== null && !num(s.bestTime, 0, 1e5)) return null;
  if (!s.player || !num(s.player.x, 0, MAX_XY) || !num(s.player.y, 0, MAX_XY)) return null;
  let car = null;
  if (s.car && num(s.car.x, 0, MAX_XY) && num(s.car.y, 0, MAX_XY) && num(s.car.angle, -100, 100) && num(s.car.health, 1, 100)) car = s.car;
  const clock = num(s.clock, 0, 1440) ? s.clock : null; // ältere Stände ohne Uhr: Uhr bleibt beim Spielstart
  const day = Number.isInteger(s.day) && s.day >= 0 && s.day < 7 ? s.day : null;
  const dayCount = Number.isInteger(s.dayCount) && s.dayCount >= 0 ? s.dayCount : null; // Tagnummer fürs Wetter
  const wet = num(s.wet, 0, 1) ? s.wet : null, snow = num(s.snow, 0, 1) ? s.snow : null; // Boden: nass, Schneedecke
  return { version: s.version, savedAt: s.savedAt ?? 0, money: s.money, completed: s.completed, bestTime: s.bestTime, clock, day, dayCount, wet, snow, player: s.player, car };
}

export function writeSave(storage, w) {
  const data = makeSave(w);
  try { storage.setItem(SAVE_KEY, JSON.stringify(data)); return true; } catch { return false; }
}

export function readSave(storage) {
  try { return validateSave(JSON.parse(storage.getItem(SAVE_KEY))); } catch { return null; }
}

// Übernimmt einen Spielstand in eine frisch erzeugte Welt (Spieler steht zu Fuß neben seinem Auto).
// Der gespeicherte Ort kann in einem noch nicht geladenen Stadtteil liegen: dann gilt er vorläufig und wird geprüft,
// sobald die Kacheln dort da sind (resolveSave, aufgerufen von updateWorld).
export function applySave(w, s) {
  w.money = s.money; w.completed = s.completed; w.bestTime = s.bestTime;
  if (s.clock !== null && s.clock !== undefined) w.clock = s.clock;
  if (s.day !== null && s.day !== undefined) w.day = s.day;
  if (s.dayCount !== null && s.dayCount !== undefined) w.dayCount = s.dayCount;
  if (s.wet != null) w.wet = s.wet;
  if (s.snow != null) w.snow = s.snow;
  w.pendingSave = s;
  const p = s.car ?? s.player;
  if (p && insideBorder(w.city, p.x, p.y)) { w.camera.x = p.x; w.camera.y = p.y; }
  resolveSave(w);
}

export function resolveSave(w) {
  const s = w.pendingSave;
  if (!s) return true;
  if (!w.city.focus(w.focusKey, w.camera.x, w.camera.y)) return false;
  w.pendingSave = null;
  const car = w.cars.find((c) => c.id === w.playerCarId);
  const valid = (p) => p && insideBorder(w.city, p.x, p.y) && !inBuilding(w.city, p.x, p.y);
  if (s.car && car && valid(s.car)) {
    Object.assign(car, { x: s.car.x, y: s.car.y, angle: s.car.angle, health: s.car.health, vx: 0, vy: 0 });
    for (const side of [1, -1, 0]) { // neben dem Auto, aber nicht in einem Haus
      w.player.x = s.car.x - Math.sin(s.car.angle) * 24 * side;
      w.player.y = s.car.y + Math.cos(s.car.angle) * 24 * side;
      if (!inBuilding(w.city, w.player.x, w.player.y)) break;
    }
  } else if (valid(s.player)) { w.player.x = s.player.x; w.player.y = s.player.y; }
  w.camera.x = w.player.x; w.camera.y = w.player.y;
  resetPopulation(w); // Verkehr am (neuen) Ort aufbauen
  return true;
}

export function memoryStorage() {
  const m = new Map();
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) };
}
