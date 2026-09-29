import test from 'node:test';
import assert from 'node:assert/strict';
import { railAt, undergroundAt, undergroundAtS, TUNNEL } from '../web/src/tunnel.js';
import { pointOn } from '../web/src/transit.js';
import { realCity, realTransit } from './helpers/city.js';

const city = realCity(), tr = realTransit();
const longest = (name) => tr.patterns.filter((p) => p.name === name).sort((a, b) => b.stops.length - a.stops.length)[0];
const stopAt = (p, name) => p.stops[p.stopNames.findIndex((n) => n.startsWith(name))];

test('Tunnel: U8 am Kottbusser Tor unter Tage, U1 an der Skalitzer Straße oberirdisch, Tram nie', () => {
  const u8 = longest('U8'), u1 = longest('U1'), m10 = longest('M10');
  const k8 = pointOn(u8, stopAt(u8, 'U Kottbusser Tor'));
  assert.equal(undergroundAt(city, 'ubahn', k8.x, k8.y), true, 'U8 Kotti');
  assert.equal(undergroundAtS(city, u8, stopAt(u8, 'U Kottbusser Tor')), true);
  const k1 = pointOn(u1, stopAt(u1, 'U Kottbusser Tor'));
  assert.equal(undergroundAt(city, 'ubahn', k1.x, k1.y), false, 'U1 Hochbahn');
  assert.ok(railAt(city, k1.x, k1.y), 'Gleis der Hochbahn');
  const m = pointOn(m10, m10.stops[3]);
  assert.equal(undergroundAt(city, 'tram', m.x, m.y), false);
  assert.equal(undergroundAt(city, 'bus', k8.x, k8.y), false);
});

test('Tunnel: nicht geladene Kachel gilt als oberirdisch; Cache folgt dem Kachelstand', () => {
  const fake = { gen: 1, ready: () => false, render: { query: () => [] } };
  assert.equal(undergroundAt(fake, 'ubahn', 100, 100), false, 'ohne Kachel keine Tunnelansicht');
  const p = { id: 1, mode: 'ubahn', shape: { pts: [0, 0, 1000, 0], cum: [0, 1000], len: 1000 } };
  const ready = { gen: 1, ready: () => true, render: { query: () => [] } };
  assert.equal(undergroundAtS(ready, p, 500), true);
  ready.render.query = () => [{ layer: 'rail', pts: [0, 0, 1000, 0] }]; ready.gen = 2;
  assert.equal(undergroundAtS(ready, p, 500), false, 'neuer Kachelstand verwirft den Cache');
  assert.equal(TUNNEL.probe, 50);
});

// Kottbusser Tor im Kleinen: eine gerade Strecke entlang +x (Fahrtrichtung (1,0) am Start), einmal mit einer
// QUERENDEN Schiene 20 px daneben (Nord-Süd, wie das U1-Viadukt zur U8), einmal mit einer PARALLELEN (Ost-West).
test('Tunnel: Fahrtrichtung filtert eine querende Schiene, lässt eine parallele gelten; railAt ohne Richtung bleibt reine Nähe', () => {
  const straight = { mode: 'ubahn', shape: { pts: [0, 0, 1000, 0], cum: [0, 1000], len: 1000 } };
  const perpendicular = [{ layer: 'rail', pts: [20, -100, 20, 100] }];
  const parallel = [{ layer: 'rail', pts: [-100, 20, 100, 20] }];

  const crossCity = { gen: 1, ready: () => true, render: { query: () => perpendicular } };
  assert.equal(undergroundAtS(crossCity, { ...straight }, 0), true, 'querende Schiene 20 px entfernt: bleibt Tunnel');

  const paraCity = { gen: 1, ready: () => true, render: { query: () => parallel } };
  assert.equal(undergroundAtS(paraCity, { ...straight }, 0), false, 'parallele Schiene 20 px entfernt: oberirdisch');

  assert.ok(railAt(crossCity, 0, 0), 'railAt ohne Fahrtrichtung findet die querende Schiene per reiner Nähe');
});
