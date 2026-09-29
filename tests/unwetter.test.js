// Unwetter (weather.js, wetfx.js): Starkregen, Sturm, Gewitter, dichter Nebel, Schneefall und Schneedecke.
import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld, resetPopulation } from '../web/src/world.js';
import { createCar, stepCar } from '../web/src/car.js';
import {
  weatherAt, blockKind, dayType, stepSnow, SNOW, weatherLight, gustAt, strikeInSlot, strikesAt, flashAt, thunderBetween,
  STRIKE, PARAMS, WEATHER_KINDS, peopleFactor, bikeFactor,
} from '../web/src/weather.js';
import { lightAt } from '../web/src/daylight.js';
import { snowCoverAlpha, snowNoise, snowFlakes, stormDebris, fogBanks, boltPath, rainDrops, roadSnowAlpha } from '../web/src/wetfx.js';
import { ambienceAt } from '../web/src/ambience.js';
import { makeSave, validateSave, applySave } from '../web/src/save.js';

const city = realCity();
const run = (w, sec) => { for (let i = 0; i < sec * 60; i++) updateWorld(w, idle(), 1 / 60); };

test('Wetterlagen: alle Unwetter kommen vor, Schnee nur an Wintertagen, Gewitter nachmittags, Nebel morgens', () => {
  const seen = {};
  for (let d = 0; d < 400; d++) {
    const type = dayType(1989, d);
    for (let b = 0; b < 8; b++) {
      const k = blockKind(1989, d, b);
      seen[k] = (seen[k] ?? 0) + 1;
      if (k === 'snow' || k === 'heavysnow') assert.equal(type, 'winter', `Schnee an einem ${type}-Tag`);
      if (k === 'thunder') assert.ok(b >= 4 && b <= 6, `Gewitter im Block ${b}`);
      if (k === 'fog' || k === 'densefog') assert.ok(b === 1 || b === 2, `Nebel im Block ${b}`);
      if (type === 'winter') assert.ok(!['rain', 'heavyrain', 'thunder', 'storm'].includes(k), 'kein Regen im Winter');
    }
  }
  for (const k of WEATHER_KINDS) assert.ok(seen[k] > 0, `${k} kommt vor`);
  assert.ok(seen.snow + seen.heavysnow < 0.2 * 3200, 'Schnee bleibt die Ausnahme');
  // alle Werte stetig über die Blockgrenzen (minütlich)
  let prev = weatherAt(1989, 0, 0), worst = 0;
  for (let t = 1; t < 20 * 1440; t++) {
    const w = weatherAt(1989, Math.floor(t / 1440), t % 1440);
    for (const k of ['cloud', 'rain', 'fog', 'snow', 'storm', 'thunder']) worst = Math.max(worst, Math.abs(w[k] - prev[k]));
    prev = w;
  }
  assert.ok(worst < 0.06, `größter Sprung ${worst}`);
  assert.ok(PARAMS.heavyrain.rain > PARAMS.rain.rain && PARAMS.densefog.fog > PARAMS.fog.fog && PARAMS.heavysnow.snow > PARAMS.snow.snow);
});

test('Sturm: Wind stark, Böen schwanken ohne Zufall, ohne Sturm keine Böen', () => {
  const calm = weatherAt(3, 2, 0, 'clear'), st = weatherAt(3, 2, 0, 'storm');
  assert.ok(Math.hypot(st.wind.x, st.wind.y) > Math.hypot(calm.wind.x, calm.wind.y) + 150, 'Sturm weht');
  assert.equal(gustAt(calm, 12.3), 1);
  const g = Array.from({ length: 200 }, (_, i) => gustAt(st, i * 0.37));
  assert.ok(Math.max(...g) > 1.4 && Math.min(...g) < 0.7, `Böen ${Math.min(...g).toFixed(2)}…${Math.max(...g).toFixed(2)}`);
  assert.equal(gustAt(st, 5), gustAt(st, 5));
  const v = { x: 0, y: 0, w: 2000, h: 1200 };
  assert.equal(stormDebris(v, calm, 1).length, 0, 'ohne Sturm fliegt nichts');
  assert.ok(stormDebris(v, st, 1).length > 30, 'Laub und Papier im Sturm');
  const a = rainDrops(v, { ...st, rain: 1 }, 1, 300), b = rainDrops(v, { ...calm, rain: 1 }, 1, 300);
  const lean = (d) => d.reduce((s, x) => s + Math.abs(x.tx), 0) / d.length;
  assert.ok(lean(a) > lean(b) * 2, 'Sturm treibt den Regen schräg');
});

test('Gewitter: Blitze deterministisch, Blitzhelligkeit mit Nachblitzen, Donner kommt mit Schallgeschwindigkeit', () => {
  assert.deepEqual(strikeInSlot(7, 123, 1), strikeInSlot(7, 123, 1));
  assert.equal(strikeInSlot(7, 123, 0), null, 'ohne Gewitter kein Blitz');
  let n = 0, near = 0;
  for (let i = 0; i < 2000; i++) { const s = strikeInSlot(7, i, 1); if (s) { n++; if (s.near) near++; } }
  assert.ok(n > 250 && n < 550, `${n} Blitze in 2000 Fenstern (≈ alle 12 s)`);
  assert.ok(near > n * 0.15 && near < n * 0.5, `nahe Blitze ${near}`);
  assert.equal(flashAt(-0.01), 0); assert.equal(flashAt(1), 0);
  assert.ok(flashAt(0.07) > 0.8, 'Hauptblitz');
  assert.ok(flashAt(0.22) > flashAt(0.19), 'Nachblitz leuchtet wieder auf');
  // Donner: je Blitz genau einmal, nach dist / Schallgeschwindigkeit, gleich ob in großen oder kleinen Schritten gefragt
  const big = thunderBetween(7, 0, 300, 1);
  let small = [];
  for (let t = 0; t < 300; t += 1 / 60) small = small.concat(thunderBetween(7, t, t + 1 / 60, 1));
  assert.equal(small.length, big.length, 'kein Donner doppelt oder verschluckt');
  assert.ok(big.length > 20);
  const s = [...Array(500).keys()].map((i) => strikeInSlot(7, i, 1)).find((x) => x && x.near);
  const delay = s.dist / STRIKE.soundSpeed, at = s.t0 + delay;
  assert.ok(thunderBetween(7, at - 0.01, at + 0.01, 1).some((c) => c.near), 'naher Donner zur richtigen Zeit');
  assert.ok(delay > 0.2 && delay < 2, `naher Blitz: Donner nach ${delay.toFixed(2)} s`);
  // genau auf der Grenze zweier Abfragen: einmal, nicht zweimal (halboffene Intervalle)
  const count = (a, b) => thunderBetween(7, a, b, 1).filter((c) => c.dist === s.dist).length;
  assert.equal(count(at - 1, at) + count(at, at + 1), 1, 'Donner auf der Grenze genau einmal');
  const lit = strikesAt(7, s.t0 + 0.07, 1);
  assert.ok(lit.some((x) => x.flash > 0.5), 'zur Blitzzeit hell');
  const bp = boltPath(100, 200, 42);
  assert.equal(bp.main[bp.main.length - 2], 100); assert.equal(bp.main[bp.main.length - 1], 200);
  assert.ok(bp.main[1] < 200 - 2000, 'kommt von oben');
  assert.ok(bp.main.every(Number.isFinite));
});

test('Schneedecke: wächst mit dem Schneefall (stark schneller als leicht), taut ohne, Regen taut schneller', () => {
  const heavy = PARAMS.heavysnow, light = PARAMS.snow;
  let a = 0, b = 0;
  for (let i = 0; i < 60; i++) { a = stepSnow(a, heavy, 1); b = stepSnow(b, light, 1); }
  assert.ok(a > b * 1.8 && a > 0.5 && b > 0.2, `stark ${a.toFixed(2)}, leicht ${b.toFixed(2)}`);
  let x = 0; for (let i = 0; i < 400; i++) x = stepSnow(x, heavy, 1);
  assert.equal(x, 1, 'geschlossen, nicht mehr');
  const dry = stepSnow(1, PARAMS.overcast, 60), wet = stepSnow(1, PARAMS.rain, 60);
  assert.ok(dry < 1 && wet < dry, `Tauwetter: trocken ${dry.toFixed(3)}, Regen ${wet.toFixed(3)}`);
  assert.equal(stepSnow(0.01, PARAMS.clear, 1e5), 0);
  assert.ok(Math.abs(stepSnow(1, PARAMS.clear, 1) - (1 - SNOW.melt)) < 1e-12);
});

test('Schneedecke im Bild: dünn fleckig, tief geschlossen; Straßen mit viel Verkehr freier als Nebenstraßen', () => {
  const N = 256, cov = (depth) => {
    let s = 0, n = 0;
    for (let y = 0; y < N; y += 4) for (let x = 0; x < N; x += 4) { s += snowCoverAlpha(snowNoise(x, y, N), depth) > 0.5 ? 1 : 0; n++; }
    return s / n;
  };
  assert.equal(cov(0), 0);
  assert.ok(cov(0.25) > 0.05 && cov(0.25) < 0.6, `dünne Decke fleckig (${cov(0.25).toFixed(2)})`);
  assert.ok(cov(1) > 0.97, `geschlossen (${cov(1).toFixed(2)})`);
  for (let d = 0.1; d < 1; d += 0.1) assert.ok(cov(d + 0.1) >= cov(d), 'mehr Schnee, mehr Fläche');
  assert.ok(Math.abs(snowNoise(0, 7, N) - snowNoise(N, 7, N)) < 1e-9, 'Textur kachelt nahtlos');
  assert.ok(roadSnowAlpha(2, 1) < roadSnowAlpha(7, 1) && roadSnowAlpha(7, 1) < roadSnowAlpha(10, 1));
  const v = { x: 0, y: 0, w: 1500, h: 900 };
  const wx = weatherAt(1, 1, 0, 'heavysnow'), lite = weatherAt(1, 1, 0, 'snow');
  assert.ok(snowFlakes(v, wx, 3, 1000).length > snowFlakes(v, lite, 3, 1000).length * 1.8, 'starker Schneefall dichter');
  assert.equal(snowFlakes(v, weatherAt(1, 1, 0, 'rain'), 3, 1000).length, 0);
  const f = snowFlakes(v, wx, 3, 600);
  assert.ok(new Set(f.map((q) => q.layer)).size === 3 && f.every((q) => Number.isFinite(q.x + q.y + q.r)), 'drei Tiefen, gültig');
  assert.ok(fogBanks(v, weatherAt(1, 1, 0, 'densefog'), 2).length > fogBanks(v, weatherAt(1, 1, 0, 'fog'), 2).length, 'dichter Nebel: mehr Schwaden');
});

test('Licht: Starkregen und dichter Nebel dunkler, Schneedecke hellt die Nacht auf, Trübe macht tags Fensterlicht', () => {
  const noon = lightAt(13 * 60), night = lightAt(23 * 60);
  assert.ok(weatherLight(noon, weatherAt(0, 0, 0, 'heavyrain')).dark > weatherLight(noon, weatherAt(0, 0, 0, 'rain')).dark);
  assert.ok(weatherLight(noon, weatherAt(0, 0, 0, 'densefog')).dark > weatherLight(noon, weatherAt(0, 0, 0, 'fog')).dark);
  const snowyNight = weatherLight(night, weatherAt(0, 0, 0, 'overcast'), 1), bare = weatherLight(night, weatherAt(0, 0, 0, 'overcast'), 0);
  assert.ok(snowyNight.dark < bare.dark, 'Schnee reflektiert');
  assert.ok(weatherLight(noon, weatherAt(0, 0, 0, 'heavyrain')).windowsLit > 0, 'bei Starkregen tagsüber Licht in Wohnungen');
  assert.equal(weatherLight(noon, weatherAt(0, 0, 0, 'clear')).windowsLit, 0);
  assert.ok(peopleFactor(weatherAt(0, 0, 0, 'heavysnow')) < peopleFactor(weatherAt(0, 0, 0, 'overcast')));
  assert.ok(bikeFactor(weatherAt(0, 0, 0, 'heavysnow')) >= 0 && bikeFactor(weatherAt(0, 0, 0, 'heavyrain')) >= 0);
});

test('Welt: Schnee fällt und bleibt liegen, Autos rutschen auf Schnee, Spielstand merkt sich Schnee und Nässe', () => {
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60;
  w.forceWeather = 'heavysnow'; resetPopulation(w); run(w, 10);
  assert.ok(w.snow > 0.08, `Schneedecke ${w.snow}`);
  assert.ok(w.cars.filter((c) => c.role !== 'curb').every((c) => c.snow === undefined || c.snow === w.snow), 'Autos kennen die Schneedecke');
  const s = validateSave(JSON.parse(JSON.stringify(makeSave(w))));
  assert.equal(s.snow, Math.round(w.snow * 100) / 100);
  const w2 = createWorld({ city }); applySave(w2, s);
  assert.equal(w2.snow, s.snow);
  assert.equal(validateSave({ ...s, snow: 7 }).snow, null, 'ungültige Schneehöhe verworfen');
  const slide = (snow) => {
    const c = createCar({ x: 0, y: 0 }); c.snow = snow;
    c.vx = 250; c.vy = 150;
    for (let i = 0; i < 12; i++) stepCar(c, 1 / 60, null);
    return Math.abs(-c.vx * Math.sin(c.angle) + c.vy * Math.cos(c.angle));
  };
  assert.ok(slide(1) > slide(0) * 1.2, `Schnee: ${slide(1)} vs ${slide(0)}`);
  // Tauwetter macht die Straßen nass
  w.forceWeather = 'rain'; const was = w.snow; run(w, 2);
  assert.ok(w.snow < was && w.wet > 0, 'taut und wird nass');
});

test('Klang: Sturm heult, Schnee dämpft die Stadt, Starkregen lauter als Regen', () => {
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.clock = 14 * 60;
  w.weather = weatherAt(0, 0, 0, 'storm');
  assert.ok(ambienceAt(w).wind > 0.2, 'Wind hörbar');
  w.weather = weatherAt(0, 0, 0, 'clear');
  assert.equal(ambienceAt(w).wind, 0);
  const loud = ambienceAt(w).hum; w.snow = 1;
  assert.ok(ambienceAt(w).hum < loud * 0.7, 'Schneedecke schluckt den Lärm');
  w.weather = weatherAt(0, 0, 0, 'heavyrain');
  const hr = ambienceAt(w).rain; w.weather = weatherAt(0, 0, 0, 'rain');
  assert.ok(hr > ambienceAt(w).rain);
});

test('Zeichnen bei jedem Wetter: gültige Koordinaten, Schnee liegt, Flocken, Blitz, Nebelschwaden, Sturm', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  const bad = [];
  const mk = () => new Proxy({ canvas: { width: 1280, height: 720 } }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'createImageData') return (wd, hg) => ({ data: new Uint8ClampedArray(wd * hg * 4) });
      return (...a) => { if (a.some((v) => typeof v === 'number' && !Number.isFinite(v))) bad.push(k); };
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  globalThis.OffscreenCanvas = class { constructor(wd, hg) { this.width = wd; this.height = hg; } getContext() { return mk(); } };
  const { Renderer } = await import('../web/src/render.js');
  const w = createWorld({ city }); w.mission.state = 'idle';
  const r = new Renderer(mk());
  for (const kind of WEATHER_KINDS) {
    w.forceWeather = kind; w.clock = kind.includes('fog') ? 7 * 60 : 22 * 60;
    w.snow = kind.includes('snow') ? 0.8 : 0; resetPopulation(w); run(w, 0.2);
    r.draw(w, 1280, 720, 1);
    assert.deepEqual(bad, [], kind);
    if (kind.includes('snow')) assert.ok(r.stats.snowCover && r.stats.flakes > 300, `${kind}: Schnee`);
    if (kind === 'storm') assert.ok(r.stats.debris > 20, 'Sturm: Laub');
    if (kind === 'densefog') assert.ok(r.stats.fogBanks > 0, 'Nebelschwaden');
    if (kind === 'heavyrain') assert.ok(r.stats.drops > 500, `Starkregen ${r.stats.drops}`);
  }
  // Gewitter: im Moment eines nahen Blitzes Strahl und Himmelsblitz
  w.forceWeather = 'thunder'; w.snow = 0; run(w, 0.1);
  const s = [...Array(3000).keys()].map((i) => strikeInSlot(w.seed, i + Math.floor(w.time / STRIKE.slot) + 1, 1)).find((x) => x && x.near);
  w.time = s.t0 + 0.07;
  r.draw(w, 1280, 720, 1);
  assert.ok(r.stats.flash > 0.5, `Blitz ${r.stats.flash}`);
  assert.equal(r.stats.bolts, 1, 'Blitzstrahl im Bild');
  assert.deepEqual(bad, []);
});

test('Spielstand: Glätte wird gespeichert; alter Spielstand ohne Glätte lädt ohne', async () => {
  const { makeSave, applySave, validateSave } = await import('../web/src/save.js');
  const { createWorld } = await import('../web/src/world.js');
  const w = createWorld({ city }); w.ice = 0.63;
  const s = makeSave(w);
  assert.equal(s.ice, 0.63);
  const w2 = createWorld({ city }); applySave(w2, s);
  assert.equal(w2.ice, 0.63);
  assert.equal(validateSave({ ...s, ice: 7 }).ice, null, 'ungültige Glätte verworfen');
  const old = { ...s }; delete old.ice;
  const w3 = createWorld({ city }); w3.ice = 0.4; applySave(w3, validateSave(old) ?? old);
  assert.equal(w3.ice, 0, 'alter Spielstand: keine Glätte');
});
