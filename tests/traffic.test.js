import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorld, updateWorld } from '../web/src/world.js';
import { tileAt, T } from '../web/src/map.js';
import { obbVsRect } from '../web/src/collision.js';
import { allowedExits, turnPath, laneCoord } from '../web/src/traffic.js';
import { idle } from './helpers/bot.js';

test('Kreuzungslogik: keine Wende, am Rand eingeschränkt', () => {
  assert.deepEqual(allowedExits(0, 0, 'N').sort(), ['E']);         // Ecke oben links, von unten kommend
  assert.ok(!allowedExits(3, 3, 'E').includes('W'));
  assert.equal(allowedExits(3, 3, 'E').length, 3);
});

test('Abbiegepfad endet exakt auf der Ausfahrtsspur (Rechtsverkehr)', () => {
  const w = createWorld({ cars: 0, pedestrians: 0 });
  const p = turnPath(w.city, 'S', 'E', 2, 2);
  const end = p[p.length - 1];
  assert.equal(end.y, laneCoord(w.city, 'E', 2));
  assert.ok(p.every((q) => q.slow));
  // Südwärts fährt auf der West-Hälfte der Straße.
  assert.ok(laneCoord(w.city, 'S', 2) < laneCoord(w.city, 'N', 2));
});

test('Dauertest 90 s mit Verkehr und Passanten: alles bleibt auf der Straße bzw. dem Gehweg', () => {
  const w = createWorld({});
  let samples = 0, offRoad = 0, pedBad = 0, progress = 0;
  const start = new Map(w.cars.filter((c) => c.driver === 'npc').map((c) => [c.id, { odo: 0, lx: c.x, ly: c.y }]));
  for (let i = 0; i < 90 * 60; i++) {
    updateWorld(w, idle(), 1 / 60);
    if (i % 30) continue;
    for (const c of w.cars) {
      assert.ok(Number.isFinite(c.x) && Number.isFinite(c.y), 'NaN-Position');
      for (const b of w.city.buildings) {
        const m = obbVsRect(c, b);
        assert.ok(!m || m.depth < 4, `Auto ${c.id} steckt ${m?.depth.toFixed(1)} px in einem Gebäude`);
      }
      if (c.driver !== 'npc') continue;
      samples++;
      const t = tileAt(w.city, c.x, c.y);
      if (t !== T.ROAD) offRoad++;
      const s = start.get(c.id);
      if (s) { s.odo += Math.hypot(c.x - s.lx, c.y - s.ly); s.lx = c.x; s.ly = c.y; }
    }
    for (const p of w.peds) {
      const t = tileAt(w.city, p.x, p.y);
      if (t === T.BUILDING || t === T.WATER) pedBad++;
    }
  }
  for (const s of start.values()) if (s.odo > 1500) progress++;
  const share = offRoad / samples;
  console.log(`# Verkehr: ${(share * 100).toFixed(1)} % Stichproben neben der Fahrbahn, ${progress}/${start.size} Autos > 1,5 km gefahren`);
  assert.ok(share < 0.02, `zu oft neben der Straße: ${(share * 100).toFixed(1)} %`);
  assert.equal(pedBad, 0, 'Passant in Gebäude/Wasser');
  assert.ok(progress >= start.size * 0.8, `Verkehr kommt nicht voran: ${progress}/${start.size}`);
});
