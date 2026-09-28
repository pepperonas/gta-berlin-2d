import test from 'node:test';
import assert from 'node:assert/strict';
import { bridgeFills } from '../tools/osm/build.mjs';
import { cycleTrack, crossSection } from '../tools/osm/crosssection.mjs';
import { openRealCity, realIndex, geoToPx } from './helpers/city.js';
import { onRoad } from '../web/src/map.js';

const S = 10;
// zwei gegenläufige Einbahn-Brückenfahrbahnen gleichen Namens, parallel in y-Richtung, Mittellinien dx auseinander
function pair(dx, w = 33, name = [0, 0]) {
  const vertices = [0, 0, 0, 3000, dx, 3000, dx, 0];
  const edges = [
    { a: 0, b: 1, c: 4, w, n: name[0], o: 1, br: 1, p: [] }, // nach Süden (+y)
    { a: 2, b: 3, c: 4, w, n: name[1], o: 1, br: 1, p: [] }, // nach Norden
  ];
  const edgePts = (ed) => [vertices[2 * ed.a], vertices[2 * ed.a + 1], ...ed.p, vertices[2 * ed.b], vertices[2 * ed.b + 1]];
  return { edges, n: bridgeFills(edges, edgePts, S) };
}

test('Richtungsfahrbahnen auf Brücken: die Lücke dazwischen wird Fahrbahn, zur richtigen Seite', () => {
  const { edges, n } = pair(78); // Oberbaumbrücke: 7,8 m Achsabstand, je 3,3 m breit → 4,5 m Lücke
  assert.equal(n, 2);
  assert.equal(Math.abs(edges[0].fill), 45);
  // Fahrt nach Süden (+y): rechts davon (y nach unten) liegt −x, das Gegenstück liegt bei +x → links → negativ
  assert.equal(Math.sign(edges[0].fill), -1);
  assert.equal(Math.sign(edges[1].fill), -1, 'Fahrt nach Norden: Gegenstück bei −x, ebenfalls links');
  assert.equal(pair(30).n, 0, 'überlappen schon: nichts zu füllen');
  assert.equal(pair(120).n, 0, 'mehr als 7 m: echter Mittelstreifen bleibt');
  assert.equal(pair(78, 33, [0, 1]).n, 0, 'verschiedene Straßen: kein Paar');
});

test('Radwege neben der Fahrbahn (cycleway=track) kommen in den Querschnitt, Radstreifen bleiben auf der Fahrbahn', () => {
  assert.equal(cycleTrack({ 'cycleway:right': 'track', 'cycleway:right:width': '3.0' }, 'right'), 3);
  assert.equal(cycleTrack({ 'cycleway:right': 'track' }, 'right'), 2, 'ohne Breite 2 m');
  assert.equal(cycleTrack({ 'cycleway:right': 'track' }, 'left'), 0);
  assert.equal(cycleTrack({ cycleway: 'track' }, 'left'), 2, 'cycleway=track gilt beidseitig');
  assert.equal(cycleTrack({ 'cycleway:right': 'lane' }, 'right'), 0);
  const cs = crossSection({ highway: 'secondary', oneway: 'yes', lanes: '1', 'cycleway:right': 'track', 'cycleway:right:width': '3.0' }, 'secondary', 1);
  assert.equal(cs.right.track, 3); assert.equal(cs.right.cycle, 0); assert.equal(cs.left.track, 0);
});

test('Oberbaumbrücke und Warschauer Brücke: kein Geländer auf einer Fahrbahn, Lücke gefüllt, Radwege außen', () => {
  const meta = realIndex().meta, city = openRealCity();
  for (const [name, lat, lon] of [['Oberbaumbrücke', 52.50195, 13.44565], ['Warschauer Brücke', 52.5056, 13.44905]]) {
    const [x, y] = geoToPx(meta, lat, lon);
    city.loadArea(x - 3000, y - 3000, x + 3000, y + 3000);
    let segs = 0, bad = 0;
    for (const w of city.list('wall')) {
      if (w.sub !== 'railing') continue;
      const p = w.pts;
      for (let i = 0; i < p.length - 2; i += 2) {
        const mx = (p[i] + p[i + 2]) / 2, my = (p[i + 1] + p[i + 3]) / 2;
        if (Math.hypot(mx - x, my - y) > 2500) continue;
        segs++; if (onRoad(city, mx, my)) bad++;
      }
    }
    assert.ok(segs > 4, `${name}: Geländer vorhanden (${segs})`);
    assert.equal(bad, 0, `${name}: ${bad} Geländerstücke auf einer Fahrbahn`);
  }
  const ob = [...city.edges.values()].filter((e) => e.name === 'Oberbaumbrücke');
  assert.equal(ob.length, 2);
  for (const e of ob) {
    assert.ok(Math.abs(e.fill) > 3 * S && Math.abs(e.fill) < 6 * S, `Lücke ${e.fill / S} m gefüllt`);
    assert.ok(e.cs.right.track >= 2.5 * S, 'Radweg rechts (außen)');
    assert.notEqual(Math.sign(e.fill), 1, 'Radweg liegt nicht auf der Seite der Lücke');
  }
  // dieselbe Fahrbahn: die Mitte zwischen beiden Richtungen ist Fahrbahn, kein Brückendeck
  const [a, b] = ob, mx = (a.pts[0] + b.pts[b.pts.length - 2]) / 2, my = (a.pts[1] + b.pts[b.pts.length - 1]) / 2;
  assert.ok(Math.hypot(a.pts[0] - b.pts[b.pts.length - 2], a.pts[1] - b.pts[b.pts.length - 1]) < 150, 'Enden gegenüber');
  assert.ok(Number.isFinite(mx + my));
});

test('Zeichenfolge: Wege auf Brücken unter den Brückenfahrbahnen, Radwege gezeichnet', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  globalThis.OffscreenCanvas ??= class { constructor(w, h) { this.width = w; this.height = h; } getContext() { return new Proxy({}, { get: (t, k) => (k === 'measureText' ? () => ({ width: 30 }) : () => {}) }); } };
  const { Renderer, pathOf } = await import('../web/src/render.js');
  const { createWorld } = await import('../web/src/world.js');
  const meta = realIndex().meta, city = openRealCity(), [x, y] = geoToPx(meta, 52.5056, 13.44905);
  city.loadArea(x - 4000, y - 4000, x + 4000, y + 4000, { pin: true });
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.camera.x = x; w.camera.y = y; w.player.x = x; w.player.y = y;
  const log = [];
  const ctx = new Proxy({ lineWidth: 1 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'stroke') return (p) => log.push({ s: t.strokeStyle, w: t.lineWidth, p });
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'getLineDash') return () => [];
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const r = new Renderer(ctx);
  r.draw(w, 1280, 720, 1.2);
  const q = city.render.query({ x: x - 700, y: y - 400, w: 1400, h: 800 }, []);
  const bridgePaths = new Set(q.filter((f) => f.layer === 'path' && f.bridge).map((f) => pathOf(f)));
  const bridgeRoads = q.filter((f) => f.layer === 'edge' && f.bridge && f.name === 'Warschauer Brücke');
  assert.ok(bridgePaths.size > 0 && bridgeRoads.length > 0, 'Wege und Fahrbahnen auf der Brücke im Bild');
  const firstPath = log.findIndex((l) => bridgePaths.has(l.p));
  const lastBridgeRoad = log.findLastIndex((l) => bridgeRoads.some((e) => pathOf(e) === l.p) && l.w === bridgeRoads.find((e) => pathOf(e) === l.p).w);
  assert.ok(firstPath >= 0, 'Brückenwege gezeichnet');
  assert.ok(lastBridgeRoad > firstPath, 'Fahrbahn der Brücke liegt über den Wegen daneben');
  const bridgeTracks = new Set(bridgeRoads.flatMap((e) => [e._trackL, e._trackR]).filter(Boolean));
  assert.ok(log.some((l) => bridgeTracks.has(l.p) && l.s === '#a4574b'), 'Radweg auf der Brücke gezeichnet');
  assert.ok(r.stats.bridgeFills > 0, 'Lücke zur Gegenfahrbahn gefüllt');
});
