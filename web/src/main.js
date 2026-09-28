// Browser-Einstieg: Canvas, Hauptschleife (fester 60-Hz-Takt), Eingabequellen, Xbox-Hüllen-Brücke.
import { DT } from './config.js';
import { createGame, updateGame, setCity, requestTeleport, confirmTeleport } from './game.js';
import { openCity } from './map.js';
import { createWorld, updateWorld, playerCar, speedOf, resetPopulation } from './world.js';
import { parseClock } from './daylight.js';
import { InputState, readKeys, readPad, fromHostReading, merge } from './input.js';
import { Renderer } from './render.js';
import { Hud, BASE } from './hud.js';
import { Sound } from './audio.js';
import { ambienceAt, bellStrikes } from './ambience.js';
import { prepareTransit } from './transit.js';
import { loadSprites } from './assets.js';
import { idleInput } from './idle.js';
import { cursorCss, cursorKind } from './cursor.js';

const canvas = document.getElementById('game');
const ctx = canvas.getContext('2d', { alpha: false });
const renderer = new Renderer(ctx);
const hud = new Hud(ctx);
const sound = new Sound();
const input = new InputState();
const keys = new Set();
const latched = new Set(); // kurze Drücke bis zum nächsten Simulationsschritt halten

// --- Xbox-Hülle (WebView2): nativer Controller + Beenden ---------------------------------
const host = globalThis.chrome?.webview ?? null;
let hostPads = [], hostPadTime = 0;
if (host) {
  host.addEventListener('message', (e) => {
    const msg = e.data;
    if (msg?.type === 'gamepad') { hostPads = msg.pads ?? []; hostPadTime = performance.now(); }
  });
  host.postMessage({ type: 'ready' });
}

let storage;
try { storage = window.localStorage; storage.getItem('probe'); } catch { storage = memoryFallback(); }
const game = createGame({ storage, canQuit: !!host });
globalThis.__gta = game; // für Tests/Debug in der Konsole
globalThis.__hud = hud;
globalThis.__renderer = renderer; // Zeichenzeit/Qualitätsstufe (renderer.stats.ms, renderer.quality)

// Karte laden: web/data/berlin/index.json (Grenzen, Orte, ~1 MB), die Kacheln (640 × 640 m) holt die Stadt selbst
// nach, sobald eine Kamera in ihre Nähe kommt; den Stadtplan (overview.json) erst im Hintergrund.
// Bis der Index da ist, zeigt der Titel „Lade …“. Titelbildschirm-Hintergrund: eine laufende Demo-Welt mit Kamerafahrt.
let demo = null;
// ?uhr=21:30 stellt die Spieluhr jeder neuen Welt (Sichtprüfung von Tag, Dämmerung, Nacht)
const forcedClock = parseClock(new URLSearchParams(location.search).get('uhr'));
let clockSetFor = null;
// ?wetter=regen|nebel|sonnig|wolkig|bedeckt legt das Wetter fest (Sichtprüfung)
const WX_PARAM = { sonnig: 'clear', wolkig: 'cloudy', bedeckt: 'overcast', regen: 'rain', nebel: 'fog' };
const forcedWeather = WX_PARAM[new URLSearchParams(location.search).get('wetter')] ?? null;
const getJson = (url) => fetch(url).then((r) => { if (!r.ok) throw new Error(`${url}: HTTP ${r.status}`); return r.json(); });
getJson('data/berlin/index.json').then((index) => {
  const city = openCity(index, (key) => getJson(`data/berlin/tiles/${key}.json`));
  setCity(game, city);
  globalThis.__city = city;
  demo = createWorld({ city, seed: 1989, cars: 14, pedestrians: 30 });
  demo.mission.state = 'idle';
  demo.clock = forcedClock ?? 19 * 60 + 30; // Titel: Abendstimmung
  demo.forceWeather = forcedWeather;
  getJson('data/berlin/overview.json').then((ov) => { city.overview = ov; hud.overview = null; }).catch((err) => console.error(err));
  // Fahrplan (VBB): ohne ihn läuft das Spiel einfach ohne Busse und Bahnen
  getJson('data/berlin/transit.json').then((tj) => { city.transit = prepareTransit(tj); city.attribution += ` · ${tj.attribution}`; }).catch((err) => console.warn('Fahrplan nicht geladen:', err.message));
}).catch((err) => { game.loadError = String(err.message ?? err); console.error(err); });

let manifest = {};
fetch('assets/manifest.json').then((r) => r.json()).then(async (m) => { manifest = m; sound.setManifest(m); await loadSprites(m); }).catch(() => {});

addEventListener('keydown', (e) => {
  keys.add(e.code); latched.add(e.code); input.lastDevice = 'keyboard'; sound.unlock();
  if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Space', 'Tab'].includes(e.code)) e.preventDefault();
});
addEventListener('keyup', (e) => keys.delete(e.code));
addEventListener('blur', () => keys.clear());
addEventListener('pointerdown', () => sound.unlock());

// Maus: Menüs (zeigen = auswählen, klicken = bestätigen), Tastenhinweise (A/B) und der Teleport-Dialog sind anklickbar;
// auf dem Stadtplan zoomt das Mausrad (um den Zeiger), Ziehen verschiebt, ein Klick wählt ein Teleport-Ziel.
// Die anklickbaren Flächen legt der HUD beim Zeichnen in hud.hits ab; Menü-Klicks gehen als abstrakte Eingabe
// (menuHover/menuPick) durch dieselbe Spiellogik wie Controller und Tastatur.
const toHud = (e) => { const r = canvas.getBoundingClientRect(); return [(e.clientX - r.left) * (canvas.width / r.width) / hud.s, (e.clientY - r.top) * (canvas.height / r.height) / hud.s]; };
const hitAt = (vx, vy) => { const hs = hud.hits ?? []; for (let i = hs.length - 1; i >= 0; i--) { const b = hs[i]; if (vx >= b.x && vx <= b.x + b.w && vy >= b.y && vy <= b.y + b.h) return b; } return null; };
const pointer = { vx: -1, vy: -1, moved: -1e9, hover: null, pick: null, key: null, drag: null, cursor: '', fire: false, firePressed: false, wheel: 0 };
const playingOnFoot = () => game.screen === 'playing' && game.world && !game.world.player.inCar && !game.showBigMap && !game.teleport && !game.resultMenu;
addEventListener('blur', () => { pointer.fire = false; });
const activeMenu = () => (game.screen === 'title' ? game.titleMenu : game.screen === 'paused' ? game.pauseMenu : game.screen === 'playing' ? game.resultMenu : null);
canvas.addEventListener('pointermove', (e) => {
  if (!hud.s) return;
  const [vx, vy] = toHud(e);
  pointer.vx = vx; pointer.vy = vy; pointer.moved = performance.now();
  input.lastDevice = 'keyboard';
  const d = pointer.drag;
  if (d) { d.moved += Math.hypot(vx - d.vx, vy - d.vy); hud.panBigMap(vx - d.vx, vy - d.vy); d.vx = vx; d.vy = vy; return; }
  const h = hitAt(vx, vy);
  if (h?.kind === 'menu') pointer.hover = h;
});
canvas.addEventListener('pointerleave', () => { pointer.vx = pointer.vy = -1; });
canvas.addEventListener('pointerdown', (e) => {
  if (!hud.s || e.button !== 0) return;
  const [vx, vy] = toHud(e);
  const h = hitAt(vx, vy);
  if (!h) { // im Spiel zu Fuß: linke Maustaste feuert (Ziel = Mauszeiger)
    if (playingOnFoot()) { pointer.fire = true; pointer.firePressed = true; }
    return;
  }
  if (h.kind === 'dialog') { confirmTeleport(game, h.yes); sound.play(h.yes ? 'ui' : 'ui-back'); }
  else if (h.kind === 'map') { pointer.drag = { vx, vy, moved: 0 }; canvas.setPointerCapture(e.pointerId); }
  else if (h.kind === 'menu') pointer.pick = h;
  else if (h.kind === 'key') pointer.key = h.key;
});
canvas.addEventListener('pointerup', () => {
  pointer.fire = false;
  const d = pointer.drag; pointer.drag = null;
  if (!d || d.moved > 6 || !game.showBigMap || game.teleport) return; // gezogen, nicht geklickt
  const m = hud.bigMap;
  if (requestTeleport(game, (d.vx - m.ox) / m.f, (d.vy - m.oy) / m.f)) sound.play('ui');
});
canvas.addEventListener('wheel', (e) => {
  if (playingOnFoot()) { e.preventDefault(); pointer.wheel = Math.sign(e.deltaY); return; } // Waffe wechseln
  if (!game.showBigMap || !hud.s) return;
  e.preventDefault();
  const [vx, vy] = toHud(e);
  hud.zoomBigMap(Math.exp(-e.deltaY * 0.0015), vx, vy);
}, { passive: false });

// Mauseingaben in den nächsten Simulationsschritt übernehmen (nur für das gerade aktive Menü).
function applyPointer(inp) {
  const menu = activeMenu();
  if (pointer.hover) { if (pointer.hover.menu === menu) inp.menuHover = pointer.hover.i; pointer.hover = null; }
  if (pointer.pick) { if (pointer.pick.menu === menu) inp.menuPick = pointer.pick.i; pointer.pick = null; }
  // Kampf mit der Maus: zielen auf den Zeiger (solange die Maus zuletzt benutzt wurde), linke Taste, Mausrad
  if (playingOnFoot()) {
    inp.fire = inp.fire || pointer.fire;
    inp.firePressed = inp.firePressed || pointer.firePressed;
    if (pointer.wheel > 0) inp.weaponNext = true; else if (pointer.wheel < 0) inp.weaponPrev = true;
    const cam = game.world.camera, s = (game.worldScale ?? 1) * cam.zoom;
    if (input.lastDevice === 'keyboard' && pointer.vx >= 0 && hud.s && s) {
      inp.aimWorld = { x: cam.x + (pointer.vx * hud.s - W / 2) / s, y: cam.y + (pointer.vy * hud.s - H / 2) / s };
    }
  }
  pointer.firePressed = false; pointer.wheel = 0;
  if (pointer.key === 'A') inp.confirm = true;
  if (pointer.key === 'B') inp.back = true;
  pointer.key = null;
}

// Zeiger im Stil des Spiels; beim Fahren/Laufen verschwindet er, wenn die Maus 2 s ruht.
function updateCursor() {
  const playing = game.screen === 'playing' && !game.showBigMap && !game.teleport && !game.resultMenu;
  const kind = cursorKind({ hit: pointer.vx >= 0 ? hitAt(pointer.vx, pointer.vy) : null, dragging: !!pointer.drag, playing, aiming: playing && playingOnFoot(), idle: (performance.now() - pointer.moved) / 1000 });
  if (kind !== pointer.cursor) { pointer.cursor = kind; canvas.style.cursor = cursorCss(kind); }
}

let W = 0, H = 0, dpr = 1;
function resize() {
  dpr = Math.min(window.devicePixelRatio || 1, 2);
  const cw = window.innerWidth, ch = window.innerHeight;
  // Höchstens 1920×1080 Pixel rendern (Xbox-Ausgabe, Leistung).
  const k = Math.min(dpr, 1920 / cw, 1080 / ch) || 1;
  W = canvas.width = Math.round(cw * k); H = canvas.height = Math.round(ch * k);
  canvas.style.width = cw + 'px'; canvas.style.height = ch + 'px';
}
addEventListener('resize', resize); resize();

function readRaw() {
  let raw = readKeys(latched.size ? new Set([...keys, ...latched]) : keys);
  latched.clear();
  const pads = [];
  if (host && performance.now() - hostPadTime < 1000) for (const r of hostPads) pads.push(fromHostReading(r));
  const web = navigator.getGamepads ? navigator.getGamepads() : [];
  for (const gp of web) if (gp && gp.connected) pads.push(gp);
  for (const gp of pads) {
    const r = readPad(gp);
    const active = r.a || r.b || r.x || r.y || r.menu || r.up || r.down || Math.abs(r.lx) > 0.3 || Math.abs(r.ly) > 0.3 || r.rt > 0.2 || r.lt > 0.2;
    if (active) { input.lastDevice = 'gamepad'; sound.unlock(); }
    raw = merge(raw, r);
  }
  return raw;
}

let last = performance.now(), acc = 0, hintT = 0;
function frame(now) {
  const elapsed = Math.min(0.1, (now - last) / 1000); last = now; acc += elapsed;
  while (acc >= DT) {
    acc -= DT;
    const inp = input.frame(readRaw(), DT);
    applyPointer(inp);
    const events = updateGame(game, inp, DT);
    if ((forcedClock !== null || forcedWeather) && game.world && game.world !== clockSetFor) { if (forcedClock !== null) game.world.clock = forcedClock; game.world.forceWeather = forcedWeather; if (forcedWeather === 'rain') game.world.wet = 1; clockSetFor = game.world; resetPopulation(game.world); }
    for (const e of events) playEvent(e);
    if (game.world) {
      renderer.handleEvents(events);
      renderer.update(game.world, DT);
      hintT = game.world.player.inCar ? hintT + DT : 0;
    } else {
      updateDemo(DT);
    }
    if (game.quitRequested && host) { host.postMessage({ type: 'quit' }); game.quitRequested = false; }
  }
  draw();
  requestAnimationFrame(frame);
}

let demoT = 0;
function updateDemo(dt) {
  if (!demo) return;
  demoT += dt;
  // Kamerafahrt: langsame Schleife um den Späti (Wrangelkiez) – vor dem Weltschritt, damit Verkehr mitwandert.
  const c = demo.city.places.giver;
  demo.camera.x = c.x + Math.cos(demoT * 0.05) * 2500; demo.camera.y = c.y + Math.sin(demoT * 0.07) * 1800; demo.camera.zoom = 0.8;
  const cam = { ...demo.camera };
  updateWorld(demo, idleInput, dt);
  Object.assign(demo.camera, cam);
  renderer.update(demo, dt);
}

function draw() {
  // Welt: immer mindestens den 16:9-Ausschnitt zeigen (wie das HUD), mehr Platz zeigt mehr Stadt
  const worldScale = Math.min(W / (BASE.w * 600 / BASE.h), H / 600);
  hud.device = input.lastDevice;
  if (!game.world) {
    if (demo) renderer.draw(demo, W, H, worldScale, false);
    else { ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.fillStyle = '#15171c'; ctx.fillRect(0, 0, W, H); }
    hud.begin(W, H);
    if (game.screen === 'controls') hud.drawControls(); else hud.drawTitle(game);
    if (!sound.ready && !host) hud.text('Taste drücken für Ton', hud.vw / 2, hud.vh - hud.m.y - 40, { size: 15, align: 'center', color: '#aaa', weight: 500 });
  } else {
    renderer.draw(game.world, W, H, worldScale);
    hud.begin(W, H);
    game.worldScale = worldScale; game.hintT = hintT;
    if (game.screen === 'playing') {
      hud.drawGameplay(game.world, game);
      if (game.showBigMap) hud.drawBigMap(game.world); else hud.mapView = null;
      if (game.teleport) hud.drawTeleportDialog(game.teleport);
      if (game.resultMenu) hud.drawResult(game);
    } else if (game.screen === 'paused') hud.drawPause(game);
    else if (game.screen === 'controls') hud.drawControls();
  }
  hud.toast(game.toast);
  updateCursor();
  const car = game.world && game.screen === 'playing' ? playerCar(game.world) : null;
  sound.setEngine(!!car && !car.wrecked, car ? Math.min(1, speedOf(car) / 330) : 0, car ? car.controls.throttle : 0);
  // Umgebungsklang: Mischung viermal je Sekunde neu, Glocke beim Überschreiten der vollen Stunde
  const w = game.world, live = w && (game.screen === 'playing' || game.screen === 'title');
  const now = performance.now();
  if (live && sound.ready && now - ambT > 250) {
    ambT = now;
    sound.setAmbience(ambienceAt(w));
    const n = prevClock === null ? 0 : bellStrikes(w, prevClock);
    if (n && game.screen === 'playing') sound.bells(n);
    prevClock = w.clock;
  } else if (!live && sound.ready && now - ambT > 250) { ambT = now; sound.setAmbience({ hum: 0, traffic: 0, birds: 0, bar: 0, water: 0, rumble: 0, rain: 0, sirens: [] }); }
}
let ambT = 0, prevClock = null;

function playEvent(e) {
  const map = { 'tram-bell': 'tram-bell', crash: 'crash', hit: 'hit', horn: 'horn', door: 'door', ui: 'ui', 'ui-move': 'ui-move', 'ui-back': 'ui-back', tick: 'tick', pickup: 'pickup', 'mission-start': 'mission-start', 'mission-success': 'mission-success', 'mission-fail': 'mission-fail', carjack: 'carjack', bump: 'hit', knock: 'impact' };
  if (e.type === 'shot') map.shot = e.weapon;
  if (e.type === 'swing') map.swing = e.hit ? 'punch' : 'swing';
  Object.assign(map, { thud: 'thud', impact: 'impact', reload: 'reload', reloaded: 'reloaded', weapon: 'weapon', 'player-hurt': 'punch', wasted: 'mission-fail', respawn: 'pickup' });
  if (!map[e.type]) return;
  if (e.type === 'horn' && e.npc) { if (game.world && Math.hypot(e.x - game.world.camera.x, e.y - game.world.camera.y) < 500) sound.play('horn', 0.5); return; }
  if (e.x !== undefined && game.world && Math.hypot(e.x - game.world.camera.x, e.y - game.world.camera.y) > 700) return;
  sound.play(map[e.type], e.strength ?? 1);
}

function memoryFallback() {
  const m = new Map();
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) };
}

requestAnimationFrame(frame);
