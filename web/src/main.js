// Browser-Einstieg: Canvas, Hauptschleife (fester 60-Hz-Takt), Eingabequellen, Xbox-Hüllen-Brücke.
import { DT } from './config.js';
import { createGame, updateGame, setCity, requestTeleport, confirmTeleport } from './game.js';
import { decodeCity } from './map.js';
import { createWorld, updateWorld, playerCar, speedOf } from './world.js';
import { InputState, readKeys, readPad, fromHostReading, merge } from './input.js';
import { Renderer } from './render.js';
import { Hud } from './hud.js';
import { Sound } from './audio.js';
import { loadSprites } from './assets.js';
import { idleInput } from './idle.js';

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

// Karte laden (web/data/city.json, ~6 MB). Bis dahin zeigt der Titel „Lade …“.
// Titelbildschirm-Hintergrund: eine laufende Demo-Welt mit Kamerafahrt über echte Straßen.
let demo = null;
fetch('data/city.json').then((r) => { if (!r.ok) throw new Error(`HTTP ${r.status}`); return r.json(); }).then((json) => {
  const city = decodeCity(json);
  setCity(game, city);
  globalThis.__city = city;
  demo = createWorld({ city, seed: 1989, cars: 14, pedestrians: 30 });
  demo.mission.state = 'idle';
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

// Maus: Klick auf den Stadtplan wählt ein Teleport-Ziel, Klick auf Ja/Nein im Dialog bestätigt.
canvas.addEventListener('pointerdown', (e) => {
  if (!hud.s) return;
  const r = canvas.getBoundingClientRect();
  const vx = (e.clientX - r.left) * (canvas.width / r.width) / hud.s, vy = (e.clientY - r.top) * (canvas.height / r.height) / hud.s;
  const inside = (b) => b && vx >= b.x && vx <= b.x + b.w && vy >= b.y && vy <= b.y + b.h;
  if (game.teleport) {
    if (inside(hud.dialogButtons?.yes)) { confirmTeleport(game, true); sound.play('ui'); }
    else if (inside(hud.dialogButtons?.no)) { confirmTeleport(game, false); sound.play('ui-back'); }
    return;
  }
  const m = hud.bigMap;
  if (game.screen === 'playing' && game.showBigMap && inside(m)) {
    if (requestTeleport(game, (vx - m.x) / m.f, (vy - m.y) / m.f)) sound.play('ui');
  }
});

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
    const events = updateGame(game, inp, DT);
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
  const worldScale = H / 600;
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
      canvas.style.cursor = game.showBigMap && !game.teleport ? 'crosshair' : '';
      if (game.showBigMap) hud.drawBigMap(game.world);
      if (game.teleport) hud.drawTeleportDialog(game.teleport);
      if (game.resultMenu) hud.drawResult(game);
    } else if (game.screen === 'paused') hud.drawPause(game);
    else if (game.screen === 'controls') hud.drawControls();
  }
  hud.toast(game.toast);
  const car = game.world && game.screen === 'playing' ? playerCar(game.world) : null;
  sound.setEngine(!!car && !car.wrecked, car ? Math.min(1, speedOf(car) / 330) : 0, car ? car.controls.throttle : 0);
}

function playEvent(e) {
  const map = { crash: 'crash', hit: 'hit', horn: 'horn', door: 'door', ui: 'ui', 'ui-move': 'ui-move', 'ui-back': 'ui-back', tick: 'tick', pickup: 'pickup', 'mission-start': 'mission-start', 'mission-success': 'mission-success', 'mission-fail': 'mission-fail', carjack: 'carjack', bump: 'hit' };
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
