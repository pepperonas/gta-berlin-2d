import test from 'node:test';
import assert from 'node:assert/strict';
import { levelOf, isBridge } from '../tools/osm/levels.mjs';
import { packLvl, unpackLvl, LVL_MIN, LVL_MAX } from '../web/src/citycodes.js';
import { stepLevel, initialLevel } from '../web/src/levels.js';
import { levelSurfaces, surfacesOver, onSurface, trackLevel } from '../web/src/occlusion.js';
import { openRealCity, realIndex, geoToPx } from './helpers/city.js';

// Warschauer Brücke / Helen-Ernst-Straße / Tamara-Danz-Straße (Unterführung unter der Brückenrampe)
function warschau() {
  const meta = realIndex().meta, city = openRealCity(), [x, y] = geoToPx(meta, 52.5056, 13.44905);
  city.loadArea(x - 4000, y - 4000, x + 4000, y + 4000, { pin: true });
  return { city, x, y };
}
const walk = (pts, step = 4) => { // Punkte im Abstand step entlang einer Linie
  const out = [];
  for (let i = 0; i < pts.length - 2; i += 2) {
    const dx = pts[i + 2] - pts[i], dy = pts[i + 3] - pts[i + 1], n = Math.max(1, Math.ceil(Math.hypot(dx, dy) / step));
    for (let k = 0; k < n; k++) out.push([pts[i] + dx * k / n, pts[i + 1] + dy * k / n]);
  }
  out.push([pts[pts.length - 2], pts[pts.length - 1]]);
  return out;
};

test('Ebene aus OSM: Brücke ≥ 1, offene Unterführung negativ, Tunnel verworfen, layer ohne Brücke = Boden', () => {
  assert.equal(levelOf({ bridge: 'yes' }), 1);
  assert.equal(levelOf({ bridge: 'yes', layer: '2' }), 2);
  assert.equal(levelOf({ bridge: 'viaduct', layer: '-1' }), 1, 'Brücke liegt immer oben');
  assert.equal(levelOf({ bridge: 'no', layer: '1' }), 0, 'bridge=no ist keine Brücke');
  assert.equal(isBridge({ bridge: 'no' }), false);
  assert.equal(levelOf({ layer: '-1' }), -1, 'Unterführung/Einschnitt');
  assert.equal(levelOf({ layer: '-5' }), LVL_MIN, 'geklemmt');
  assert.equal(levelOf({ bridge: 'yes', layer: '7' }), LVL_MAX, 'geklemmt');
  assert.equal(levelOf({ layer: '1' }), 0, 'layer ohne Brückentag: Boden');
  assert.equal(levelOf({ tunnel: 'yes', layer: '-1' }), null, 'Tunnel: verworfen');
  assert.equal(levelOf({ tunnel: 'culvert' }), null);
  assert.equal(levelOf({ tunnel: 'building_passage' }), 0, 'Durchfahrt bleibt am Boden');
  assert.equal(levelOf({}), 0);
  for (let l = LVL_MIN; l <= LVL_MAX; l++) assert.equal(unpackLvl(packLvl(l)), l, `Bits für ${l}`);
  assert.equal(unpackLvl(packLvl(-1) << 4 >> 4), -1);
});

test('Warschauer Brücke: Brücke oben, Tamara-Danz-Straße als Unterführung, keine Kreuzung zwischen beiden', () => {
  const { city, x, y } = warschau();
  const edges = [...city.edges.values()];
  const bridge = edges.filter((e) => e.name === 'Warschauer Brücke'), under = edges.filter((e) => e.name === 'Tamara-Danz-Straße');
  const helen = edges.filter((e) => e.name === 'Helen-Ernst-Straße');
  assert.ok(bridge.length > 0 && under.length > 0 && helen.length > 0, 'alle drei Straßen geladen');
  assert.ok(bridge.every((e) => e.lvl >= 1), 'Brücke oben');
  assert.ok(under.some((e) => e.lvl === -1), 'Tamara-Danz-Straße unter der Rampe');
  assert.ok(helen.every((e) => e.lvl === 0), 'Helen-Ernst-Straße am Boden');
  const nodes = (list) => new Set(list.flatMap((e) => [e.a, e.b]));
  const nb = nodes(bridge), nu = nodes(under), nh = nodes(helen);
  assert.ok(![...nu].some((n) => nb.has(n)), 'Unterführung und Brücke teilen keinen Knoten');
  assert.ok([...nu].some((n) => nh.has(n)), 'Helen-Ernst geht in die Tamara-Danz-Straße über');
  // Übergang zwischen den Ebenen gibt es nur in einem Portal
  const portals = city.portals.query({ x: x - 3000, y: y - 3000, w: 6000, h: 6000 }, []);
  assert.ok(portals.some((p) => p.lo === -1 && p.hi === 0), 'Portal Helen-Ernst ↔ Tamara-Danz');
  assert.ok(portals.some((p) => p.lo === 0 && p.hi === 1), 'Portal am Brückenkopf');
});

test('Ebene je Objekt: am Brückenkopf hinauf, unter der Brücke hindurch bleibt man unten', () => {
  const { city } = warschau();
  const edges = [...city.edges.values()];
  // eine Brückenfahrbahn mit einem Ende am Boden: Bodenkante → Knoten → Brückenkante
  let upPath = null;
  for (const b of edges.filter((e) => e.name === 'Warschauer Brücke' && e.lvl === 1)) {
    for (const [nid, atStart] of [[b.a, true], [b.b, false]]) {
      const g = edges.find((e) => e.lvl === 0 && e.cls <= 8 && (e.a === nid || e.b === nid));
      if (!g) continue;
      const gp = g.b === nid ? g.pts : reverse(g.pts), bp = atStart ? b.pts : reverse(b.pts);
      upPath = [...gp.slice(0, -2), ...bp];
      break;
    }
    if (upPath) break;
  }
  assert.ok(upPath, 'Rampe gefunden');
  const car = { x: upPath[0], y: upPath[1], lvl: undefined };
  car.lvl = initialLevel(city, car.x, car.y, Math.atan2(upPath[3] - upPath[1], upPath[2] - upPath[0]));
  assert.equal(car.lvl, 0, 'startet am Boden');
  for (const [x, y] of walk(upPath)) { car.x = x; car.y = y; stepLevel(city, car); }
  assert.equal(car.lvl, 1, 'oben auf der Brücke angekommen');

  // Tamara-Danz-Straße unter der Rampe hindurch: bleibt die ganze Zeit auf −1
  const under = edges.filter((e) => e.name === 'Tamara-Danz-Straße' && e.lvl === -1).sort((a, b) => polyLen(b.pts) - polyLen(a.pts))[0];
  const ped = { x: under.pts[0], y: under.pts[1], lvl: -1 };
  const levels = new Set();
  for (const [x, y] of walk(under.pts)) { ped.x = x; ped.y = y; stepLevel(city, ped); if (!inPortal(city, x, y)) levels.add(ped.lvl); }
  assert.deepEqual([...levels], [-1], 'außerhalb der Portale immer unten');
});

test('Portal: wer auf seiner Straße ist, bleibt auf ihrer Ebene, auch wenn die Rampe näher liegt; neben beiden ändert sich nichts', () => {
  // Rampenfuß bei (0, 0): Bodenstraße entlang x, Brückenrampe (Ebene 1) nach Norden; beide 6 m breit
  const hash = (items) => ({ query: () => items });
  const seg = (e, ax, ay, bx, by) => ({ e, ax, ay, bx, by });
  const groundE = { lvl: 0, w: 60, cs: {} }, rampE = { lvl: 1, w: 60, cs: {} };
  const city = {
    portals: hash([{ x: 0, y: 0, r: 100, lo: 0, hi: 1 }]),
    edgeSegs: hash([seg(groundE, -500, 0, 500, 0), seg(rampE, 0, 0, 0, -500)]),
    render: hash([]), polys: hash([]),
  };
  const car = { x: 20, y: -25, lvl: 0 }; // auf der Bodenstraße (25 px neben der Achse), der Rampenachse näher (20 px)
  stepLevel(city, car);
  assert.equal(car.lvl, 0, 'bleibt unten');
  const up = { x: 20, y: -25, lvl: 1 };
  stepLevel(city, up);
  assert.equal(up.lvl, 1, 'wer oben ist, bleibt oben');
  const side = { x: 60, y: -60, lvl: 0 }; // auf keiner der beiden Fahrbahnen (Gehweg)
  stepLevel(city, side);
  assert.equal(side.lvl, 0);
  const onRamp = { x: 5, y: -60, lvl: 0 }; // mitten auf der Rampe, nicht mehr auf der Straße
  stepLevel(city, onRamp);
  assert.equal(onRamp.lvl, 1, 'auf der Rampe: hinauf');
});

test('Verdeckung nach Ebenen: Flächen darüber decken, im Portal nicht; Gleis auf der Brücke liegt oben', () => {
  const road = { lvl: 1, w: 60, cs: { left: {}, right: { track: 20 } }, pts: [0, 0, 1000, 0], bbox: { x: 0, y: 0, w: 1000, h: 0 } };
  const ground = { lvl: 0, w: 60, cs: {}, pts: [500, -500, 500, 500], bbox: { x: 500, y: -500, w: 0, h: 1000 } };
  const deck = { lvl: 2, rings: [[0, 100, 100, 100, 100, 200, 0, 200]], bbox: { x: 0, y: 100, w: 100, h: 100 } };
  const S = levelSurfaces({ edges: [road, ground], decks: [deck] }, 1);
  assert.equal(S.length, 2, 'nur Ebenen ≥ 1');
  assert.ok(onSurface(S[0], 200, 30 + 24 - 1), 'Radweg gehört zur Brückenfläche');
  assert.ok(!onSurface(S[0], 200, 30 + 24 + 2), 'daneben nicht');
  assert.equal(surfacesOver([[200, 10]], 0, S).length, 1, 'am Boden unter der Brücke: verdeckt');
  assert.equal(surfacesOver([[200, 10]], 1, S).length, 0, 'auf der Brücke: nicht von sich selbst verdeckt');
  assert.equal(surfacesOver([[50, 150]], 1, S)[0]?.kind, 'deck', 'Deck der Ebene 2 liegt über Ebene 1');
  assert.equal(surfacesOver([[200, 10]], 0, S, (x) => x < 300).length, 0, 'im Portal (Rampenfuß) nicht verdeckt');
  const G = levelSurfaces({ edges: [ground] }, 0);
  assert.equal(trackLevel(200, 0, S, G), 1, 'nur Brücke: oben');
  assert.equal(trackLevel(500, 0, S, G, 1), 1, 'Brücke über Straße: wie davor');
  assert.equal(trackLevel(500, 0, S, G, 0), 0);
  assert.equal(trackLevel(500, 300, S, G), 0, 'nur Straße: unten');
});

test('Zeichnen nach Ebenen: Auto unter der Brücke kommt vor der Brücke und bekommt einen Umriss, Auto oben nicht', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  globalThis.OffscreenCanvas ??= class { constructor(w, h) { this.width = w; this.height = h; } getContext() { return new Proxy({}, { get: (t, k) => (k === 'measureText' ? () => ({ width: 30 }) : () => {}) }); } };
  const { Renderer } = await import('../web/src/render.js');
  const { createWorld } = await import('../web/src/world.js');
  const { city } = warschau();
  const edges = [...city.edges.values()];
  const bridgeSurf = levelSurfaces({ edges: edges.filter((e) => e.name === 'Warschauer Brücke') }, 1);
  const under = edges.filter((e) => e.name === 'Tamara-Danz-Straße' && e.lvl === -1);
  let spot = null;
  for (const e of under) for (const [x, y] of walk(e.pts, 10)) if (!spot && !inPortal(city, x, y) && bridgeSurf.some((s) => onSurface(s, x, y))) spot = [x, y, Math.atan2(e.pts[3] - e.pts[1], e.pts[2] - e.pts[0])];
  assert.ok(spot, 'Stelle der Tamara-Danz-Straße unter der Brücke');
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  const [x, y, a] = spot;
  let onTop = null; // Punkt auf der Brückenfahrbahn nahe der Unterführung
  for (const sf of bridgeSurf) for (const [px, py] of walk(sf.pts, 10)) { const d = Math.hypot(px - x, py - y); if (d > 60 && (!onTop || d < onTop.d)) onTop = { x: px, y: py, d }; }
  const { createCar } = await import('../web/src/car.js');
  const low = Object.assign(createCar({ x, y, angle: a, role: 'parked' }), { lvl: -1 });
  const high = Object.assign(createCar({ x: onTop.x, y: onTop.y, role: 'parked' }), { lvl: 1 });
  w.cars.push(low, high);
  w.camera.x = x; w.camera.y = y; w.player.x = x + 3000; w.player.y = y;
  const ctx = new Proxy({ lineWidth: 1 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'getLineDash') return () => [];
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const r = new Renderer(ctx);
  r.draw(w, 1280, 720, 1.2);
  const below = r._movers.find((m) => m.o === low), above = r._movers.find((m) => m.o === high);
  assert.ok(r.stats.levels >= 3, 'Unterführung, Boden, Brücke im Bild');
  assert.ok(below.early, 'unter der Brücke: vor der Brücke gezeichnet');
  assert.ok(r._covered.some((c) => Math.abs(c.x - x) < 1 && c.occ.some((o) => o.kind === 'road' || o.kind === 'disc' || o.kind === 'deck')), 'Umriss unter der Brücke');
  assert.ok(!above.early, 'auf der Brücke: in der Tiefenliste danach');
  assert.ok(!(above.over ?? []).length, 'auf der Brücke von nichts Höherem verdeckt');
});

function reverse(p) { const o = []; for (let i = p.length - 2; i >= 0; i -= 2) o.push(p[i], p[i + 1]); return o; }
function polyLen(p) { let s = 0; for (let i = 0; i < p.length - 2; i += 2) s += Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]); return s; }
function inPortal(city, x, y) { return city.portals.query({ x: x - 1, y: y - 1, w: 2, h: 2 }, []).some((p) => Math.hypot(x - p.x, y - p.y) <= p.r + 20); }
