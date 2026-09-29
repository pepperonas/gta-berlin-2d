// Zu Fuß am PC (0.32.0): Tempo, Ausdauer, Zielen, Zoom, Steuerschema, Wegfindung, Klick-Angriff
import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorld, updateWorld, updateCamera, FOOT_ZOOM } from '../web/src/world.js';
import { PLAYER, PED, STAMINA } from '../web/src/config.js';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';

const city = realCity();
// freie Fläche: Mitte einer langen Straße (Fahrbahn, keine Häuser)
const road = city.list('edge').find((e) => !e.bridge && (e.lvl ?? 0) === 0 && e.cls <= 5 && e.len > 1500);
const k = Math.min((road.pts.length >> 1) & ~1, road.pts.length - 4), P0 = { x: road.pts[k], y: road.pts[k + 1] };
const ang = Math.atan2(road.pts[k + 3] - road.pts[k + 1], road.pts[k + 2] - road.pts[k]);
function foot() {
  const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle';
  w.player.x = P0.x; w.player.y = P0.y; w.player.inCar = null; w.camera.x = P0.x; w.camera.y = P0.y;
  return w;
}
const along = (patch, secs, w = foot()) => {
  const x0 = w.player.x, y0 = w.player.y;
  for (let i = 0; i < secs * 60; i++) { updateWorld(w, { ...idle(), moveX: Math.cos(ang), moveY: Math.sin(ang), ...patch }, 1 / 60); w.camera.x = w.player.x; w.camera.y = w.player.y; }
  return { v: Math.hypot(w.player.x - x0, w.player.y - y0) / secs, w };
};

test('Tempo zu Fuß realistisch: joggen 3,5 m/s, sprinten 7 m/s, langsam gehen 1,5 m/s, Stick halb = gehen', () => {
  assert.equal(PLAYER.jog, 35); assert.equal(PLAYER.sprint, 70); assert.equal(PLAYER.walk, 15);
  assert.ok(Math.abs(along({}, 1).v - 35) < 2, 'joggen');
  assert.ok(Math.abs(along({ sprint: true }, 1).v - 70) < 3, 'sprinten');
  assert.ok(Math.abs(along({ walkSlow: true }, 1).v - 15) < 1.5, 'langsam gehen');
  const half = (() => { const w = foot(), x0 = w.player.x; for (let i = 0; i < 60; i++) updateWorld(w, { ...idle(), moveX: Math.cos(ang) * 0.5, moveY: Math.sin(ang) * 0.5 }, 1 / 60); return Math.hypot(w.player.x - x0, w.player.y - P0.y) ; })();
  assert.ok(Math.abs(half - 15) < 2, `Stick halb: gehen (${half.toFixed(1)})`);
});

test('Ausdauer: Sprint leert sie in ~12 s, dann nur joggen; erholt sich nach einer Pause', () => {
  const r = along({ sprint: true }, STAMINA.drain + 1);
  assert.ok(r.w.player.stamina <= 0.01, `leer (${r.w.player.stamina})`);
  const tired = along({ sprint: true }, 1, r.w);
  assert.ok(Math.abs(tired.v - PLAYER.jog) < 3, `erschöpft: nur joggen (${tired.v.toFixed(1)})`);
  for (let i = 0; i < (STAMINA.recover + 2) * 60; i++) updateWorld(r.w, idle(), 1 / 60);
  assert.ok(r.w.player.stamina > 0.99, 'erholt');
});

test('Passanten realistisch: gehen ~1,3 m/s, rennen 4,5 m/s', () => {
  assert.equal(PED.walk, 13); assert.equal(PED.run, 45);
});
