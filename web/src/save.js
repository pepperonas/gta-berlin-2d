// Spielstand: ein Speicherplatz in localStorage (in der Xbox-Hülle: WebView2-Profil im App-Datenordner).
// storage ist injizierbar (Tests nutzen eine Map-Attrappe).
import { playerCar } from './world.js';
import { WORLD_W, WORLD_H } from './config.js';

export const SAVE_KEY = 'gta-berlin.save';
export const SAVE_VERSION = 1;

export function makeSave(w, now = Date.now()) {
  const car = w.cars.find((c) => c.id === w.playerCarId);
  const pos = playerCar(w) ?? w.player;
  return {
    version: SAVE_VERSION, savedAt: now,
    money: w.money, completed: w.completed, bestTime: w.bestTime,
    player: { x: Math.round(pos.x), y: Math.round(pos.y) },
    car: car && !car.wrecked ? { x: Math.round(car.x), y: Math.round(car.y), angle: car.angle, health: car.health } : null,
  };
}

const num = (v, lo, hi) => typeof v === 'number' && Number.isFinite(v) && v >= lo && v <= hi;

export function validateSave(s) {
  if (!s || typeof s !== 'object' || s.version !== SAVE_VERSION) return null;
  if (!num(s.money, 0, 1e9) || !num(s.completed, 0, 1e6)) return null;
  if (s.bestTime !== null && !num(s.bestTime, 0, 1e5)) return null;
  if (!s.player || !num(s.player.x, 0, WORLD_W) || !num(s.player.y, 0, WORLD_H)) return null;
  let car = null;
  if (s.car && num(s.car.x, 0, WORLD_W) && num(s.car.y, 0, WORLD_H) && num(s.car.angle, -100, 100) && num(s.car.health, 1, 100)) car = s.car;
  return { version: s.version, savedAt: s.savedAt ?? 0, money: s.money, completed: s.completed, bestTime: s.bestTime, player: s.player, car };
}

export function writeSave(storage, w) {
  const data = makeSave(w);
  try { storage.setItem(SAVE_KEY, JSON.stringify(data)); return true; } catch { return false; }
}

export function readSave(storage) {
  try { return validateSave(JSON.parse(storage.getItem(SAVE_KEY))); } catch { return null; }
}

// Übernimmt einen Spielstand in eine frisch erzeugte Welt (Spieler steht zu Fuß neben seinem Auto).
export function applySave(w, s) {
  w.money = s.money; w.completed = s.completed; w.bestTime = s.bestTime;
  const car = w.cars.find((c) => c.id === w.playerCarId);
  if (s.car && car) {
    Object.assign(car, { x: s.car.x, y: s.car.y, angle: s.car.angle, health: s.car.health, vx: 0, vy: 0 });
    w.player.x = s.car.x - Math.sin(s.car.angle) * 24;
    w.player.y = s.car.y + Math.cos(s.car.angle) * 24;
  } else { w.player.x = s.player.x; w.player.y = s.player.y; }
  w.camera.x = w.player.x; w.camera.y = w.player.y;
}

export function memoryStorage() {
  const m = new Map();
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) };
}
