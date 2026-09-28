import test from 'node:test';
import assert from 'node:assert/strict';
import { vehicleState, transitNear, speedOfPattern, alightSpot, stationExit, RIDE } from '../web/src/ride.js';
import { positionAt, pointOn } from '../web/src/transit.js';
import { createWorld, updateWorld, resetPopulation, endRide, teleportTo } from '../web/src/world.js';
import { updateFight, hurtPlayer } from '../web/src/combat.js';
import { execute } from '../web/src/console.js';
import { makeSave, validateSave } from '../web/src/save.js';
import { realCity, realTransit, realOverview } from './helpers/city.js';
import { idle } from './helpers/bot.js';

const city = realCity(), tr = realTransit();
const m10 = tr.patterns.filter((p) => p.name === 'M10').sort((a, b) => b.stops.length - a.stops.length)[0];

function worldWithTram(tauAt = 'moving') {
  const mid = pointOn(m10, m10.stops[Math.floor(m10.stops.length / 2)]);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60; w.day = 1;
  w.camera.x = mid.x; w.camera.y = mid.y; w.player.x = mid.x; w.player.y = mid.y; resetPopulation(w);
  for (let i = 0; i < 30; i++) updateWorld(w, idle(), 1 / 60);
  const s = w.transit.tracked.get(m10.id);
  let tau = 0; const want = tauAt === 'moving' ? (q) => !q.dwelling : (q) => q.dwelling;
  while (!(positionAt(m10, tau).s > m10.stops[Math.floor(m10.stops.length / 2)] - 400 && want(positionAt(m10, tau)))) tau += 0.5;
  const v = { tau, delay: 0, key: 'test' }; s.veh.push(v);
  return { w, v };
}

test('Fahrzeuglage: Wagen, Tempo aus dem Fahrplan, beim Halten 0; verschwundenes Fahrzeug → null', () => {
  const { w, v } = worldWithTram('moving');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  assert.equal(st.mode, 'tram'); assert.equal(st.cars.length, 3); assert.ok(st.speed > 20, `Tempo ${st.speed}`);
  assert.equal(st.speed, speedOfPattern(m10, v.tau));
  v.gone = true; w.transit.tracked.get(m10.id).veh = w.transit.tracked.get(m10.id).veh.filter((x) => !x.gone);
  assert.equal(vehicleState(w, { pid: m10.id, key: 'test' }), null);
  assert.equal(vehicleState(w, { carId: -1 }), null);
  w.transit.tracked.get(m10.id).veh.push({ tau: m10.duration + 1, delay: 0, key: 'test-end' });
  assert.equal(vehicleState(w, { pid: m10.id, key: 'test-end' }), null, 'Linienende (pos.done) → null');
  let tau = 0; while (!positionAt(m10, tau).dwelling || tau < 30) tau += 0.5;
  assert.equal(speedOfPattern(m10, tau), 0);
});

test('In Reichweite: nur nah am Wagen-Rechteck (25 px), sortiert, mit Wagennummer und Abstand zur Spitze', () => {
  const { w } = worldWithTram('moving');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  const c2 = st.cars[2], nx = -Math.sin(c2.angle), ny = Math.cos(c2.angle);
  const near = transitNear(w, c2.x + nx * (c2.W / 2 + 10), c2.y + ny * (c2.W / 2 + 10), RIDE.reach);
  const hit = near.find((h) => h.ref.key === 'test');
  assert.ok(hit, 'neben Wagen 3'); assert.equal(hit.car, 2); assert.ok(hit.front > 200);
  assert.ok(!transitNear(w, c2.x + nx * (c2.W / 2 + 40), c2.y + ny * (c2.W / 2 + 40), RIDE.reach).some((h) => h.ref.key === 'test'), 'zu weit');
  for (let i = 1; i < near.length; i++) assert.ok(near[i].dist >= near[i - 1].dist);
});

test('Aussteigen: Platz neben dem Wagen, sonst null; Straßenausgang am Bahnhof', () => {
  const { w } = worldWithTram('dwell');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  const sp = alightSpot(w, st, 1);
  assert.ok(sp && Math.hypot(sp.x - st.cars[1].x, sp.y - st.cars[1].y) < 60);
  const u8 = tr.patterns.filter((p) => p.name === 'U8').sort((a, b) => b.stops.length - a.stops.length)[0];
  const i = u8.stopNames.findIndex((n) => n.startsWith('U Kottbusser Tor'));
  const ex = stationExit(w, u8, i), stop = pointOn(u8, u8.stops[i]);
  assert.ok(Math.hypot(ex.x - stop.x, ex.y - stop.y) < 400, 'Ausgang in der Nähe des Bahnsteigs');
});

// Kandidaten exakt wie alightSpot sie berechnet, damit der Test Hindernisse dorthin legen kann.
function alightCandidates(st, i) {
  const c = st.cars[Math.min(i, st.cars.length - 1)], nx = -Math.sin(c.angle), ny = Math.cos(c.angle), d = c.W / 2 + 12;
  const last = st.cars[st.cars.length - 1];
  return {
    right: { x: c.x + nx * d, y: c.y + ny * d },
    left: { x: c.x - nx * d, y: c.y - ny * d },
    behind: { x: last.x - Math.cos(last.angle) * (last.L / 2 + 14), y: last.y - Math.sin(last.angle) * (last.L / 2 + 14) },
  };
}

test('Aussteigen bei besetzten Plätzen: rechts (Baum) → links, auch das (parkendes Auto) → hinter dem letzten Wagen, auch das (Zaun) → kein Platz', () => {
  // city.solids ist die geteilte Hash von realCity() (Singleton über die ganze Testdatei) – jedes Einfügen
  // muss wieder entfernt werden, sonst blieben die Hindernisse für spätere Tests an derselben Stelle liegen.
  const { w } = worldWithTram('dwell');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  const { right, left, behind } = alightCandidates(st, 1);

  let sp = alightSpot(w, st, 1);
  assert.ok(sp && Math.hypot(sp.x - right.x, sp.y - right.y) < 1, 'frei: rechts gewählt');

  const tree = { x: right.x, y: right.y, r: 10 }; // Kreis-Hindernis (Baum/Poller) – kein s.seg
  const treeKeys = w.solids.insert(tree, { x: tree.x - tree.r, y: tree.y - tree.r, w: 2 * tree.r, h: 2 * tree.r });
  try {
    sp = alightSpot(w, st, 1);
    assert.ok(sp && Math.hypot(sp.x - left.x, sp.y - left.y) < 1, 'rechts durch Baum blockiert → links gewählt');

    const parked = { id: -1001, x: left.x, y: left.y, angle: 0, hw: 20, hh: 10 };
    w.cars.push(parked);
    try {
      sp = alightSpot(w, st, 1);
      assert.ok(sp && Math.hypot(sp.x - behind.x, sp.y - behind.y) < 1, 'links durch parkendes Auto blockiert → hinter dem letzten Wagen');

      const wall = { ax: behind.x - 20, ay: behind.y, bx: behind.x + 20, by: behind.y, seg: true, kind: 'fence' };
      const wallKeys = w.solids.insert(wall, { x: wall.ax, y: wall.ay - 1, w: wall.bx - wall.ax, h: 2 });
      try {
        assert.equal(alightSpot(w, st, 1), null, 'alle drei Plätze blockiert → kein Platz');
      } finally {
        w.solids.remove(wall, wallKeys);
      }
    } finally {
      w.cars.pop();
    }
  } finally {
    w.solids.remove(tree, treeKeys);
  }
});

test('Fahrzeuglage für ein Bus-Muster ohne materialisiertes Auto: null statt Absturz (trainCars kennt keine Bus-Wagen)', () => {
  const { w } = worldWithTram('moving');
  const busPattern = tr.patterns.find((p) => p.mode === 'bus');
  assert.ok(busPattern, 'Vorbedingung: es gibt ein Bus-Muster in den Testdaten');
  w.transit.tracked.set(busPattern.id, { veh: [{ tau: 100, delay: 0, key: 'bus-test' }], acc: 0, n: 0 });
  assert.equal(vehicleState(w, { pid: busPattern.id, key: 'bus-test' }), null, 'Bus-Fahrplan-Fahrzeug ohne KI-Auto → null');
});

test('Fahrzeuglage für ein Bus-Auto: nach dem Absturz (wrecked) null statt eines Geisterbusses', () => {
  const { w } = worldWithTram('moving');
  const bus = { id: 9001, x: 100, y: 100, angle: 0, vx: 10, vy: 0, wrecked: false, duty: { bus: true, pid: 0, stop: 2, s: 500, off: 0, boarding: false } };
  w.cars.push(bus);
  assert.ok(vehicleState(w, { carId: 9001 }), 'zunächst vorhanden');
  bus.wrecked = true;
  assert.equal(vehicleState(w, { carId: 9001 }), null, 'wrecked → null');
});

const press = (w, patch) => updateWorld(w, { ...idle(), ...patch }, 1 / 60);

function standBeside(w, key, car = 1) {
  const st = vehicleState(w, { pid: m10.id, key }), c = st.cars[car], nx = -Math.sin(c.angle), ny = Math.cos(c.angle);
  w.player.x = c.x + nx * (c.W / 2 + 12); w.player.y = c.y + ny * (c.W / 2 + 12);
}

test('Fahrgast: G neben einer haltenden Tram steigt ein, Spieler fährt mit, G steigt neben der Tür aus', () => {
  const { w } = worldWithTram('dwell');
  standBeside(w, 'test');
  press(w, { ride: true });
  assert.equal(w.player.ride?.kind, 'passenger'); assert.equal(w.player.ride.mode, 'tram');
  assert.ok(w.events.some((e) => e.type === 'board' && !e.hop));
  let moved = 0; const x0 = w.player.x, y0 = w.player.y;
  // bis zum nächsten Halt mitfahren (Verkehr vor der Tram kann sie aufhalten – daher großzügige Obergrenze)
  for (let i = 0; i < 60 * 120; i++) {
    press(w, {}); w.camera.x = w.player.x; w.camera.y = w.player.y;
    moved = Math.max(moved, Math.hypot(w.player.x - x0, w.player.y - y0));
    if (moved > 100 && vehicleState(w, w.player.ride.ref)?.dwelling) break;
  }
  assert.ok(moved > 100, `fährt mit (${moved.toFixed(0)} px)`);
  const st = vehicleState(w, w.player.ride.ref);
  assert.ok(st.dwelling, 'Vorbedingung: Tram hält wieder');
  press(w, { ride: true });
  assert.equal(w.player.ride, null);
  assert.ok(w.events.some((e) => e.type === 'alight' && !e.hop));
  assert.ok(Math.hypot(w.player.x - st.cars[1].x, w.player.y - st.cars[1].y) < 80, 'neben dem Wagen');
  assert.ok(w.player.stun <= 0, 'kein Sturz bei stehender Bahn');
});

test('Aufspringen und Abspringen während der Fahrt; Fahrt-Ende, wenn das Fahrzeug verschwindet', () => {
  const { w, v } = worldWithTram('moving');
  standBeside(w, 'test', 0);
  press(w, { ride: true });
  assert.ok(w.player.ride, 'aufgesprungen'); assert.ok(w.events.some((e) => e.type === 'board' && e.hop));
  for (let i = 0; i < 20; i++) press(w, {});
  const st = vehicleState(w, w.player.ride.ref);
  assert.ok(st && !st.dwelling && st.speed > RIDE.hopOff, `Vorbedingung: Tram fährt schneller als hopOff (${st?.speed})`);
  const hp = w.player.hp;
  press(w, { ride: true });
  assert.equal(w.player.ride, null);
  assert.ok(w.events.some((e) => e.type === 'alight' && e.hop), 'Absprung');
  assert.ok(w.player.stun > 0, 'betäubt nach dem Absprung');
  if (st.speed > RIDE.hurtFrom) assert.ok(w.player.hp < hp, 'verletzt bei hohem Tempo'); else assert.ok(w.player.hp <= hp);
  // Fahrzeug verschwindet → Notausstieg an der letzten Haltestelle
  standBeside(w, 'test', 0); press(w, { ride: true });
  assert.ok(w.player.ride, 'wieder aufgesprungen');
  const last = w.player.ride.lastStop;
  v.gone = true;
  press(w, {});
  assert.equal(w.player.ride, null);
  assert.ok(Number.isFinite(w.player.x) && Number.isFinite(w.player.y) && Math.hypot(w.player.x - last.x, w.player.y - last.y) < 400, 'an der letzten Haltestelle');
  assert.ok(w.events.some((e) => e.type === 'ride-end' && e.reason === 'gone'));
});

test('Während der Fahrt: kein Laufen, kein Schießen, Kamera folgt; Speichern = letzte Haltestelle zu Fuß; tp beendet die Fahrt', () => {
  const { w } = worldWithTram('dwell');
  standBeside(w, 'test'); press(w, { ride: true });
  const st = vehicleState(w, w.player.ride.ref);
  press(w, { moveX: 1, fire: true, firePressed: true });
  assert.ok(Math.hypot(w.player.x - st.cars[w.player.ride.car].x, w.player.y - st.cars[w.player.ride.car].y) < 5, 'sitzt im Wagen');
  assert.ok(!w.events.some((e) => e.type === 'shot' || e.type === 'swing'));
  const save = validateSave(makeSave(w));
  assert.ok(Math.hypot(save.player.x - w.player.ride.lastStop.x, save.player.y - w.player.ride.lastStop.y) < 1);
  endRide(w, 'teleport');
  assert.equal(w.player.ride, null);
  // umgehauen während der Fahrt (z. B. Schuss durchs Fenster): Fahrt endet, niemand fährt als Toter weiter
  standBeside(w, 'test'); press(w, { ride: true });
  assert.ok(w.player.ride);
  hurtPlayer(w, 999, w.player.x + 30, w.player.y); // wie ein echter K. o. (setzt dead + deadT)
  assert.ok(w.player.dead); press(w, {});
  assert.equal(w.player.ride, null);
  assert.ok(w.events.some((e) => e.type === 'ride-end' && e.reason === 'ko'), 'Fahrt endet wegen K. o., nicht erst beim Aufwachen');
  assert.ok(Number.isFinite(w.player.x) && Number.isFinite(w.player.y));
});

test('Fahrt-Ende von außen: Teleport (Karte/Konsole), Schläger lassen vom Fahrgast ab', () => {
  const { w } = worldWithTram('dwell');
  standBeside(w, 'test'); press(w, { ride: true });
  assert.ok(w.player.ride);
  // Gegner geben auf, solange der Spieler im Wagen sitzt
  const ped = { x: w.player.x + 10, y: w.player.y, state: 'fight', fightT: 0 };
  assert.equal(updateFight(ped, w, 1 / 60, () => {}), false, 'Schläger lässt vom Fahrgast ab');
  // Konsolen-tp beendet die Fahrt sofort (Teleport folgt, sobald die Kacheln da sind)
  city.overview ??= realOverview(); // placeIndex (tp) braucht die Übersicht
  const g = { world: w, teleport: null };
  const res = execute('tp Kottbusser Tor', { game: g, world: w, city });
  assert.notEqual(res.ok, false, 'tp ausgeführt');
  assert.equal(w.player.ride, null); assert.ok(g.teleport);
  // Teleport über die Karte (teleportTo) während einer Fahrt: Fahrt endet, Spieler steht an der Zielstelle
  standBeside(w, 'test'); press(w, { ride: true });
  if (!w.player.ride) { standBeside(w, 'test', 0); press(w, { ride: true }); }
  assert.ok(w.player.ride, 'wieder eingestiegen');
  teleportTo(w, { x: w.player.x + 50, y: w.player.y + 50, angle: 0 });
  assert.equal(w.player.ride, null);
  assert.ok(w.events.some((e) => e.type === 'ride-end' && e.reason === 'teleport'));
  press(w, {});
  assert.equal(w.player.ride, null, 'bleibt ausgestiegen');
});
