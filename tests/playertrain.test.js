import test from 'node:test';
import assert from 'node:assert/strict';
import { takeTrain, trainAhead, turnAround } from '../web/src/playertrain.js';
import { transitNear, vehicleState, RIDE } from '../web/src/ride.js';
import { positionAt, pointOn, trainCars } from '../web/src/transit.js';
import { createWorld, updateWorld, resetPopulation } from '../web/src/world.js';
import { realCity, realTransit } from './helpers/city.js';
import { idle } from './helpers/bot.js';

const city = realCity(), tr = realTransit();
const pat = (name, mode) => tr.patterns.filter((p) => p.name === name && p.mode === mode).sort((a, b) => b.stops.length - a.stops.length)[0];
const press = (w, patch = {}) => { updateWorld(w, { ...idle(), ...patch }, 1 / 60); w.camera.x = w.player.x; w.camera.y = w.player.y; };

function atFrontOf(p, stopIdx) {
  const at = pointOn(p, p.stops[stopIdx]);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60; w.day = 1;
  w.camera.x = at.x; w.camera.y = at.y; w.player.x = at.x; w.player.y = at.y; resetPopulation(w);
  for (let i = 0; i < 30; i++) press(w);
  const s = w.transit.tracked.get(p.id);
  let tau = 0; while (!(positionAt(p, tau).stop === stopIdx && positionAt(p, tau).dwelling)) tau += 0.5;
  s.veh.push({ tau, delay: 0, key: 'mine' });
  const st = vehicleState(w, { pid: p.id, key: 'mine' }), f = st.cars[0];
  w.player.x = f.x + Math.cos(f.angle) * (f.L / 2 + 10); w.player.y = f.y + Math.sin(f.angle) * (f.L / 2 + 10);
  return w;
}

test('Übernehmen: Y am Führerstand macht aus der Fahrplan-Bahn den Spielerzug – gleiche Lage, Fahrplan-Fahrzeug weg', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  const before = vehicleState(w, { pid: p.id, key: 'mine' });
  press(w, { enterExit: true });
  assert.ok(w.playerTrain, 'Spielerzug'); assert.equal(w.player.ride.kind, 'driver');
  assert.ok(Math.abs(w.playerTrain.s - before.s) < 1);
  assert.equal(vehicleState(w, { pid: p.id, key: 'mine' }), null, 'aus dem Fahrplan genommen');
  // weiter hinten stehend (nicht am Führerstand) übernimmt man nicht
  const w2 = atFrontOf(p, 3), st = vehicleState(w2, { pid: p.id, key: 'mine' }), c = st.cars[2];
  w2.player.x = c.x + 20; w2.player.y = c.y + 20;
  press(w2, { enterExit: true });
  assert.equal(w2.playerTrain ?? null, null);
});

test('Fahren: Gas bewegt entlang der Linie, Türen nur im Stand an der Haltestelle, Trinkgeld beim sauberen Halt', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  // ohne Straßenverkehr: geprüft wird Fahren/Türen/Trinkgeld; Hindernisse auf dem Gleis prüft der Test weiter unten.
  // (Auf der M10 liegt der Linienweg stellenweise in der Gegenspur – entgegenkommende Busse blockieren dort.)
  w.trafficScale = 0;
  const drive = (patch) => { w.cars.length = 0; press(w, patch); };
  drive({ enterExit: true });
  const t = w.playerTrain, s0 = t.s;
  for (let i = 0; i < 60 * 8; i++) drive({ throttle: 1 });
  assert.ok(t.s > s0 + 200, 'fährt'); assert.ok(t.v > 0);
  drive({ action: true });
  assert.equal(t.drive.doors, 'closed', 'Türen nicht in Fahrt');
  // sanft auf die nächste Haltestelle zu
  const target = p.stops[4]; let money = w.money;
  for (let i = 0; i < 60 * 120 && !(t.v === 0 && Math.abs(t.s - target) < 60); i++) {
    const rest = target - t.s, want = Math.sqrt(Math.max(0, 2 * 10 * Math.max(0, rest - 10)));
    drive(t.v > want ? { brake: Math.min(1, (t.v - want) / 20 + 0.3) } : { throttle: rest > 20 ? 0.6 : 0 });
  }
  assert.ok(Math.abs(t.s - target) < 60, `steht am Halt (${(t.s - target).toFixed(0)} px)`);
  drive({ action: true });
  assert.equal(t.drive.doors, 'open');
  assert.ok(w.events.some((e) => e.type === 'tip') && w.money > money, 'Trinkgeld');
  for (let i = 0; i < 60 * 25; i++) drive({ throttle: 1 });
  assert.equal(t.drive.doors, 'closed', 'schließen nach 20 s selbst');
});

test('Zug voraus: der Spielerzug hält davor und fährt nie hinein; Fahrplan-Zug dahinter wartet', () => {
  const p = pat('U1', 'ubahn'), w = atFrontOf(p, 2);
  press(w, { enterExit: true });
  const t = w.playerTrain, s = w.transit.tracked.get(p.id);
  // Fahrplan-Zug 500 px voraus (Spitze), auf demselben Muster
  let tau = 0; while (positionAt(p, tau).s < t.s + 500 + 6 * 165) tau += 0.2;
  const ahead = { tau, delay: 0, key: 'ahead' }; s.veh.push(ahead);
  const ahead0 = positionAt(p, tau).s;
  assert.ok(trainAhead(w, t) < 700);
  for (let i = 0; i < 60 * 30; i++) { ahead.tau = tau; press(w, { throttle: 1 }); }
  const tail = ahead0 - 6 * 165;
  assert.ok(t.s < tail - 50, `Abstand gehalten (${(tail - t.s).toFixed(0)} px)`);
  // Fahrplan-Zug hinter dem Spielerzug holt nicht auf
  let tb = 0; while (positionAt(p, tb).s < t.s - 6 * 165 - 700) tb += 0.2;
  const behind = { tau: tb, delay: 0, key: 'behind' }; s.veh.push(behind);
  for (let i = 0; i < 60 * 60; i++) press(w);
  assert.ok(positionAt(p, behind.tau).s < t.s - 6 * 165 - 400, 'wartet hinter dem Spielerzug');
});

test('Linienende: Zug hält, Wenden auf die Gegenrichtung, sonst nur aussteigen', () => {
  const p = pat('M10', 'tram'), n = p.stops.length, w = atFrontOf(p, n - 2);
  press(w, { enterExit: true });
  const t = w.playerTrain;
  for (let i = 0; i < 60 * 180; i++) press(w, { throttle: 1 });
  assert.ok(t.s <= p.stops[n - 1] + 1 && t.v === 0, 'steht am Endhalt');
  press(w, { action: true }); assert.equal(t.drive.doors, 'open', 'erst Türen');
  press(w, { action: true }); assert.equal(t.drive.doors, 'closed');
  const ok = turnAround(w);
  const back = tr.patterns.find((q) => q.name === p.name && q.id !== p.id && Math.hypot(pointOn(q, q.stops[0]).x - pointOn(p, p.stops[n - 1]).x, pointOn(q, q.stops[0]).y - pointOn(p, p.stops[n - 1]).y) < 600);
  assert.equal(ok, !!back);
  if (back) { assert.equal(w.playerTrain.pid, back.id); assert.ok(Math.abs(w.playerTrain.s - back.stops[0]) < 5); }
});
test('Eigene Straßenbahn hält vor einem Auto auf dem Gleis', async () => {
  const { createCar } = await import('../web/src/car.js');
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  const t = w.playerTrain, q = pointOn(p, t.s + 900);
  const car = createCar({ x: q.x, y: q.y, angle: q.angle }); car.driver = null; w.cars.push(car);
  for (let i = 0; i < 60 * 30; i++) press(w, { throttle: 1 });
  const head = pointOn(p, t.s);
  assert.ok(Math.hypot(head.x - car.x, head.y - car.y) > 30, `Spitze ${Math.hypot(head.x - car.x, head.y - car.y).toFixed(0)} px vor dem Auto`);
  assert.equal(t.v, 0);
});

test('Wenden über die Taste am Endhalt der M10 (Endhalt steht im Fahrplan doppelt)', () => {
  const p = pat('M10', 'tram'), n = p.stops.length, w = atFrontOf(p, n - 2);
  w.trafficScale = 0;
  const drive = (patch) => { w.cars.length = 0; press(w, patch); };
  drive({ enterExit: true });
  const t = w.playerTrain;
  for (let i = 0; i < 60 * 180 && !(t.v === 0 && t.s >= p.stops[n - 1] - 30); i++) drive({ throttle: 1 });
  assert.ok(t.v === 0 && Math.abs(t.s - p.stops[n - 1]) < 30, 'steht am Endhalt');
  drive({ action: true }); assert.equal(t.drive.doors, 'open');
  drive({ action: true }); assert.equal(t.drive.doors, 'closed');
  const pid = t.pid;
  drive({ action: true });
  assert.notEqual(w.playerTrain.pid, pid, 'E/A am Endhalt wendet');
});

test('Auftrag neu starten während der Fahrt als Zugführer: zu Fuß am Start, nicht mehr im Zug', async () => {
  const { restartMission } = await import('../web/src/world.js');
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  assert.equal(w.player.ride?.kind, 'driver');
  restartMission(w);
  press(w);
  assert.equal(w.player.ride, null);
  const sp = w.city.places.playerSpawn;
  assert.ok(Math.hypot(w.player.x - sp.x, w.player.y - sp.y) < 60, 'am Startpunkt');
});

test('Den eigenen, stehengelassenen Zug wieder übernehmen', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  const t = w.playerTrain;
  press(w, { enterExit: true });
  assert.equal(w.player.ride, null, 'ausgestiegen');
  const f = trainCars(p, t.s)[0];
  w.player.x = f.x + Math.cos(f.angle) * (f.L / 2 + 10); w.player.y = f.y + Math.sin(f.angle) * (f.L / 2 + 10);
  press(w, { enterExit: true });
  assert.equal(w.player.ride?.kind, 'driver', 'wieder am Führerstand');
  assert.equal(w.playerTrain, t, 'derselbe Zug');
});

test('Zugführer auf der Hochbahn: aussteigen nur am Bahnhof, nicht zwischen zwei Bahnhöfen auf dem Viadukt', async () => {
  const { railAt } = await import('../web/src/tunnel.js');
  const p = pat('U1', 'ubahn'), i = p.stopNames.findIndex((n) => n.includes('Kottbusser Tor')), w = atFrontOf(p, i);
  press(w, { enterExit: true });
  const t = w.playerTrain;
  assert.ok(t, 'übernommen');
  while (t.s < p.stops[i] + 700) press(w, { throttle: 1 });
  for (let k = 0; k < 60 * 20 && t.v > 0; k++) press(w, { brake: 1 });
  assert.equal(t.v, 0); assert.equal(t.atStop, null, 'zwischen zwei Bahnhöfen');
  const f = trainCars(p, t.s)[0];
  assert.ok((railAt(city, f.x, f.y)?.lvl ?? 0) >= 1, 'auf dem Viadukt');
  press(w, { enterExit: true });
  assert.equal(w.player.ride?.kind, 'driver', 'bleibt im Führerstand');
  assert.match(w.notice?.text ?? '', /Bahnhof/);
});

test('Haftung des eigenen Zugs: oberirdisch nass/Frost geringer, im Tunnel immer 1', async () => {
  const { trainAdhesion } = await import('../web/src/playertrain.js');
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  w.wet = 0; w.ice = 0; assert.equal(trainAdhesion(w, w.playerTrain), 1);
  w.wet = 1; assert.equal(trainAdhesion(w, w.playerTrain), 0.75);
  w.ice = 1; assert.equal(trainAdhesion(w, w.playerTrain), 0.6);
  const u8 = tr.patterns.filter((q) => q.name === 'U8' && q.mode === 'ubahn').sort((a, b) => b.stops.length - a.stops.length)[0];
  const i = u8.stopNames.findIndex((n) => n.includes('Kottbusser Tor'));
  assert.equal(trainAdhesion(w, { pid: u8.id, s: u8.stops[i] }), 1, 'unter Tage trocken');
});

test('Eigener Zug auf nassen Schienen: längerer Bremsweg in der Welt', () => {
  const brake = (wet) => {
    const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
    w.trafficScale = 0; w.wet = wet; w.forceWeather = wet ? 'rain' : 'clear';
    const drive = (patch) => { w.cars.length = 0; w.wet = wet; press(w, patch); };
    drive({ enterExit: true });
    const t = w.playerTrain;
    while (t.v < 80) drive({ throttle: 1 });
    const s0 = t.s;
    for (let i = 0; i < 60 * 30 && t.v > 0; i++) drive({ brake: 1 });
    return t.s - s0;
  };
  const dry = brake(0), wet = brake(1);
  assert.ok(wet > dry * 1.2, `nass ${wet.toFixed(0)} px, trocken ${dry.toFixed(0)} px`);
});

test('Straßenbahn auf einer Straßenbrücke: die Zugspitze liegt auf der Ebene der Brücke, nicht darunter; nass bleibt nass', async () => {
  const { trainAdhesion } = await import('../web/src/playertrain.js');
  const { spotLevel } = await import('../web/src/traction.js');
  const { nearestEdge } = await import('../web/src/map.js');
  const w = atFrontOf(pat('M10', 'tram'), 3);
  let p = null, s = null, lvl = 0;
  for (const q of tr.patterns) {
    if (q.name !== 'M10' || q.mode !== 'tram') continue;
    for (let x = 0; x < q.shape.len && s === null; x += 20) { const c = pointOn(q, x), e = nearestEdge(city, c.x, c.y, 25); if (e && e.e.bridge && (e.e.lvl ?? 0) >= 1 && e.d < 6) { p = q; s = x; lvl = e.e.lvl; } }
    if (s !== null) break;
  }
  assert.ok(s !== null, 'M10 fährt über eine Straßenbrücke');
  const h = pointOn(p, s);
  assert.equal(spotLevel(w, 'tram', h.x, h.y), lvl, 'Ebene der Brücke');
  w.wet = 1; w.ice = 0;
  assert.equal(trainAdhesion(w, { pid: p.id, s }), 0.75, 'nass auf der Brücke');
});

test('Wenden am Endhalt öffnet nicht zugleich die Türen am neuen ersten Halt', () => {
  const p = pat('M1', 'tram'), n = p.stops.length, w = atFrontOf(p, n - 2);
  w.trafficScale = 0;
  const drive = (patch) => { w.cars.length = 0; press(w, patch); };
  drive({ enterExit: true });
  const t = w.playerTrain;
  for (let i = 0; i < 60 * 180 && !(t.v === 0 && t.s >= p.stops[n - 1] - 30); i++) drive({ throttle: 1 });
  drive({ action: true }); drive({ action: true });
  const pid = t.pid;
  drive({ action: true });
  assert.notEqual(w.playerTrain.pid, pid, 'gewendet');
  assert.equal(w.playerTrain.drive.doors, 'closed', 'Türen bleiben zu');
});

test('Türen am selben Halt erneut öffnen: nur das erste Öffnen ist ein bedienter Halt', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  const opens = [];
  for (let k = 0; k < 4; k++) { press(w, { action: true }); opens.push(...w.events.filter((e) => e.type === 'doors-open')); }
  assert.equal(opens.length, 2, 'zweimal geöffnet');
  assert.deepEqual(opens.map((e) => e.first), [true, false]);
});

test('Einen zweiten Zug übernehmen: der erste fährt als Fahrplanzug weiter, statt zu verschwinden', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  const first = w.playerTrain, s0 = first.s;
  press(w, { enterExit: true }); // aussteigen
  // zweites Fahrzeug derselben Linie am selben Halt, Spieler an dessen Führerstand
  let tau = 0; while (!(positionAt(p, tau).stop === 3 && positionAt(p, tau).dwelling)) tau += 0.5;
  w.transit.tracked.get(p.id).veh.push({ tau, delay: 0, key: 'zwei' });
  const f = vehicleState(w, { pid: p.id, key: 'zwei' }).cars[0];
  // das zweite Fahrzeug steht an derselben Stelle wie der erste Zug: den ersten 1 km zurücksetzen, damit sie sich nicht decken
  first.s = s0 - 1000;
  w.player.x = f.x + Math.cos(f.angle) * (f.L / 2 + 10); w.player.y = f.y + Math.sin(f.angle) * (f.L / 2 + 10);
  press(w, { enterExit: true });
  assert.notEqual(w.playerTrain, first, 'neuer Zug');
  const back = w.transit.tracked.get(p.id).veh.find((v) => !v.gone && Math.abs(positionAt(p, v.tau).s - (s0 - 1000)) < 60);
  assert.ok(back, 'der erste Zug steht als Fahrplanzug an seiner Stelle');
});

test('Straßenbahn hat Vorrang: mit echtem Verkehr (Gegenspur an der M10) kommt der eigene Zug in 120 s zum nächsten Halt', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  const t = w.playerTrain, target = p.stops[4];
  for (let i = 0; i < 60 * 120 && !(t.v === 0 && Math.abs(t.s - target) < 60); i++) {
    const rest = target - t.s, want = Math.sqrt(Math.max(0, 2 * 10 * Math.max(0, rest - 10)));
    press(w, t.v > want ? { brake: Math.min(1, (t.v - want) / 20 + 0.3) } : { throttle: rest > 20 ? 0.6 : 0 });
  }
  assert.ok(Math.abs(t.s - target) < 60, `steht am Halt (${(t.s - target).toFixed(0)} px)`);
});

test('Straßenbahn hat Vorrang: eine Fahrplanbahn kommt an derselben Stelle durch', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3), s = w.transit.tracked.get(p.id);
  const v = s.veh.find((x) => x.key === 'mine');
  // Spieler aus dem Weg (atFrontOf stellt ihn vor die Spitze – dort wartet die Bahn zu Recht); Kamera fährt mit der Bahn
  const at = pointOn(p, positionAt(p, v.tau).s); w.player.x = at.x - Math.sin(at.angle) * 150; w.player.y = at.y + Math.cos(at.angle) * 150;
  // 150 s: durchkommen statt ewig hängen; Passanten queren mit realistischen 1,3 m/s und halten die Bahn länger auf
  for (let i = 0; i < 60 * 150 && positionAt(p, v.tau).s < p.stops[4]; i++) { const h = pointOn(p, positionAt(p, v.tau).s); updateWorld(w, idle(), 1 / 60); w.camera.x = h.x; w.camera.y = h.y; }
  // Diagnose für den Fehlerfall: was steht vor dem Kopf der Bahn?
  const head = positionAt(p, v.tau).s, ha = pointOn(p, head).angle, what = [];
  for (const d of [20, 45, 75]) { const q = pointOn(p, head + d);
    for (const c of w.cars) if (Math.hypot(c.x - q.x, c.y - q.y) < 24 + c.hw * 0.4) what.push(`${d}:Auto ${c.kind}/${c.driver} cos ${Math.cos(c.angle - ha).toFixed(2)} rev ${c.ai?.reverseT?.toFixed(2)}`);
    for (const e of w.peds) if (e.state !== 'dead' && e.state !== 'hang' && Math.hypot(e.x - q.x, e.y - q.y) < 16) what.push(`${d}:Passant ${e.state}`);
    for (const b of w.bikes ?? []) if (b.state === 'ride' && Math.hypot(b.x - q.x, b.y - q.y) < 18) what.push(`${d}:Rad`); }
  assert.ok(head >= p.stops[4] - 5, `Fahrplanbahn bei ${(head - p.stops[4]).toFixed(0)} px vor dem Halt; davor: ${what.join(' | ') || 'nichts'}`);
});

test('Eigene Straßenbahn vor einem entgegenkommenden Auto, das nicht zurück kann: nach der Wartezeit fährt sie an', async () => {
  const { createCar } = await import('../web/src/car.js');
  const { TRAM_PATIENCE } = await import('../web/src/transitlive.js');
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  w.trafficScale = 0;
  press(w, { enterExit: true });
  const t = w.playerTrain, q = pointOn(p, t.s + 250);
  // Auto frontal auf dem Gleis, festgesetzt (Handbremse, kein Fahrer = kann nicht ausweichen)
  const car = createCar({ x: q.x, y: q.y, angle: q.angle + Math.PI }); car.driver = null; car.role = 'stuck'; w.cars = [car];
  const keep = (patch) => { w.cars = w.cars.filter((c) => c === car); press(w, patch); };
  for (let i = 0; i < 60 * 8; i++) keep({ throttle: 1 });
  assert.equal(t.v, 0, 'wartet zuerst');
  const s0 = t.s;
  for (let i = 0; i < 60 * (TRAM_PATIENCE + 6); i++) keep({ throttle: 1 });
  assert.ok(t.s > s0 + 30, `fährt nach der Wartezeit an (${(t.s - s0).toFixed(0)} px)`);
});
