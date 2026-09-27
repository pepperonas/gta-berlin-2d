// Tagesrhythmus (rhythm.js) und Stadtleben (life.js): Kurven, örtliche Belebung, Daten aus dem Build
// (Verkehrsmengen auf Kanten, Einwohnerdichte-Raster), Plätze mit Tätigkeiten und deren Verwaltung in der Welt.
import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';
import { createWorld, updateWorld, manageLife, resetPopulation } from '../web/src/world.js';
import { TRAFFIC } from '../web/src/config.js';
import { trafficLevel, peopleLevel, nightlife, populationTargets, localPeople, densityNear, dayName, isWeekend, START_DAY } from '../web/src/rhythm.js';
import { activityFor, lifeSpots, walkerStyle, parkLevel, LIFE } from '../web/src/life.js';
import { densityAt, inBuilding, onRoad } from '../web/src/map.js';
import { scare } from '../web/src/pedestrians.js';
import { assignTraffic, densityGrid } from '../tools/osm/build.mjs';
import { DTV_ESTIMATE, FURN_KIND, AREA_KIND } from '../web/src/citycodes.js';
import { pointInRings } from '../web/src/geom.js';
import { makeSave, validateSave, applySave } from '../web/src/save.js';

const city = realCity();
const giver = city.places.giver;
const FR = 4, SA = 5, SO = 6, MO = 0, DI = 1;

test('Rhythmus: Berufsverkehr, ruhige Nacht, Wochenende später und flacher', () => {
  assert.ok(trafficLevel(450, DI) > 0.95, 'Werktag 7:30 Spitze');
  assert.ok(trafficLevel(240, DI) < 0.2, 'Werktag 4:00 ruhig');
  assert.ok(trafficLevel(450, SO) < trafficLevel(450, DI) / 2, 'Sonntag früh kaum Verkehr');
  assert.ok(peopleLevel(900, SA) > peopleLevel(540, SA), 'Samstag nachmittags mehr los als morgens');
  assert.ok(isWeekend(SA) && isWeekend(SO) && !isWeekend(FR));
  assert.equal(dayName(START_DAY), 'Fr');
  assert.equal(dayName(7), 'Mo');
  // stetig: keine Sprünge über eine Woche in Minutenschritten (auch über Mitternacht)
  for (const f of [trafficLevel, peopleLevel, nightlife]) {
    let prev = f(0, MO), worst = 0;
    for (let t = 1; t < 7 * 1440; t++) { const v = f(t % 1440, Math.floor(t / 1440)); worst = Math.max(worst, Math.abs(v - prev)); prev = v; }
    assert.ok(worst < 0.02, `${f.name}: größter Sprung ${worst}`);
  }
});

test('Nachtleben: Freitag und Samstag voll, Nacht zählt bis 6 Uhr zum Vortag, tagsüber nichts', () => {
  assert.equal(nightlife(23 * 60, FR), 1);
  assert.equal(nightlife(2 * 60, SA), 1, 'Sa 2:00 gehört zur Freitagnacht');
  assert.equal(nightlife(2 * 60, SO), 1, 'So 2:00 gehört zur Samstagnacht');
  assert.ok(nightlife(2 * 60, MO) < 1, 'Mo 2:00 (Sonntagnacht) ruhiger');
  assert.ok(nightlife(23 * 60, DI) < 0.5, 'Dienstagnacht ruhig');
  assert.equal(nightlife(14 * 60, SA), 0);
  assert.equal(nightlife(7 * 60, SA), 0);
});

test('Zielbevölkerung am Späti: Nacht weniger Autos, Freitagnacht mehr Menschen als Dienstagnacht, Grenzen', () => {
  const base = { cars: TRAFFIC.cars, pedestrians: TRAFFIC.pedestrians };
  const rush = populationTargets(city, giver.x, giver.y, 450, DI, base);
  const night = populationTargets(city, giver.x, giver.y, 240, DI, base);
  assert.ok(rush.cars > night.cars * 3, `Berufsverkehr ${rush.cars} vs Nacht ${night.cars}`);
  const fr = populationTargets(city, giver.x, giver.y, 1350, FR, base), di = populationTargets(city, giver.x, giver.y, 1350, DI, base);
  assert.ok(fr.peds > di.peds * 1.3, `Fr ${fr.peds} vs Di ${di.peds}`);
  for (let m = 0; m < 1440; m += 30) for (let d = 0; d < 7; d++) {
    const t = populationTargets(city, giver.x, giver.y, m, d, base);
    assert.ok(t.cars >= 3 && t.cars <= Math.round(base.cars * 1.7) && t.peds >= 4 && t.peds <= Math.round(base.pedestrians * 1.6), `${m}/${d}`);
  }
  // Kreuzberg ist dichter bewohnt und belebter als eine Stelle ohne Wohnblöcke und Läden (Wasser des Landwehrkanals weit ab)
  assert.ok(densityNear(city, giver.x, giver.y) > 100, 'Wrangelkiez dicht bewohnt');
  assert.ok(localPeople(city, giver.x, giver.y) > localPeople(city, giver.x + 1e6, giver.y + 1e6));
});

test('Einwohnerdichte: bewohnte Zellen im Kiez, 0 für nicht geladene Kacheln', () => {
  let inhabited = 0;
  for (let dx = -600; dx <= 600; dx += 100) for (let dy = -600; dy <= 600; dy += 100) if (densityAt(city, giver.x + dx, giver.y + dy) > 0) inhabited++;
  assert.ok(inhabited > 40, `bewohnte Probepunkte ${inhabited}`);
  assert.equal(densityAt(city, -1e7, -1e7), 0);
});

test('Build: Verkehrsmengen landen auf der passenden Kante, nicht auf der Querstraße; Rest geschätzt', () => {
  const S = 10, toPx = (lat, lon) => [lon, lat]; // Testdaten schon in px (Paar [lon, lat] → [x, y])
  const vertices = [0, 0, 2000, 0, 1000, -1000, 1000, 1000, 5000, 5000, 5000, 6000];
  const edges = [
    { a: 0, b: 1, p: [], c: 2 }, // Hauptstraße West–Ost
    { a: 2, b: 3, p: [], c: 2 }, // Querstraße Nord–Süd, kreuzt die Hauptstraße
    { a: 4, b: 5, p: [], c: 5 }, // Nebenstraße weit weg
    { a: 0, b: 2, p: [], c: 9 }, // Weg: bekommt nie einen Wert
  ];
  // Zähllinie 6 m neben der Hauptstraße, gleiche Richtung; ein Abtastpunkt liegt genau auf der Querstraße (x = 1000),
  // die näher ist – nur die Richtungsprüfung hält die Zählung von ihr fern
  const verkehr = [[30000, [[0, 60, 2000, 60]]]];
  const r = assignTraffic(edges, vertices, verkehr, { toPx, S });
  assert.equal(edges[0].dtv, 30000); assert.equal(edges[0].dtvMeasured, true);
  assert.equal(edges[1].dtvMeasured, false, 'Querstraße bekommt die Zählung nicht');
  assert.equal(edges[1].dtv, DTV_ESTIMATE[2]);
  assert.equal(edges[2].dtv, DTV_ESTIMATE[5]);
  assert.equal(edges[3].dtv, DTV_ESTIMATE[9]);
  assert.equal(r.measured, 1);
  // zu weit weg (30 m) → keine Zuordnung
  const e2 = [{ a: 0, b: 1, p: [], c: 2 }];
  assignTraffic(e2, vertices, [[30000, [[100, 300, 1900, 300]]]], { toPx, S });
  assert.equal(e2[0].dtvMeasured, false);
});

test('Build: Dichteraster füllt Zellen innerhalb des Blocks, spart Löcher aus, nimmt bei Überlappung den höheren Wert', () => {
  const S = 10, toPx = (lat, lon) => [lon, lat], cell = 64 * S;
  const sq = (x0, y0, x1, y1) => [x0, y0, x1, y0, x1, y1, x0, y1, x0, y0];
  const dichte = [
    [350, [sq(0, 0, 1280, 1280)]], // dichter Teilblock oben links (kommt zuerst: der spätere, dünnere darf ihn nicht überschreiben)
    [200, [sq(0, 0, 6400, 6400), sq(2560, 2560, 3840, 3840)]], // 10×10 Zellen mit 2×2-Loch
    [0, [sq(8000, 8000, 9000, 9000)]], // unbewohnt
  ];
  const g = densityGrid(dichte, { toPx, W: 12800, H: 12800, S });
  assert.equal(g.cell, cell);
  const at = (i, j) => g.v[j * g.nx + i];
  assert.equal(at(7, 7), 200);
  assert.equal(at(4, 4), 0, 'im Loch unbewohnt'); assert.equal(at(5, 5), 0);
  assert.equal(at(0, 0), 350, 'höherer Wert gewinnt');
  assert.equal(at(13, 13), 0);
  assert.equal(g.filled, 100 - 4);
});

test('Tätigkeiten an Orten: Uhrzeit und Wochentag entscheiden', () => {
  const at = (cat, kind, m, d) => activityFor({ x: 1, y: 2, cat, kind }, m, d);
  assert.equal(at('drink', 'nightclub', 23 * 60 + 30, FR)?.act, 'queue');
  assert.equal(at('drink', 'nightclub', 23 * 60 + 30, DI), null, 'Dienstag keine Schlange');
  assert.equal(at('drink', 'nightclub', 15 * 60, SA), null);
  assert.equal(at('drink', 'bar', 22 * 60, DI)?.act, 'smoke');
  assert.equal(at('drink', 'bar', 11 * 60, DI), null);
  assert.equal(at('cafe', 'cafe', 3 * 60, DI), null, 'nachts kein Café');
  assert.equal(at('shop', 'convenience', 22 * 60, FR)?.act, 'drink');
  assert.equal(at('bus', 'bus_stop', 3 * 60, DI), null, 'Nachtpause');
  const w = at('bus', 'bus_stop', 8 * 60, DI);
  assert.ok(w === null || w.act === 'wait');
  assert.equal(walkerStyle(3 * 60, () => 0), 'dog', 'nachts keine Jogger, aber Gassigeher');
  assert.equal(walkerStyle(7 * 60, () => 0), 'jog');
  assert.equal(walkerStyle(7 * 60, () => 0.99), null);
  assert.equal(parkLevel(3 * 60, SA), 0);
  assert.ok(parkLevel(15 * 60, SA) > parkLevel(15 * 60, DI));
});

test('Plätze: deterministisch, nie im Haus, Gäste vor Lokalen nicht auf der Fahrbahn; Freitagnacht Leute am Späti', () => {
  const a = lifeSpots(city, giver.x, giver.y, 22 * 60 + 30, FR), b = lifeSpots(city, giver.x, giver.y, 22 * 60 + 30, FR);
  assert.deepEqual(a.map((s) => [s.key, s.x, s.y, s.act]), b.map((s) => [s.key, s.x, s.y, s.act]));
  assert.ok(a.length > 12, `Plätze ${a.length}`);
  const keys = new Set(a.map((s) => s.key));
  assert.equal(keys.size, a.length, 'Schlüssel eindeutig');
  for (const s of a) {
    assert.ok(Number.isFinite(s.x) && Number.isFinite(s.y) && Number.isFinite(s.face), s.key);
    assert.ok(!inBuilding(city, s.x, s.y), `${s.key} (${s.act}) im Haus`);
  }
  const sp = a.filter((s) => s.g === 'spaeti');
  assert.ok(sp.length >= 3 && sp.every((s) => s.act === 'drink'), 'Stammgäste mit Flasche');
  assert.ok(a.some((s) => s.act === 'smoke'), 'Raucher vor Bars');
  // Anordnung um den Gruppenmittelpunkt, der selbst auf dem Gehweg liegt
  for (const s of a) if (s.gx !== undefined && !s.key.startsWith('a')) assert.ok(!onRoad(city, s.gx, s.gy), `${s.key} Mitte auf der Fahrbahn`);
  // tagsüber andere Szene: keine Raucher-Schlangen, dafür Cafégäste
  const day = lifeSpots(city, giver.x, giver.y, 14 * 60, FR);
  assert.ok(day.some((s) => s.act === 'sit'));
  assert.ok(!day.some((s) => s.act === 'queue'));
});

test('Parks: Gruppen auf Decken liegen auf der Wiese, nur bei Tag, am Wochenende mehr', () => {
  const box = { x: giver.x - 3000, y: giver.y - 3000, w: 6000, h: 6000 };
  const grass = city.render.query(box, []).filter((f) => f.layer === 'area' && f.kind === AREA_KIND.grass);
  const lie = (m, d) => lifeSpots(city, giver.x, giver.y, m, d, 3000).filter((s) => s.act === 'lie');
  const sat = lie(15 * 60, SA);
  assert.ok(sat.length > 6, `Liegende ${sat.length}`);
  for (const s of sat) {
    assert.ok(grass.some((a) => pointInRings(s.gx, s.gy, a.rings)), `${s.key} nicht auf einer Wiese`);
    assert.ok(!inBuilding(city, s.gx, s.gy) && !onRoad(city, s.gx, s.gy));
  }
  assert.equal(lie(2 * 60, SA).length, 0, 'nachts niemand im Park');
  assert.ok(lie(15 * 60, DI).length < sat.length, 'werktags weniger');
});

test('Bänke aus OSM: im Kiez vorhanden, Sitzende nur auf Bänken', () => {
  const box = { x: giver.x - 3000, y: giver.y - 3000, w: 6000, h: 6000 };
  const furn = city.render.query(box, []).filter((f) => f.layer === 'furn');
  const benches = furn.filter((f) => f.kind === FURN_KIND.bench);
  assert.ok(benches.length > 20, `Bänke ${benches.length}`);
  assert.ok(furn.some((f) => f.kind === FURN_KIND.bicycle) && furn.some((f) => f.kind === FURN_KIND.bin));
  const at = new Set(benches.map((f) => `b${f.x},${f.y}`));
  const onBench = lifeSpots(city, giver.x, giver.y, 15 * 60, SA, 3000).filter((s) => s.bench);
  assert.ok(onBench.length > 0);
  for (const s of onBench) assert.ok(at.has(s.g), s.key);
});

const run = (w, sec) => { for (let i = 0; i < sec * 60; i++) updateWorld(w, idle(), 1 / 60); };

test('Welt: Leben entsteht außer Sicht, verschwindet nur außer Sicht, Erschreckte geben ihren Platz auf', () => {
  const w = createWorld({ city });
  assert.ok(w.rhythm);
  w.mission.state = 'idle';
  w.clock = 22 * 60 + 30; w.day = FR;
  resetPopulation(w); run(w, 1 / 60); // Aufbau im nächsten Schritt
  assert.ok(w.hangers.size > 10, `Hänger ${w.hangers.size}`);
  assert.ok(w.hangers.size <= LIFE.maxHangers);
  for (const p of w.hangers.values()) { assert.equal(p.state, 'hang'); assert.ok(w.peds.includes(p)); }
  const cam = w.camera;
  const inView = (p) => Math.abs(p.x - cam.x) < LIFE.viewHalfX && Math.abs(p.y - cam.y) < LIFE.viewHalfY;
  const visible = [...w.hangers.values()].filter(inView);
  assert.ok(visible.length > 0, 'nach dem Aufbau sind Leute im Bild');
  // Szene wechselt auf den Mittag: wer im Bild steht, bleibt (kein Aufploppen, kein Verschwinden vor den Augen)
  const known = new Set(w.hangers.values());
  w.clock = 13 * 60; run(w, 1.2);
  for (const p of visible) if (inView(p) && p.state === 'hang') assert.ok(w.peds.includes(p), 'sichtbarer Hänger verschwand');
  for (const p of w.hangers.values()) if (!known.has(p)) assert.ok(!inView(p), 'neuer Hänger im Bild erzeugt');
  // laufender Betrieb: fehlende Plätze werden nur außer Sicht besetzt
  for (const p of w.hangers.values()) w.peds.splice(w.peds.indexOf(p), 1);
  w.hangers.clear(); w._lifeT = -99;
  manageLife(w);
  assert.ok(w.hangers.size > 0, 'außer Sicht neu besetzt');
  for (const p of w.hangers.values()) assert.ok(!inView(p), 'Hänger im Bild aufgeploppt');
  // Erschrecken: flieht und fällt aus der Verwaltung
  const [key, p] = w.hangers.entries().next().value;
  scare(p, p.x + 10, p.y, 2);
  manageLife(w, true);
  assert.ok(!w.hangers.has(key) || w.hangers.get(key) !== p, 'Erschreckter bleibt nicht am Platz registriert');
  assert.equal(p.hang.released, true);
  run(w, 3);
  assert.notEqual(p.state, 'hang');
});

test('Welt: feste Bevölkerung (Tests, Demo) bleibt ohne Rhythmus und ohne Hänger', () => {
  const w = createWorld({ city, cars: 5, pedestrians: 5 });
  assert.equal(w.rhythm, false);
  run(w, 1);
  assert.equal(w.hangers?.size ?? 0, 0);
});

test('Uhr: nach Mitternacht beginnt der nächste Wochentag; Wochentag wird gespeichert', () => {
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  assert.equal(w.day, START_DAY);
  w.clock = 1439.9; w.day = 6;
  run(w, 0.5);
  assert.equal(w.day, 0, 'Sonntag → Montag');
  assert.ok(w.clock < 1);
  const s = validateSave(JSON.parse(JSON.stringify(makeSave(w))));
  const w2 = createWorld({ city, cars: 0, pedestrians: 0 });
  applySave(w2, s);
  assert.equal(w2.day, 0);
  assert.equal(validateSave({ ...makeSave(w), day: 9 }).day, null, 'ungültiger Tag verworfen');
});
