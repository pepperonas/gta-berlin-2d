import test from 'node:test';
import assert from 'node:assert/strict';
import { surfaceIndex, cutWhere } from '../tools/osm/build.mjs';
import { realCity, openRealCity, realIndex, geoToPx, reachability } from './helpers/city.js';
import { segDist2 } from '../web/src/geom.js';

const S = 10;

// Unsichtbare Wände (Ufer, Gleisränder, Brückengeländer) auf befahrbarer Fläche: Fahrbahn, Lücke zur Gegenfahrbahn,
// Radweg neben der Fahrbahn, Kreuzungsscheibe. Zäune sind sichtbar und zählen nicht.
function invisibleOnSurface(city, [x0, y0, x1, y1]) {
  const q = [], hits = [];
  const onSurface = (x, y) => {
    for (const s of city.edgeSegs.query({ x: x - 1, y: y - 1, w: 2, h: 2 }, q)) {
      const e = s.e;
      if (!e?.cs) continue;
      const d = Math.sqrt(segDist2(x, y, s.ax, s.ay, s.bx, s.by));
      if (e.junction) { if (d <= e.w / 2 - 1) return 'Kreuzung'; continue; }
      if (e.cls > 8) continue;
      if (d <= e.w / 2 - 1) return e.name || 'Fahrbahn';
      const dx = s.bx - s.ax, dy = s.by - s.ay, side = Math.sign((x - s.ax) * -dy + (y - s.ay) * dx) || 1;
      if (e.fill && Math.sign(e.fill) === side && d <= e.w / 2 + Math.abs(e.fill) - 1) return `${e.name} (Lücke)`;
      const tr = side > 0 ? e.cs.right.track : e.cs.left.track;
      if (tr && d >= e.w / 2 + 5 && d <= e.w / 2 + 4 + tr - 2) return `${e.name} (Radweg)`;
    }
    return null;
  };
  for (const w of city.list('wall')) {
    if (!['quay', 'rail', 'railing'].includes(w.sub)) continue;
    const p = w.pts;
    for (let i = 0; i < p.length - 2; i += 2) {
      const L = Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]), n = Math.max(1, Math.round(L / S));
      for (let k = 0; k < n; k++) {
        const f = (k + 0.5) / n, x = p[i] + (p[i + 2] - p[i]) * f, y = p[i + 1] + (p[i + 3] - p[i + 1]) * f;
        if (x < x0 || x > x1 || y < y0 || y > y1) continue;
        const h = onSurface(x, y);
        if (h) hits.push(`${w.sub} auf ${h} bei ${Math.round(x)},${Math.round(y)}`);
      }
    }
  }
  return hits;
}

test('Keine unsichtbare Wand auf befahrbarer Fläche: Kerngebiet und die Brücken, an denen es klemmte', () => {
  const core = realCity();
  const all = [];
  all.push(...invisibleOnSurface(core, [0, 0, core.width, core.height]));
  const meta = realIndex().meta, city = openRealCity();
  const spots = [[52.50195, 13.44565], [52.5056, 13.44905], [52.5285, 13.3290], [52.5540, 13.2860], [52.4690, 13.2240], [52.4680, 13.5630], [52.4990, 13.2690]];
  for (const [lat, lon] of spots) {
    const [x, y] = geoToPx(meta, lat, lon);
    city.loadArea(x - 3500, y - 3500, x + 3500, y + 3500);
    all.push(...invisibleOnSurface(city, [x - 3000, y - 3000, x + 3000, y + 3000]));
  }
  assert.deepEqual(all.slice(0, 10), [], `${all.length} Stücke unsichtbarer Wand auf Fahrfläche`);
});

test('Oberbaumbrücke: über die ganze Breite (beide Richtungen, Lücke, Radwege) mit dem Auto befahrbar', () => {
  const meta = realIndex().meta, city = openRealCity(), [x, y] = geoToPx(meta, 52.50195, 13.44565), box = [x - 3000, y - 3000, x + 3000, y + 3000];
  city.loadArea(...box);
  const e = [...city.edges.values()].find((q) => q.name === 'Oberbaumbrücke');
  const p = e.pts, dx = p[2] - p[0], dy = p[3] - p[1], L = Math.hypot(dx, dy), rx = -dy / L, ry = dx / L;
  const car = reachability(city, box, [p[0] + dx * 0.1, p[1] + dy * 0.1], 11, { cell: 4, solidFilter: (s) => s.layer !== 'barrier' });
  let tot = 0; const miss = [];
  for (let f = 0.1; f <= 0.91; f += 0.1) for (let o = -e.w / 2 - Math.abs(e.fill) + 12; o <= e.w / 2 + 4 + e.cs.right.track - 12; o += 5) {
    tot++; if (!car(p[0] + dx * f + rx * o, p[1] + dy * f + ry * o)) miss.push([f.toFixed(1), Math.round(o)]);
  }
  assert.ok(tot > 100);
  assert.deepEqual(miss, [], 'jede Stelle der Fahrfläche erreichbar');
});

test('Fahrfläche als Punkttest: Fahrbahn, Lücke und Radweg je Seite, Kreuzungsscheibe; eigene Kante auslassbar', () => {
  const vertices = [0, 0, 1000, 0];
  const ed = { a: 0, b: 1, w: 100, fill: 40, x: Array(15).fill(0), c: 4 }; // 10 m, Lücke 4 m rechts, Radweg links 2 m
  ed.x[13] = 20;
  const edgePts = (e) => [vertices[2 * e.a], vertices[2 * e.a + 1], vertices[2 * e.b], vertices[2 * e.b + 1]];
  const reachOf = (e, side) => e.w / 2 + (side > 0 ? e.fill : 0) + (side < 0 ? 0.4 * S + e.x[13] / 10 * S : 0);
  const on = surfaceIndex([ed], edgePts, [{ x: 2000, y: 0, r: 80 }], reachOf, S);
  assert.ok(on(500, 40), 'Fahrbahn');
  assert.ok(on(500, 85), 'Lücke rechts (y nach unten ist rechts bei Fahrt nach Osten)');
  assert.ok(!on(500, 100), 'hinter der Lücke');
  assert.ok(on(500, -70), 'Radweg links');
  assert.ok(!on(500, -80), 'hinter dem Radweg');
  assert.ok(on(2050, 0), 'Kreuzungsscheibe');
  assert.ok(!on(500, 40, (e) => e === ed), 'eigene Kante ausgelassen');
  const pieces = cutWhere([0, 50, 1000, 50], (x) => x > 400 && x < 600, S);
  assert.equal(pieces.length, 2, 'Lücke geschnitten');
  assert.ok(pieces[0][pieces[0].length - 2] <= 400 && pieces[1][0] >= 600);
});
