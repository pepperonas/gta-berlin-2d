// ÖPNV: ZIP/CSV-Leser, Fahrplan-Build (tools/osm/transit.mjs) an einem kleinen künstlichen GTFS, Laufzeit (transit.js),
// die ausgelieferten Fahrplandaten und das Verhalten in der Welt (Busse, Straßenbahnen, Züge).
import test from 'node:test';
import assert from 'node:assert/strict';
import { writeFileSync, mkdtempSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { makeZip, listZip, zipCsv, csvRow } from '../tools/osm/zip.mjs';
import { buildTransit, modeOf, pickDates, candidateDates, serviceDays, projectionFromMeta, undelta1 } from '../tools/osm/transit.mjs';
import { prepareTransit, departuresPerHour, positionAt, initialVehicles, stepTransit, pointOn, patternsNear, trainCars, dayType } from '../web/src/transit.js';
import { realCity, realIndex, realTransit } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld, resetPopulation } from '../web/src/world.js';
import { transitVisible } from '../web/src/transitlive.js';
import { buildLaneGraph } from '../web/src/roadgraph.js';
import { createCar } from '../web/src/car.js';
import { projectNear, spawnSpot } from '../web/src/traffic.js';
import { mulberry32 } from '../web/src/rng.js';
import { insideBorder } from '../web/src/map.js';

const index = realIndex();

test('ZIP und CSV: Einträge, Zeilen, Anführungszeichen und Kommas im Feld', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'gtfs-'));
  const f = join(dir, 'a.zip');
  writeFileSync(f, makeZip({ 'x.txt': '"a","b"\n1,"hallo, welt"\n2,"sagt ""hi"""\n', 'y.txt': 'nur\n' }));
  const e = listZip(f);
  assert.deepEqual([...e.keys()], ['x.txt', 'y.txt']);
  const rows = []; for await (const r of zipCsv(f, 'x.txt', e)) rows.push(r);
  assert.deepEqual(rows, [{ a: '1', b: 'hallo, welt' }, { a: '2', b: 'sagt "hi"' }]);
  assert.deepEqual(csvRow('1,,3'), ['1', '', '3']);
  if (existsSync('data/raw/gtfs.zip')) { // echter VBB-Auszug (deflate): Kopfzeile lesbar
    const v = listZip('data/raw/gtfs.zip');
    assert.equal(v.get('stop_times.txt').method, 8);
    for await (const r of zipCsv('data/raw/gtfs.zip', 'routes.txt', v)) { assert.ok('route_type' in r); break; }
  }
});

test('Verkehrsmittel: Bus, Straßenbahn, S-Bahn, U-Bahn; Regionalbahn und Fähre bleiben draußen', () => {
  assert.equal(modeOf('700'), 'bus'); assert.equal(modeOf('3'), 'bus');
  assert.equal(modeOf('900'), 'tram'); assert.equal(modeOf('109'), 'sbahn'); assert.equal(modeOf('400'), 'ubahn');
  assert.equal(modeOf('100'), null); assert.equal(modeOf('1000'), null);
  assert.equal(dayType(0), 0); assert.equal(dayType(4), 0); assert.equal(dayType(5), 1); assert.equal(dayType(6), 2);
});

test('Stichtage: der normalste Tag gewinnt (Bauarbeiten oder Feiertag am ersten Dienstag)', () => {
  const calendar = [{ service_id: 'wd', monday: '1', tuesday: '1', wednesday: '1', thursday: '1', friday: '1', saturday: '0', sunday: '0', start_date: '20261001', end_date: '20261231' }];
  const calendarDates = [{ service_id: 'wd', date: '20261013', exception_type: '2' }]; // am ersten Kandidaten-Dienstag fällt alles aus
  const cands = candidateDates(calendar);
  assert.equal(cands.w[0], '20261013');
  const dates = pickDates(cands, calendar, calendarDates, new Map([['wd', 100]]));
  assert.notEqual(dates.w, '20261013');
  assert.ok(serviceDays(calendar, calendarDates, { w: dates.w }).get('wd').has('w'));
});

// Kleines GTFS mitten in Kreuzberg: Buslinie 99 mit drei Halten (dritter außerhalb Berlins → abgeschnitten),
// Fahrten Mo–Fr um 8:00/8:10/8:20 und eine nach Mitternacht (24:30), dazu eine Regionalbahn (fällt weg).
function fixture() {
  const [la, lo] = [52.4990, 13.4200];
  const files = {
    'routes.txt': 'route_id,route_short_name,route_type,route_color\nR1,99,700,\nR2,RE9,100,\n',
    'calendar.txt': 'service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nS,1,1,1,1,1,0,0,20261001,20261231\n',
    'calendar_dates.txt': 'service_id,date,exception_type\n',
    'trips.txt': 'route_id,service_id,trip_id,shape_id\nR1,S,t1,sh\nR1,S,t2,sh\nR1,S,t3,sh\nR1,S,t4,sh\nR2,S,t9,sh\n',
    'stops.txt': `stop_id,stop_name,stop_lat,stop_lon\nA,Anfang,${la},${lo}\nB,Mitte,${la},${lo + 0.006}\nC,Mond,50.0,10.0\n`,
    'stop_times.txt': 'trip_id,stop_id,stop_sequence,arrival_time,departure_time\n'
      + ['t1,8:00', 't2,8:10', 't3,8:20', 't4,24:30', 't9,8:00'].map((s) => {
        const [t, hm] = s.split(','), [h, m] = hm.split(':').map(Number), f = (x) => `${Math.floor(x / 60)}:${String(x % 60).padStart(2, '0')}:00`;
        const m0 = h * 60 + m;
        return `${t},A,1,${f(m0)},${f(m0)}\n${t},B,2,${f(m0 + 4)},${f(m0 + 4)}\n${t},C,3,${f(m0 + 60)},${f(m0 + 60)}\n`;
      }).join(''),
    'shapes.txt': `shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nsh,${la},${lo - 0.001},0\nsh,${la},${lo + 0.003},1\nsh,${la},${lo + 0.007},2\nsh,50.0,10.0,3\n`,
  };
  const dir = mkdtempSync(join(tmpdir(), 'gtfs-'));
  const f = join(dir, 'gtfs.zip');
  writeFileSync(f, makeZip(files));
  return f;
}

test('Fahrplan-Build: Linie 99 gekürzt auf Berlin, Halte auf dem Weg, Fahrzeiten, Abfahrten inklusive nach Mitternacht; Regionalbahn raus', async () => {
  const t = await buildTransit(fixture(), index);
  assert.equal(t.lines.length, 1);
  assert.deepEqual(t.lines[0], ['99', 'bus', '']);
  assert.equal(t.patterns.length, 1);
  const p = t.patterns[0];
  assert.equal(p.st.length, 2, 'Halt außerhalb Berlins abgeschnitten');
  assert.equal(p.st[0], 0);
  assert.ok(p.st[1] > 3500 && p.st[1] < 4600, `Abstand der Halte ${p.st[1]} px (≈ 400 m)`);
  assert.deepEqual(p.off, [0, 240]);
  assert.deepEqual(undelta1(p.d[0]), [480, 490, 500, 1470]);
  assert.deepEqual(p.d[1], []); assert.deepEqual(p.d[2], []);
  assert.deepEqual(t.names.slice(0, 2), ['Anfang', 'Mitte']);
  assert.match(t.attribution, /VBB Verkehrsverbund Berlin-Brandenburg GmbH/);
  // Projektion wie der Karten-Build: der Anfangshalt liegt im Kartengebiet
  const toPx = projectionFromMeta(index.meta), [x, y] = toPx(52.499, 13.42);
  const pts = undelta1; // (nur um die Hilfsfunktion zu nutzen)
  assert.ok(x > 0 && y > 0 && x < index.meta.width && y < index.meta.height && typeof pts === 'function');
});

test('Laufzeit: Takt je Uhrzeit (auch Fahrten nach Mitternacht am Folgetag), Lage mit Halten, gleichmäßig verteilte Fahrzeuge', async () => {
  const tr = prepareTransit(await buildTransit(fixture(), index));
  const p = tr.patterns[0];
  assert.equal(departuresPerHour(p, 8 * 60 + 10, 1), 3);
  assert.equal(departuresPerHour(p, 12 * 60, 1), 0);
  assert.equal(departuresPerHour(p, 12 * 60, 5), 0, 'Samstag nichts');
  assert.equal(departuresPerHour(p, 30, 2), 1, 'Di 0:30 = Fahrt 24:30 vom Montag');
  assert.equal(departuresPerHour(p, 30, 5), 1, 'Sa 0:30 = Fahrt 24:30 vom Freitag');
  assert.equal(departuresPerHour(p, 30, 6), 0, 'So 0:30: samstags keine');
  // Lage: fährt, hält die letzten Sekunden vor der Abfahrt am Halt, am Ende fertig
  const a = positionAt(p, 0), b = positionAt(p, 100), c = positionAt(p, 235), d = positionAt(p, 999);
  assert.ok(a.s === 0 && b.s > 0 && b.s < p.stops[1] && !b.dwelling);
  assert.ok(c.s > p.stops[1] * 0.95, 'kurz vor dem Endhalt');
  assert.ok(d.done && d.s === p.stops[1], 'am Endhalt fertig');
  let prev = -1;
  for (let tau = 0; tau <= p.duration; tau += 3) { const s = positionAt(p, tau).s; assert.ok(s >= prev, 'nie rückwärts'); prev = s; }
  const v = initialVehicles({ ...p, duration: 3600 }, 6);
  assert.equal(v.length, 6);
  for (let i = 1; i < v.length; i++) assert.ok(Math.abs(v[i].tau - v[i - 1].tau - 600) < 1e-6, 'Takt 10 min');
});

test('Laufzeit: neue Fahrzeuge im Takt, fertige verschwinden, verfolgte Muster folgen der Kamera', async () => {
  const tr = prepareTransit(await buildTransit(fixture(), index));
  const p = tr.patterns[0], mid = pointOn(p, p.stops[1] / 2);
  const st = { tracked: new Map(), seed: 0 };
  let spawned = 0;
  for (let t = 0; t < 3600; t += 1) {
    const before = st.tracked.get(0)?.n ?? 0;
    stepTransit(st, tr, mid, 8 * 60 + 10, 1, 1, t);
    spawned += (st.tracked.get(0)?.n ?? 0) - before;
    for (const v of st.tracked.get(0).veh) assert.ok(v.tau <= p.duration + p.dwell);
  }
  assert.ok(spawned >= 2 && spawned <= 4, `${spawned} Abfahrten in einer Stunde bei 3/h`);
  stepTransit(st, tr, { x: mid.x + 1e6, y: mid.y }, 8 * 60 + 10, 1, 1, 9999);
  assert.equal(st.tracked.size, 0, 'Kamera weit weg: nicht mehr verfolgt');
  assert.ok(patternsNear(tr, mid.x, mid.y, 100).has(p));
});

test('Echte Fahrpläne: U1, U8, S7, M29, M10 fahren werktags morgens dicht, Halte liegen auf dem Weg im Kartengebiet', () => {
  const tr = realTransit();
  assert.ok(tr, 'transit.json vorhanden');
  const perHour = (name) => tr.patterns.filter((p) => p.name === name).reduce((a, p) => a + departuresPerHour(p, 8 * 60 + 30, 1), 0);
  for (const n of ['U1', 'U8', 'S7', 'M29', 'M10']) assert.ok(perHour(n) >= 6, `${n}: ${perHour(n)} je Stunde`);
  const modes = new Set(tr.patterns.map((p) => p.mode));
  for (const m of ['bus', 'tram', 'sbahn', 'ubahn']) assert.ok(modes.has(m), m);
  const idx = realIndex();
  for (const p of tr.patterns) {
    assert.equal(p.stops.length, p.off.length);
    for (let i = 1; i < p.stops.length; i++) { assert.ok(p.stops[i] >= p.stops[i - 1]); assert.ok(p.off[i] >= p.off[i - 1]); }
    assert.ok(p.stops[p.stops.length - 1] <= p.shape.len + 1);
    const q = pointOn(p, p.stops[0]);
    assert.ok(q.x >= 0 && q.y >= 0 && q.x <= idx.meta.width && q.y <= idx.meta.height);
  }
});

const run = (w, sec, at = null) => { for (let i = 0; i < sec * 60; i++) { updateWorld(w, idle(), 1 / 60); if (at) { w.camera.x = at.x; w.camera.y = at.y; } } };
const city = realCity();

test('Busse: fahren als KI-Fahrzeuge ihre Linie, halten an Halten; der übrige Verkehr nutzt keine Busspuren', () => {
  const e = city.list('edge').find((x) => x.name === 'Sonnenallee' && x.len > 300), k = e.pts.length >> 1 & ~1;
  const at = { x: e.pts[k], y: e.pts[k + 1] };
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 8 * 60 + 30; w.day = 1;
  w.player.x = at.x; w.player.y = at.y; w.camera.x = at.x; w.camera.y = at.y; resetPopulation(w);
  let halts = 0, buses = 0, onLine = true, held = 0, maxRun = 0;
  const seen = new Map(), stopsAt = [];
  for (let s = 0; s < 240 * 60; s++) {
    updateWorld(w, idle(), 1 / 60); w.camera.x = at.x; w.camera.y = at.y;
    for (const ev of w.events) if (ev.type === 'bus-stop') { halts++; stopsAt.push({ t: w.time, bus: w.cars.find((c) => c.duty?.bus && c.duty.boarding && Math.hypot(c.x - ev.x, c.y - ev.y) < 400) }); }
    // am Halt steht der Bus (2 s nach dem Halten)
    for (const h of stopsAt) if (!h.checked && w.time - h.t > 2) { h.checked = true; if (h.bus && Math.hypot(h.bus.vx, h.bus.vy) < 5) held++; }
    for (const c of w.cars) if (c.duty?.bus) {
      const p = realTransit().patterns[c.duty.pid];
      if (!seen.has(c.id)) { // beim Aufsetzen: dicht am eigenen Linienweg
        const q = projectNear(p.shape.pts, p.shape.cum, c.x, c.y, c.duty.s - 200, 600);
        assert.ok(q.d < 80, `Bus ${c.line} ${q.d.toFixed(0)} px neben seinem Linienweg aufgesetzt`);
        seen.set(c.id, c.duty.s);
      }
      maxRun = Math.max(maxRun, c.duty.s - seen.get(c.id));
    }
    if (s % 60) continue;
    for (const c of w.cars) {
      if (c.duty?.bus && !seen.has(c.id)) { buses++; if (c.line !== 'M41' && !/\d/.test(c.line)) onLine = false; }
      if (c.driver === 'npc' && c.kind !== 'bus' && c.ai) for (const sg of c.ai.segs) assert.ok(!sg.lane.busOnly, 'Pkw auf Busspur');
    }
  }
  assert.ok(seen.size >= 2, `Busse ${seen.size}`);
  assert.ok(held >= 1, 'Bus steht am Halt');
  assert.ok(maxRun > 3000, `ein Bus fuhr ${(maxRun / 10).toFixed(0)} m seine Linie entlang`);
  assert.ok(halts >= 2, `bediente Halte ${halts}`);
  assert.ok(onLine);
  const busLanes = [...buildLaneGraph(city).lanes].filter((l) => l.busOnly);
  assert.ok(busLanes.length, 'Busspuren im Spurgraph');
  // neue Autos entstehen nie auf einer Busspur
  const rng = mulberry32(5), bl = busLanes[0], mid = bl.pts.length >> 1 & ~1;
  for (let i = 0; i < 300; i++) { const sp = spawnSpot(city, rng, bl.pts[mid], bl.pts[mid + 1], 0, 150); if (sp) assert.ok(!sp.lane.busOnly, 'Spawn auf Busspur'); }
});

test('Straßenbahn: hält vor einem Hindernis auf dem Gleis und klingelt; Wagen schieben Autos weg', () => {
  const tr = realTransit();
  const p = tr.patterns.filter((x) => x.mode === 'tram' && x.name === 'M10').sort((a, b) => b.stops.length - a.stops.length)[0];
  const mid = pointOn(p, p.stops[Math.floor(p.stops.length / 2)]);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60; w.day = 1;
  w.camera.x = mid.x; w.camera.y = mid.y; w.player.x = mid.x; w.player.y = mid.y; resetPopulation(w);
  run(w, 1, mid);
  // eine Bahn dieses Musters kurz vor der Mitte ansetzen und den Spieler aufs Gleis stellen
  const s = w.transit.tracked.get(p.id);
  const v = { tau: 0, delay: 0, key: 'test' }; s.veh.push(v);
  let tau = 0; while (positionAt(p, tau).s < p.stops[Math.floor(p.stops.length / 2)] - 600 || positionAt(p, tau).dwelling) tau += 1;
  v.tau = tau;
  const head = pointOn(p, positionAt(p, tau).s + 40);
  w.player.x = head.x; w.player.y = head.y;
  let bell = false;
  const t0 = v.tau;
  for (let i = 0; i < 4 * 60; i++) { updateWorld(w, idle(), 1 / 60); w.camera.x = mid.x; w.camera.y = mid.y; w.player.x = head.x; w.player.y = head.y; if (w.events.some((e) => e.type === 'tram-bell')) bell = true; }
  assert.ok(v.tau - t0 < 0.1, `Bahn fuhr trotz Hindernis weiter (${(v.tau - t0).toFixed(2)} s)`);
  assert.ok(bell, 'klingelt');
  // Wagen als Hindernis: ein Auto mitten hinein wird herausgeschoben
  const o = w.railObs[0];
  assert.ok(o, 'Straßenbahnwagen als Hindernis');
  const car = createCar({ x: o.x, y: o.y, angle: o.angle }); w.cars.push(car);
  run(w, 0.2, mid);
  assert.ok(Math.hypot(car.x - o.x, car.y - o.y) > 10, 'Auto aus dem Wagen geschoben');
});

test('S- und U-Bahn nur sichtbar, wo ihr Gleis oberirdisch liegt; Straßenbahnen immer', () => {
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 8 * 60; w.day = 1;
  const kotti = city.list('poi').find((q) => q.cat === 'ubahn' && q.name === 'Kottbusser Tor');
  w.camera.x = kotti.x; w.camera.y = kotti.y;
  run(w, 2, kotti);
  const v = { x: kotti.x - 20000, y: kotti.y - 20000, w: 40000, h: 40000 };
  const all = transitVisible(w, v, () => true), none = transitVisible(w, v, () => false);
  assert.ok(all.some((t) => t.mode === 'ubahn' || t.mode === 'sbahn'), 'Züge im Umkreis');
  assert.ok(none.every((t) => t.mode === 'tram'), 'ohne oberirdisches Gleis keine Züge');
  for (const t of all) for (const c of t.cars) assert.ok(Number.isFinite(c.x) && Number.isFinite(c.angle) && c.L > 0);
  const cars = trainCars(tr0(), 5000);
  assert.equal(cars.length, 6); assert.ok(cars[0].first && cars[5].last);
  assert.ok(insideBorder(city, kotti.x, kotti.y));
});
const tr0 = () => realTransit().patterns.find((p) => p.mode === 'ubahn');
