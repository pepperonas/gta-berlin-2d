import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { edgeDecals, DECAL } from '../web/src/decals.js';
import { projectOnPolyline } from '../web/src/geom.js';
import { SURFACE, PARK } from '../web/src/citycodes.js';
import { parkingStrip } from '../web/src/street.js';

const city = realCity();
const S = city.scale;

test('Gullys, Kanaldeckel, Flicken, Risse und Ölflecken liegen auf der eigenen Fahrbahn, nicht in der Kreuzung', () => {
  let n = 0;
  const kinds = new Map();
  for (const e of city.list('edge')) {
    const ds = edgeDecals(city, e);
    assert.ok(ds.length <= DECAL.maxPerEdge);
    for (const d of ds) {
      n++; kinds.set(d.t, (kinds.get(d.t) ?? 0) + 1);
      const p = projectOnPolyline(e.pts, d.x, d.y);
      const where = `${d.t} an ${e.name ?? e.id}`;
      assert.ok(Math.sqrt(p.d2) <= e.w / 2 + 0.01, `${where} liegt neben der Fahrbahn (${(Math.sqrt(p.d2) / S).toFixed(2)} m)`);
      assert.ok(p.s > Math.min(e.len / 2, e.w / 2) - 1 && p.s < e.len - Math.min(e.len / 2, e.w / 2) + 1, `${where} in der Kreuzungsfläche`);
      assert.ok(Number.isFinite(d.a) && d.l > 0);
    }
  }
  assert.ok(n > 20000, `${n} Decals im Kerngebiet`);
  for (const t of ['gully', 'manhole', 'patch', 'crack', 'oil']) assert.ok(kinds.get(t) > 100, `${t}: ${kinds.get(t)}`);
});

test('Decals: Pflasterstraßen ohne Asphaltflicken, Ölflecken nur auf Parkstreifen, deterministisch', () => {
  const es = city.list('edge').filter((e) => e.cls <= 8 && e.cs);
  const cob = es.filter((e) => e.cs.surface === SURFACE.cobble);
  assert.ok(cob.length > 50);
  for (const e of cob) assert.ok(!edgeDecals(city, e).some((d) => d.t === 'patch' || d.t === 'crack'), `Flicken auf Pflaster: ${e.name}`);
  for (const e of es) {
    const oil = edgeDecals(city, e).filter((d) => d.t === 'oil');
    if (!oil.length) continue;
    const hasPark = [-1, 1].some((side) => { const ps = parkingStrip(e.cs, side); return ps.kind === PARK.lane || ps.kind === PARK.half; });
    assert.ok(hasPark, `Ölfleck ohne Parkstreifen: ${e.name}`);
  }
  const e = es.find((x) => x.name === 'Sonnenallee' && x.len > 300);
  const a = JSON.stringify(edgeDecals(city, e));
  delete e._decals;
  assert.equal(JSON.stringify(edgeDecals(city, e)), a);
});

test('Bodentexturen: je Art ein Muster, einmal je Zeichenfläche erzeugt; ohne Canvas einfarbiger Rückfall', async () => {
  const { texture, TEXTURE_KINDS } = await import('../web/src/textures.js');
  const saved = globalThis.OffscreenCanvas;
  let tiles = 0;
  globalThis.OffscreenCanvas = class { constructor(w, h) { tiles++; this.width = w; this.height = h; } getContext() { return new Proxy({}, { get: () => () => {} }); } };
  const ctx = { createPattern: () => ({}) };
  const a = TEXTURE_KINDS.map((k) => texture(ctx, k));
  assert.ok(a.every((p) => p), 'jede Art hat ein Muster');
  assert.deepEqual(TEXTURE_KINDS.map((k) => texture(ctx, k)), a, 'zwischengespeichert');
  assert.equal(tiles, TEXTURE_KINDS.length, 'jede Kachel genau einmal gemalt');
  assert.ok(['sidewalk', 'asphalt', 'cobble', 'grass', 'wood', 'sand', 'rail'].every((k) => TEXTURE_KINDS.includes(k)));
  const failing = { createPattern: () => { throw new Error('kein Canvas'); } };
  assert.equal(texture(failing, 'grass'), null, 'ohne Canvas: null → render.js zeichnet einfarbig');
  globalThis.OffscreenCanvas = saved;
});

test('Gebrauchsspuren: Schmutz/Ausbleichen nur in Senken bzw. auf Kuppen, Ölband in jeder Spurmitte auf der Fahrbahn', async () => {
  const { grimeRGBA, laneWear } = await import('../web/src/grime.js');
  for (const kind of ['dirt', 'bleach', 'roof', 'water']) {
    for (let n = 0; n <= 1; n += 0.05) {
      const c = grimeRGBA(kind, n);
      assert.equal(c.length, 4);
      assert.ok(c.every((x) => Number.isInteger(x) && x >= 0 && x <= 255), `${kind} bei ${n}`);
    }
  }
  // Schmutz wächst mit n, Ausbleichen fällt; in der Mitte bleibt der Boden unverändert (durchsichtig)
  assert.equal(grimeRGBA('dirt', 0.2)[3], 0); assert.equal(grimeRGBA('dirt', 0.9)[3], 255);
  assert.equal(grimeRGBA('bleach', 0.9)[3], 0); assert.ok(grimeRGBA('bleach', 0.1)[3] > 200);
  assert.equal(grimeRGBA('dirt', 0.44)[3] < 20 && grimeRGBA('bleach', 0.44)[3] < 20, true, 'Mitte fast unverändert');
  let edges = 0;
  for (const e of city.list('edge')) {
    const offs = laneWear(city, e);
    if (e.cs?.surface === SURFACE.cobble || e.cls > 8) { assert.deepEqual(offs, [], `kein Band auf ${e.name}`); continue; }
    if (!offs.length) continue;
    edges++;
    for (const o of offs) assert.ok(Math.abs(o) < e.w / 2, `Band neben der Fahrbahn: ${e.name}`);
    assert.equal(new Set(offs.map((o) => Math.round(o))).size, offs.length, 'jede Spur einmal');
    assert.equal(laneWear(city, e), offs, 'zwischengespeichert');
  }
  assert.ok(edges > 2000, `${edges} Kanten mit Ölband`);
});

test('Zeichnen: Schmutz, Ölband, Kontaktschatten, Dachmoos, Wasserglanz und Vignette bei hoher Qualität', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  const mk = () => new Proxy({ canvas: { width: 1280, height: 720 }, globalAlpha: 1 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {}, setTransform() {} });
      if (k === 'createImageData') return (wd, hg) => ({ data: new Uint8ClampedArray(wd * hg * 4) });
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  globalThis.OffscreenCanvas = class { constructor(wd, hg) { this.width = wd; this.height = hg; } getContext() { return mk(); } };
  const { Renderer } = await import('../web/src/render.js');
  const { createWorld } = await import('../web/src/world.js');
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.camera.x = w.city.places.giver.x; w.camera.y = w.city.places.giver.y;
  const r = new Renderer(mk());
  r.draw(w, 1280, 720, 1.2);
  assert.equal(r.stats.grime, 3, 'drei Schmutzebenen');
  assert.ok(r.stats.laneWear > 0, 'Ölbänder');
  assert.ok(r.stats.contact > 10, `Kontaktschatten an ${r.stats.contact} Häusern`);
  assert.equal(r.stats.vignette, true);
  w.snow = 1; r.draw(w, 1280, 720, 1.2);
  assert.equal(r.stats.grime, 0, 'Schnee deckt den Schmutz zu');
});
