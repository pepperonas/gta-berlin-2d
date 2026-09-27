import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';

// Aufzeichnender Canvas-Ersatz: merkt sich für jeden gestrichelten Strich den Versatz (lineDashOffset).
function recordingContext() {
  const log = [];
  let dash = [];
  const ctx = new Proxy({ lineDashOffset: 0, canvas: { width: 1280, height: 720 } }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'setLineDash') return (d) => { dash = d; };
      if (k === 'getLineDash') return () => dash;
      if (k === 'stroke') return () => { if (dash.length) log.push(t.lineDashOffset); };
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'createPattern' || k === 'createLinearGradient') return () => ({ addColorStop() {} });
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  return { ctx, log };
}

test('gestrichelte Straßenmarkierungen „fließen“ nicht (kein wandernder Strichversatz)', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  globalThis.OffscreenCanvas ??= class { getContext() { return new Proxy({}, { get: () => () => {} }); } };
  const { Renderer } = await import('../web/src/render.js');
  const { createWorld } = await import('../web/src/world.js');
  const w = createWorld({ city: realCity(), cars: 0, pedestrians: 0 });
  // Missionsmarker im Bild: der pulsierende Kreis ist absichtlich animiert.
  w.camera.x = w.city.places.giver.x; w.camera.y = w.city.places.giver.y;
  const { ctx, log } = recordingContext();
  const r = new Renderer(ctx);
  for (const t of [0, 0.5, 1.3]) { w.time = t; log.length = 0; r.draw(w, 1280, 720, 1.2); }
  // Letztes Bild: alle gestrichelten Striche außer dem Missionskreis haben Versatz 0.
  const moving = log.filter((o) => o !== 0);
  assert.ok(log.length > 3, 'Mittellinien wurden gezeichnet');
  assert.ok(moving.length <= 1, `${moving.length} gestrichelte Striche mit Versatz ${moving.join(', ')}`);
});

test('Gleise maßstäblich und in Ebenen: erst Bett/Viadukt, dann Schwellen, dann Schienen (Spurweite 1435 mm)', async () => {
  const { Renderer, TRACK } = await import('../web/src/render.js');
  const { createWorld } = await import('../web/src/world.js');
  const city = realCity();
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  // Kottbusser Tor: U1-Viadukt mit zwei Gleisen
  const kotti = city.list('poi').find((q) => q.cat === 'ubahn' && q.name === 'Kottbusser Tor');
  w.camera.x = kotti.x; w.camera.y = kotti.y;
  const strokes = [];
  let dash = [];
  const ctx = new Proxy({ lineDashOffset: 0, lineWidth: 1, strokeStyle: '' }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'setLineDash') return (d) => { dash = d; };
      if (k === 'stroke') return () => strokes.push({ w: t.lineWidth, dash: dash.slice(), color: t.strokeStyle });
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'createPattern' || k === 'createLinearGradient') return () => ({ addColorStop() {} });
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  new Renderer(ctx).draw(w, 1280, 720, 1.2);
  const deck = strokes.map((x, i) => [x, i]).filter(([x]) => x.w === TRACK.deck);
  const sleepers = strokes.map((x, i) => [x, i]).filter(([x]) => x.w === TRACK.sleeper && x.dash.length);
  const rails = strokes.map((x, i) => [x, i]).filter(([x]) => x.w === TRACK.rail);
  assert.ok(deck.length >= 2, 'Viadukt mit mindestens zwei Gleisen im Bild');
  assert.equal(rails.length, 2 * deck.length, 'je Gleis genau zwei Schienen');
  assert.ok(Math.max(...deck.map(([, i]) => i)) < Math.min(...sleepers.map(([, i]) => i)), 'alle Betten vor allen Schwellen');
  assert.ok(Math.max(...sleepers.map(([, i]) => i)) < Math.min(...rails.map(([, i]) => i)), 'alle Schwellen vor allen Schienen');
  // Maße: Spurweite 1,435 m, Schwelle 2,6 m, Gleisbett schmaler als der Gleisabstand von 4 m + Bett
  assert.equal(TRACK.gauge, 14.35);
  assert.ok(TRACK.sleeper > TRACK.gauge && TRACK.sleeper < 30);
  assert.ok(TRACK.deck <= 50, 'Viaduktbreite je Gleis höchstens 5 m');
});

test('Tag/Nacht: bei Tag Hausschatten und keine Lichtkarte, nachts Lichtkarte per „multiply“ mit Scheinwerfern', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  // Offscreen-Ebenen (Schatten, Lichtkarte, Licht-Sprites) brauchen Verläufe
  globalThis.OffscreenCanvas = class {
    constructor(w, h) { this.width = w; this.height = h; }
    getContext() { return new Proxy({}, { get: (t, k) => (k === 'createRadialGradient' || k === 'createLinearGradient' ? () => ({ addColorStop() {} }) : () => {}) }); }
  };
  const { Renderer } = await import('../web/src/render.js');
  const { createWorld, updateWorld } = await import('../web/src/world.js');
  const { idle } = await import('./helpers/bot.js');
  const w = createWorld({ city: realCity(), seed: 3 });
  w.camera.x = w.city.places.giver.x; w.camera.y = w.city.places.giver.y;
  for (let i = 0; i < 30; i++) updateWorld(w, idle(), 1 / 60); // Verkehr erzeugen
  const draws = [];
  const ctx = new Proxy({ globalCompositeOperation: 'source-over', globalAlpha: 1, lineDashOffset: 0 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'drawImage') return (img, ...a) => { if (a.every((x) => Number.isFinite(x))) draws.push({ op: t.globalCompositeOperation, alpha: t.globalAlpha }); else draws.push({ nan: true }); };
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'getLineDash') return () => [];
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const r = new Renderer(ctx);
  w.clock = 11 * 60; draws.length = 0; r.draw(w, 1280, 720, 1.2);
  assert.ok(r.stats.shadows > 10, `Hausschatten bei Tag (${r.stats.shadows} Häuser)`);
  assert.ok(!draws.some((d) => d.op === 'multiply'), 'bei Tag keine Lichtkarte');
  assert.ok(draws.some((d) => d.op === 'source-over' && d.alpha > 0 && d.alpha < 0.5), 'Schattenebene halbtransparent aufgetragen');
  w.clock = 23 * 60; draws.length = 0; r.draw(w, 1280, 720, 1.2);
  assert.equal(r.stats.shadows, 0, 'nachts keine Sonnenschatten');
  assert.equal(draws.filter((d) => d.op === 'multiply').length, 1, 'genau eine Lichtkarte je Bild');
  assert.ok(r.stats.lights >= 3, `Lichtquellen (${r.stats.lights})`);
  assert.ok(!draws.some((d) => d.nan), 'keine NaN-Koordinaten');
});

test('Nachtfenster: je Haus fest (kein Flackern), mehr Licht am späten Abend, tagsüber keines', async () => {
  const { nightVariant, NIGHT_DENSITY } = await import('../web/src/render.js');
  const { lightAt } = await import('../web/src/daylight.js');
  const houses = Array.from({ length: 400 }, (_, i) => ({ seed: (i * 7919 + 13) % 1000003 }));
  const mean = (wl) => houses.reduce((a, b) => a + (nightVariant(b, wl) + 1), 0) / houses.length;
  assert.ok(houses.every((b) => nightVariant(b, lightAt(12 * 60).windowsLit) === -1), 'mittags kein Fenster erleuchtet');
  assert.ok(mean(lightAt(22 * 60).windowsLit) > mean(lightAt(3 * 60).windowsLit), 'abends mehr Licht als um 3 Uhr');
  const v = houses.map((b) => nightVariant(b, 0.5));
  assert.deepEqual(houses.map((b) => nightVariant(b, 0.5)), v, 'deterministisch');
  assert.ok(new Set(v).size >= 3, 'Häuser unterscheiden sich');
  assert.ok(v.every((k) => k >= -1 && k < NIGHT_DENSITY.length));
});

test('Qualitätsstufe: wechselt erst über dem Budget auf „niedrig“ und erst deutlich darunter zurück', async () => {
  const { nextQuality } = await import('../web/src/render.js');
  const { RENDER } = await import('../web/src/config.js');
  assert.equal(nextQuality('high', RENDER.budgetMs - 1), 'high');
  assert.equal(nextQuality('high', RENDER.budgetMs + 1), 'low');
  assert.equal(nextQuality('low', RENDER.budgetMs - 1), 'low', 'Hysterese: nicht sofort zurück');
  assert.equal(nextQuality('low', RENDER.recoverMs - 1), 'high');
});

test('Nacht: Laternen gezeichnet und in der Lichtkarte; Häuser verdecken Bodenlicht nur in hoher Qualität', async () => {
  const { Renderer } = await import('../web/src/render.js');
  const { createWorld } = await import('../web/src/world.js');
  const w = createWorld({ city: realCity(), cars: 0, pedestrians: 0 });
  w.camera.x = w.city.places.pickup.x; w.camera.y = w.city.places.pickup.y + 300;
  const ctx = new Proxy({ globalCompositeOperation: 'source-over', globalAlpha: 1, lineDashOffset: 0 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'getLineDash') return () => [];
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const r = new Renderer(ctx);
  let occl = 0;
  const orig = r.drawBuilding.bind(r);
  r.drawBuilding = (b, cam, g, night) => { if (night) occl++; return orig(b, cam, g, night); };
  w.clock = 23 * 60; r.draw(w, 1280, 720, 1.2);
  assert.ok(r._lamps.length >= 3, `Laternen im Bild (${r._lamps.length})`);
  assert.ok(r.stats.lights >= r._lamps.length, 'jede Laterne wirft Licht');
  assert.ok(occl > 5, `Häuser in der Lichtkarte (${occl})`);
  occl = 0; r.quality = 'low'; r.draw(w, 1280, 720, 1.2);
  assert.equal(occl, 0, 'niedrige Qualität: kein zweiter Hausdurchgang');
  occl = 0; r.quality = 'high'; w.clock = 12 * 60; r.draw(w, 1280, 720, 1.2);
  assert.equal(occl, 0, 'tagsüber keine Lichtkarte');
});
