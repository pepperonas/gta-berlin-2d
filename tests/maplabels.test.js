import test from 'node:test';
import assert from 'node:assert/strict';
import { mapLabels, prepareStreets, LABEL_TIERS } from '../web/src/maplabels.js';
import { undelta, delta } from '../web/src/geom.js';
import { realOverview } from './helpers/city.js';

const measure = (t, st) => t.length * st.size * 0.55; // grobe Textbreite ohne Canvas
const ov = realOverview();
const data = { labels: ov.labels, ortsteile: ov.ortsteile, kieze: ov.kieze, stations: ov.stations, streets: prepareStreets(ov.roads, ov.names, undelta) };
// Ansicht (HUD 1280 × 588 px) mit gegebenem Maßstab (m je Bildpunkt) um einen Weltpunkt
const view = (wx, wy, mpp) => { const f = 1 / (mpp * 10), w = 1180, h = 588, x = 50, y = 66; return { f, ox: x + w / 2 - wx * f, oy: y + h / 2 - wy * f, x, y, w, h, mpp }; };
const at = (name, list = ov.kieze) => list.find((q) => q[2] === name);
const kinds = (ls) => new Set(ls.map((l) => l.kind));
const overlap = (a, b) => a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0;

test('Stadtplan-Beschriftung: Bezirke, beim Heranzoomen Ortsteile, dann Kieze und Bahnhöfe, dann Straßennamen', () => {
  const [kx, ky] = at('Flughafenkiez');
  const far = mapLabels(data, view(232000, 190000, 64), measure);
  assert.deepEqual([...kinds(far)], ['bezirk'], 'ganz Berlin: nur Bezirke');
  assert.ok(far.length >= 11, `${far.length} Bezirke (Textbreite hier nur geschätzt; im Browser stehen alle 12)`);
  const mid = mapLabels(data, view(kx, ky, 20), measure);
  assert.ok(kinds(mid).has('ortsteil') && !kinds(mid).has('kiez') && !kinds(mid).has('street'));
  const t = mapLabels(data, view(...at('Tegel', ov.ortsteile).slice(0, 2), 20), measure);
  assert.ok(t.some((l) => l.text === 'Tegel'), 'Ortsteil Tegel');
  const p = mapLabels(data, view(...at('Prenzlauer Berg', ov.ortsteile).slice(0, 2), 20), measure);
  assert.ok(p.some((l) => l.text === 'Prenzlauer Berg'), 'Ortsteil Prenzlauer Berg');
  const near = mapLabels(data, view(kx, ky, 6), measure);
  assert.ok(near.some((l) => l.kind === 'kiez' && l.text === 'Flughafenkiez'), 'Flughafenkiez');
  assert.ok(near.some((l) => l.kind === 'station' && l.text === 'Rathaus Neukölln'), 'Bahnhof');
  assert.ok(!kinds(near).has('bezirk') && !kinds(near).has('street'));
  const main = mapLabels(data, view(kx, ky, 3), measure);
  assert.ok(main.some((l) => l.kind === 'mainStreet' && l.text === 'Karl-Marx-Straße'), 'Hauptstraße ab ~3 m/px');
  const close = mapLabels(data, view(kx, ky, 1.5), measure);
  const streets = close.filter((l) => l.kind === 'street' || l.kind === 'mainStreet').map((l) => l.text);
  for (const n of ['Flughafenstraße', 'Boddinstraße']) assert.ok(streets.includes(n), `${n}: ${streets.join(', ')}`);
  assert.ok(streets.length >= 8, `${streets.length} Straßennamen`);
  assert.deepEqual(LABEL_TIERS.street, [0, 2.6]);
});

test('Stadtplan-Beschriftung: nichts überlappt, nichts ragt aus der Karte oder über die Hinweisleiste, Text nie kopfüber', () => {
  const [kx, ky] = at('Flughafenkiez');
  for (const mpp of [64, 30, 12, 6, 3, 1.5]) {
    const v = view(kx, ky, mpp);
    const hint = { x0: v.x + 10, y0: v.y + v.h - 40, x1: v.x + 480, y1: v.y + v.h - 10 };
    const ls = mapLabels(data, v, measure, [hint]);
    const boxes = ls.map((l) => {
      const tw = measure(l.text, l), c = Math.abs(Math.cos(l.angle)), s = Math.abs(Math.sin(l.angle));
      const hw = (tw * c + l.size * s) / 2, hh = (tw * s + l.size * c) / 2;
      return { x0: l.x - hw, y0: l.y - hh, x1: l.x + hw, y1: l.y + hh, l };
    });
    for (const b of boxes) {
      assert.ok(b.x0 >= v.x && b.x1 <= v.x + v.w && b.y0 >= v.y && b.y1 <= v.y + v.h, `${b.l.text} ragt heraus`);
      assert.ok(!overlap(b, hint), `${b.l.text} unter der Hinweisleiste`);
      assert.ok(Math.abs(b.l.angle) <= Math.PI / 2 + 1e-9, `${b.l.text} kopfüber`);
    }
    for (let i = 0; i < boxes.length; i++) for (let j = i + 1; j < boxes.length; j++) assert.ok(!overlap(boxes[i], boxes[j]), `${boxes[i].l.text} überlappt ${boxes[j].l.text} (${mpp} m/px)`);
    // ein Straßenname nicht mehrfach dicht nebeneinander
    const byName = new Map();
    for (const l of ls.filter((q) => q.kind.endsWith('treet'))) (byName.get(l.text) ?? byName.set(l.text, []).get(l.text)).push(l);
    for (const [n, list] of byName) for (let i = 0; i < list.length; i++) for (let j = i + 1; j < list.length; j++) assert.ok(Math.hypot(list[i].x - list[j].x, list[i].y - list[j].y) >= 320, `${n} doppelt`);
  }
});

test('Stadtplan-Daten: Straßen zu Straßenzügen verkettet, alle Ortsteile und Kieze mit Beschriftungspunkt im Inneren', async () => {
  const { insideIndex, ringIndex } = await import('../web/src/geom.js');
  const { realIndex } = await import('./helpers/city.js');
  const idx = realIndex();
  // Karl-Marx-Straße: aus vielen Kanten wenige lange Züge
  const kms = ov.roads.filter(([, n]) => ov.names[n] === 'Karl-Marx-Straße');
  const len = (d) => { const p = undelta(d); let L = 0; for (let i = 2; i < p.length; i += 2) L += Math.hypot(p[i] - p[i - 2], p[i + 1] - p[i - 1]); return L / 10; };
  assert.ok(kms.length < 40, `${kms.length} Züge statt ${ov.roads.length > 0 ? 'Dutzender' : ''} Einzelkanten`);
  assert.ok(Math.max(...kms.map(([, , d]) => len(d))) > 1500, 'ein Zug über 1,5 km');
  for (const d of idx.districts) {
    const lp = ov.ortsteile.find((q) => q[2] === d.n);
    assert.ok(lp, `${d.n} beschriftet`);
    assert.ok(insideIndex(ringIndex(d.r.map(undelta)), lp[0], lp[1]), `${d.n}: Beschriftung liegt im Ortsteil`);
  }
  assert.ok(ov.kieze.length >= 537, `${ov.kieze.length} Kieze`);
  for (const n of ['Flughafenkiez', 'Schillerkiez', 'Reuterkiez', 'Wrangelkiez', 'Kollwitzkiez', 'Sprengelkiez']) assert.ok(at(n), n);
  void delta;
});
