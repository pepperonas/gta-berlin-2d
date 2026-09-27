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
