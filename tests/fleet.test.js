// Fahrzeugarten (fleet.js), Arbeitshalte und Einsätze (services.js), Zielfahrt (traffic.js goalField) und Räder (bikes.js).
import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { createCar, stepCar, speedOf } from '../web/src/car.js';
import { KINDS, pickKind, mayStopOn, nextStopAfter, sirenHigh, SIREN } from '../web/src/fleet.js';
import { buildLaneGraph } from '../web/src/roadgraph.js';
import { placeOnLane, goalField, spawnSpot } from '../web/src/traffic.js';
import { manageEmergency, updateService, EMERG } from '../web/src/services.js';
import { bikeable, bikePath, createBike, updateBike, parkedScooters, BIKE } from '../web/src/bikes.js';
import { onRoad, inBuilding } from '../web/src/map.js';
import { projectOnPolyline } from '../web/src/geom.js';
import { LIFE } from '../web/src/life.js';

const city = realCity();
const giver = city.places.giver;
const DI = 1, SO = 6;
const run = (w, sec) => { for (let i = 0; i < sec * 60; i++) updateWorld(w, idle(), 1 / 60); };
const lanes = () => [...buildLaneGraph(city).lanes];
// ruhige Welt ohne Verkehr, mit Rhythmus-Schalter nach Wunsch
function quiet() { const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle'; return w; }

test('Fahrzeugarten: Müllauto nur werktags morgens in Wohnstraßen, Paketwagen nicht sonntags, LKW tags auf Hauptstraßen', () => {
  const share = (m, d, cls, kind) => { let n = 0; for (let i = 0; i < 1000; i++) if (pickKind(m, d, cls, i / 1000) === kind) n++; return n / 1000; };
  assert.ok(share(8 * 60, DI, 7, 'garbage') > 0.03);
  assert.equal(share(15 * 60, DI, 7, 'garbage'), 0, 'nachmittags kein Müllauto');
  assert.equal(share(8 * 60, SO, 7, 'garbage'), 0, 'sonntags kein Müllauto');
  assert.equal(share(8 * 60, DI, 3, 'garbage'), 0, 'nicht auf Hauptstraßen');
  assert.equal(share(11 * 60, SO, 5, 'delivery'), 0, 'sonntags keine Pakete');
  assert.ok(share(11 * 60, DI, 5, 'delivery') > 0.05);
  assert.ok(share(11 * 60, DI, 3, 'truck') > share(3 * 60, DI, 3, 'truck'), 'LKW tagsüber häufiger');
  for (const k of Object.keys(KINDS)) {
    const c = createCar({ x: 0, y: 0, kind: k });
    assert.equal(c.hw, KINDS[k].L / 2); assert.equal(c.hh, KINDS[k].W / 2); assert.equal(c.kind, k);
  }
  assert.equal(createCar({ x: 0, y: 0 }).kind, 'car');
});

test('LKW ist träger und langsamer als ein Pkw', () => {
  const a = createCar({ x: 0, y: 0 }), b = createCar({ x: 0, y: 0, kind: 'truck' });
  for (const c of [a, b]) { c.controls.throttle = 1; for (let i = 0; i < 600; i++) stepCar(c, 1 / 60, null); }
  assert.ok(speedOf(b) < speedOf(a) * 0.9, `LKW ${speedOf(b).toFixed(0)} vs Pkw ${speedOf(a).toFixed(0)}`);
});

// Zwei Fahrzeuge hintereinander auf einer langen Spur: der Vordermann hält, der hintere hält Abstand
function convoy(frontKind, backKind) {
  const w = quiet();
  // lange, ruhige Spur nahe der Spielfigur (die Kamera folgt ihr; Fernes baut die Welt ab)
  const pl = w.player;
  const lane = lanes().filter((l) => l.len > 900 && l.edge.cls >= 5 && l.edge.cls <= 7 && !l.narrow && !city.signals.has(l.to)
    && Math.hypot(l.pts[0] - pl.x, l.pts[1] - pl.y) < 1400).sort((a, b) => b.len - a.len)[0];
  const front = createCar({ x: 0, y: 0, kind: frontKind }), back = createCar({ x: 0, y: 0, kind: backKind });
  placeOnLane(front, city, lane, 500, w.rng); placeOnLane(back, city, lane, 150, w.rng);
  front.driver = back.driver = 'npc';
  w.cars.push(front, back);
  w.camera.x = front.x; w.camera.y = front.y;
  return { w, front, back, lane };
}
test('Abstand gilt zwischen den Stoßstangen: LKW hinter Pkw und Pkw hinter Müllauto fahren nicht auf', () => {
  for (const [f, b] of [['car', 'truck'], ['garbage', 'car'], ['truck', 'truck']]) {
    const { w, front, back } = convoy(f, b);
    front.ai.hold = 1e9; // steht (wie bei einem Arbeitshalt)
    let crash = 0;
    for (let i = 0; i < 25 * 60; i++) { updateWorld(w, idle(), 1 / 60); crash += w.events.filter((e) => e.type === 'crash').length; w.camera.x = front.x; w.camera.y = front.y; }
    const gap = Math.hypot(front.x - back.x, front.y - back.y) - front.hw - back.hw;
    assert.equal(crash, 0, `${b} hinter ${f}: Auffahrunfall`);
    assert.ok(gap > 8 && gap < 45, `${b} hinter ${f}: Lücke ${gap.toFixed(1)} px`);
  }
});

test('Paketwagen hält in zweiter Reihe mit Warnblinker, der Verkehr dahinter wartet, danach geht es weiter', () => {
  const { w, front, back, lane } = convoy('delivery', 'car');
  w.rhythm = false;
  run(w, 2);
  front.ai.odo = 1e9; front.ai.nextStop = 0; // Halt fällig
  run(w, 0.5);
  assert.ok(front.ai.hold > 0 && front.hazard, 'hält mit Warnblinker');
  run(w, 6);
  assert.ok(speedOf(front) < 1 && speedOf(back) < 5, 'beide stehen');
  front.ai.hold = 0.1;
  run(w, 3);
  assert.equal(front.hazard, false, 'Warnblinker aus');
  assert.ok(speedOf(front) > 20, 'fährt weiter');
  // Regeln: nicht an der Kreuzung, Paketwagen nie auf Autobahn/Schnellstraße, Müllauto nur in Nebenstraßen
  assert.equal(mayStopOn('delivery', lane, 100, 400), false, 'nicht kurz vor der Kreuzung');
  assert.equal(mayStopOn('delivery', lane, 400, 60), false, 'nicht direkt nach der Kreuzung');
  assert.equal(mayStopOn('delivery', { edge: { cls: 2 } }, 900, 900), false);
  assert.equal(mayStopOn('garbage', { edge: { cls: 3 } }, 900, 900), false);
  assert.equal(mayStopOn('car', lane, 900, 900), false);
  assert.ok(nextStopAfter('garbage', 0.5) < nextStopAfter('delivery', 0.5), 'Müllauto hält öfter');
});

test('Zielfahrt: jeder Schritt entlang des Entfernungsfeldes kommt dem Ziel näher und erreicht es', () => {
  const start = { x: giver.x + 2500, y: giver.y + 1500 };
  const f = goalField(city, start.x, start.y, giver.x, giver.y);
  assert.ok(f && f.dist.size > 300, `Feld ${f?.dist.size}`);
  let lane = [...f.dist.keys()].filter((l) => Math.hypot(l.pts[0] - start.x, l.pts[1] - start.y) < 400).sort((a, b) => f.dist.get(a) - f.dist.get(b))[0];
  assert.ok(lane, 'Startspur im Feld');
  let d = f.dist.get(lane), steps = 0;
  while (lane !== f.goal && steps++ < 200) {
    const next = lane.next.filter((n) => f.dist.has(n)).sort((a, b) => f.dist.get(a) - f.dist.get(b))[0];
    assert.ok(next, 'Nachfolger mit Restentfernung');
    assert.ok(f.dist.get(next) < d, 'Restentfernung sinkt');
    lane = next; d = f.dist.get(next);
  }
  assert.equal(lane, f.goal);
});

test('Tote auf der Straße: Rettungswagen kommt außer Sicht mit Martinshorn, hält am Einsatzort, nimmt den Toten mit, fährt ab', () => {
  const w = createWorld({ city }); // Standardbevölkerung = Einsätze aktiv
  w.mission.state = 'idle'; w.clock = 11 * 60; w.day = DI;
  run(w, 1 / 60);
  const p = w.peds.find((q) => q.state === 'walk' && Math.hypot(q.x - w.player.x, q.y - w.player.y) < 400);
  p.state = 'dead'; p.deadT = 0;
  let amb = null, bornInView = null, arrived = false, sirenOnWay = false;
  for (let s = 0; s < 150 * 60 && !(amb && amb.done); s++) {
    updateWorld(w, idle(), 1 / 60);
    const a = w.cars.find((c) => c.kind === 'ambulance' && c.duty?.inc);
    if (a && !amb) { amb = a; bornInView = Math.abs(a.x - w.camera.x) < LIFE.viewHalfX && Math.abs(a.y - w.camera.y) < LIFE.viewHalfY; }
    if (amb && amb.duty.phase === 'drive' && amb.siren) sirenOnWay = true;
    if (amb?.duty.phase === 'scene') { arrived = true; assert.ok(Math.hypot(amb.x - p.x, amb.y - p.y) < EMERG.arrive + 30); }
  }
  assert.ok(amb, 'Rettungswagen alarmiert');
  assert.equal(bornInView, false, 'entsteht außer Sicht');
  assert.ok(sirenOnWay, 'Anfahrt mit Sondersignal');
  assert.ok(arrived, 'am Einsatzort angekommen');
  assert.ok(amb.done && !w.peds.includes(p), 'Toter mitgenommen');
  assert.equal(amb.siren, true, 'Abfahrt mit Sondersignal ins Krankenhaus');
});

test('Schüsse rufen die Polizei – aber nicht für jeden Schuss einen neuen Wagen', () => {
  const w = createWorld({ city }); w.mission.state = 'idle';
  run(w, 1 / 60);
  w.events.push({ type: 'shot', x: w.player.x, y: w.player.y });
  manageEmergency(w, 0);
  w.events.push({ type: 'shot', x: w.player.x, y: w.player.y });
  manageEmergency(w, 0);
  assert.equal(w.emerg.incidents.filter((i) => i.kind === 'police').length, 1);
  run(w, EMERG.policeDelay + 1);
  const pol = w.cars.find((c) => c.kind === 'police' && c.duty?.inc);
  assert.ok(pol && pol.siren && pol.ai.urgent && pol.ai.field, 'Streifenwagen mit Sondersignal unterwegs');
  // feste Bevölkerung (Tests, Demo): keine Einsätze
  const q = quiet(); q.events.push({ type: 'shot', x: 0, y: 0 }); manageEmergency(q, 20);
  assert.equal(q.emerg, undefined);
});

test('Martinshorn wechselt zwischen 440 und 585 Hz im 0,6-s-Takt', () => {
  assert.equal(sirenHigh(0.1), false); assert.equal(sirenHigh(0.7), true); assert.equal(sirenHigh(1.3), false);
  assert.deepEqual([SIREN.low, SIREN.high], [440, 585]);
});

test('Radfahrer: auf der rechten Spur, auf dem Radstreifen wo es einen gibt, nie auf Hauptstraßen ohne Radstreifen', () => {
  const all = lanes();
  const bl = all.filter(bikeable);
  assert.ok(bl.length > 500);
  for (const l of bl) {
    assert.equal(l.k, l.n - 1, 'rechte Spur');
    assert.ok(l.edge.cls >= 3 && l.edge.cls <= 8);
    if (l.edge.cls <= 4) assert.ok((l.dir === 1 ? l.edge.cs.right : l.edge.cs.left).cycle > 0, 'Hauptstraße nur mit Radstreifen');
  }
  let cyc = 0;
  for (const l of bl.slice(0, 2000)) {
    const p = bikePath(city, l), side = l.dir === 1 ? l.edge.cs.right : l.edge.cs.left;
    const mid = projectOnPolyline(l.edge.pts, p.pts[2], p.pts[3]);
    const off = Math.hypot(mid.x - p.pts[2], mid.y - p.pts[3]);
    assert.ok(off <= l.edge.cs.width / 2 + 1, 'innerhalb der Fahrbahn');
    if (side.cycle > 0) { cyc++; const want = l.edge.cs.width / 2 - side.parkW - side.cycle / 2; assert.ok(Math.abs(off - want) < 6, `Radstreifenmitte ${off.toFixed(1)} vs ${want.toFixed(1)}`); }
  }
  assert.ok(cyc > 20, `Spuren mit Radstreifen ${cyc}`);
});

test('Radverkehr in der Welt: fährt auf der Fahrbahn, bleibt nicht hängen; ein angefahrenes Rad stürzt', () => {
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 17 * 60; w.day = DI;
  run(w, 1 / 60);
  assert.ok(w.bikes.length > 3, `Räder ${w.bikes.length}`);
  const still = new Map();
  let worst = 0;
  for (let s = 0; s < 90 * 60; s++) {
    updateWorld(w, idle(), 1 / 60);
    if (s % 30) continue;
    for (const b of w.bikes) {
      if (b.state !== 'ride') continue;
      if (!b.cross) assert.ok(onRoad(w.city, b.x, b.y, 4) && !inBuilding(w.city, b.x, b.y), 'Rad auf der Fahrbahn');
      const t = b.speed < 2 ? (still.get(b) ?? 0) + 0.5 : 0; still.set(b, t); worst = Math.max(worst, t);
    }
  }
  assert.ok(worst < 60, `ein Rad stand ${worst} s`);
  // Unfall: schnelles Auto trifft ein Rad
  const b = w.bikes.find((x) => x.state === 'ride');
  const car = createCar({ x: b.x - Math.cos(b.angle) * 30, y: b.y - Math.sin(b.angle) * 30, angle: b.angle });
  car.vx = Math.cos(b.angle) * 250; car.vy = Math.sin(b.angle) * 250;
  w.cars.push(car);
  run(w, 0.3);
  assert.equal(b.state, 'lying');
  assert.ok(w.peds.some((p) => p.state === 'down' && Math.hypot(p.x - b.x, p.y - b.y) < 60), 'Fahrer liegt');
});

test('Räder halten meist bei Rot; wer nicht gehorcht, fährt durch', () => {
  const w = quiet();
  const l = lanes().find((x) => bikeable(x) && city.signals.has(x.to) && x.len > 300);
  assert.ok(l);
  const stopAt = (obeys) => {
    const b = createBike(city, l, bikePath(city, l).len - 120, w.rng);
    b.obeys = obeys; w.bikes = [b];
    w.time = 0;
    let passedRed = false;
    for (let i = 0; i < 20 * 60; i++) {
      // Ampel für die Zufahrt dauerhaft rot: Zeit festhalten auf eine Rotphase
      w.time = redTime(l);
      updateBike(b, w, 1 / 60);
      if (b.lane !== l || b.cross) { passedRed = true; break; }
    }
    return passedRed;
  };
  assert.equal(stopAt(true), false, 'wartet bei Rot');
  assert.equal(stopAt(false), true, 'fährt bei Rot');
});
async function importSignals() { return import('../web/src/signals.js'); }
const { signalState } = await importSignals();
function redTime(l) { // eine Zeit, zu der die Zufahrt Rot hat
  const p = bikePath(city, l).pts, n = p.length, h = Math.atan2(p[n - 1] - p[n - 3], p[n - 2] - p[n - 4]);
  for (let t = 0; t < 60; t += 0.5) if (signalState(city, l.to, h, t) === 'red') return t;
  return 0;
}

test('Abgestellte E-Roller stehen auf dem Gehweg, nicht auf der Fahrbahn oder im Haus, und immer gleich', () => {
  let n = 0;
  for (const e of city.list('edge')) {
    const a = parkedScooters(city, e);
    for (const s of a) { n++; assert.ok(!onRoad(city, s.x, s.y) && !inBuilding(city, s.x, s.y)); }
    delete e._scoot;
    assert.deepEqual(parkedScooters(city, e).map((s) => [s.x, s.y]), a.map((s) => [s.x, s.y]));
  }
  assert.ok(n > 200, `Roller ${n}`);
});
