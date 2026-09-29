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

test('Dächer: Flächen zur Sonne heller, abgewandte dunkler; Ziegelreihen und Gauben nur in hoher Qualität', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  const { Renderer } = await import('../web/src/render.js');
  const { roofOf } = await import('../web/src/roofs.js');
  const { packLook, ROOF_SHAPE, BUILDING_KIND } = await import('../web/src/citycodes.js');
  const fills = [], strokes = [];
  const ctx = new Proxy({ lineWidth: 1 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'fill') return (p) => { if (p && typeof p === 'object') fills.push(t.fillStyle); };
      if (k === 'stroke') return (p) => strokes.push({ w: t.lineWidth, s: t.strokeStyle, path: !!p });
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'measureText') return () => ({ width: 10 });
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const r = new Renderer(ctx);
  const b = { rings: [[0, 0, 300, 0, 300, 120, 0, 120]], cx: 150, cy: 60, bbox: { x: 0, y: 0, w: 300, h: 120 }, kind: BUILDING_KIND.house,
    meters: 8, height: 80, seed: 99, roofRgb: -1, wallRgb: -1, look: packLook({ shape: ROOF_SHAPE.gabled }), sign: [1] };
  const roof = roofOf(b);
  const col = { roof: '#808080', roofHex: '#808080', center: '#808080', parapet: '#999', line: '#333', lit: [], faces: ['#aaa', '#999', '#888'] };
  const lum = (s) => s.match(/\d+/g).slice(0, 3).map(Number).reduce((a, c) => a + c, 0);
  const shades = (dy) => { fills.length = 0; col.lit = []; r.light = { sun: { dx: 0, dy, strength: 1 } }; r.drawRoof(ctx, b, col, roof, new Path2D()); return fills.slice(1).map(lum); };
  // Schatten zeigt nach unten (+y): Sonne im Norden, die Nordfläche (fällt nach −y ab) ist die helle
  const north = roof.geo.facets.findIndex((f) => f.ny < 0);
  const a = shades(1), bb = shades(-1);
  assert.equal(a.length, 2, 'zwei Dachflächen');
  assert.ok(a[north] > a[1 - north], 'Sonnenseite heller');
  assert.ok(bb[north] < bb[1 - north], 'Sonne gewechselt: andere Seite hell');
  r.quality = 'high'; strokes.length = 0; shades(1);
  const courses = strokes.filter((s) => s.path && s.w === 1).length;
  r.quality = 'low'; strokes.length = 0; shades(1);
  assert.ok(courses > strokes.filter((s) => s.path && s.w === 1).length, 'Ziegelreihen nur in hoher Qualität');
});

// Welt mit einem eigenen U8-Zug, der in Kottbusser Tor (unter Tage) hält; der Spieler sitzt als Fahrgast darin.
async function u8RideWorld() {
  const { realTransit } = await import('./helpers/city.js');
  const { createWorld, updateWorld, resetPopulation } = await import('../web/src/world.js');
  const { pointOn, positionAt } = await import('../web/src/transit.js');
  const { idle } = await import('./helpers/bot.js');
  const city = realCity(), tr = realTransit();
  const u8 = tr.patterns.filter((p) => p.name === 'U8' && p.mode === 'ubahn').sort((a, b) => b.stops.length - a.stops.length)[0];
  const i = u8.stopNames.findIndex((n) => n.includes('Kottbusser Tor')), at = pointOn(u8, u8.stops[i]);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60; w.day = 1;
  w.camera.x = at.x; w.camera.y = at.y; w.player.x = at.x; w.player.y = at.y; resetPopulation(w);
  for (let k = 0; k < 30; k++) updateWorld(w, idle(), 1 / 60);
  let tau = 0; while (!(positionAt(u8, tau).stop === i && positionAt(u8, tau).dwelling)) tau += 0.5;
  w.transit.tracked.get(u8.id).veh.push({ tau, delay: 0, key: 'mine' });
  w.player.ride = { kind: 'passenger', ref: { pid: u8.id, key: 'mine' }, mode: 'ubahn', car: 2, lastStop: { x: at.x, y: at.y, name: 'x', i, pid: u8.id }, since: 0, line: 'U8', dest: 'x' };
  for (let k = 0; k < 90; k++) { updateWorld(w, idle(), 1 / 60); w.camera.x = w.player.x; w.camera.y = w.player.y; }
  return { w, at };
}

test('Tunnelansicht: als Fahrgast in der U8 unter Tage wird die Röhre mit Bahnsteigen und dem eigenen Zug gezeichnet', async () => {
  const { drawTunnels } = await import('../web/src/tunnelview.js');
  const { w, at } = await u8RideWorld();
  assert.ok(w.underground > 0.8, `Überblendung ${w.underground}`);
  const { ctx } = recordingContext();
  const v = { x: at.x - 800, y: at.y - 450, w: 1600, h: 900 };
  const r = drawTunnels(ctx, w, v, 0, w.underground);
  assert.ok(r.tubes >= 1 && r.platforms >= 1 && r.trains >= 1, JSON.stringify(r));
  assert.equal(drawTunnels(ctx, w, v, 0, 0).tubes, 0, 'oben: nichts');
});

test('Fahrgast: der Spieler wird nicht als Person gezeichnet (er sitzt im Wagen)', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  globalThis.OffscreenCanvas ??= class { getContext() { return new Proxy({}, { get: () => () => {} }); } };
  const { Renderer } = await import('../web/src/render.js');
  const { w } = await u8RideWorld();
  const r = new Renderer(recordingContext().ctx);
  const movers = r.collectMovers(w, { x: w.player.x - 800, y: w.player.y - 450, w: 1600, h: 900 }, null, { sun: null, dark: 0 }, 0, [], null);
  assert.equal(movers.filter((m) => m.o === w.player).length, 0, 'kein Spieler-Mover während der Fahrt');
  w.player.ride = null;
  const movers2 = r.collectMovers(w, { x: w.player.x - 800, y: w.player.y - 450, w: 1600, h: 900 }, null, { sun: null, dark: 0 }, 0, [], null);
  assert.equal(movers2.filter((m) => m.o === w.player).length, 1, 'zu Fuß wieder gezeichnet');
});

test('Tunnelansicht: jeder Bahnhof einmal beschriftet, ein Bahnsteig je Fahrtrichtung, ohne „(Berlin)“', async () => {
  const { drawTunnels } = await import('../web/src/tunnelview.js');
  const { w, at } = await u8RideWorld();
  const { ctx } = recordingContext();
  const labels = [];
  ctx.fillText = (s) => labels.push(String(s));
  const r = drawTunnels(ctx, w, { x: at.x - 800, y: at.y - 450, w: 1600, h: 900 }, 0, 1);
  assert.ok(labels.length >= 1, 'Bahnhof beschriftet');
  assert.equal(new Set(labels).size, labels.length, `doppelt: ${labels.join(' | ')}`);
  // je Fahrtrichtung ein Bahnsteig (Hin- und Rückrichtung liegen in eigenen Röhren), Linienvarianten zusammengefasst
  assert.ok(r.platforms >= labels.length && r.platforms <= 2 * labels.length, `${r.platforms} Bahnsteige für ${labels.length} Bahnhöfe`);
  assert.ok(labels.every((s) => !s.includes('(')), labels.join(' | '));
});

test('Tunnelansicht: Bahnhofsnamen kommen zuletzt (keine Röhre oder kein Zug darüber)', async () => {
  const { drawTunnels } = await import('../web/src/tunnelview.js');
  const { w, at } = await u8RideWorld();
  const { ctx } = recordingContext();
  const ops = [];
  ctx.fillText = () => ops.push('text');
  ctx.stroke = () => ops.push('stroke');
  ctx.fillRect = () => ops.push('rect');
  drawTunnels(ctx, w, { x: at.x - 800, y: at.y - 450, w: 1600, h: 900 }, 0, 1);
  const firstText = ops.indexOf('text'), lastOther = Math.max(ops.lastIndexOf('stroke'), ops.lastIndexOf('rect'));
  assert.ok(firstText > lastOther, `Name bei ${firstText}, danach noch gezeichnet bis ${lastOther}`);
});

test('Tunnelansicht: der eigene Zug steht am Bahnhof neben einem Bahnsteig', async () => {
  const { drawTunnels } = await import('../web/src/tunnelview.js');
  const { vehicleState } = await import('../web/src/ride.js');
  const { w, at } = await u8RideWorld();
  const st = vehicleState(w, w.player.ride.ref), mid = st.cars[Math.floor(st.cars.length / 2)];
  const r = drawTunnels(recordingContext().ctx, w, { x: at.x - 800, y: at.y - 450, w: 1600, h: 900 }, 0, 1);
  // Bahnsteig-Mittelpunkte meldet drawTunnels mit: ein Bahnsteig, dessen Achse der mittlere Wagen quer auf < 40 px
  // trifft und der ihn längs abdeckt (halbe Zuglänge), steht neben dem Zug
  const half = (st.cars.length * st.cars[0].L) / 2;
  const beside = r.platformAt.map((q) => { const dx = mid.x - q.x, dy = mid.y - q.y; return { along: Math.abs(dx * Math.cos(q.a) + dy * Math.sin(q.a)), lat: Math.abs(-dx * Math.sin(q.a) + dy * Math.cos(q.a)) }; });
  assert.ok(beside.some((b) => b.lat < 40 && b.along < half), JSON.stringify(beside.map((b) => [Math.round(b.along), Math.round(b.lat)])));
});
