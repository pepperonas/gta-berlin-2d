// Reproduzierbare Grafikszenen; bewusst separat vom Spiel und seinen Spielständen.
import { openCity, surfaceAt, T } from './map.js';
import { createWorld, updateWorld, resetPopulation } from './world.js';
import { Renderer } from './render.js';
import { weatherAt } from './weather.js';
import { geoToPx } from './projection.js';
import { idleInput } from './idle.js';
import { prepareTransit } from './transit.js';
import { stationsNear } from './station.js';
import { setPeopleDetail } from './people.js';
const canvas = document.querySelector('canvas'), status = document.querySelector('output');
const scene = document.querySelector('#scene'), zoom = document.querySelector('#zoom'), quality = document.querySelector('#quality');
const json = async (url) => { const r = await fetch(url); if (!r.ok) throw Error(`${r.status}: ${url}`); return r.json(); };
const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
const index = await json('data/berlin/index.json');
const city = openCity(index, key => json(`data/berlin/tiles/${key}.json`));
city.transit = prepareTransit(await json('data/berlin/transit.json'));
const project = geoToPx(city.meta);
const scenes = {
  block: { clock: 780, weather: 'clear' },
  avenue: { geo: [52.4814, 13.4346], clock: 1170, weather: 'clear' },
  park: { geo: [52.4857, 13.4131], clock: 990, weather: 'clear' },
  water: { geo: [52.5081, 13.4295], clock: 1170, weather: 'clear' },
  rain: { clock: 1320, weather: 'rain', wet: 1 },
  snow: { clock: 840, weather: 'snow', snow: 0.6 },
  station: { geo: [52.499, 13.418], clock: 1320, weather: 'clear' },
};
let world, renderer, busy = false;
async function loadScene() {
  if (busy) return; busy = true; status.textContent = 'Lädt …';
  try {
    if (world) city.release(world.focusKey);
    const cfg = scenes[scene.value];
    const [x, y] = cfg.geo ? project(...cfg.geo) : [city.places.playerSpawn.x, city.places.playerSpawn.y];
    const started = performance.now();
    while (!city.focus('review', x, y)) { if (performance.now() - started > 30000) throw Error('Kartenkacheln nicht geladen'); await frame(); }
    world = createWorld({ city, seed: 1989, cars: 22, pedestrians: 55 });
    Object.assign(world.player, { x, y, inCar: null }); Object.assign(world.camera, { x, y, zoom: +zoom.value });
    resetPopulation(world); world.clock = cfg.clock; world.forceWeather = cfg.weather;
    for (let i = 0; i < 120; i++) updateWorld(world, idleInput, 1 / 60);
    Object.assign(world, { time: 120, clock: cfg.clock, weather: weatherAt(1989, 0, cfg.clock, cfg.weather), wet: cfg.wet ?? 0, snow: cfg.snow ?? 0 });
    Object.assign(world.camera, { x, y, zoom: +zoom.value });
    if (scene.value === 'station') {
      const st = stationsNear(city, x, y, 4000)[0]; if (!st) throw Error('Bahnhof fehlt');
      Object.assign(world.player, { x: st.x, y: st.y, inside: { id: st.id }, lvl: -2 }); Object.assign(world.camera, { x: st.x, y: st.y });
    }
    renderer = new Renderer(canvas.getContext('2d', { alpha: false })); renderer.quality = quality.value;
    draw(); status.textContent = 'Bereit · 1920 × 1080';
  } catch (e) { status.textContent = e.message; console.error(e); }
  finally { busy = false; }
}
function draw() { setPeopleDetail(renderer.quality === 'high'); renderer.drawFrame(world, 1920, 1080, 1, false); }
scene.onchange = loadScene;
zoom.onchange = () => { if (!busy && world) { world.camera.zoom = +zoom.value; draw(); } };
quality.onchange = () => { if (!busy && renderer) { renderer.quality = quality.value; draw(); } };
async function measure() {
  if (busy || !renderer) return; busy = true; status.textContent = 'Misst …'; const samples = [];
  try { for (let i = 0; i < 150; i++) { await frame(); const t0 = performance.now(); draw(); if (i >= 30) samples.push(performance.now() - t0); }
    samples.sort((a,b) => a-b); const result = { scene: scene.value, quality: quality.value, zoom: +zoom.value, median: +samples[60].toFixed(1), p95: +samples[114].toFixed(1) };
    status.textContent = `Median ${result.median} ms · P95 ${result.p95} ms · ${quality.value}`; return result;
  } finally { busy = false; }
}
document.querySelector('#measure').onclick = measure;
document.querySelector('#suite').onclick = async () => {
  if (busy) return;
  const results = [], controls = [...document.querySelectorAll('button,select')]; controls.forEach(c => c.disabled = true);
  try { for (const key of Object.keys(scenes)) { scene.value = key; await loadScene();
    for (const q of ['high', 'low']) { quality.value = q; renderer.quality = q; results.push(await measure()); document.querySelector('#results').textContent = JSON.stringify(results); }
  } status.textContent = 'Messreihe abgeschlossen'; }
  finally { controls.forEach(c => c.disabled = false); }
};
document.querySelector('#profile').onclick = async () => {
  if (busy || !renderer) return; busy = true; const totals = {}, originals = new Map();
  for (const key of Object.getOwnPropertyNames(Renderer.prototype)) {
    if (['constructor', 'draw', 'drawFrame'].includes(key) || typeof renderer[key] !== 'function') continue;
    const fn = renderer[key]; originals.set(key, fn); renderer[key] = function (...args) { const t0 = performance.now(); try { return fn.apply(this, args); } finally { totals[key] = (totals[key] ?? 0) + performance.now() - t0; } };
  }
  try { for (let i = 0; i < 60; i++) { await frame(); draw(); } document.querySelector('#results').textContent = JSON.stringify(Object.entries(totals).sort((a,b) => b[1]-a[1]).slice(0,12).map(([method,ms]) => ({method, ms: +(ms/60).toFixed(2)}))); status.textContent = 'Renderprofil abgeschlossen'; }
  finally { for (const [key, fn] of originals) renderer[key] = fn; busy = false; }
};
document.querySelector('#motion').onclick = async () => {
  if (busy || !renderer) return; busy = true;
  if (scene.value === 'water') {
    // Den Wassereintritt sichtbar machen, auch wenn der Szenenmittelpunkt auf einem Ufergebäude liegt.
    let spot;
    search: for (let r = 0; r < 600; r += 20) for (let a = 0; a < Math.PI * 2; a += Math.PI / 16) {
      const x = world.camera.x + Math.cos(a) * r, y = world.camera.y + Math.sin(a) * r;
      if (surfaceAt(city, x, y) === T.WATER && surfaceAt(city, x + 120, y) === T.WATER) { spot = { x, y }; break search; }
    }
    if (spot) { Object.assign(world.player, spot, { jumpZ: 0.6, jumpV: 0, swimming: false, lvl: 0 }); Object.assign(world.camera, spot); }
  }
  const camera = { ...world.camera }, clock = world.clock;
  try { for (let i = 0; i < 180; i++) {
    await frame(); updateWorld(world, { ...idleInput, moveX: 1, sprint: true, jumpPressed: i === 0 }, 1 / 60);
    renderer.handleEvents(world.events); renderer.update(world, 1 / 60); world.clock = clock; Object.assign(world.camera, camera); draw();
    status.textContent = `Bewegung · ${renderer.fx?.particles.length ?? 0} Partikel · ${renderer.fx?.rings.length ?? 0} Wasserringe`;
  } status.textContent = 'Bewegung abgeschlossen'; } finally { busy = false; }
};
document.querySelector('#save').onclick = () => { if (busy) return; canvas.toBlob(blob => { const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = `berlin-${scene.value}-${quality.value}.png`; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 1000); }); };
await loadScene();
