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
  const quay = city.list('wall').filter((f) => f.kind === 'wall').map((f) => f.pts).find((wl) => {
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
