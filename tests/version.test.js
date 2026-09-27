import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { VERSION } from '../web/src/version.js';

const read = (p) => readFileSync(new URL(`../${p}`, import.meta.url), 'utf8');

test('Version: SemVer X.Y.Z, gleich in package.json, Spiel und App-Paket', () => {
  assert.match(VERSION, /^\d+\.\d+\.\d+$/);
  assert.equal(JSON.parse(read('package.json')).version, VERSION);
  const m = /<Identity\b[^>]*\bVersion="([^"]+)"/.exec(read('xbox/GtaBerlin/Package.appxmanifest'));
  assert.equal(m?.[1], `${VERSION}.0`, 'App-Paket trägt X.Y.Z.0');
});

test('Version: CHANGELOG hat einen datierten Eintrag für die aktuelle Version, neueste zuerst', () => {
  const log = read('CHANGELOG.md');
  const heads = [...log.matchAll(/^## \[(\d+\.\d+\.\d+)\] – (\d{4}-\d{2}-\d{2})$/gm)];
  assert.ok(heads.length, 'keine Versionseinträge');
  assert.equal(heads[0][1], VERSION, 'oberster Eintrag = aktuelle Version');
  const key = (v) => v.split('.').map(Number).reduce((a, n) => a * 1000 + n, 0);
  for (let i = 1; i < heads.length; i++) {
    assert.ok(key(heads[i - 1][1]) > key(heads[i][1]), 'Versionen absteigend');
    assert.ok(heads[i - 1][2] >= heads[i][2], 'Daten absteigend');
  }
  assert.equal(new Set(heads.map((h) => h[1])).size, heads.length, 'keine doppelten Versionen');
});

test('Version: Titelbildschirm zeigt die Version aus version.js statt einer festen Zahl', () => {
  const hud = read('web/src/hud.js');
  assert.ok(hud.includes('v${VERSION}'));
  assert.doesNotMatch(hud, /'v\d+\.\d+/);
});
