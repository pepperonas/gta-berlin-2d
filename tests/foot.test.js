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

test('Zoom zu Fuß: Standard 2,0, einstellbar 1,5–2,6; im Auto unverändert', async () => {
  const { setFootZoom, FOOT_ZOOM_RANGE } = await import('../web/src/world.js');
  assert.equal(FOOT_ZOOM, 2); assert.deepEqual(FOOT_ZOOM_RANGE, [1.5, 2.6]);
  const w = foot();
  for (let i = 0; i < 600; i++) updateCamera(w, 1 / 60);
  assert.ok(Math.abs(w.camera.zoom - 2) < 0.01, `Standard ${w.camera.zoom}`);
  setFootZoom(w, 9); assert.equal(w.footZoom, 2.6);
  setFootZoom(w, 0.1); assert.equal(w.footZoom, 1.5);
  for (let i = 0; i < 600; i++) updateCamera(w, 1 / 60);
  assert.ok(Math.abs(w.camera.zoom - 1.5) < 0.01, 'eingestellter Zoom');
  const car = w.cars.find((c) => c.id === w.playerCarId); w.player.inCar = car.id; car.driver = 'player';
  for (let i = 0; i < 600; i++) updateCamera(w, 1 / 60);
  assert.ok(w.camera.zoom <= 1.01, `im Auto wie bisher (${w.camera.zoom})`);
});

test('Wegfindung zu Fuß: um ein Haus herum, kein Weg durch Wände, unerreichbar → nächster Punkt, schnell', async () => {
  const { findFootPath } = await import('../web/src/footpath.js');
  const { inBuilding } = await import('../web/src/map.js');
  const { circleVsSegment } = await import('../web/src/collision.js');
  const w = foot();
  // ein Haus mittlerer Größe mit freiem Umfeld: Start vor, Ziel hinter dem Haus
  const b = city.list('building').find((q) => q.bbox.w > 150 && q.bbox.w < 400 && q.bbox.h > 150 && q.bbox.h < 400 && !inBuilding(city, q.bbox.x - 30, q.bbox.y + q.bbox.h / 2) && !inBuilding(city, q.bbox.x + q.bbox.w + 30, q.bbox.y + q.bbox.h / 2));
  const from = { x: b.bbox.x - 30, y: b.bbox.y + b.bbox.h / 2 }, to = { x: b.bbox.x + b.bbox.w + 30, y: b.bbox.y + b.bbox.h / 2 };
  const t0 = performance.now(), path = findFootPath(w, from, to, 0), ms = performance.now() - t0;
  assert.ok(path && path.length >= 2, 'Weg gefunden');
  assert.ok(ms < 60, `schnell (${ms.toFixed(1)} ms)`);
  const last = path.at(-1);
  assert.ok(Math.hypot(last.x - to.x, last.y - to.y) < 12, 'am Ziel');
  // jedes Wegstück frei von Hauswänden (Abtastung alle 2 px)
  for (let i = 1; i < path.length; i++) {
    const a = path[i - 1], c = path[i], L = Math.hypot(c.x - a.x, c.y - a.y);
    for (let s = 0; s <= L; s += 2) { const x = a.x + (c.x - a.x) * s / L, y = a.y + (c.y - a.y) * s / L; assert.ok(!inBuilding(city, x, y), `Weg durchs Haus bei ${x.toFixed(0)},${y.toFixed(0)}`); }
  }
  // Ziel im Haus → nächster erreichbarer Punkt außerhalb
  const inside = { x: b.bbox.x + b.bbox.w / 2, y: b.bbox.y + b.bbox.h / 2 };
  if (inBuilding(city, inside.x, inside.y)) { const p2 = findFootPath(w, from, inside, 0); assert.ok(p2 && !inBuilding(city, p2.at(-1).x, p2.at(-1).y), 'nächster Punkt draußen'); }
});

// Klick-Steuerung (Diablo-Schema): abstrakte Felder clickWorld/clickPressed/clickHeld/clickForce
const click = (at, extra = {}) => ({ clickWorld: at, clickPressed: true, clickHeld: false, ...extra });

test('Klick auf den Boden: die Figur läuft hin (um Hindernisse herum); WASD bricht ab', async () => {
  const { inBuilding } = await import('../web/src/map.js');
  const w = foot(), to = { x: P0.x + Math.cos(ang) * 150, y: P0.y + Math.sin(ang) * 150 };
  updateWorld(w, { ...idle(), ...click(to) }, 1 / 60);
  for (let i = 0; i < 60 * 8; i++) { updateWorld(w, idle(), 1 / 60); assert.ok(!inBuilding(city, w.player.x, w.player.y)); }
  assert.ok(Math.hypot(w.player.x - to.x, w.player.y - to.y) < 12, 'angekommen');
  const w2 = foot();
  updateWorld(w2, { ...idle(), ...click(to) }, 1 / 60);
  for (let i = 0; i < 20; i++) updateWorld(w2, idle(), 1 / 60);
  updateWorld(w2, { ...idle(), moveX: -Math.cos(ang), moveY: -Math.sin(ang) }, 1 / 60);
  const x = w2.player.x, y = w2.player.y;
  for (let i = 0; i < 60; i++) updateWorld(w2, idle(), 1 / 60);
  assert.ok(Math.hypot(w2.player.x - x, w2.player.y - y) < 1, 'Klick-Weg abgebrochen');
});

test('Klick auf eine Person: hinlaufen bis in Reichweite, dann angreifen; Shift-Klick: stehen bleiben und schießen', async () => {
  const { createPed } = await import('../web/src/pedestrians.js');
  const { nearestSpot } = await import('../web/src/pedestrians.js');
  const w = foot();
  const PISTOL = (await import('../web/src/combat.js')).WEAPONS.findIndex((q) => q.id === 'pistol');
  w.player.weapon = PISTOL;
  const sp = nearestSpot(city, P0.x, P0.y), ped = createPed(city, sp, w.rng);
  ped.x = P0.x + Math.cos(ang) * 700; ped.y = P0.y + Math.sin(ang) * 700; ped.speed = 0; w.peds.push(ped); // steht (Tempo 0)
  updateWorld(w, { ...idle(), ...click({ x: ped.x, y: ped.y }) }, 1 / 60);
  let shots = 0, d0 = Math.hypot(ped.x - w.player.x, ped.y - w.player.y);
  for (let i = 0; i < 60 * 10 && !shots; i++) { updateWorld(w, idle(), 1 / 60); shots += w.events.filter((e) => e.type === 'shot').length; }
  const d1 = Math.hypot(ped.x - w.player.x, ped.y - w.player.y);
  assert.ok(d1 < d0 - 50, `hingelaufen (${d0.toFixed(0)} → ${d1.toFixed(0)})`);
  assert.ok(shots >= 1, 'geschossen');
  // Shift-Klick auf leeren Boden: kein Laufen, Schuss dorthin
  const w2 = foot(); w2.player.weapon = PISTOL;
  const at = { x: P0.x + Math.cos(ang) * 200, y: P0.y + Math.sin(ang) * 200 };
  let s2 = 0; const x = w2.player.x;
  for (let i = 0; i < 20; i++) { updateWorld(w2, { ...idle(), ...click(at, { clickForce: true, clickPressed: i === 0, clickHeld: true }) }, 1 / 60); s2 += w2.events.filter((e) => e.type === 'shot').length; }
  assert.ok(s2 >= 1 && Math.abs(w2.player.x - x) < 0.5, 'steht und schießt');
});
