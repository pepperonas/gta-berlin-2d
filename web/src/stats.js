// Statistik (rein, ohne DOM): Zähler je Spiel und über alle Spiele. Gespeist aus den Ereignissen der Simulation und
// einer Messung je Schritt (Strecke, Zeit, Tempo). Gespeichert wird in statsdb.js (IndexedDB) – hier nur Rechnen.
import { WEAPONS } from './combat.js';
import { SPEED_TO_KMH } from './config.js';

const PX_PER_KM = 10000; // 10 px = 1 m

// Anzeige: Abschnitte mit Einträgen [Schlüssel, Beschriftung, Format]
export const STAT_SECTIONS = [
  ['Unterwegs', [
    ['kmTotal', 'Strecke gesamt', 'km'], ['kmCar', 'davon im Auto', 'km'], ['kmFoot', 'davon zu Fuß', 'km'],
    ['topKmh', 'Höchstgeschwindigkeit', 'kmh'], ['timePlayed', 'Spielzeit', 'time'], ['timeCar', 'Zeit im Auto', 'time'],
    ['bridges', 'Brücken befahren', 'n'], ['teleports', 'Teleports', 'n'],
  ]],
  ['Verkehr', [
    ['pedsRunOver', 'Menschen überfahren', 'n'], ['cyclistsHit', 'Radfahrer umgefahren', 'n'], ['crashes', 'Unfälle', 'n'],
    ['bollards', 'Poller umgefahren', 'n'], ['carjacks', 'Autos geklaut', 'n'], ['carsEntered', 'Autos gefahren', 'n'],
    ['ownWrecks', 'eigene Autos Schrott', 'n'],
  ]],
  ['Kampf', [
    ['kills', 'Menschen getötet', 'n'], ['killsShot', 'davon erschossen', 'n'], ['killsMelee', 'davon im Nahkampf', 'n'],
    ['shots', 'Schüsse', 'n'], ['bullets', 'Kugeln', 'n'], ['hits', 'Treffer', 'n'], ['carsDestroyed', 'Autos zerstört', 'n'],
    ['deaths', 'selbst umgehauen', 'n'],
  ]],
  ['Aufträge', [
    ['missions', 'Aufträge erledigt', 'n'], ['missionsFailed', 'Aufträge verpatzt', 'n'], ['moneyEarned', 'Geld verdient', 'eur'],
    ['hospitalFees', 'Krankenhauskosten', 'eur'], ['cheats', 'Konsolenbefehle (Cheats)', 'n'],
  ]],
];
export const STAT_KEYS = STAT_SECTIONS.flatMap(([, rows]) => rows.map(([k]) => k));
const MAX_KEYS = new Set(['topKmh']); // Rekorde: Höchstwert, nicht Summe

export function createStats() {
  const s = { v: 1 };
  for (const k of STAT_KEYS) s[k] = 0;
  s.weapons = Object.fromEntries(WEAPONS.map((w) => [w.id, { shots: 0, bullets: 0, hits: 0, kills: 0 }]));
  return s;
}

// Alten/fremden Stand auf die aktuelle Form bringen (neue Zähler 0, unbekannte weg, Zahlen geprüft)
export function normalizeStats(raw) {
  const s = createStats();
  if (!raw || typeof raw !== 'object') return s;
  for (const k of STAT_KEYS) if (Number.isFinite(raw[k]) && raw[k] >= 0) s[k] = raw[k];
  for (const id of Object.keys(s.weapons)) {
    const r = raw.weapons?.[id];
    if (r) for (const f of ['shots', 'bullets', 'hits', 'kills']) if (Number.isFinite(r[f]) && r[f] >= 0) s.weapons[id][f] = r[f];
  }
  return s;
}

const add = (sets, k, n = 1) => { for (const s of sets) s[k] = MAX_KEYS.has(k) ? Math.max(s[k], n) : s[k] + n; };
const addW = (sets, id, f, n = 1) => { for (const s of sets) if (s.weapons[id]) s.weapons[id][f] += n; };

// Tracker je Welt: merkt sich die letzte Position/Lage für Strecke und Übergänge
export function createTracker() { return { x: null, y: null, inCar: null, lvl: 0, money: null }; }

// Ein Simulationsschritt: Ereignisse und Bewegung der Spielfigur in beide Stände (sets = [game, total]) buchen
export function trackStep(sets, tr, world, events, dt) {
  const p = world.player, car = p.inCar ? world.cars.find((c) => c.id === p.inCar) : null;
  add(sets, 'timePlayed', dt);
  if (car) add(sets, 'timeCar', dt);
  // Strecke: Sprünge (Teleport, Respawn) zählen nicht
  if (tr.x !== null) {
    const d = Math.hypot(p.x - tr.x, p.y - tr.y);
    if (d < 600) {
      add(sets, 'kmTotal', d / PX_PER_KM);
      add(sets, car ? 'kmCar' : 'kmFoot', d / PX_PER_KM);
    }
  }
  if (car) add(sets, 'topKmh', Math.hypot(car.vx, car.vy) * SPEED_TO_KMH);
  if (p.inCar && p.inCar !== tr.inCar) add(sets, 'carsEntered');
  const lvl = (car ?? p).lvl ?? 0;
  if (lvl >= 1 && !(tr.lvl >= 1)) add(sets, 'bridges');
  // Geld: jedes Plus aus Aufträgen (Ausgaben wie das Krankenhaus zählen extra)
  if (tr.money !== null && world.money > tr.money) add(sets, 'moneyEarned', world.money - tr.money);
  Object.assign(tr, { x: p.x, y: p.y, inCar: p.inCar, lvl, money: world.money });
  for (const e of events) {
    switch (e.type) {
      case 'hit': if (e.player) add(sets, e.bike ? 'cyclistsHit' : 'pedsRunOver'); break;
      case 'kill':
        if (!e.player) break;
        add(sets, 'kills');
        if (e.weapon) { addW(sets, e.weapon, 'kills'); add(sets, WEAPONS.find((w) => w.id === e.weapon)?.melee ? 'killsMelee' : 'killsShot'); }
        break;
      case 'shot': if (e.player !== false) { add(sets, 'shots'); add(sets, 'bullets', e.traces?.length ?? 1); addW(sets, e.weapon, 'shots'); addW(sets, e.weapon, 'bullets', e.traces?.length ?? 1); } break;
      case 'swing': if (e.weapon) addW(sets, e.weapon, 'shots'); break; // Nahkampf: Schläge
      case 'weapon-hit': if (e.player) { add(sets, 'hits'); addW(sets, e.weapon, 'hits'); } break;
      case 'wreck': if (e.player) add(sets, 'carsDestroyed'); else if (e.carId === world.playerCarId || e.carId === p.inCar) add(sets, 'ownWrecks'); break;
      case 'crash': if (e.carId !== undefined && e.carId === p.inCar && e.strength > 0.15) add(sets, 'crashes'); break;
      case 'knock': if (e.carId === p.inCar) add(sets, 'bollards'); break;
      case 'carjack': add(sets, 'carjacks'); break;
      case 'wasted': add(sets, 'deaths'); break;
      case 'respawn': add(sets, 'hospitalFees', e.fee ?? 0); if (tr) tr.money = world.money; break;
      case 'mission-success': add(sets, 'missions'); break;
      case 'mission-fail': add(sets, 'missionsFailed'); break;
      case 'teleport': add(sets, 'teleports'); break;
      case 'cheat': add(sets, 'cheats'); break;
    }
  }
}

// Wert für die Anzeige
export function formatStat(v, fmt) {
  switch (fmt) {
    case 'km': return v < 1 ? `${Math.round(v * 1000)} m` : `${v.toFixed(v < 10 ? 2 : 1).replace('.', ',')} km`;
    case 'kmh': return `${Math.round(v)} km/h`;
    case 'time': { const h = Math.floor(v / 3600), m = Math.floor(v / 60) % 60, s = Math.floor(v) % 60; return h ? `${h} h ${String(m).padStart(2, '0')} min` : m ? `${m} min ${String(s).padStart(2, '0')} s` : `${s} s`; }
    case 'eur': return `${Math.round(v).toLocaleString('de-DE')} €`;
    default: return Math.round(v).toLocaleString('de-DE');
  }
}

// Trefferquote einer Waffe in Prozent (Kugeln → Treffer), Nahkampf: Schläge → Treffer
export function accuracy(w) { const n = w.bullets || w.shots; return n ? Math.round((w.hits / n) * 100) : 0; }

// Zwei Stände zusammenzählen (Rekorde: Höchstwert) – wenn die Datenbank erst nach dem Spielstart antwortet
export function mergeStats(a, b) {
  const s = normalizeStats(a);
  if (!b) return s;
  for (const k of STAT_KEYS) s[k] = MAX_KEYS.has(k) ? Math.max(s[k], b[k] ?? 0) : s[k] + (b[k] ?? 0);
  for (const id of Object.keys(s.weapons)) for (const f of ['shots', 'bullets', 'hits', 'kills']) s.weapons[id][f] += b.weapons?.[id]?.[f] ?? 0;
  return s;
}
