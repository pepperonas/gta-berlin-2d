import test from 'node:test';
import assert from 'node:assert/strict';
import { levelOf, isBridge } from '../tools/osm/levels.mjs';
import { packLvl, unpackLvl, LVL_MIN, LVL_MAX, SURFACE } from '../web/src/citycodes.js';
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

// ---------------------------------------------------------------------------------------------------------------
// Physik nach Ebenen: Wände, Kontakte, KI, Schüsse und Untergrund gelten nur auf der eigenen Ebene
// ---------------------------------------------------------------------------------------------------------------
import { blocks, createCar, stepCar, collideCarWorld } from '../web/src/car.js';
import { touch } from '../web/src/levels.js';
import { obstacleAhead } from '../web/src/traffic.js';
import { castRay } from '../web/src/combat.js';
import { surfaceAt, T, onRoad } from '../web/src/map.js';

test('Sperren nach Ebene: Stadtgrenze immer, Häuser/Bäume/Poller am Boden, Wände nur auf ihrer Ebene', () => {
  const w = {};
  assert.ok(blocks(w, { seg: true, kind: 'border' }, 2), 'Stadtgrenze sperrt jede Ebene');
  assert.ok(blocks(w, { seg: true, kind: 'building' }, 0) && blocks(w, { seg: true, kind: 'building' }, -1));
  assert.ok(!blocks(w, { seg: true, kind: 'building' }, 1), 'Brücke fährt über das Haus hinweg');
  assert.ok(blocks(w, { x: 0, y: 0, r: 3 }, 0) && !blocks(w, { x: 0, y: 0, r: 3 }, 1), 'Baum');
  assert.ok(blocks(w, { seg: true, kind: 'wall', lvl: 1 }, 1) && !blocks(w, { seg: true, kind: 'wall', lvl: 1 }, 0) && !blocks(w, { seg: true, kind: 'wall', lvl: 1 }, -1), 'Geländer');
  assert.ok(blocks(w, { seg: true, kind: 'wall' }, 0) && !blocks(w, { seg: true, kind: 'wall' }, 1), 'Ufer am Boden');
  assert.ok(!blocks({ knocked: new Map([['k', 0]]) }, { layer: 'barrier', key: 'k', x: 0, y: 0, r: 2 }, 0), 'umgefahrener Poller');
});

test('Warschauer Brücke: das Geländer steht über der Unterführung – unten fährt man durch, oben nicht; Ufer laufen unter Brücken weiter', () => {
  const { city } = warschau();
  const under = [...city.edges.values()].filter((e) => e.name === 'Tamara-Danz-Straße' && e.lvl === -1);
  // eine Stelle der Unterführung, über der ein Brückengeländer (Ebene ≥ 1) die Fahrbahn quert
  let cross = null;
  for (const e of under) for (const [x, y] of walk(e.pts, 5)) {
    if (cross) break;
    for (const s of city.solids.query({ x: x - 3, y: y - 3, w: 6, h: 6 }, [])) {
      if (!s.seg || s.kind !== 'wall' || !(s.lvl >= 1)) continue;
      const dx = s.bx - s.ax, dy = s.by - s.ay, L2 = dx * dx + dy * dy || 1, t = Math.max(0, Math.min(1, ((x - s.ax) * dx + (y - s.ay) * dy) / L2));
      if (Math.hypot(s.ax + dx * t - x, s.ay + dy * t - y) < 3) { cross = { x, y, e, wall: s }; break; }
    }
  }
  assert.ok(cross, 'Geländer der Brücke quert die Unterführung (vorher wurde es dort weggeschnitten)');
  // ein Auto fährt die Unterführung entlang durch diese Stelle: auf Ebene −1 frei, mit Ebene 1 prallt es am Geländer ab
  const drive = (lvl) => {
    const p = cross.e.pts;
    let best = 0; for (let i = 0; i < p.length - 2; i += 2) { const d = Math.hypot(p[i] - cross.x, p[i + 1] - cross.y); if (i === 0 || d < best) best = i; }
    const i = Math.min(best, p.length - 4), a = Math.atan2(p[i + 3] - p[i + 1], p[i + 2] - p[i]);
    const c = createCar({ x: cross.x - Math.cos(a) * 80, y: cross.y - Math.sin(a) * 80, angle: a }); c.lvl = lvl;
    c.vx = Math.cos(a) * 250; c.vy = Math.sin(a) * 250;
    const world = { solids: city.solids, knocked: new Map() }, ev = [];
    for (let k = 0; k < 60; k++) { c.controls.throttle = 0.4; stepCar(c, 1 / 60, city); collideCarWorld(c, world, ev); }
    return (c.x - cross.x) * Math.cos(a) + (c.y - cross.y) * Math.sin(a); // wie weit hinter dem Geländer
  };
  assert.ok(drive(-1) > 60, 'unten: unter der Brücke hindurch');
  assert.ok(drive(1) < 5, 'Gegenprobe: auf Ebene 1 ist es eine Wand');
  // Ufer unter der Oberbaumbrücke: Uferwand (Ebene 0) liegt jetzt unter der Brückenfahrbahn
  const meta = realIndex().meta, c2 = openRealCity(), [ox, oy] = geoToPx(meta, 52.50195, 13.44565);
  c2.loadArea(ox - 3000, oy - 3000, ox + 3000, oy + 3000);
  const ob = [...c2.edges.values()].filter((e) => e.name === 'Oberbaumbrücke');
  const quayUnder = c2.list('wall').filter((w) => w.sub === 'quay').some((w) => w.pts.some((_, i) => i % 2 === 0 && ob.some((e) => onSurface(levelSurfaces({ edges: [e] }, 1)[0], w.pts[i], w.pts[i + 1]))));
  assert.ok(quayUnder, 'Kaimauer läuft unter der Brücke durch');
  // Untergrund: oben Brücke, unten Wasser
  const e = ob[0], mx = (e.pts[0] + e.pts[e.pts.length - 2]) / 2, my = (e.pts[1] + e.pts[e.pts.length - 1]) / 2;
  assert.notEqual(surfaceAt(c2, mx, my, 1), T.WATER, 'auf der Brücke kein Wasser');
  // auf der Fahrbahnachse mitten über der Spree: oben Straße, unten Wasser
  const onWater = [];
  const along = walk(e.pts, 10);
  for (const [x, y] of along.slice(Math.floor(along.length * 0.2), Math.ceil(along.length * 0.8))) if (surfaceAt(c2, x, y, 0) === T.WATER) onWater.push([x, y]);
  assert.ok(onWater.length, 'unter der Brückenfahrbahn liegt Wasser (Ebene 0)');
  const [wx, wy] = onWater[Math.floor(onWater.length / 2)];
  assert.equal(surfaceAt(c2, wx, wy, 1), e.cs.surface === SURFACE.cobble ? T.COBBLE : T.ROAD, 'oben: Fahrbahn der Brücke');
  // neben der Fahrbahn auf der Brücke (Gehweg): oben fester Grund, unten weiter Wasser
  const ux = e.pts[e.pts.length - 2] - e.pts[0], uy = e.pts[e.pts.length - 1] - e.pts[1], ul = Math.hypot(ux, uy) || 1;
  let side = null;
  for (const sgn of [1, -1]) {
    const off = e.w / 2 + Math.abs(e.fill ?? 0) + 30, sx = wx - uy / ul * off * sgn, sy = wy + ux / ul * off * sgn;
    if (!side && surfaceAt(c2, sx, sy, 0) === T.WATER && !onRoad(c2, sx, sy, 0, 1)) side = [sx, sy];
  }
  assert.ok(side, 'Stelle neben der Brückenfahrbahn über dem Wasser');
  assert.equal(surfaceAt(c2, side[0], side[1], 1), T.PLAZA, 'Gehweg/Deck der Brücke, nicht Wasser');
});

test('Kontakte nur auf derselben Ebene (im Portal beide): Auto oben bremst nicht für Auto unten, Schuss trifft nicht durchs Deck', () => {
  const city = { portals: { query: () => [{ x: 0, y: 0, r: 50, lo: 0, hi: 1 }] } };
  assert.ok(touch(city, { x: 500, y: 0, lvl: 1 }, { x: 510, y: 0, lvl: 1 }));
  assert.ok(!touch(city, { x: 500, y: 0, lvl: 1 }, { x: 510, y: 0, lvl: 0 }), 'über/unter der Brücke');
  assert.ok(touch(city, { x: 10, y: 0, lvl: 1 }, { x: -10, y: 0, lvl: 0 }), 'am Rampenfuß berühren sich beide Ebenen');
  assert.ok(!touch(city, { x: 10, y: 0, lvl: 2 }, { x: -10, y: 0, lvl: 0 }), 'Portal verbindet nur seine Ebenen');
  // KI: stehendes Auto direkt voraus – auf derselben Ebene ein Hindernis, eine Ebene tiefer nicht
  const me = createCar({ x: 1000, y: 1000, angle: 0 }); me.lvl = 1; me.driver = 'npc';
  const other = createCar({ x: 1050, y: 1000, angle: 0 }); other.driver = null;
  const world = { city: { portals: { query: () => [] } }, cars: [me, other], peds: [], bikes: [], railObs: [], player: { inCar: 1, x: 0, y: 0 } };
  other.lvl = 1; assert.ok(obstacleAhead(me, world).dOther < 100, 'gleiche Ebene: bremsen');
  other.lvl = -1; assert.equal(obstacleAhead(me, world).dOther, Infinity, 'darunter: weiterfahren');
  // Schuss: trifft den Passanten auf der eigenen Ebene, den auf der Straße unter der Brücke nicht
  const ped = { x: 1100, y: 1000, lvl: 0, state: 'walk' };
  const w2 = { solids: { query: () => [] }, peds: [ped], cars: [], player: { inCar: null } };
  assert.equal(castRay(w2, 1000, 1000, 0, 300, null, 0).hit?.obj, ped);
  assert.equal(castRay(w2, 1000, 1000, 0, 300, null, 1).hit, null, 'durchs Deck hindurch trifft man nicht');
});
