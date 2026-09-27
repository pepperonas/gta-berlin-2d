import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { createPed, nearestSpot } from '../web/src/pedestrians.js';
import { createCar } from '../web/src/car.js';
import { WEAPONS, KICK, castRay, aimAssist, BODY_KEEP, GUNSHOT_SCARE, rayObb, raySegment, rayCircle } from '../web/src/combat.js';
import { readPad, readKeys, InputState, merge } from '../web/src/input.js';

const city = realCity();
const slot = (id) => WEAPONS.findIndex((w) => w.id === id) + 1;

// Welt ohne Verkehr, Spieler zu Fuß; Richtung mit freier Sicht über 250 px suchen
function arena() {
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.mission.state = 'idle';
  const p = w.player;
  let ang = 0;
  for (let k = 0; k < 64; k++) { const a = k / 64 * Math.PI * 2; const r = castRay(w, p.x, p.y, a, 260); if (!r.hit) { ang = a; break; } }
  return { w, p, ang };
}
function pedAt(w, x, y) {
  const ped = createPed(city, nearestSpot(city, x, y), w.rng);
  Object.assign(ped, { x, y, state: 'idle', t: 1e6 });
  w.peds.push(ped);
  return ped;
}
const step = (w, inp = {}, n = 1) => { for (let i = 0; i < n; i++) updateWorld(w, { ...idle(), ...inp }, 1 / 60); };
const aimAt = (p, x, y) => ({ aimWorld: { x, y } });
// Waffe wählen und die Wechselpause (0,15 s) abwarten
const equip = (w, id) => { step(w, { weaponSlot: slot(id) }); step(w, {}, 15); };

test('Nahkampf: Faust trifft, wer vorn steht, nicht wer hinten steht; Tritt stößt härter', () => {
  const { w, p, ang } = arena();
  const front = pedAt(w, p.x + Math.cos(ang) * 16, p.y + Math.sin(ang) * 16);
  const back = pedAt(w, p.x - Math.cos(ang) * 16, p.y - Math.sin(ang) * 16);
  step(w, { ...aimAt(p, front.x, front.y), fire: true, firePressed: true });
  assert.equal(front.hp, 100 - WEAPONS[0].dmg, 'vorn getroffen');
  assert.equal(back.hp, undefined, 'hinten nicht');
  assert.equal(front.state, 'down');
  const hp = front.hp;
  step(w, {}, 40);
  step(w, { ...aimAt(p, front.x, front.y), kick: true });
  assert.ok(front.hp <= hp - KICK.dmg || front.state === 'dead', 'Tritt trifft');
});

test('Pistole: drei Treffer töten, Tote bleiben liegen und verschwinden erst später außer Sicht', () => {
  const { w, p, ang } = arena();
  const ped = pedAt(w, p.x + Math.cos(ang) * 150, p.y + Math.sin(ang) * 150);
  equip(w, 'pistol');
  const kills = [];
  for (let i = 0; i < 3; i++) {
    step(w, { ...aimAt(p, ped.x, ped.y), fire: true, firePressed: true });
    kills.push(...w.events.filter((e) => e.type === 'kill'));
    step(w, { ...aimAt(p, ped.x, ped.y) }, 20);
  }
  assert.equal(ped.state, 'dead', `nach drei Schüssen tot (hp ${ped.hp})`);
  assert.equal(kills.length, 1);
  assert.equal(p.mag[slot('pistol') - 1], WEAPONS[slot('pistol') - 1].mag - 3);
  step(w, {}, 60 * 5);
  assert.ok(w.peds.includes(ped), 'liegt noch da');
  ped.deadT = BODY_KEEP + 1; w.camera.x += 5000; w.camera.y += 5000;
  step(w);
  assert.ok(!w.peds.includes(ped), 'außer Sicht nach der Frist weg');
});

test('Kugeln stoppen an Hauswänden (Einschlag an der Wand, dahinter kein Treffer)', () => {
  const { w, p } = arena();
  // Richtung, in der eine Hauswand innerhalb von 200 px liegt
  let ang = null, wall = null;
  for (let k = 0; k < 128 && ang === null; k++) {
    const a = k / 128 * Math.PI * 2, r = castRay(w, p.x, p.y, a, 200);
    if (r.hit?.type === 'wall' && r.t > 30) { ang = a; wall = r; }
  }
  assert.ok(ang !== null, 'Wand in der Nähe gefunden');
  const behind = pedAt(w, p.x + Math.cos(ang) * (wall.t + 30), p.y + Math.sin(ang) * (wall.t + 30));
  equip(w, 'pistol');
  step(w, { aimWorld: { x: behind.x, y: behind.y }, fire: true, firePressed: true });
  assert.equal(behind.hp, undefined, 'hinter der Wand unverletzt');
  assert.ok(w.events.some((e) => e.type === 'impact'), 'Einschlag an der Wand');
});

test('Schrotflinte fächert 8 Kugeln; MP schießt Dauerfeuer und lädt nach leerem Magazin nach', () => {
  const { w, p, ang } = arena();
  const tx = p.x + Math.cos(ang) * 200, ty = p.y + Math.sin(ang) * 200;
  equip(w, 'shotgun');
  step(w, { aimWorld: { x: tx, y: ty }, fire: true, firePressed: true });
  const shot = w.events.find((e) => e.type === 'shot');
  assert.equal(shot.traces.length, 8);
  const angs = shot.traces.map(([x, y]) => Math.atan2(y - p.y, x - p.x));
  assert.ok(Math.max(...angs) - Math.min(...angs) > 0.2, 'Fächer');
  step(w, {}, 60);
  equip(w, 'smg');
  step(w, {}, 20);
  let shots = 0, reloads = 0;
  for (let i = 0; i < 60 * 4; i++) {
    step(w, { aimWorld: { x: tx, y: ty }, fire: true });
    shots += w.events.filter((e) => e.type === 'shot').length;
    reloads += w.events.filter((e) => e.type === 'reload').length;
  }
  const smg = WEAPONS[slot('smg') - 1];
  assert.ok(reloads >= 1, 'nachgeladen');
  assert.ok(shots >= smg.mag && shots <= smg.mag * 3, `${shots} Schüsse in 4 s`);
  // Pistole: Halten feuert nicht automatisch weiter
  equip(w, 'pistol');
  let ps = 0;
  step(w, { aimWorld: { x: tx, y: ty }, fire: true, firePressed: true }); ps += w.events.filter((e) => e.type === 'shot').length;
  for (let i = 0; i < 60; i++) { step(w, { aimWorld: { x: tx, y: ty }, fire: true }); ps += w.events.filter((e) => e.type === 'shot').length; }
  assert.equal(ps, 1, 'Einzelfeuer');
});

test('Zielhilfe: rastet auf das nächste Ziel im Kegel ein, nicht daneben und nicht hinter Wänden', () => {
  const { w, p, ang } = arena();
  const near = pedAt(w, p.x + Math.cos(ang + 0.15) * 120, p.y + Math.sin(ang + 0.15) * 120);
  pedAt(w, p.x + Math.cos(ang + 1.2) * 60, p.y + Math.sin(ang + 1.2) * 60); // außerhalb des Kegels
  const a = aimAssist(w, p, ang, 600);
  assert.equal(a.target, near);
  assert.ok(Math.abs(a.ang - (ang + 0.15)) < 0.02);
  const none = aimAssist(w, p, ang + Math.PI, 600);
  assert.equal(none.target, null);
});

test('Autos: Kugeln beschädigen bis zum Wrack, der Fahrer steigt aus und flieht', () => {
  const { w, p, ang } = arena();
  const car = createCar({ x: p.x + Math.cos(ang) * 120, y: p.y + Math.sin(ang) * 120, angle: ang + Math.PI / 2 });
  car.driver = 'npc'; car.ai = null;
  w.cars.push(car);
  const peds0 = w.peds.length;
  equip(w, 'pistol');
  step(w, { aimWorld: { x: car.x, y: car.y }, fire: true, firePressed: true });
  assert.ok(car.health < 100, 'beschädigt');
  assert.equal(car.driver, null, 'Fahrer ist raus');
  assert.ok(w.peds.length > peds0, 'als Passant');
  equip(w, 'smg');
  for (let i = 0; i < 60 * 6 && !car.wrecked; i++) step(w, { aimWorld: { x: car.x, y: car.y }, fire: true });
  assert.ok(car.wrecked, 'Wrack');
});

test('Schüsse erschrecken Passanten im Umkreis; im Auto wird nicht geschossen', () => {
  const { w, p, ang } = arena();
  const by = pedAt(w, p.x - Math.cos(ang) * (GUNSHOT_SCARE - 60), p.y - Math.sin(ang) * (GUNSHOT_SCARE - 60));
  equip(w, 'pistol');
  step(w, { aimWorld: { x: p.x + Math.cos(ang) * 100, y: p.y + Math.sin(ang) * 100 }, fire: true, firePressed: true });
  assert.equal(by.state, 'flee');
  const car = w.cars.find((c) => c.id === w.playerCarId);
  p.inCar = car.id; car.driver = 'player';
  const mag = p.mag[slot('pistol') - 1];
  step(w, {}, 30);
  step(w, { fire: true, firePressed: true, kick: true });
  assert.ok(!w.events.some((e) => e.type === 'shot' || e.type === 'swing'));
  assert.equal(p.mag[slot('pistol') - 1], mag);
});

test('Tote halten den Verkehr nicht auf', async () => {
  const { buildLaneGraph } = await import('../web/src/roadgraph.js');
  const { placeOnLane } = await import('../web/src/traffic.js');
  const g = buildLaneGraph(city);
  const lane = [...g.lanes].find((l) => l.len > 600 && !l.narrow && l.edge.cls <= 6);
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.mission.state = 'idle';
  const car = createCar({ x: 0, y: 0 }); car.driver = 'npc';
  placeOnLane(car, city, lane, 100, w.rng);
  w.cars.push(car);
  const ped = pedAt(w, car.x + Math.cos(car.angle) * 80, car.y + Math.sin(car.angle) * 80);
  ped.state = 'dead'; ped.deadT = 0;
  const x0 = car.x, y0 = car.y;
  for (let i = 0; i < 60 * 4; i++) { w.camera.x = car.x; w.camera.y = car.y; step(w); }
  assert.ok(Math.hypot(car.x - x0, car.y - y0) > 150, 'Auto fährt weiter');
});

test('Strahltests: Kreis, Strecke, gedrehtes Rechteck', () => {
  assert.equal(rayCircle(0, 0, 1, 0, 10, 0, 2), 8);
  assert.equal(rayCircle(0, 0, -1, 0, 10, 0, 2), Infinity, 'hinter dem Strahl');
  assert.ok(Math.abs(raySegment(0, 0, 1, 0, { ax: 5, ay: -1, bx: 5, by: 1 }) - 5) < 1e-9);
  assert.equal(raySegment(0, 0, 1, 0, { ax: 5, ay: 1, bx: 5, by: 3 }), Infinity);
  assert.ok(Math.abs(rayObb(0, 0, 1, 0, { x: 20, y: 0, angle: Math.PI / 2, hw: 21, hh: 10 }) - 10) < 1e-9, 'gedrehtes Auto');
});

test('Eingabe: RT feuert (nicht die W-Taste), B/V treten, LB/RB/Q wechseln, 1–6 wählen, rechter Stick zielt', () => {
  const pad = (over) => ({ connected: true, buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: !!over[i], value: over[i] ? 1 : 0 })), axes: over.axes ?? [0, 0, 0, 0] });
  const inp = new InputState();
  let f = inp.frame(merge(readKeys(new Set()), readPad(pad({ 7: 1 }))), 1 / 60);
  assert.ok(f.fire && f.firePressed);
  f = inp.frame(merge(readKeys(new Set(['KeyW'])), readPad(pad({}))), 1 / 60);
  assert.ok(!f.fire, 'W ist Laufen/Gas, nicht Feuern');
  f = inp.frame(merge(readKeys(new Set()), readPad(pad({ 1: 1 }))), 1 / 60); assert.ok(f.kick, 'B tritt');
  inp.frame(merge(readKeys(new Set()), readPad(pad({}))), 1 / 60); // loslassen
  f = inp.frame(merge(readKeys(new Set(['KeyV'])), readPad(pad({}))), 1 / 60); assert.ok(f.kick, 'V tritt');
  f = inp.frame(merge(readKeys(new Set()), readPad(pad({ 5: 1 }))), 1 / 60); assert.ok(f.weaponNext, 'RB');
  f = inp.frame(merge(readKeys(new Set()), readPad(pad({ 4: 1 }))), 1 / 60); assert.ok(f.weaponPrev, 'LB');
  f = inp.frame(merge(readKeys(new Set(['Digit5'])), readPad(pad({}))), 1 / 60); assert.equal(f.weaponSlot, 5);
  f = inp.frame(merge(readKeys(new Set(['Digit5'])), readPad(pad({}))), 1 / 60); assert.equal(f.weaponSlot, 0, 'nur beim Drücken');
  f = inp.frame(merge(readKeys(new Set(['ControlLeft', 'KeyR'])), readPad(pad({}))), 1 / 60); assert.ok(f.fire && f.reload);
  f = inp.frame(merge(readKeys(new Set()), readPad(pad({ axes: [0, 0, 0.9, 0] }))), 1 / 60); assert.ok(f.aimX > 0.8);
});
