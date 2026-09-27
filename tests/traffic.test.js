import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorld, updateWorld } from '../web/src/world.js';
import { surfaceAt, inBuilding, T, isRoadSurface } from '../web/src/map.js';
import { obbVsSegment } from '../web/src/collision.js';
import { obbBounds } from '../web/src/collision.js';
import { buildLaneGraph, chooseNext, turnAngle } from '../web/src/roadgraph.js';
import { mulberry32 } from '../web/src/rng.js';
import { idle } from './helpers/bot.js';
import { realCity } from './helpers/city.js';

const city = realCity();
const g = buildLaneGraph(city);

test('Spurgraph: Rechtsverkehr, Einbahnstraßen nur in Fahrtrichtung', () => {
  const lanes = [...g.lanes];
  const twoWay = lanes.find((l) => !l.edge.oneway && l.dir === 1 && l.edge.len > 400 && l.edge.pts.length === 4);
  assert.ok(twoWay, 'gerade Straße mit Gegenverkehr gefunden');
  const e = twoWay.edge, ux = (e.pts[2] - e.pts[0]) / e.len, uy = (e.pts[3] - e.pts[1]) / e.len;
  const mx = (twoWay.pts[0] + twoWay.pts[2]) / 2 - (e.pts[0] + e.pts[2]) / 2, my = (twoWay.pts[1] + twoWay.pts[3]) / 2 - (e.pts[1] + e.pts[3]) / 2;
  // rechts in Fahrtrichtung bei y nach unten: (-uy, ux)
  assert.ok(mx * -uy + my * ux > e.w / 8, 'Spur liegt rechts der Mitte');
  const oneway = city.list('edge').filter((x) => x.inside && x.oneway === 1 && x.cls <= 7);
  assert.ok(oneway.length > 50);
  for (const x of oneway) assert.ok(!lanes.some((l) => l.edge === x && l.dir === -1), 'Gegenspur auf Einbahnstraße');
});

test('Abbiegen: keine Wende außer in der Sackgasse; meist geradeaus', () => {
  const rng = mulberry32(3);
  let straight = 0, n = 0;
  for (const l of g.lanes) {
    // Wenden nur, wo von diesem Knoten keine andere Spur abgeht (Sackgasse oder nur einmündende Einbahnstraßen)
    const other = (g.out.get(l.to) ?? []).some((m) => m.edge !== l.edge);
    if (other) assert.ok(l.next.every((m) => m.edge !== l.edge), `Wende an einer Kreuzung (Kante ${l.edge.id})`);
    if (l.next.length >= 3) for (let k = 0; k < 5; k++) { n++; if (Math.abs(turnAngle(l, chooseNext(l, rng))) < 0.4) straight++; }
  }
  assert.ok(straight / n > 0.4, `geradeaus nur ${(straight / n * 100).toFixed(0)} %`);
});

test('Dauertest 90 s mit Verkehr und Passanten: auf der Fahrbahn bzw. nicht in Häusern', () => {
  const w = createWorld({ city });
  let samples = 0, offRoad = 0, pedBad = 0, moved = 0;
  const odo = new Map();
  for (let i = 0; i < 90 * 60; i++) {
    updateWorld(w, idle(), 1 / 60);
    if (i % 30) continue;
    for (const c of w.cars) {
      assert.ok(Number.isFinite(c.x) && Number.isFinite(c.y), 'NaN-Position');
      for (const s of w.solids.query(obbBounds(c), [])) {
        if (!s.seg || s.kind !== 'building') continue;
        const m = obbVsSegment(c, s);
        assert.ok(!m || m.depth < 6, `Auto ${c.id} steckt ${m?.depth.toFixed(1)} px in einer Hauswand`);
      }
      if (c.driver !== 'npc') continue;
      samples++;
      if (!isRoadSurface(surfaceAt(city, c.x, c.y))) offRoad++;
      const o = odo.get(c.id) ?? { d: 0, x: c.x, y: c.y };
      o.d += Math.hypot(c.x - o.x, c.y - o.y); o.x = c.x; o.y = c.y; odo.set(c.id, o);
    }
    for (const p of w.peds) if (inBuilding(city, p.x, p.y)) pedBad++;
  }
  for (const o of odo.values()) if (o.d > 500) moved++;
  const share = offRoad / samples;
  console.log(`# Verkehr: ${w.cars.length} Autos, ${(share * 100).toFixed(1)} % Stichproben neben der Fahrbahn, ${moved}/${odo.size} Autos > 50 m gefahren`);
  assert.ok(share < 0.03, `zu oft neben der Straße: ${(share * 100).toFixed(1)} %`);
  assert.equal(pedBad, 0, 'Passant in einem Gebäude');
  assert.ok(moved >= odo.size * 0.7, `Verkehr kommt nicht voran: ${moved}/${odo.size}`);
});

test('Bevölkerung folgt der Kamera', () => {
  const w = createWorld({ city, cars: 10, pedestrians: 20 });
  const far = city.places.pickup;
  w.player.x = far.x; w.player.y = far.y; w.camera.x = far.x; w.camera.y = far.y;
  for (let i = 0; i < 10 * 60; i++) { w.player.x = far.x; w.player.y = far.y; updateWorld(w, idle(), 1 / 60); }
  const npc = w.cars.filter((c) => c.driver === 'npc');
  assert.ok(npc.length >= 8, `nur ${npc.length} Autos`);
  assert.ok(npc.every((c) => Math.hypot(c.x - far.x, c.y - far.y) < 2600));
  assert.ok(w.peds.length >= 15 && w.peds.every((p) => Math.hypot(p.x - far.x, p.y - far.y) < 2600));
});

test('Mit Vollgas gegen Hauswand, ins Wasser und über die Gebietsgrenze: das Auto bleibt draußen', async () => {
  const { playerCar } = await import('../web/src/world.js');
  const { insideBorder } = await import('../web/src/map.js');
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  const car = w.cars.find((c) => c.id === w.playerCarId);
  w.player.inCar = car.id; car.driver = 'player';
  // Jeder Schritt wird geprüft (am Ende allein könnte das Auto schon auf der anderen Seite wieder herausgekommen sein).
  const ram = (x, y, angle, bad) => {
    Object.assign(car, { x, y, angle, vx: 0, vy: 0, angVel: 0, health: 100, wrecked: false });
    w.camera.x = x; w.camera.y = y; // die Kamera folgt dem Auto; dort lädt die Stadt nach
    let moved = 0;
    for (let i = 0; i < 4 * 60; i++) {
      const px = car.x, py = car.y;
      updateWorld(w, { ...idle(), throttle: 1 }, 1 / 60);
      moved += Math.hypot(car.x - px, car.y - py);
      assert.ok(!bad(car.x, car.y), `durchgebrochen nach ${i} Schritten bei ${car.x.toFixed(0)},${car.y.toFixed(0)}`);
    }
    assert.ok(moved > 20, 'Auto ist überhaupt losgefahren');
    return playerCar(w);
  };
  // Hauswand: vom Abgabeort (Fahrbahn) quer zum Späti
  const { BUILDING_KIND } = await import('../web/src/citycodes.js');
  const shop = city.list('building').find((b) => b.kind === BUILDING_KIND.spaeti);
  const g0 = city.places.giver;
  const a = Math.atan2(shop.cy - g0.y, shop.cx - g0.x);
  ram(g0.x - Math.cos(a) * 60, g0.y - Math.sin(a) * 60, a, (x, y) => inBuilding(city, x, y));
  // Wasser: von einem Kai-Punkt (Wandzug am Wasser) senkrecht ins Wasser
  const quay = city.list('wall').filter((f) => f.sub === 'quay').map((f) => f.pts).find((wl) => {
    const L = Math.hypot(wl[2] - wl[0], wl[3] - wl[1]);
    if (wl.length <= 40 || L <= 60) return false;
    const mx = (wl[0] + wl[2]) / 2, my = (wl[1] + wl[3]) / 2, nx = -(wl[3] - wl[1]) / L, ny = (wl[2] - wl[0]) / L;
    const a = surfaceAt(city, mx + nx * 30, my + ny * 30), b = surfaceAt(city, mx - nx * 30, my - ny * 30);
    return (a === T.WATER) !== (b === T.WATER) && [a, b].some((t) => t === T.SIDEWALK || t === T.PLAZA); // Kai: Wasser auf einer Seite, Gehweg auf der anderen
  });
  const qx = (quay[0] + quay[2]) / 2, qy = (quay[1] + quay[3]) / 2, L = Math.hypot(quay[2] - quay[0], quay[3] - quay[1]);
  const nx = -(quay[3] - quay[1]) / L, ny = (quay[2] - quay[0]) / L;
  const side = surfaceAt(city, qx + nx * 30, qy + ny * 30) === T.WATER ? 1 : -1; // Normale zeigt ins Wasser
  ram(qx - nx * side * 40, qy - ny * side * 40, Math.atan2(ny * side, nx * side), (x, y) => surfaceAt(city, x, y) === T.WATER);
  // Gebietsgrenze
  // Stadtgrenze: ein längeres Grenzstück, senkrecht darauf zu
  const b = city.border[0];
  let i = 0; while (Math.hypot(b[i + 2] - b[i], b[i + 3] - b[i + 1]) < 400) i += 2;
  const bx = (b[i] + b[i + 2]) / 2, by = (b[i + 1] + b[i + 3]) / 2, bl = Math.hypot(b[i + 2] - b[i], b[i + 3] - b[i + 1]);
  let bnx = -(b[i + 3] - b[i + 1]) / bl, bny = (b[i + 2] - b[i]) / bl;
  if (!insideBorder(city, bx + bnx * 50, by + bny * 50)) { bnx = -bnx; bny = -bny; } // Normale zeigt nach Berlin hinein
  ram(bx + bnx * 60, by + bny * 60, Math.atan2(-bny, -bnx), (x, y) => !insideBorder(city, x, y));
});

test('Ampel: KI-Auto hält bei Rot an der Haltelinie und fährt bei Grün', async () => {
  const { signalState, SIGNAL } = await import('../web/src/signals.js');
  const { placeOnLane } = await import('../web/src/traffic.js');
  const { createCar } = await import('../web/src/car.js');
  const { laneDir } = await import('../web/src/roadgraph.js');
  // eine gerade, lange Zufahrt auf eine Ampelkreuzung
  const lane = [...g.lanes].find((l) => city.signals.has(l.to) && l.len > 700 && l.edge.cls <= 5 && l.next.length);
  assert.ok(lane, 'Zufahrt gefunden');
  const [ux, uy] = laneDir(lane, true), heading = Math.atan2(uy, ux);
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  // Zeit = Beginn einer Rotphase (Rot dauert je Achse cycle − grün − gelb = 27 s, das Auto braucht ~8 s bis zur Linie)
  let t0 = 0.5;
  while (!(signalState(city, lane.to, heading, t0) === 'red' && signalState(city, lane.to, heading, t0 - 0.5) !== 'red')) t0 += 0.5;
  assert.equal(signalState(city, lane.to, heading, t0 + 20), 'red');
  w.time = t0;
  const car = createCar({ x: 0, y: 0 }); car.driver = 'npc';
  placeOnLane(car, city, lane, lane.len - 600, w.rng);
  w.cars.push(car);
  w.camera.x = car.x; w.camera.y = car.y;
  const end = { x: lane.pts[lane.pts.length - 2], y: lane.pts[lane.pts.length - 1] };
  for (let i = 0; i < 20 * 60; i++) { updateWorld(w, idle(), 1 / 60); w.camera.x = car.x; w.camera.y = car.y; }
  const before = (end.x - car.x) * ux + (end.y - car.y) * uy;
  assert.ok(Math.hypot(car.vx, car.vy) < 5, 'steht');
  assert.ok(before > -5 && before < 80, `hält ${(before / 10).toFixed(1)} m vor der Haltelinie`);
  // bis Grün warten, dann fährt es über die Kreuzung
  let k = 0;
  const follow = () => { w.camera.x = car.x; w.camera.y = car.y; };
  while (signalState(city, lane.to, heading, w.time) !== 'green' && k++ < SIGNAL.cycle * 60) { updateWorld(w, idle(), 1 / 60); follow(); }
  for (let i = 0; i < 8 * 60; i++) { updateWorld(w, idle(), 1 / 60); follow(); }
  const after = (end.x - car.x) * ux + (end.y - car.y) * uy;
  assert.ok(after < -40 || Math.hypot(car.x - end.x, car.y - end.y) > 150, 'bei Grün weitergefahren');
});

test('Geparkte Autos stehen auf den Parkstreifen, nicht auf den Fahrstreifen', async () => {
  const { parkingStrip } = await import('../web/src/street.js');
  const { nearestEdge } = await import('../web/src/map.js');
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  const e = city.list('edge').find((x) => x.name === 'Weserstraße' && x.len > 800);
  const m = (e.pts.length / 2 | 0) & ~1;
  w.camera.x = w.player.x = e.pts[m]; w.camera.y = w.player.y = e.pts[m + 1];
  for (let i = 0; i < 30; i++) updateWorld(w, idle(), 1 / 60);
  const parked = w.cars.filter((c) => c.role === 'curb');
  assert.ok(parked.length > 20, `${parked.length} geparkte Autos`);
  let checked = 0;
  for (const c of parked) {
    const ne = nearestEdge(city, c.x, c.y, 300, (x) => x.cls <= 8);
    if (!ne || ne.e !== e) continue;
    const lat = (c.x - ne.x) * -ne.uy + (c.y - ne.y) * ne.ux, side = Math.sign(lat);
    assert.ok(Math.abs(Math.abs(lat) - Math.abs(parkingStrip(e.cs, side).offset)) < 3, `seitlich ${lat.toFixed(0)} px statt Parkstreifen`);
    checked++;
  }
  assert.ok(checked >= 5, `${checked} auf der Weserstraße geprüft`);
});

// Verkehrschaos (gemeldet mit Bild): Autos rammten den Vordermann in der Schlange, fuhren gegen Poller und Zäune auf der
// Fahrbahn, blockierten sich an Kreuzungen und Engstellen gegenseitig und warteten ewig hinter liegenden Fußgängern.
test('Kreuzung: Bewegungen, die sich nicht kreuzen, dürfen gleichzeitig hinein', async () => {
  const { movesConflict } = await import('../web/src/traffic.js');
  const A = {}, B = {}, C = {};
  // Nord-Süd-Straße, Knoten bei (0,0), Spuren 14 px rechts der Mitte; Ost-West-Straße kreuzt
  const southbound = { ax: -14, ay: -60, bx: -14, by: 60, to: A };  // geradeaus nach Süden
  const northbound = { ax: 14, ay: 60, bx: 14, by: -60, to: B };    // Gegenverkehr geradeaus
  const northLeft = { ax: 14, ay: 60, bx: -60, by: -14, to: C };    // Gegenverkehr biegt links ab (nach Westen)
  const northRight = { ax: 14, ay: 60, bx: 60, by: 14, to: C };     // biegt rechts ab (nach Osten)
  const westIntoSouth = { ax: 60, ay: -14, bx: -14, by: 60, to: A }; // mündet in dieselbe Spur wie southbound
  assert.equal(movesConflict(southbound, northbound), false, 'Gegenverkehr geradeaus stört sich nicht');
  assert.equal(movesConflict(southbound, northRight), false, 'Rechtsabbieger des Gegenverkehrs stört nicht');
  assert.equal(movesConflict(southbound, northLeft), true, 'Linksabbieger kreuzt den Gegenverkehr');
  assert.equal(movesConflict(southbound, westIntoSouth), true, 'gleiche Zielspur = Konflikt');
});

test('Verkehr ohne Knoten: 3 min an den engsten Stellen – niemand steht über 90 s, kaum Zusammenstöße', async () => {
  const { speedOf } = await import('../web/src/car.js');
  const weser = city.list('edge').find((e) => e.name === 'Weserstraße' && e.len > 600), k = weser.pts.length >> 1 & ~1;
  // Rixdorf (enge Gassen, Engstellen, dicht aufeinanderfolgende Kreuzungen), Wrangelkiez, Weserstraße (Engstelle Nansenstraße)
  for (const p of [city.places.pickup, city.places.giver, { x: weser.pts[k], y: weser.pts[k + 1] }]) {
    const w = createWorld({ city, seed: 5 });
    w.camera.x = p.x; w.camera.y = p.y;
    const still = new Map();
    let crashes = 0, worst = 0;
    for (let i = 0; i < 180 * 60; i++) {
      updateWorld(w, idle(), 1 / 60);
      w.camera.x = p.x; w.camera.y = p.y;
      crashes += w.events.filter((e) => e.type === 'crash').length;
      if (i % 30) continue;
      for (const c of w.cars) {
        if (c.driver !== 'npc') continue;
        const t = speedOf(c) < 5 ? (still.get(c.id) ?? 0) + 0.5 : 0;
        still.set(c.id, t); worst = Math.max(worst, t);
      }
    }
    assert.ok(worst < 90, `ein Auto stand ${worst} s am Stück`);
    assert.ok(crashes < 15, `${crashes} Zusammenstöße in 3 min`);
  }
});

test('Engstellen: nie Gegenverkehr gleichzeitig darin, und wer darin fährt, ist eingetragen', async () => {
  const { narrowKey, narrowDir } = await import('../web/src/traffic.js');
  const weser = city.list('edge').find((e) => e.name === 'Weserstraße' && e.len > 600), k = weser.pts.length >> 1 & ~1;
  let samples = 0;
  for (const p of [city.places.pickup, { x: weser.pts[k], y: weser.pts[k + 1] }]) {
    const w = createWorld({ city, seed: 5 });
    w.camera.x = p.x; w.camera.y = p.y;
    for (let i = 0; i < 150 * 60; i++) {
      updateWorld(w, idle(), 1 / 60);
      w.camera.x = p.x; w.camera.y = p.y;
      if (i % 15) continue;
      const dirs = new Map();
      for (const c of w.cars) {
        const ai = c.ai;
        if (c.driver !== 'npc' || c.wrecked || !ai?.segs?.length) continue;
        let j = 0;
        for (let q = 0; q < ai.segs.length; q++) if (ai.segs[q].k0 <= ai.i) j = q;
        const lane = ai.segs[j].lane;
        if (!lane.narrow || (ai.segs[j].kEnd !== undefined && ai.i > ai.segs[j].kEnd)) continue; // nur wer wirklich auf der Engstelle fährt
        const nk = narrowKey(city, lane.edge), d = narrowDir(city, lane);
        samples++;
        assert.ok(w.nres?.get(nk)?.cars.has(c.id), `t=${w.time.toFixed(1)}: KI#${c.id} fährt auf der Engstelle ${lane.edge.name} ohne Eintrag`);
        const seen = dirs.get(nk);
        assert.ok(!seen || seen.d === d, `t=${w.time.toFixed(1)}: Gegenverkehr auf der Engstelle ${lane.edge.name} (KI#${seen?.id} und KI#${c.id})`);
        dirs.set(nk, { d, id: c.id });
      }
    }
  }
  assert.ok(samples > 200, `zu wenig Engstellen-Fahrten beobachtet (${samples})`);
});

test('Engstelle in Gegenrichtung belegt: das Auto wartet vor der Linie und rollt nicht hinein', async () => {
  const { placeOnLane, narrowKey, narrowDir } = await import('../web/src/traffic.js');
  const { createCar } = await import('../web/src/car.js');
  // Zufahrt ohne Abzweig, die geradewegs in eine Engstelle führt
  const lane = [...g.lanes].find((l) => !l.narrow && l.len > 500 && l.next.length === 1 && l.next[0].narrow && l.next[0].next.length);
  assert.ok(lane, 'keine passende Zufahrt gefunden');
  const nlane = lane.next[0], nk = narrowKey(city, nlane.edge);
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  const b = createCar({ x: 0, y: 0 }); b.driver = 'npc'; placeOnLane(b, city, lane, lane.len - 320, w.rng);
  // der Gegenverkehr: ein Auto auf einer weit entfernten Spur, das die Engstelle in Gegenrichtung hält
  const far = [...g.lanes].find((l) => l.len > 300 && (Math.hypot(l.pts[0] - b.x, l.pts[1] - b.y) > 900 && Math.hypot(l.pts[0] - b.x, l.pts[1] - b.y) < 1500));
  const a = createCar({ x: 0, y: 0 }); a.driver = 'npc'; placeOnLane(a, city, far, 100, w.rng);
  w.cars.push(b, a);
  const hold = () => {
    a.vx = a.vy = 0;
    a.ai.claims = [{ kind: 'n', edge: nk, seg: null, lane: nlane }];
    w.nres = new Map([[nk, { dir: -narrowDir(city, nlane), cars: new Set([a.id]) }]]);
    w.nresGen = city.gen;
  };
  let entered = false, minGap = Infinity;
  for (let i = 0; i < 25 * 60; i++) {
    hold();
    w.camera.x = b.x; w.camera.y = b.y;
    updateWorld(w, idle(), 1 / 60);
    const ai = b.ai;
    let j = 0;
    for (let q = 0; q < ai.segs.length; q++) if (ai.segs[q].k0 <= ai.i) j = q;
    if (ai.segs[j].lane === nlane || (ai.segs[j].kEnd !== undefined && ai.i > ai.segs[j].kEnd)) entered = true;
    assert.ok(w.cars.includes(a), 'der Gegenverkehr wurde abgebaut – Test ungültig');
    const e = nlane.pts;
    minGap = Math.min(minGap, Math.hypot(e[0] - b.x, e[1] - b.y));
  }
  assert.equal(entered, false, 'das Auto ist in die belegte Engstelle gerollt');
  assert.ok(minGap > 30, `hielt nur ${minGap.toFixed(0)} px vor der Engstelle`);
});

test('Kreuzung von kreuzendem Verkehr belegt: das Auto wartet vor der Linie und rollt nicht hinein', async () => {
  const { placeOnLane } = await import('../web/src/traffic.js');
  const { createCar } = await import('../web/src/car.js');
  const lane = [...g.lanes].find((l) => !l.narrow && l.len > 500 && !city.signals.has(l.to) && (city.nodes.get(l.to)?.edges.length ?? 0) >= 3
    && l.next.length === 1 && !l.next[0].narrow);
  assert.ok(lane, 'keine passende Zufahrt gefunden');
  const to = lane.next[0], v = lane.to;
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  const b = createCar({ x: 0, y: 0 }); b.driver = 'npc'; placeOnLane(b, city, lane, lane.len - 320, w.rng);
  const far = [...g.lanes].find((l) => l.len > 300 && Math.hypot(l.pts[0] - b.x, l.pts[1] - b.y) > 900 && Math.hypot(l.pts[0] - b.x, l.pts[1] - b.y) < 1500);
  const a = createCar({ x: 0, y: 0 }); a.driver = 'npc'; placeOnLane(a, city, far, 100, w.rng);
  w.cars.push(b, a);
  // a „fährt“ aus einer anderen Zufahrt in dieselbe Zielspur – das kreuzt b in jedem Fall
  const hold = () => {
    a.vx = a.vy = 0;
    a.ai.claims = [{ kind: 'j', v, seg: null }];
    w.jres = new Map([[v, { approach: 'andere', cars: new Set([a.id]), since: w.time, moves: new Map([[a.id, { ax: 0, ay: 0, bx: 1, by: 1, to }]]) }]]);
  };
  let entered = false;
  for (let i = 0; i < 25 * 60; i++) {
    hold();
    w.camera.x = b.x; w.camera.y = b.y;
    updateWorld(w, idle(), 1 / 60);
    assert.ok(w.cars.includes(a), 'der Querverkehr wurde abgebaut – Test ungültig');
    const ai = b.ai;
    let j = 0;
    for (let q = 0; q < ai.segs.length; q++) if (ai.segs[q].k0 <= ai.i) j = q;
    if (ai.segs[j].lane !== lane || (ai.segs[j].kEnd !== undefined && ai.i > ai.segs[j].kEnd)) entered = true;
  }
  assert.equal(entered, false, 'das Auto ist in die belegte Kreuzung gerollt');
  assert.ok(Math.hypot(b.vx, b.vy) < 5, 'das Auto steht nicht');
});

test('Schlange an der roten Ampel: der Hintermann wartet, statt aufzufahren', async () => {
  const { signalState } = await import('../web/src/signals.js');
  const { placeOnLane } = await import('../web/src/traffic.js');
  const { createCar } = await import('../web/src/car.js');
  const { laneDir } = await import('../web/src/roadgraph.js');
  const { obbVsObb } = await import('../web/src/collision.js');
  const lane = [...g.lanes].find((l) => city.signals.has(l.to) && l.len > 700 && l.edge.cls <= 5 && l.next.length);
  const [ux, uy] = laneDir(lane, true), heading = Math.atan2(uy, ux);
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  let t0 = 0.5;
  while (!(signalState(city, lane.to, heading, t0) === 'red' && signalState(city, lane.to, heading, t0 - 0.5) !== 'red')) t0 += 0.5;
  w.time = t0;
  const cars = [0, 1, 2].map((k) => { const c = createCar({ x: 0, y: 0 }); c.driver = 'npc'; placeOnLane(c, city, lane, lane.len - 500 - k * 70, w.rng); w.cars.push(c); return c; });
  w.camera.x = cars[1].x; w.camera.y = cars[1].y; // sonst gelten die Autos als weit weg und werden abgebaut
  let crashes = 0, overlap = 0;
  for (let i = 0; i < 25 * 60; i++) {
    updateWorld(w, idle(), 1 / 60);
    w.camera.x = cars[1].x; w.camera.y = cars[1].y;
    crashes += w.events.filter((e) => e.type === 'crash').length;
    for (let a = 0; a < 3; a++) for (let b = a + 1; b < 3; b++) if (obbVsObb(cars[a], cars[b])) overlap++;
  }
  assert.ok(cars.every((c) => Math.hypot(c.vx, c.vy) < 5), 'alle stehen an Rot');
  assert.equal(crashes, 0, 'niemand fährt auf');
  assert.equal(overlap, 0, 'keine Berührung');
});

test('Angefahrene Fußgänger stehen wieder auf (sonst wartet der Verkehr ewig vor ihnen)', async () => {
  const { knockDown, updatePed, createPed, nearestSpot } = await import('../web/src/pedestrians.js');
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  const ped = createPed(city, nearestSpot(city, w.player.x, w.player.y), w.rng);
  knockDown(ped, ped.x + 30, ped.y);
  for (let i = 0; i < 5 * 60; i++) updatePed(ped, w, 1 / 60);
  assert.notEqual(ped.state, 'down', 'nach 5 s wieder auf den Beinen');
});
