import test from 'node:test';
import assert from 'node:assert/strict';
import { makeProjection, makeInverseProjection } from '../tools/osm/geo.mjs';

// Ursprung wie in web/data/berlin/index.json (Gesamtberlin-Raster) – siehe tests/helpers/city.js.
const LAT0 = 52.506876635, LON0 = 13.424753365;

test('makeInverseProjection kehrt makeProjection über ein 50-m-Raster der ganzen Karte um (< 1 cm, Soll ≪ 1 mm)', () => {
  const proj = makeProjection(LAT0, LON0);
  const inv = makeInverseProjection(LAT0, LON0);
  // Kartenausschnitt aus dem Geländemodell-Plan (Spielmeter x −4022…3839, z −3419…2675) plus Rand, damit auch
  // die Ränder des Ausschnitts abgedeckt sind.
  let maxErr = 0, at = [0, 0];
  for (let x = -4500; x <= 4300; x += 50) {
    for (let y = -3900; y <= 3200; y += 50) {
      const [lat, lon] = inv(x, y);
      assert.ok(Number.isFinite(lat) && Number.isFinite(lon), `${x},${y} -> ${lat},${lon}`);
      const [x2, y2] = proj(lat, lon);
      const err = Math.hypot(x2 - x, y2 - y);
      if (err > maxErr) { maxErr = err; at = [x, y]; }
    }
  }
  assert.ok(maxErr < 0.01, `max Fehler ${maxErr} m bei ${at}`);
});

test('makeInverseProjection: lat/lon -> Projektion -> zurück stimmt ebenfalls (< 1 cm)', () => {
  const proj = makeProjection(LAT0, LON0);
  const inv = makeInverseProjection(LAT0, LON0);
  const pts = [
    [52.487034, 13.424777], // Hermannplatz laut Phase-0-Referenz
    [52.520817, 13.40943], // Fernsehturm
    [LAT0, LON0], // Projektionsursprung selbst
    [52.49, 13.40],
    [52.53, 13.46],
  ];
  for (const [lat, lon] of pts) {
    const [x, y] = proj(lat, lon);
    const [lat2, lon2] = inv(x, y);
    // Vergleich in Metern über die Projektion, nicht in Grad (Grad sind nicht linear zu Metern)
    const [x2, y2] = proj(lat2, lon2);
    const err = Math.hypot(x2 - x, y2 - y);
    assert.ok(err < 0.01, `${lat},${lon}: Fehler ${err} m`);
  }
});

test('makeInverseProjection: Ursprung selbst bildet auf (lat0, lon0) ab', () => {
  const inv = makeInverseProjection(LAT0, LON0);
  const [lat, lon] = inv(0, 0);
  assert.ok(Math.abs(lat - LAT0) < 1e-9, `lat ${lat}`);
  assert.ok(Math.abs(lon - LON0) < 1e-9, `lon ${lon}`);
});
