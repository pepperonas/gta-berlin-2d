import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { roofOf, roofStyle, facadeStyle, roofDecor, roofGeometry, mainAxis, lookOf, PITCHED } from '../web/src/roofs.js';
import { wallColor, roofColors, PAL } from '../web/src/buildcolors.js';
import { parseColour, buildingLook } from '../tools/osm/looks.mjs';
import { pointInRings } from '../web/src/geom.js';
import { BUILDING_KIND, ROOF_SHAPE, ROOF_MAT, WALL_MAT, BUILDING_SUB, packLook, unpackLook } from '../web/src/citycodes.js';

const city = realCity();
const S = city.scale;
const buildings = city.list('building');
const K = BUILDING_KIND;

test('Dachaufbauten liegen ganz im Grundriss, überlappen nicht und folgen der Hauptachse', () => {
  let n = 0;
  for (const b of buildings) {
    const ds = roofOf(b, S).decor, a = mainAxis(b);
    for (const d of ds) {
      n++;
      const ca = Math.cos(d.a), sa = Math.sin(d.a);
      for (const [u, v] of [[1, 1], [-1, 1], [-1, -1], [1, -1]]) {
        const x = d.x + ca * u * d.l / 2 - sa * v * d.w / 2, y = d.y + sa * u * d.l / 2 + ca * v * d.w / 2;
        assert.ok(pointInRings(x, y, b.rings), `${d.t} ragt aus Haus ${b.id}`);
      }
      assert.equal(d.a, a);
    }
    for (let i = 0; i < ds.length; i++) for (let j = i + 1; j < ds.length; j++) {
      const p = ds[i], q = ds[j];
      assert.ok(Math.hypot(p.x - q.x, p.y - q.y) >= (Math.max(p.l, p.w) + Math.max(q.l, q.w)) / 2, `Aufbauten überlappen auf Haus ${b.id}`);
    }
  }
  assert.ok(n > 60000, `${n} Dachaufbauten im Kerngebiet`);
});

test('Dachformen: OSM-Angabe hat Vorrang, sonst passend zu Art, Höhe und Typ; Vielfalt im Kerngebiet', () => {
  const count = {};
  const map = { [ROOF_SHAPE.gabled]: 'gabled', [ROOF_SHAPE.hipped]: 'hipped', [ROOF_SHAPE.mansard]: 'mansard', [ROOF_SHAPE.skillion]: 'skillion', [ROOF_SHAPE.dome]: 'dome' };
  let tagged = 0;
  for (const b of buildings) {
    const st = roofStyle(b, S), fa = facadeStyle(b, S), lk = lookOf(b);
    count[st] = (count[st] ?? 0) + 1;
    if (map[lk.shape] && b.kind !== K.spaeti && b.kind !== K.warehouse) { tagged++; assert.equal(st, map[lk.shape], `OSM-Dachform an Haus ${b.id}`); }
    if (!lk.shape && (b.kind === K.industrial || b.kind === K.warehouse)) assert.ok(st === 'corrugated' || (st === 'gabled' && b.kind === K.industrial) || (st === 'flat' && (lk.rmat === ROOF_MAT.green || lk.rmat === ROOF_MAT.tar)), `${st} auf Halle`);
    if (b.kind === K.industrial || b.kind === K.warehouse) assert.equal(fa, 'industry');
    if (!lk.shape && st === 'berlin') assert.ok(b.kind === K.house && b.meters >= 12 && b.meters <= 26, 'Berliner Dach nur auf Altbauhöhe');
    if (!lk.shape && (st === 'gabled' || st === 'hipped') && b.kind === K.house) assert.ok(b.meters <= 14 || lk.sub === BUILDING_SUB.terrace, `geschätztes Steildach auf ${b.meters} m hohem Haus`);
    if (fa === 'platte') assert.ok(b.meters >= 14, 'Plattenbau nur ab 5 Geschossen');
  }
  assert.ok(tagged > 3000, `${tagged} Häuser mit OSM-Dachform`);
  for (const st of ['flat', 'berlin', 'gabled', 'hipped', 'mansard', 'skillion', 'corrugated']) assert.ok(count[st] > 100, `${st}: ${count[st]}`);
  assert.ok(count.dome > 0, 'Kuppeln');
});

test('Dachflächen: im Grundriss, keine verdrehten Flächen, Gauben und Ziegelreihen begrenzt', () => {
  let facets = 0, inside = 0, dormers = 0;
  for (const b of buildings) {
    const { style, geo } = roofOf(b, S);
    if (!PITCHED.has(style) && style !== 'berlin') assert.equal(geo.facets.length, 0, `${style} ohne geneigte Flächen`);
    else assert.ok(geo.facets.length > 0, `${style}-Dach auf Haus ${b.id} ohne Flächen`);
    for (const f of geo.facets) {
      const p = f.pts;
      facets++;
      if (pointInRings((p[0] + p[2] + p[4] + p[6]) / 4, (p[1] + p[3] + p[5] + p[7]) / 4, b.rings)) inside++;
      assert.ok(Math.abs(Math.hypot(f.nx, f.ny) - 1) < 1e-6, 'Fallrichtung normiert');
      if (f.edge && p[0] !== p[6]) { // Streifen: Innenkante läuft wie die Traufe (keine „Fliege“)
        const dot = (p[4] - p[6]) * (p[2] - p[0]) + (p[5] - p[7]) * (p[3] - p[1]);
        assert.ok(dot >= -1e-6, `verdrehte Dachfläche an Haus ${b.id}`);
      }
      // Fallrichtung zeigt nach außen: ein Stück von der Traufe in Fallrichtung liegt nicht im Dachinneren der Fläche
      const ex = (p[0] + p[2]) / 2, ey = (p[1] + p[3]) / 2, ix = (p[4] + p[6]) / 2, iy = (p[5] + p[7]) / 2;
      if (Math.hypot(ix - ex, iy - ey) > 1) assert.ok((ex - ix) * f.nx + (ey - iy) * f.ny > 0, 'Fallrichtung zur Traufe');
    }
    assert.ok(geo.courses.length / 4 <= 320, 'Ziegelreihen begrenzt');
    for (const d of geo.dormers) {
      dormers++;
      assert.ok(pointInRings(d.x, d.y, b.rings), 'Gaube im Grundriss');
    }
    assert.ok(geo.dormers.length <= 24);
  }
  assert.ok(inside / facets > 0.97, `${inside}/${facets} Dachflächen liegen im Grundriss`);
  assert.ok(dormers > 5000, `${dormers} Gauben`);
});

test('Satteldach über Rechteck: zwei Flächen, First mittig entlang der Hauptachse', () => {
  const r = [0, 0, 200, 0, 200, 100, 0, 100];
  const b = { rings: [r], cx: 100, cy: 50, bbox: { x: 0, y: 0, w: 200, h: 100 }, kind: K.house, meters: 8, seed: 7, look: packLook({ shape: ROOF_SHAPE.gabled }) };
  const g = roofGeometry(b, 'gabled', S);
  assert.equal(g.facets.length, 2);
  assert.deepEqual(g.ridges.map(Math.round), [0, 50, 200, 50]);
  const ny = g.facets.map((f) => Math.sign(f.ny)).sort();
  assert.deepEqual(ny, [-1, 1], 'die Hälften fallen zu den Traufseiten ab');
  const h = roofGeometry({ ...b, look: 0 }, 'hipped', S);
  assert.equal(h.facets.length, 4, 'Walmdach: vier Flächen');
  const tips = h.facets.filter((f) => f.pts[4] === f.pts[6] && f.pts[5] === f.pts[7]);
  assert.equal(tips.length, 2, 'zwei Walmdreiecke an den Schmalseiten');
  for (const f of tips) assert.ok(Math.abs(f.pts[5] - 50) < 1e-6, 'Walmspitze auf dem First');
});

test('Aussehen aus OSM-Tags: Farben, Formen, Materialien, Typen', () => {
  assert.equal(parseColour('grey'), 0x8a8a8a);
  assert.equal(parseColour('#E98C56'), 0xe98c56);
  assert.equal(parseColour('#abc'), 0xaabbcc);
  assert.equal(parseColour('red;grey'), parseColour('red'));
  assert.equal(parseColour('wabern'), -1);
  const lk = buildingLook({ building: 'detached', 'roof:shape': 'half-hipped', 'roof:material': 'roof_tiles', 'building:material': 'plaster', 'roof:colour': 'red' });
  assert.deepEqual({ ...lk, rc: lk.rc > 0 }, { shape: ROOF_SHAPE.hipped, rmat: ROOF_MAT.tiles, wmat: WALL_MAT.plaster, sub: BUILDING_SUB.villa, rc: true, fc: 0 });
  assert.equal(buildingLook({ building: 'yes' }).shape, 0);
  for (const v of [{ shape: 15, rmat: 7, wmat: 7, sub: 7, bez: 12 }, { shape: 2, rmat: 0, wmat: 3, sub: 1, bez: 4 }]) assert.deepEqual(unpackLook(packLook(v)), v);
});

test('Farben: OSM-Farbe vor Material vor Schätzung; Steildächer in Ziegel/Schiefer, Flachdächer grau', () => {
  const base = { rings: [[0, 0, 100, 0, 100, 100, 0, 100]], cx: 50, cy: 50, bbox: { x: 0, y: 0, w: 100, h: 100 }, kind: K.house, meters: 8, seed: 12345, roofRgb: -1, wallRgb: -1 };
  const hex = (h) => parseInt(h.slice(1), 16);
  assert.equal(roofColors({ ...base, roofRgb: 0x123456 }, 'gabled', '#ffffff').skin, '#123456');
  assert.equal(wallColor({ ...base, wallRgb: 0xabcdef }, 'altbau'), '#abcdef');
  assert.ok(PAL.brick.includes(wallColor({ ...base, look: packLook({ wmat: WALL_MAT.brick }) }, 'altbau')));
  assert.ok(PAL.green.includes(roofColors({ ...base, look: packLook({ rmat: ROOF_MAT.green }) }, 'flat', '#ffffff').skin));
  let steep = 0, grey = 0;
  for (let s = 0; s < 400; s++) {
    const c = roofColors({ ...base, seed: s * 7919 }, 'gabled', '#e0d4bd').skin;
    if ([...PAL.tile, ...PAL.slate, ...PAL.brownTile].includes(c)) steep++;
    const f = roofColors({ ...base, seed: s * 7919 }, 'flat', '#e0d4bd').skin, n = hex(f);
    const r = n >> 16, g = (n >> 8) & 255, bl = n & 255;
    if (Math.max(r, g, bl) - Math.min(r, g, bl) < 40) grey++;
  }
  assert.equal(steep, 400, 'Steildächer immer in Ziegel/Schiefer');
  assert.ok(grey > 380, 'Flachdächer grau (Bitumen, Kies)');
  const berlin = roofColors(base, 'berlin', '#e0d4bd');
  assert.notEqual(berlin.skin, berlin.center, 'Berliner Dach: Ziegelstreifen um flache Mitte');
  // Bezirk: hohe Wohnhäuser in Marzahn-Hellersdorf sind Plattenbauten
  let platte = 0, mitte = 0;
  for (let s = 0; s < 200; s++) {
    const m = { ...base, seed: s * 104729, meters: 18, look: packLook({ bez: 4 }) };
    if (facadeStyle(m, S) === 'platte') platte++;
    assert.equal(roofStyle(m, S), 'flat', 'Platte mit Flachdach');
    if (facadeStyle({ ...m, look: packLook({ bez: 5 }) }, S) === 'platte') mitte++;
  }
  assert.ok(platte > 150, `${platte}/200 fünfgeschossige Häuser in Marzahn-Hellersdorf sind Platte`);
  assert.equal(mitte, 0, 'gleiche Häuser in Mitte: Altbau/Neubau');
});

test('Dach einmal je Gebäude bestimmt und deterministisch', () => {
  const b = buildings.find((x) => roofOf(x, S).decor.length > 3);
  const r = roofOf(b, S);
  assert.equal(roofOf(b, S), r, 'zwischengespeichert');
  const again = JSON.stringify(roofDecor(b, S, r.style, roofGeometry(b, r.style, S)));
  assert.equal(JSON.stringify(r.decor), again);
  assert.equal(JSON.stringify(roofGeometry(b, r.style, S)), JSON.stringify(r.geo));
});
