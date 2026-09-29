// Nachtleben (nightlife.js): Feed-Parser für die Bar-Auslastung (gostumblr), Verlauf nach Uhrzeit/Wochentag, Zuordnung
// zu den OSM-Lokalen, was man an der Kamera hört, und mehr Leute vor vollen Bars (life.js).
import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity, geoToPx } from './helpers/city.js';
import { parseBarFeed, parseWeek, normName, attachBars, feedBarFor, barLevel, typicalLevel, nightlifeAt, NIGHT } from '../web/src/nightlife.js';
import { geoToPx as gameGeoToPx } from '../web/src/projection.js';
import { lifeSpots, frontOf } from '../web/src/life.js';
import { ambienceAt } from '../web/src/ambience.js';

const city = realCity();
const MO = 0, DI = 1, FR = 4, SA = 5;
const hours = (f) => Array.from({ length: 24 }, (_, h) => f(h));

test('Feed-Parser: Google-Stoßzeiten, Prozent-Auslastung, GeoJSON; Unbrauchbares fällt weg', () => {
  const g = parseBarFeed([{ name: 'Würgeengel', coordinates: { lat: 52.4995, lng: 13.4180 }, current_popularity: 64,
    populartimes: [{ name: 'Monday', data: hours(() => 10) }, { name: 'Saturday', data: hours((h) => (h >= 22 ? 90 : 20)) }] }]);
  assert.equal(g.bars.length, 1);
  const b = g.bars[0];
  assert.equal(b.key, 'wurgeengel');
  assert.equal(b.current, 0.64, 'Prozent → 0..1');
  assert.equal(b.week[0][12], 0.1);
  assert.equal(b.week[5][23], 0.9, 'Samstag an Index 5 (Mo zuerst)');
  assert.equal(b.week[1], null, 'fehlende Tage bleiben leer');

  const p = parseBarFeed({ updated: '2026-09-26T22:00:00Z', bars: [{ title: 'Bar A', lat: '52.5', lon: '13.4', auslastung: 0.3 }, { foo: 1 }, { name: 'Nur Name', occupancy: 80 }] });
  assert.equal(p.bars.length, 2, 'Eintrag ohne Namen und Koordinaten fällt weg');
  assert.equal(p.at, Date.parse('2026-09-26T22:00:00Z') / 1000);
  assert.equal(p.bars[0].lat, 52.5); assert.equal(p.bars[1].lat, null);
  assert.equal(p.bars[1].current, 0.8);

  const gj = parseBarFeed({ type: 'FeatureCollection', features: [{ type: 'Feature', geometry: { type: 'Point', coordinates: [13.42, 52.49] }, properties: { name: 'Geo', load: 55 } }] });
  assert.deepEqual([gj.bars[0].lat, gj.bars[0].lon, gj.bars[0].current], [52.49, 13.42, 0.55], 'GeoJSON: [lon, lat]');

  assert.throws(() => parseBarFeed({ nix: true }), /keine Liste/);
  assert.throws(() => parseBarFeed([{ foo: 1 }]), /keine Bar/);
  assert.deepEqual(parseWeek(hours(() => 50)).map((d) => d[3]), new Array(7).fill(0.5), 'ein Tagesprofil gilt für alle Tage');
  assert.equal(parseWeek({ fr: hours(() => 100) })[FR][0], 1, 'Wochentag als Schlüssel');
  assert.equal(normName('Bar Tausend Berlin'), 'tausend');
  assert.equal(normName('Möbel-Olfe'), 'mobel olfe');
});

test('Verlauf: Freitag-/Samstagnacht voll, Dienstagmittag still, Clubs erst ab 23 Uhr, Kneipe schon zum Feierabend', () => {
  assert.ok(typicalLevel('bar', 23 * 60 + 30, FR) > 0.8);
  assert.ok(typicalLevel('bar', 1 * 60, SA) > 0.8, 'Nacht auf Samstag zählt zum Freitag');
  assert.equal(typicalLevel('bar', 12 * 60, DI), 0);
  assert.equal(typicalLevel('nightclub', 21 * 60, SA), 0);
  assert.ok(typicalLevel('nightclub', 2 * 60, SA) > 0.9);
  assert.ok(typicalLevel('nightclub', 2 * 60, DI) < 0.1, 'unter der Woche fast nichts');
  assert.ok(typicalLevel('pub', 19 * 60, DI) > 0.15, 'Feierabendbier');
  assert.equal(typicalLevel('biergarten', 9 * 60, SA), 0);
  // Feed: Wochenprofil gilt zur Spielzeit, zwischen den Stunden linear
  const bar = { week: new Array(7).fill(null).map(() => hours((h) => h / 23)), current: null, at: null };
  assert.ok(Math.abs(barLevel('bar', bar, 12 * 60 + 30, MO) - 12.5 / 23) < 1e-9);
  // nur Live-Wert: gleicher Tagesverlauf, volle Bar lauter als leere, tagsüber still
  const full = { week: null, current: 0.9, at: null }, empty = { week: null, current: 0.1, at: null };
  assert.ok(barLevel('bar', full, 23 * 60, FR) > barLevel('bar', empty, 23 * 60, FR));
  assert.equal(barLevel('bar', full, 12 * 60, DI), 0);
  assert.equal(barLevel('bar', { ...full, at: 0 }, 23 * 60, FR, NIGHT.stale + 1), barLevel('bar', { ...full, current: null, at: 0 }, 23 * 60, FR), 'veralteter Live-Wert zählt nicht');
});

test('Projektion im Spiel = Projektion des Builds', () => {
  const toPx = gameGeoToPx(city.meta);
  for (const [lat, lon] of [[52.4995, 13.418], [52.52, 13.405], [52.45, 13.3]]) assert.deepEqual(toPx(lat, lon), geoToPx(city.meta, lat, lon));
});

test('Zuordnung und Hörbarkeit: Feed-Bar mit OSM-Namen, Feed-Bar ohne OSM-Eintrag; tagsüber still', () => {
  const pois = city.list('poi').filter((q) => q.cat === 'drink' && q.kind === 'bar' && q.name && frontOf(city, q));
  assert.ok(pois.length > 20, 'genug Bars in Kreuzberg/Neukölln');
  const q = pois[0], x = q.x + 30, y = q.y;
  const toPx = gameGeoToPx(city.meta);
  // Feed-Bar ohne OSM-Gegenstück: an der Görlitzer Straße, über 100 m vom nächsten OSM-Lokal
  const extra = { lat: 52.4978, lon: 13.4338 };
  const [ex, ey] = toPx(extra.lat, extra.lon);
  const alone = city.list('poi').every((p) => p.cat !== 'drink' || Math.hypot(p.x - ex, p.y - ey) > NIGHT.hearFeed);
  attachBars(city, parseBarFeed([{ name: q.name, current: 95 }, { name: 'Nur im Feed', ...extra, current: 90 }]), toPx);
  try {
    assert.equal(feedBarFor(city, q)?.name, q.name, 'gleicher Name → zugeordnet');
    assert.ok(feedBarFor(city, q).osm);
    const other = pois.find((p) => normName(p.name) !== normName(q.name) && Math.hypot(p.x - q.x, p.y - q.y) > NIGHT.near);
    assert.equal(feedBarFor(city, other), null);
    const sat = nightlifeAt(city, x, y, 23 * 60 + 30, FR, { front: (p) => frontOf(city, p) });
    assert.ok(sat.crowd > 0.3, `Freitagnacht vor der Bar laut (${sat.crowd})`);
    assert.ok(sat.sources[0].feed && sat.sources[0].name === q.name, 'lauteste Quelle ist die Feed-Bar');
    assert.ok(sat.feed > 0);
    const day = nightlifeAt(city, x, y, 11 * 60, DI, { front: (p) => frontOf(city, p) });
    assert.equal(day.crowd, 0, 'Dienstagvormittag still');
    assert.ok(alone, 'Testpunkt liegt außer Hörweite aller OSM-Lokale');
    const near = nightlifeAt(city, ex + 50, ey, 60, SA);
    assert.ok(near.crowd > 0.2 && near.sources.some((s) => s.name === 'Nur im Feed'), 'Feed-Bar ohne OSM-Eintrag hörbar');
    assert.equal(nightlifeAt(city, ex + NIGHT.hearFeed + 400, ey, 60, SA).sources.some((s) => s.name === 'Nur im Feed'), false, 'außer Hörweite still');
    // Regen treibt die Leute rein
    const wet = nightlifeAt(city, x, y, 23 * 60 + 30, FR, { front: (p) => frontOf(city, p), weather: { rain: 1 } });
    assert.ok(wet.crowd < sat.crowd);
    // Mehr Raucher vor der vollen Feed-Bar als vor ihr ohne Feed
    const count = () => lifeSpots(city, q.x, q.y, 23 * 60, FR, 200).filter((s) => s.g === `p${q.x},${q.y}`).length;
    const withFeed = count();
    city.bars = null;
    const without = count();
    assert.ok(withFeed > without, `volle Bar: ${withFeed} statt ${without} Leute davor`);
  } finally { city.bars = null; }
});

test('Umgebungsklang: Nachtleben in der Mischung, im Auto gedämpft', () => {
  const q = city.list('poi').find((p) => p.cat === 'drink' && p.kind === 'bar' && frontOf(city, p));
  const f = frontOf(city, q);
  const w = { camera: { x: f.x, y: f.y }, city, cars: [], peds: [], time: 0, clock: 23 * 60 + 30, day: FR, weather: null, player: { inCar: null } };
  const m = ambienceAt(w);
  assert.ok(m.bar > 0.2, `Stimmengewirr vor der Bar (${m.bar})`);
  assert.ok(m.music > 0, 'gedämpfte Musik');
  assert.equal(m.muffle, 0);
  assert.ok(ambienceAt({ ...w, player: { inCar: 3 } }).muffle > 0.5, 'im Auto dumpf');
  assert.equal(ambienceAt({ ...w, clock: 11 * 60, day: DI }).bar, 0, 'vormittags still');
});
