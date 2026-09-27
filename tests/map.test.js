import test from 'node:test';
import assert from 'node:assert/strict';
import { buildCity, tileAt, T, LAYOUT, locationName } from '../web/src/map.js';
import { MAP_W, MAP_H, TILE, WORLD_W, WORLD_H } from '../web/src/config.js';

test('Stadt ist deterministisch', () => {
  const a = buildCity({ seed: 5 }), b = buildCity({ seed: 5 });
  assert.deepEqual(a.tiles, b.tiles);
  assert.equal(a.buildings.length, b.buildings.length);
});

test('alle Missionsorte existieren und liegen auf begehbarem/befahrbarem Grund', () => {
  const c = buildCity();
  for (const k of ['giver', 'pickup', 'dropoff', 'playerSpawn', 'playerCar']) {
    const p = c.places[k];
    assert.ok(p, k);
    assert.notEqual(tileAt(c, p.x, p.y), T.BUILDING, `${k} liegt in einem Gebäude`);
    assert.notEqual(tileAt(c, p.x, p.y), T.WATER, `${k} liegt im Wasser`);
  }
});

test('Straßennetz ist zusammenhängend (Flutfüllung über Straße/Platz/Gehweg erreicht Abhol- und Abgabeort)', () => {
  const c = buildCity();
  const ok = (t) => t === T.ROAD || t === T.PLAZA || t === T.SIDEWALK;
  const seen = new Uint8Array(MAP_W * MAP_H);
  const start = [Math.floor(c.places.giver.x / TILE), Math.floor(c.places.giver.y / TILE)];
  const q = [start]; seen[start[1] * MAP_W + start[0]] = 1;
  while (q.length) {
    const [x, y] = q.pop();
    for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
      const nx = x + dx, ny = y + dy;
      if (nx < 0 || ny < 0 || nx >= MAP_W || ny >= MAP_H) continue;
      const i = ny * MAP_W + nx;
      if (!seen[i] && ok(c.tiles[i])) { seen[i] = 1; q.push([nx, ny]); }
    }
  }
  for (const k of ['pickup', 'dropoff', 'playerCar']) {
    const p = c.places[k];
    assert.ok(seen[Math.floor(p.y / TILE) * MAP_W + Math.floor(p.x / TILE)], `${k} nicht erreichbar`);
  }
});

test('Gebäude-Kacheln und Gebäude-Rechtecke stimmen überein', () => {
  const c = buildCity();
  for (const b of c.buildings) {
    assert.equal(tileAt(c, b.x + b.w / 2, b.y + b.h / 2), T.BUILDING);
    assert.ok(b.x >= 0 && b.y >= 0 && b.x + b.w <= WORLD_W && b.y + b.h <= WORLD_H);
  }
});

test('falsches LAYOUT wird abgelehnt, fehlender Späti ebenso', () => {
  assert.throws(() => buildCity({ layout: ['BBB'] }));
  assert.throws(() => buildCity({ layout: LAYOUT.map((r) => r.replace('S', 'B')) }), /giver/);
});

test('Straßennamen für die HUD-Anzeige', () => {
  const c = buildCity();
  assert.match(locationName(c, c.vRoads[1] + 10, 500), /straße|Straße/);
  assert.match(locationName(c, c.vRoads[1] + 10, c.hRoads[0] + 10), / \/ /);
});
