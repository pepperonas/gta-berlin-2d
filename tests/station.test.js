// Begehbare U-Bahnhöfe (station.js): aus dem Fahrplan erzeugt, Eingänge an der Straße, Bahnsteig mit Treppen, Einsteigen
// am Bahnsteig, Aussteigen auf den Bahnsteig des nächsten Bahnhofs, Spielstand, Trennung von oben und unten.
import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { stationsNear, stationById, toLocal, toWorld, STATION, stationName, trainsAt, boardable, departures, entranceNear } from '../web/src/station.js';
import { pointOn, positionAt } from '../web/src/transit.js';
import { inBuilding } from '../web/src/map.js';
import { pickTarget } from '../web/src/combat.js';
import { ambienceAt } from '../web/src/ambience.js';
import { makeSave } from '../web/src/save.js';

const city = realCity();
const u8 = city.transit.patterns.find((q) => q.name === 'U8' && q.stopNames.some((n) => n.includes('Kottbusser')) && q.stopNames.some((n) => n.includes('Hermannplatz')));
const iKotti = u8.stopNames.findIndex((n) => n.includes('Kottbusser'));
const kotti = pointOn(u8, u8.stops[iKotti]);
const station = () => stationsNear(city, kotti.x, kotti.y, 800).find((s) => s.key === 'kottbusser tor');

function world(live = false) {
  const w = live ? createWorld({ city }) : createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle'; w.clock = 12 * 60;
  const st = station(), ex = st.exits[0];
  w.player.x = ex.x + 30; w.player.y = ex.y; w.player.inCar = null; w.player.lvl = 0;
  w.camera.x = ex.x; w.camera.y = ex.y;
  return { w, st };
}
const walk = (w, dx, dy, secs) => { for (let i = 0; i < secs * 60; i++) updateWorld(w, { ...idle(), moveX: dx, moveY: dy }, 1 / 60); };
const toward = (w, x, y, secs, stop = () => false) => {
  for (let i = 0; i < secs * 60 && !stop(); i++) { const d = Math.hypot(x - w.player.x, y - w.player.y) || 1; updateWorld(w, { ...idle(), moveX: (x - w.player.x) / d, moveY: (y - w.player.y) / d }, 1 / 60); }
};
// Seit b4d1fc3f öffnet Hineinlaufen keinen Bahnhof mehr: zum Eingang gehen, dann F (enterExit)
const enterAt = (w, ex, secs) => {
  toward(w, ex.x, ex.y, secs, () => Math.hypot(ex.x - w.player.x, ex.y - w.player.y) < STATION.reach * 0.5);
  for (let i = 0; i < 40; i++) updateWorld(w, idle(), 1 / 60); // Eingangsliste (alle 0,5 s) auffrischen
  updateWorld(w, { ...idle(), enterExit: true }, 1 / 60);
};

test('Bahnhöfe aus dem Fahrplan: Kottbusser Tor (U8) unter der Erde, die Hochbahn U1/U3 dort nicht; Eingänge am Gehweg', () => {
  assert.equal(stationName('S+U Alexanderplatz Bhf (Berlin)'), 'Alexanderplatz');
  assert.equal(stationName('U Kottbusser Tor (Berlin)'), 'Kottbusser Tor');
  const list = stationsNear(city, kotti.x, kotti.y, 800).filter((s) => s.key === 'kottbusser tor');
  assert.equal(list.length, 1, 'ein unterirdischer Bahnsteig');
  const st = list[0];
  assert.deepEqual(st.lines, ['U8']);
  assert.ok(st.HL * 2 >= 990, `Bahnsteig so lang wie ein Zug (${Math.round(st.HL * 2 / 10)} m)`);
  assert.ok(st.halts.some((h) => h.dir === 1) && st.halts.some((h) => h.dir === -1), 'beide Richtungen');
  for (const ex of st.exits) {
    assert.ok(!inBuilding(city, ex.x, ex.y), 'Eingang nicht im Haus');
    assert.ok(Math.hypot(ex.x - st.x, ex.y - st.y) < st.HL + 400, 'Eingang über dem Bahnsteig');
  }
  assert.equal(stationById(city, st.id), st);
  // Hochbahnhöfe (U1 Görlitzer Bahnhof) sind nicht begehbar
  const u1 = city.transit.patterns.find((q) => q.name === 'U1' && q.stopNames.some((n) => n.includes('Görlitzer')));
  const g = pointOn(u1, u1.stops[u1.stopNames.findIndex((n) => n.includes('Görlitzer'))]);
  assert.ok(!stationsNear(city, g.x, g.y, 300).some((s) => s.key === 'gorlitzer bahnhof'));
});

test('Hinein über den Eingang, unten nur auf dem Bahnsteig (Kanten, Säulen), hinaus über die Treppe, nicht gleich wieder hinein', () => {
  const { w, st } = world();
  const ex = st.exits[0];
  enterAt(w, ex, 3);
  assert.ok(w.player.inside, 'unten');
  assert.equal(w.player.inside.id, st.id);
  assert.equal(w.player.lvl, -2);
  // quer zum Gleis laufen: bleibt auf dem Bahnsteig
  walk(w, -st.ay, st.ax, 3);
  let l = toLocal(st, w.player.x, w.player.y);
  assert.ok(Math.abs(l.v) <= STATION.half, `an der Kante gehalten (v=${l.v.toFixed(1)})`);
  walk(w, st.ay, -st.ax, 5);
  l = toLocal(st, w.player.x, w.player.y);
  assert.ok(Math.abs(l.v) <= STATION.half);
  // zur anderen Treppe (Ende +1) laufen → oben am anderen Eingang
  const far = toWorld(st, st.HL, 0);
  toward(w, far.x, far.y, 60, () => !w.player.inside);
  assert.equal(w.player.inside, null, 'oben');
  const ex1 = st.exits[1];
  assert.ok(Math.hypot(w.player.x - ex1.x, w.player.y - ex1.y) < 2, 'am Ausgang der anderen Treppe');
  // stehen bleiben: nicht von selbst wieder hinunter
  for (let i = 0; i < 30; i++) updateWorld(w, idle(), 1 / 60);
  assert.equal(w.player.inside, null);
  // weg und wieder hin, F: hinunter
  walk(w, 1, 0, 1.5);
  enterAt(w, ex1, 4);
  assert.ok(w.player.inside, 'wieder unten');
});

test('Einsteigen am Bahnsteig in einen haltenden Zug, aussteigen am nächsten U-Bahnhof auf dessen Bahnsteig', () => {
  const { w, st } = world(true); // mit Stadtleben: dann läuft der Fahrplan
  enterAt(w, st.exits[0], 3);
  assert.ok(w.player.inside);
  for (let i = 0; i < 90; i++) updateWorld(w, idle(), 1 / 60); // Fahrplan verfolgt die Muster um die Kamera
  // einen Zug Richtung Hermannplatz in den Bahnhof stellen (Fahrzeit = kurz vor der Abfahrt am Halt)
  const h = st.halts.find((x) => x.pid === u8.id) ?? st.halts[0], p = city.transit.patterns[h.pid];
  const s = w.transit.tracked.get(h.pid);
  assert.ok(s, 'Muster wird verfolgt');
  s.veh.push({ tau: p.off[h.i] - 3, delay: 0, key: 'test' });
  const tr = trainsAt(w, st).find((x) => x.ref.key === 'test');
  assert.ok(tr && tr.dwelling, 'hält am Bahnsteig');
  assert.ok(departures(w, st).length > 0, 'Fahrgastinfo');
  // an die Bahnsteigkante neben den zweiten Wagen
  const c = tr.cars[1], at = toWorld(st, c.u, tr.dir * (STATION.half - 8));
  w.player.x = at.x; w.player.y = at.y;
  assert.ok(boardable(w, st, at.x, at.y), 'einsteigbar');
  // auf der Mitte des Bahnsteigs nicht
  const mid = toWorld(st, c.u, 0);
  assert.equal(boardable(w, st, mid.x, mid.y), null);
  updateWorld(w, { ...idle(), ride: true }, 1 / 60);
  assert.ok(w.player.ride && w.player.ride.ref.key === 'test', 'fährt mit');
  assert.equal(w.player.inside, null);
  // mitfahren bis zum nächsten Halt, dort aussteigen
  let t = 0;
  const next = h.i + 1;
  while (t < 400) {
    updateWorld(w, idle(), 1 / 60); t += 1 / 60;
    const pos = positionAt(p, s.veh.find((v) => v.key === 'test')?.tau ?? 0);
    if (pos.dwelling && pos.stop === next) break;
  }
  updateWorld(w, { ...idle(), ride: true }, 1 / 60);
  assert.ok(w.player.inside, 'unten auf dem Bahnsteig ausgestiegen');
  const st2 = stationById(city, w.player.inside.id);
  assert.equal(st2.key, stationName(p.stopNames[next]).toLowerCase(), `am nächsten Bahnhof (${st2.name})`);
  const l = toLocal(st2, w.player.x, w.player.y);
  assert.ok(Math.abs(l.v) <= STATION.half && Math.abs(l.u) <= st2.HL, 'auf dem Bahnsteig');
});

test('Unten und oben getrennt: keine Ziele von der Straße, gedämpfter Klang, Spielstand oben am Ausgang', () => {
  const { w, st } = world();
  enterAt(w, st.exits[0], 3);
  const p = w.player;
  // ein Passant „über“ der Figur auf der Straße ist kein Ziel
  w.peds.push({ id: 999999, x: p.x, y: p.y, state: 'walk', lvl: 0, hp: 100 });
  assert.equal(pickTarget(w, p.x, p.y), null);
  w.peds.pop();
  const m = ambienceAt(w);
  assert.equal(m.station, true); assert.equal(m.traffic, 0); assert.ok(m.muffle > 0.8);
  const s = makeSave(w);
  assert.ok(st.exits.some((ex) => Math.hypot(ex.x - s.player.x, ex.y - s.player.y) < 2), 'Spielstand am Ausgang oben');
});

test('Eingang dort, wo man ihn sucht: am U-Symbol; F in der Nähe führt hinunter; oberirdische S-Bahnhöfe nicht begehbar', () => {
  const pois = city.list('poi').filter((q) => q.cat === 'ubahn' || q.cat === 'sbahn');
  const norm = (n) => stationName(n).toLowerCase().replace(/stra(ß|ss)e\b/g, 'str').replace(/str\./g, 'str').replace(/[^a-zäöüß0-9]/g, '');
  let checked = 0;
  for (const q of pois) {
    const sts = stationsNear(city, q.x, q.y, 600).filter((st) => norm(st.name) === norm(q.name) && q.cat === (st.sbahn ? 'sbahn' : 'ubahn'));
    if (!sts.length || sts.some((st) => Math.hypot(st.x - q.x, st.y - q.y) > st.HL + 400)) continue;
    const d = Math.min(...sts.flatMap((st) => st.exits.map((ex) => Math.hypot(ex.x - q.x, ex.y - q.y))));
    assert.ok(d < 200, `${q.name}: nächster Eingang ${Math.round(d / 10)} m vom Symbol`);
    for (const st of sts) for (const ex of st.exits) assert.ok(!inBuilding(city, ex.x, ex.y), `${st.name}: Eingang im Haus`);
    checked++;
  }
  assert.ok(checked >= 15, `${checked} Bahnhöfe geprüft`);
  for (const key of ['ostkreuz', 'plänterwald']) {
    const q = pois.find((x) => stationName(x.name).toLowerCase() === key);
    if (q) assert.ok(!stationsNear(city, q.x, q.y, 800).some((st) => st.key === key), `${key} ist oberirdisch`);
  }
  // E (Aktion) in Reichweite eines Eingangs: hinunter; weiter weg nicht
  const { w, st } = world();
  const ex = st.exits[0];
  w.player.x = ex.x + STATION.reach * 0.7; w.player.y = ex.y;
  for (let i = 0; i < 40; i++) updateWorld(w, idle(), 1 / 60);
  assert.ok(entranceNear(w._stNear, w.player.x, w.player.y), 'Eingang in Reichweite (Hinweis)');
  assert.equal(w.player.inside ?? null, null, 'ohne Taste nicht');
  updateWorld(w, { ...idle(), enterExit: true }, 1 / 60);
  assert.ok(w.player.inside, 'mit F unten');
  const { w: w2, st: st2 } = world();
  w2.player.x = st2.exits[0].x + STATION.reach * 1.6; w2.player.y = st2.exits[0].y;
  for (let i = 0; i < 40; i++) updateWorld(w2, idle(), 1 / 60);
  updateWorld(w2, { ...idle(), enterExit: true }, 1 / 60);
  assert.equal(w2.player.inside ?? null, null, 'zu weit weg');
});

test('Fehlerfälle: Bahnhöfe unabhängig vom ersten Blick, Neustart/Teleport holen heraus, oben merkt niemand die Figur unten', async () => {
  const { restartMission, teleportTo } = await import('../web/src/world.js');
  const { obstacleAt } = await import('../web/src/transitlive.js');
  // Yorckstraße: zuerst von der S-Bahn her, dann bei der U7 – die U7 ist trotzdem da
  const u7 = city.transit.patterns.find((q) => q.name === 'U7' && q.stopNames.some((n) => stationName(n) === 'Yorckstr.'));
  const sb = city.transit.patterns.find((q) => q.mode === 'sbahn' && q.stopNames.some((n) => stationName(n).startsWith('Yorckstr')));
  const at = (p) => pointOn(p, p.stops[p.stopNames.findIndex((n) => stationName(n).startsWith('Yorckstr'))]);
  const fresh = () => { city._stations = null; };
  fresh(); const direct = stationsNear(city, at(u7).x, at(u7).y, 300).map((s) => s.id).sort();
  fresh(); if (sb) stationsNear(city, at(sb).x, at(sb).y, 100);
  assert.deepEqual(stationsNear(city, at(u7).x, at(u7).y, 300).map((s) => s.id).sort(), direct, 'gleiche Bahnsteige');
  assert.ok(direct.some((id) => id.includes('U7')), 'U7-Bahnsteig vorhanden');
  // Neustart im Bahnhof: oben am Start, nicht zurück auf den Bahnsteig
  const { w, st } = world();
  enterAt(w, st.exits[0], 3);
  assert.ok(w.player.inside);
  // unten: keine Straßenbahn hält für die Figur, Schüsse rufen oben keine Polizei
  assert.equal(obstacleAt(w, w.player.x, w.player.y), false, 'Straßenbahn oben hält nicht');
  restartMission(w);
  for (let i = 0; i < 30; i++) updateWorld(w, idle(), 1 / 60);
  assert.equal(w.player.inside, null); assert.notEqual(w.player.lvl, -2);
  assert.ok(Math.hypot(w.player.x - city.places.playerSpawn.x, w.player.y - city.places.playerSpawn.y) < 80, 'am Start');
  // Teleport genau auf einen Eingang: nicht sofort hinunter (erst F)
  teleportTo(w, { x: st.exits[0].x, y: st.exits[0].y, angle: 0 });
  for (let i = 0; i < 30; i++) updateWorld(w, idle(), 1 / 60);
  assert.equal(w.player.inside, null, 'bleibt oben');
  updateWorld(w, { ...idle(), enterExit: true }, 1 / 60);
  assert.ok(w.player.inside, 'F führt hinunter');
});
