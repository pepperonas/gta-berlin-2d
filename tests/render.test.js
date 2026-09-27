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
  globalThis.Path2D ??= class { moveTo() {} lineTo() {} closePath() {} rect() {} addPath() {} arc() {} };
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
