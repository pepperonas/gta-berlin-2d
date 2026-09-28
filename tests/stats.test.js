// Statistik (stats.js), Speicher (statsdb.js) und Anbindung ans Spiel (game.js) und HUD (hud.js)
import test from 'node:test';
import assert from 'node:assert/strict';
import { createStats, normalizeStats, mergeStats, createTracker, trackStep, formatStat, accuracy, STAT_KEYS, STAT_SECTIONS } from '../web/src/stats.js';
import { memoryStatsStore, openStatsStore } from '../web/src/statsdb.js';
import { WEAPONS } from '../web/src/combat.js';
import { createGame, updateGame, applyStoredStats } from '../web/src/game.js';
import { memoryStorage, writeSave } from '../web/src/save.js';
import { idle } from './helpers/bot.js';
import { realCity } from './helpers/city.js';

const PISTOL = WEAPONS.find((w) => !w.melee && (w.pellets ?? 1) === 1).id;
const SHOTGUN = WEAPONS.find((w) => (w.pellets ?? 1) > 1)?.id;
const MELEE = WEAPONS.find((w) => w.melee).id;

// Kleinste Welt, die trackStep braucht
function fakeWorld() { return { player: { x: 0, y: 0, inCar: null, lvl: 0 }, cars: [], money: 0, playerCarId: null }; }
function sets() { return [createStats(), createStats()]; }

test('Grundform: alle Zähler 0, je Waffe Schüsse/Kugeln/Treffer/Tötungen; Abschnitte ohne doppelte Schlüssel', () => {
  const s = createStats();
  for (const k of STAT_KEYS) assert.equal(s[k], 0, k);
  assert.equal(new Set(STAT_KEYS).size, STAT_KEYS.length);
  assert.deepEqual(Object.keys(s.weapons), WEAPONS.map((w) => w.id));
  for (const w of Object.values(s.weapons)) assert.deepEqual(w, { shots: 0, bullets: 0, hits: 0, kills: 0 });
  for (const [, rows] of STAT_SECTIONS) for (const [, label, fmt] of rows) { assert.ok(label); assert.ok(['km', 'kmh', 'time', 'n', 'eur'].includes(fmt)); }
});

test('Einlesen: Unsinn, Minuswerte und fremde Schlüssel fallen weg, gültige Werte bleiben', () => {
  const raw = { kills: 7, shots: -3, hits: 'viel', kmTotal: NaN, crashes: Infinity, foo: 1, weapons: { [PISTOL]: { shots: 4, bullets: -1 }, laser: { shots: 9 } } };
  const s = normalizeStats(raw);
  assert.equal(s.kills, 7); assert.equal(s.shots, 0); assert.equal(s.hits, 0); assert.equal(s.kmTotal, 0); assert.equal(s.crashes, 0);
  assert.equal(s.foo, undefined); assert.equal(s.weapons.laser, undefined);
  assert.equal(s.weapons[PISTOL].shots, 4); assert.equal(s.weapons[PISTOL].bullets, 0);
  assert.deepEqual(normalizeStats(null), createStats()); assert.deepEqual(normalizeStats('x'), createStats());
});

test('Zusammenzählen: Summen addieren, Rekorde nehmen den Höchstwert', () => {
  const a = createStats(), b = createStats();
  a.kills = 2; b.kills = 3; a.topKmh = 120; b.topKmh = 90; a.weapons[PISTOL].hits = 1; b.weapons[PISTOL].hits = 4;
  const m = mergeStats(a, b);
  assert.equal(m.kills, 5); assert.equal(m.topKmh, 120); assert.equal(m.weapons[PISTOL].hits, 5);
  assert.deepEqual(mergeStats(a, null), normalizeStats(a));
  assert.equal(a.kills, 2, 'Eingaben bleiben unverändert');
});

test('Strecke: zu Fuß und im Auto getrennt, Sprünge (Teleport, Krankenhaus) zählen nicht', () => {
  const w = fakeWorld(), tr = createTracker(), S = sets();
  trackStep(S, tr, w, [], 1 / 60); // erste Messung legt nur den Startpunkt fest
  assert.equal(S[0].kmTotal, 0);
  for (let i = 0; i < 100; i++) { w.player.x += 10; trackStep(S, tr, w, [], 1 / 60); } // 100 m zu Fuß
  const car = { id: 7, x: 0, y: 0, vx: 500, vy: 0 };
  w.cars.push(car); w.player.inCar = 7;
  for (let i = 0; i < 200; i++) { w.player.x += 20; trackStep(S, tr, w, [], 1 / 60); } // 400 m im Auto
  w.player.x += 50000; trackStep(S, tr, w, [], 1 / 60); // Sprung
  for (const s of S) {
    assert.ok(Math.abs(s.kmFoot - 0.1) < 1e-9, `zu Fuß ${s.kmFoot}`);
    assert.ok(Math.abs(s.kmCar - 0.4) < 1e-9, `im Auto ${s.kmCar}`);
    assert.ok(Math.abs(s.kmTotal - 0.5) < 1e-9);
    assert.equal(s.carsEntered, 1, 'einmal eingestiegen');
    assert.ok(s.topKmh > 0); assert.ok(Math.abs(s.timeCar - 201 / 60) < 1e-9); assert.ok(Math.abs(s.timePlayed - 302 / 60) < 1e-9);
  }
  const top = S[0].topKmh; car.vx = 10; trackStep(S, tr, w, [], 1 / 60);
  assert.equal(S[0].topKmh, top, 'Rekord sinkt nicht');
});

test('Ereignisse: Schüsse und Kugeln je Waffe, Treffer, Tötungen nach Art; nur was der Spieler tut', () => {
  const w = fakeWorld(), tr = createTracker(), S = sets();
  const ev = [
    { type: 'shot', weapon: PISTOL, player: true, traces: [{}] },
    { type: 'shot', weapon: PISTOL, player: true, traces: [{}] },
    { type: 'shot', weapon: PISTOL, player: false, traces: [{}] }, // ein Passant schießt zurück
    { type: 'weapon-hit', weapon: PISTOL, player: true },
    { type: 'weapon-hit', weapon: PISTOL, player: false },
    { type: 'kill', weapon: PISTOL, player: true },
    { type: 'kill', weapon: MELEE, player: true },
    { type: 'kill', weapon: PISTOL, player: false },
    { type: 'swing', weapon: MELEE },
    { type: 'hit', player: true }, { type: 'hit', player: true, bike: true }, { type: 'hit', player: false },
    { type: 'wreck', player: true, carId: 3 },
    { type: 'carjack' }, { type: 'wasted' }, { type: 'respawn', fee: 250 },
    { type: 'mission-success' }, { type: 'mission-fail' }, { type: 'teleport' }, { type: 'cheat' },
  ];
  if (SHOTGUN) ev.push({ type: 'shot', weapon: SHOTGUN, player: true, traces: [{}, {}, {}, {}, {}, {}] });
  trackStep(S, tr, w, ev, 0);
  for (const s of S) {
    assert.equal(s.shots, SHOTGUN ? 3 : 2); assert.equal(s.bullets, SHOTGUN ? 8 : 2, 'Schrot: jede Kugel');
    assert.deepEqual(s.weapons[PISTOL], { shots: 2, bullets: 2, hits: 1, kills: 1 });
    if (SHOTGUN) assert.equal(s.weapons[SHOTGUN].bullets, 6);
    assert.equal(s.weapons[MELEE].shots, 1, 'Schläge'); assert.equal(s.weapons[MELEE].kills, 1);
    assert.equal(s.hits, 1); assert.equal(s.kills, 2); assert.equal(s.killsShot, 1); assert.equal(s.killsMelee, 1);
    assert.equal(s.pedsRunOver, 1); assert.equal(s.cyclistsHit, 1); assert.equal(s.carsDestroyed, 1);
    assert.equal(s.carjacks, 1); assert.equal(s.deaths, 1); assert.equal(s.hospitalFees, 250);
    assert.equal(s.missions, 1); assert.equal(s.missionsFailed, 1); assert.equal(s.teleports, 1); assert.equal(s.cheats, 1);
  }
});

test('Auto und Geld: eigene Unfälle, Poller, eigenes Wrack, Brücken; verdient zählt nur Zuwachs', () => {
  const w = fakeWorld(), tr = createTracker(), S = sets();
  w.cars.push({ id: 1, x: 0, y: 0, vx: 0, vy: 0, lvl: 0 }); w.player.inCar = 1; w.playerCarId = 1;
  trackStep(S, tr, w, [], 0);
  trackStep(S, tr, w, [{ type: 'crash', carId: 1, strength: 0.5 }, { type: 'crash', carId: 1, strength: 0.05 }, { type: 'crash', carId: 2, strength: 1 },
    { type: 'knock', carId: 1 }, { type: 'knock', carId: 2 }, { type: 'wreck', carId: 1 }], 0);
  assert.equal(S[0].crashes, 1, 'nur eigene, spürbare Unfälle'); assert.equal(S[0].bollards, 1); assert.equal(S[0].ownWrecks, 1);
  w.cars[0].lvl = 1; trackStep(S, tr, w, [], 0); trackStep(S, tr, w, [], 0);
  w.cars[0].lvl = 0; trackStep(S, tr, w, [], 0); w.cars[0].lvl = 2; trackStep(S, tr, w, [], 0);
  assert.equal(S[0].bridges, 2, 'je Auffahrt einmal');
  w.money = 300; trackStep(S, tr, w, [], 0);
  w.money = 100; trackStep(S, tr, w, [], 0); // Ausgabe
  w.money = 150; trackStep(S, tr, w, [], 0);
  assert.equal(S[0].moneyEarned, 350);
  w.money = 0; trackStep(S, tr, w, [{ type: 'respawn', fee: 150 }], 0);
  w.money = 40; trackStep(S, tr, w, [], 0);
  assert.equal(S[0].moneyEarned, 390);
});

test('Anzeige: Strecke, Tempo, Zeit, Geld, Quote', () => {
  assert.equal(formatStat(0.42, 'km'), '420 m'); assert.equal(formatStat(3.456, 'km'), '3,46 km'); assert.equal(formatStat(123.44, 'km'), '123,4 km');
  assert.equal(formatStat(87.6, 'kmh'), '88 km/h');
  assert.equal(formatStat(42, 'time'), '42 s'); assert.equal(formatStat(125, 'time'), '2 min 05 s'); assert.equal(formatStat(3 * 3600 + 7 * 60, 'time'), '3 h 07 min');
  assert.equal(formatStat(12345, 'eur'), '12.345 €'); assert.equal(formatStat(1234, 'n'), '1.234');
  assert.equal(accuracy({ shots: 10, bullets: 10, hits: 4 }), 40); assert.equal(accuracy({ shots: 5, bullets: 0, hits: 5 }), 100, 'Nahkampf: Schläge');
  assert.equal(accuracy({ shots: 0, bullets: 0, hits: 0 }), 0);
});

test('Speicher: Rundreise normalisiert; ohne IndexedDB oder bei Fehler bleibt ein Speicher im Arbeitsspeicher', async () => {
  const st = memoryStatsStore();
  assert.equal(await st.get('total'), null);
  const s = createStats(); s.kills = 3; s.weapons[PISTOL].bullets = 9;
  await st.put('total', s);
  s.kills = 99; // die Kopie im Speicher bleibt
  const back = await st.get('total');
  assert.equal(back.kills, 3); assert.equal(back.weapons[PISTOL].bullets, 9);
  await st.clear('total'); assert.equal(await st.get('total'), null);
  assert.equal((await openStatsStore(null)).kind, 'memory');
  const broken = { open() { const r = {}; setTimeout(() => { r.error = new Error('kaputt'); r.onerror?.(); }, 0); return r; } };
  const orig = console.warn; console.warn = () => {};
  try { assert.equal((await openStatsStore(broken)).kind, 'memory'); } finally { console.warn = orig; }
});

test('Speicher: IndexedDB-Weg legt die Tabelle an und liest/schreibt über Transaktionen', async () => {
  // kleine IndexedDB-Attrappe mit dem Ablauf der echten API (Ereignisse asynchron)
  const data = new Map(); let created = false;
  const store = { get: (k) => ({ result: data.get(k) }), put: (v, k) => { data.set(k, structuredClone(v)); return {}; }, delete: (k) => { data.delete(k); return {}; } };
  const db = { objectStoreNames: { contains: () => created }, createObjectStore: () => { created = true; },
    transaction: () => { const t = { objectStore: () => store }; setTimeout(() => t.oncomplete?.(), 0); return t; } };
  const idb = { open() { const r = { result: db }; setTimeout(() => { r.onupgradeneeded?.(); r.onsuccess?.(); }, 0); return r; } };
  const st = await openStatsStore(idb);
  assert.equal(st.kind, 'indexeddb'); assert.ok(created, 'Tabelle angelegt');
  const s = createStats(); s.missions = 2;
  await st.put('game', s);
  assert.equal((await st.get('game')).missions, 2);
  assert.equal(await st.get('total'), null);
});

test('Im Spiel: neues Spiel zählt neu, Gesamtstand läuft weiter; Datenbank-Antwort mischt nie ein altes Spiel in ein neues', () => {
  const city = realCity();
  const storage = memoryStorage();
  const g = createGame({ storage, city });
  const press = (patch = {}) => updateGame(g, { ...idle(), ...patch }, 1 / 60);
  press({ confirm: true });
  for (let i = 0; i < 60; i++) press();
  assert.ok(g.stats.game.timePlayed > 0.9 && g.stats.total.timePlayed > 0.9);
  assert.ok(g.statsDirty);
  // Datenbank antwortet erst jetzt, nach Beginn eines neuen Spiels
  const oldTotal = createStats(); oldTotal.kills = 40; oldTotal.timePlayed = 1000;
  const oldGame = createStats(); oldGame.kills = 12;
  applyStoredStats(g, oldTotal, oldGame);
  assert.equal(g.stats.total.kills, 40); assert.ok(g.stats.total.timePlayed > 1000.9);
  assert.equal(g.stats.game.kills, 0, 'neues Spiel bleibt sauber');
  // Fortsetzen: das gespeicherte Spiel bringt seine Statistik mit
  writeSave(storage, g.world);
  const h = createGame({ storage, city });
  applyStoredStats(h, oldTotal, oldGame); // Antwort vor dem Spielstart
  assert.equal(h.stats.savedGame.kills, 12);
  updateGame(h, { ...idle(), confirm: true }, 1 / 60); // Fortsetzen ist vorausgewählt
  assert.equal(h.screen, 'playing'); assert.equal(h.stats.game.kills, 12);
  // Antwort nach dem Fortsetzen: zusammenzählen
  const k = createGame({ storage, city });
  updateGame(k, { ...idle(), confirm: true }, 1 / 60);
  k.stats.game.kills = 1;
  applyStoredStats(k, null, oldGame);
  assert.equal(k.stats.game.kills, 13);
  // Statistik aus dem Titel und aus der Pause, zurück zum Aufrufer
  const m = createGame({ storage, city });
  while (m.titleMenu.items[m.titleMenu.index].id !== 'stats') updateGame(m, { ...idle(), menuDown: true }, 1 / 60);
  updateGame(m, { ...idle(), confirm: true }, 1 / 60); assert.equal(m.screen, 'stats');
  updateGame(m, { ...idle(), back: true }, 1 / 60); assert.equal(m.screen, 'title');
});

test('HUD: Statistikseite zeigt alle Abschnitte, die Werte und die Waffentabelle; Konsole zeichnet Vorschläge', async () => {
  const { Hud } = await import('../web/src/hud.js');
  const texts = [];
  const ctx = new Proxy({ canvas: { width: 1920, height: 1080 } }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'measureText') return (s) => ({ width: String(s).length * 8 });
      if (k === 'createLinearGradient' || k === 'createRadialGradient' || k === 'createPattern') return () => ({ addColorStop() {} });
      if (k === 'fillText') return (s, x, y) => { if (![x, y].every(Number.isFinite)) throw new Error('NaN'); texts.push(String(s)); };
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const hud = new Hud(ctx);
  hud.begin(1920, 1080);
  const game = createStats(), total = createStats();
  game.kills = 3; total.kills = 1234; total.kmTotal = 12.5; total.weapons[PISTOL] = { shots: 20, bullets: 20, hits: 5, kills: 2 };
  hud.drawStats({ game, total }, true);
  for (const [title, rows] of STAT_SECTIONS) {
    assert.ok(texts.some((t) => t.toUpperCase() === title.toUpperCase()), `Abschnitt ${title}`);
    for (const [, label] of rows) assert.ok(texts.includes(label), `Zeile ${label}`);
  }
  assert.ok(texts.includes('1.234') && texts.includes('12,5 km') && texts.includes('3'), texts.join(' | '));
  assert.ok(texts.includes('0 % / 25 %'), 'Trefferquote der Pistole (Spiel / gesamt)');
  assert.ok(hud.counts.statsBottom <= 720, `passt in den 720er-Rahmen (${hud.counts.statsBottom})`);
  texts.length = 0;
  hud.begin(1920, 1080);
  hud.drawConsole({ open: true, text: 'wetter sch', sel: 1, log: [{ text: '> zeit 12', ok: true, t: 0 }], sugg: { items: [{ label: 'schnee', hint: 'Schnee' }, { label: 'schneesturm', hint: 'Schneesturm' }], ghost: 'nee' } }, 1);
  assert.equal(hud.counts.suggestions, 2);
  assert.ok(texts.includes('wetter sch') && texts.includes('schneesturm') && texts.includes('> zeit 12'));
});
