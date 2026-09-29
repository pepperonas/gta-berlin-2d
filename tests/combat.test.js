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

// --- Stufe 2: Gegenwehr, Lebenspunkte, Krankenhaus ------------------------------------------------------------
import { isFighter, FIGHT, REGEN, RESPAWN_DELAY, HOSPITAL_FEE, PLAYER_HP, hurtPlayer } from '../web/src/combat.js';
import { nearestHospital } from '../web/src/world.js';

// Passant mit gewünschtem Charakter (wehrt sich / wehrt sich nicht)
function pedOfKind(w, x, y, fighter) {
  for (let i = 0; i < 200; i++) { const q = pedAt(w, x, y); if (isFighter(q) === fighter) return q; w.peds.pop(); }
  throw new Error('kein passender Passant');
}

test('Gegenwehr: wer sich wehrt, steht nach dem Schlag auf, kommt zurück und trifft; andere fliehen', () => {
  const { w, p, ang } = arena();
  const f = pedOfKind(w, p.x + Math.cos(ang) * 16, p.y + Math.sin(ang) * 16, true);
  step(w, { aimWorld: { x: f.x, y: f.y }, fire: true, firePressed: true });
  assert.equal(f.state, 'down');
  for (let i = 0; i < 60 * 4 && f.state !== 'fight'; i++) step(w);
  assert.equal(f.state, 'fight', 'schlägt zurück');
  const hp0 = p.hp;
  for (let i = 0; i < 60 * 3; i++) step(w);
  assert.ok(p.hp <= hp0 - FIGHT.dmg, `Spielfigur getroffen (${hp0} → ${p.hp})`);
  // wer sich nicht wehrt, flieht
  const { w: w2, p: p2, ang: a2 } = arena();
  const n = pedOfKind(w2, p2.x + Math.cos(a2) * 16, p2.y + Math.sin(a2) * 16, false);
  step(w2, { aimWorld: { x: n.x, y: n.y }, fire: true, firePressed: true });
  for (let i = 0; i < 60 * 4; i++) step(w2);
  // flieht statt zu kämpfen – danach geht er weiter (walk/cross/return; mit realistischem Tempo ist die Flucht nach 4 s vorbei)
  assert.ok(n.state !== 'fight' && n.state !== 'down', `flieht (${n.state})`);
  assert.equal(p2.hp, PLAYER_HP);
});

test('Gegenwehr: Kämpfer in der Nähe mischen bei einer Schlägerei mit; Lebenspunkte kommen nach einer Pause zurück', () => {
  const { w, p, ang } = arena();
  const victim = pedOfKind(w, p.x + Math.cos(ang) * 16, p.y + Math.sin(ang) * 16, false);
  const buddy = pedOfKind(w, p.x - Math.cos(ang) * 40, p.y - Math.sin(ang) * 40, true);
  step(w, { aimWorld: { x: victim.x, y: victim.y }, fire: true, firePressed: true });
  assert.equal(buddy.state, 'fight');
  hurtPlayer(w, 30, p.x + 10, p.y);
  const hp = p.hp;
  w.peds = w.peds.filter((q) => q !== buddy);
  step(w, {}, Math.round(60 * (REGEN.delay - 1)));
  assert.equal(Math.round(p.hp), Math.round(hp), 'erst warten');
  step(w, {}, 60 * 3);
  assert.ok(p.hp > hp + 5, 'dann heilt es');
});

test('K. o.: bei 0 Lebenspunkten Neustart am nächsten Krankenhaus, Auftrag gescheitert, 10 % Geld weg', () => {
  const { w, p } = arena();
  assert.ok(w.city.hospitals.length >= 40, 'Krankenhäuser im Index');
  w.money = 1000;
  w.mission.state = 'toPickup'; w.mission.timer = 500;
  const h = nearestHospital(w.city, p.x, p.y);
  hurtPlayer(w, 500, p.x + 5, p.y);
  assert.ok(p.dead && w.events.some((e) => e.type === 'wasted'));
  step(w, { moveX: 1, fire: true, firePressed: true }, 30);
  assert.ok(p.dead, 'liegt');
  for (let i = 0; i < 60 * (RESPAWN_DELAY + 5) && p.dead; i++) step(w);
  assert.ok(!p.dead, 'wieder auf den Beinen');
  assert.equal(p.hp, PLAYER_HP);
  assert.ok(Math.hypot(p.x - h.x, p.y - h.y) < 3000, `am Krankenhaus ${h.name} (${Math.round(Math.hypot(p.x - h.x, p.y - h.y))} px)`);
  assert.equal(w.money, 1000 - Math.floor(1000 * HOSPITAL_FEE));
  assert.equal(w.mission.state, 'failed');
  assert.match(w.mission.result.reason, /Krankenhaus/);
});

test('Anfahren verletzt die Spielfigur', () => {
  const { w, p, ang } = arena();
  const car = createCar({ x: p.x - Math.cos(ang) * 60, y: p.y - Math.sin(ang) * 60, angle: ang });
  car.vx = Math.cos(ang) * 250; car.vy = Math.sin(ang) * 250;
  w.cars.push(car);
  for (let i = 0; i < 30; i++) { car.vx = Math.cos(ang) * 250; car.vy = Math.sin(ang) * 250; step(w); }
  assert.ok(p.hp < PLAYER_HP, `Lebenspunkte ${p.hp}`);
});

test('Im Auto und am Boden keine weiteren Treffer (kein doppeltes K. o.)', () => {
  const { w, p } = arena();
  const car = w.cars.find((c) => c.id === w.playerCarId);
  p.inCar = car.id; car.driver = 'player';
  hurtPlayer(w, 50, p.x + 5, p.y);
  assert.equal(p.hp, PLAYER_HP, 'im Auto unverletzt');
  p.inCar = null; car.driver = null;
  w.events.length = 0;
  hurtPlayer(w, 500, p.x + 5, p.y);
  hurtPlayer(w, 500, p.x + 5, p.y);
  assert.equal(w.events.filter((e) => e.type === 'wasted').length, 1);
});
