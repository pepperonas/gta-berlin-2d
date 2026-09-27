import test from 'node:test';
import assert from 'node:assert/strict';
import { crossSection, parkingSide, cycleLane, maxspeedOf, surfaceOf } from '../tools/osm/crosssection.mjs';
import { laneOffsets, parkingStrip } from '../web/src/street.js';
import { PARK, SURFACE } from '../web/src/citycodes.js';

const near = (a, b, eps = 1e-6) => Math.abs(a - b) < eps;

test('Fahrbahnbreite: width:carriageway vor width, sonst Summe aus Spuren, Park- und Radstreifen', () => {
  assert.equal(crossSection({ 'width:carriageway': '15', width: '9' }, 'residential', 0).width, 15);
  assert.equal(crossSection({ width: '11' }, 'residential', 0).width, 11);
  // keine Breite: 5,5 m Fahrbahn + 2 × 2 m Parken + 2 × 1,6 m Radstreifen
  const cs = crossSection({ 'parking:both': 'lane', 'parking:both:orientation': 'parallel', 'cycleway:both': 'lane' }, 'residential', 0);
  assert.ok(near(cs.width, 5.5 + 4 + 3.2));
  assert.equal(crossSection({ lanes: '4' }, 'primary', 0).width, 4 * 3.25);
});

test('Parkstreifen je Seite: Aufstellung, halb auf dem Bordstein, altes Schema, getrennt gemappt', () => {
  assert.deepEqual(parkingSide({ 'parking:left': 'lane', 'parking:left:orientation': 'perpendicular' }, 'left'), { kind: PARK.lane, orient: 'perpendicular', width: 5 });
  assert.deepEqual(parkingSide({ 'parking:both': 'lane', 'parking:both:orientation': 'diagonal' }, 'right'), { kind: PARK.lane, orient: 'diagonal', width: 4.5 });
  assert.equal(parkingSide({ 'parking:right': 'half_on_kerb' }, 'right').width, 1);
  assert.equal(parkingSide({ 'parking:both': 'separate' }, 'left').width, 0);
  assert.equal(parkingSide({ 'parking:both': 'on_kerb' }, 'left').kind, PARK.kerb);
  assert.deepEqual(parkingSide({ 'parking:lane:both': 'parallel' }, 'left'), { kind: PARK.lane, orient: 'parallel', width: 2 });
  assert.equal(parkingSide({ 'parking:lane:both': 'parallel', 'parking:lane:both:parallel': 'half_on_kerb' }, 'left').kind, PARK.half);
  assert.equal(parkingSide({}, 'left').kind, PARK.none);
});

test('Radfahrstreifen, Tempo, Belag', () => {
  assert.equal(cycleLane({ 'cycleway:right': 'lane', 'cycleway:right:width': '2.1' }, 'right'), 2.1);
  assert.equal(cycleLane({ 'cycleway:right': 'track' }, 'right'), 0, 'Radweg neben der Fahrbahn zählt nicht');
  assert.equal(cycleLane({ cycleway: 'lane' }, 'left'), 1.6);
  assert.equal(maxspeedOf({ maxspeed: '30' }, 'residential'), 30);
  assert.equal(maxspeedOf({}, 'primary'), 50);
  assert.equal(maxspeedOf({}, 'residential'), 30);
  assert.equal(maxspeedOf({ maxspeed: 'walk' }, 'living_street'), 7);
  assert.equal(surfaceOf({ surface: 'sett' }), SURFACE.cobble);
  assert.equal(surfaceOf({ surface: 'asphalt' }), SURFACE.asphalt);
});

test('Spuren: Einbahn, lanes:forward/backward, breite Hauptstraßen mit zwei Spuren je Richtung', () => {
  const ow = crossSection({ oneway: 'yes', lanes: '2' }, 'secondary', 1);
  assert.deepEqual([ow.fwd, ow.bwd], [2, 0]);
  const fb = crossSection({ 'lanes:forward': '2', 'lanes:backward': '1', lanes: '3' }, 'primary', 0);
  assert.deepEqual([fb.fwd, fb.bwd], [2, 1]);
  const wide = crossSection({ width: '20' }, 'primary', 0);
  assert.deepEqual([wide.fwd, wide.bwd], [3, 3]);
  const narrow = crossSection({ width: '7' }, 'residential', 0);
  assert.deepEqual([narrow.fwd, narrow.bwd], [1, 1]);
});

test('Spurlage: rechts in Fahrtrichtung zwischen Parkstreifen und Mitte; enge Straßen fahren mittig', () => {
  const cs = crossSection({ width: '12', 'parking:both': 'lane', 'parking:both:orientation': 'parallel' }, 'residential', 0);
  const lo = laneOffsets(cs);
  assert.ok(lo.fwd[0] > 0 && lo.bwd[0] < 0, 'Rechtsverkehr');
  assert.ok(lo.fwd[0] + 1 < cs.width / 2 - cs.right.parkW, 'Auto (2 m breit) bleibt links vom Parkstreifen');
  assert.ok(near(parkingStrip(cs, 1).offset, 6 - 1));
  // 7 m mit beidseitigem Parken: je Richtung nur 1,5 m → beide Richtungen fahren in der Mitte (man weicht aus)
  const tight = laneOffsets(crossSection({ width: '7', 'parking:both': 'lane' }, 'residential', 0));
  assert.ok(tight.narrow && tight.fwd[0] === tight.bwd[0]);
  // Einheit: dieselbe Lage in px bei 10 px/m
  const px = laneOffsets({ ...cs, width: cs.width * 10, left: { ...cs.left, parkW: cs.left.parkW * 10 }, right: { ...cs.right, parkW: cs.right.parkW * 10 } }, 10);
  assert.ok(near(px.fwd[0], lo.fwd[0] * 10));
});
