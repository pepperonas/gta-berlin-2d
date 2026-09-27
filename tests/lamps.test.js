import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { edgeLamps, LAMP, LAMP_RGB } from '../web/src/lamps.js';
import { treeOnRoad, inBuilding, surfaceAt, T } from '../web/src/map.js';

const city = realCity();
const S = city.scale;

test('Straßenlaternen stehen auf dem Gehweg: nie auf einer Fahrbahn, in einem Haus oder im Wasser', () => {
  let n = 0;
  for (const e of city.list('edge')) {
    for (const lp of edgeLamps(city, e)) {
      n++;
      const where = `${e.name ?? e.id} (${Math.round(lp.x)}, ${Math.round(lp.y)})`;
      assert.equal(treeOnRoad(city, { x: lp.x, y: lp.y, r: 0.3 * S }), null, `Laterne auf der Fahrbahn: ${where}`);
      assert.equal(inBuilding(city, lp.x, lp.y), null, `Laterne im Haus: ${where}`);
      assert.notEqual(surfaceAt(city, lp.x, lp.y), T.WATER, `Laterne im Wasser: ${where}`);
      assert.ok(Math.abs(Math.hypot(lp.nx, lp.ny) - 1) < 1e-6, 'Ausleger-Richtung normiert');
    }
  }
  assert.ok(n > 10000, `${n} Laternen im Kerngebiet`);
});

test('Laternen: sinnvolle Dichte, Ausleger zur Fahrbahn, Gaslaternen warm, deterministisch', () => {
  const byName = (name) => city.list('edge').filter((e) => e.name === name);
  const perKm = (name) => {
    const es = byName(name), len = es.reduce((a, e) => a + e.len, 0) / S / 1000;
    return es.reduce((a, e) => a + edgeLamps(city, e).length, 0) / len;
  };
  // Hauptstraße, beidseitig: rund 2 × 1000/25 abzüglich Kreuzungen und Zufahrten
  const sa = perKm('Sonnenallee');
  assert.ok(sa > 30 && sa < 90, `Sonnenallee: ${sa.toFixed(1)} Laternen je km`);
  const ws = perKm('Weserstraße');
  assert.ok(ws > 15 && ws < 80, `Weserstraße: ${ws.toFixed(1)} Laternen je km`);
  // Ausleger zeigt zur Straßenmitte
  for (const e of byName('Oranienstraße')) for (const lp of edgeLamps(city, e)) {
    const tip = [lp.x + lp.nx * e.w / 2, lp.y + lp.ny * e.w / 2];
    assert.ok(treeOnRoad(city, { x: tip[0], y: tip[1], r: 1 }), 'Ausleger zeigt über die Fahrbahn');
  }
  const gas = city.list('edge').filter((e) => e.cs?.gaslight).flatMap((e) => edgeLamps(city, e));
  assert.ok(gas.length > 20, `${gas.length} Gaslaternen`);
  assert.ok(gas.every((lp) => lp.gas && lp.rgb === LAMP_RGB.gas));
  // Neu berechnet ergibt dasselbe
  const e = byName('Sonnenallee')[3];
  const a = JSON.stringify(edgeLamps(city, e));
  delete e._lamps;
  assert.equal(JSON.stringify(edgeLamps(city, e)), a);
  assert.ok(LAMP.mainSpacing < LAMP.sideSpacing);
});

test('Grundstückszufahrten zählen nicht als Kreuzung (sonst fehlen Laternen an jeder Einfahrt)', () => {
  // Nebenstraße in Wohnlage: mindestens eine Laterne je 60 m Straße (ohne die Regel waren es auf kurzen Stücken 0)
  const ws = city.list('edge').filter((e) => e.name === 'Wrangelstraße');
  const len = ws.reduce((a, e) => a + e.len, 0) / S;
  const n = ws.reduce((a, e) => a + edgeLamps(city, e).length, 0);
  assert.ok(n >= len / 60, `Wrangelstraße: ${n} Laternen auf ${len.toFixed(0)} m`);
});
