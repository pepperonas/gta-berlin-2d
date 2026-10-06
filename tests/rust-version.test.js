import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  bumpPatch, setLockVersions, setWorkspaceVersion, touchesRust, workspaceVersion,
} from '../tools/rust-version.mjs';

test('Rust-Fassung: Änderungen an Crates, Daten und Cargo-Dateien zählen, Web nicht', () => {
  assert.ok(touchesRust(['crates/engine/src/hud.rs']));
  assert.ok(touchesRust(['docs/x.md', 'data/gfx/palette.json']));
  assert.ok(touchesRust(['Cargo.lock']));
  assert.ok(touchesRust(['tools/physics-calibrate/src/main.rs']));
  assert.ok(!touchesRust(['web/src/main.js', 'docs/NATIVE-RUST.md', 'CHANGELOG.md', 'tools/badges.mjs']));
});

test('Rust-Fassung: Patch-Stelle steigt, nur im Arbeitsbereich und bei eigenen Paketen', () => {
  const toml = '[workspace]\nmembers = []\n\n[workspace.package]\nedition = "2024"\nversion = "0.1.9"\n\n'
    + '[workspace.dependencies]\nfoo = { version = "0.1.9" }\n';
  assert.equal(workspaceVersion(toml), '0.1.9');
  assert.equal(bumpPatch('0.1.9'), '0.1.10');
  const out = setWorkspaceVersion(toml, '0.1.9', '0.1.10');
  assert.equal(workspaceVersion(out), '0.1.10');
  assert.ok(out.includes('foo = { version = "0.1.9" }'), 'Abhängigkeiten bleiben');
  const lock = 'version = 4\n\n[[package]]\nname = "gta-berlin"\nversion = "0.1.9"\ndependencies = []\n\n'
    + '[[package]]\nname = "fremd"\nversion = "0.1.9"\nsource = "registry+https://x"\n';
  const l2 = setLockVersions(lock, '0.1.9', '0.1.10');
  assert.ok(l2.includes('name = "gta-berlin"\nversion = "0.1.10"'));
  assert.ok(l2.includes('name = "fremd"\nversion = "0.1.9"'), 'fremde Pakete bleiben');
});

test('Rust-Fassung: Cargo.lock trägt für alle eigenen Pakete die Arbeitsbereichs-Version', () => {
  const v = workspaceVersion(readFileSync('Cargo.toml', 'utf8'));
  const lock = readFileSync('Cargo.lock', 'utf8');
  const own = lock.split('[[package]]').slice(1).filter((b) => !/\nsource = /.test(b));
  assert.ok(own.length >= 7);
  for (const b of own) assert.match(b, new RegExp(`\\nversion = "${v.replaceAll('.', '\\.')}"\\n`), b.slice(0, 60));
});
