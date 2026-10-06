// Version der Rust-Fassung (Cargo-Arbeitsbereich, [workspace.package] version) automatisch hochzählen.
// Läuft im Pre-commit-Hook: steckt im Commit eine Änderung an der Rust-Fassung (crates/, tools/physics-calibrate/,
// Cargo.toml, Cargo.lock oder Daten, die sie einbettet: data/), steigt die Patch-Stelle um eins – in Cargo.toml und
// bei den eigenen Paketen in Cargo.lock. Wer Minor oder Major von Hand anhebt (Cargo.toml im Commit mit anderer
// Version als HEAD), dem zählt der Hook nichts dazu.
//
//   node tools/rust-version.mjs          # Hook-Modus (prüft den Index, ändert und staged)
//   node tools/rust-version.mjs --zeige  # aktuelle Version ausgeben
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

/** Pfade, deren Änderung die Rust-Fassung betrifft */
export function touchesRust(paths) {
  return paths.some((p) =>
    /^(crates\/|tools\/physics-calibrate\/|data\/)/.test(p) || p === 'Cargo.toml' || p === 'Cargo.lock');
}

/** Version aus [workspace.package] */
export function workspaceVersion(cargoToml) {
  const sec = cargoToml.split(/^\[/m).find((s) => s.startsWith('workspace.package]'));
  const m = sec && sec.match(/^version = "(\d+)\.(\d+)\.(\d+)"/m);
  if (!m) throw new Error('Cargo.toml: [workspace.package] version fehlt');
  return `${m[1]}.${m[2]}.${m[3]}`;
}

export function bumpPatch(v) {
  const [a, b, c] = v.split('.').map(Number);
  return `${a}.${b}.${c + 1}`;
}

/** Cargo.toml mit neuer Arbeitsbereichs-Version (nur in [workspace.package]) */
export function setWorkspaceVersion(cargoToml, from, to) {
  const parts = cargoToml.split(/^(?=\[)/m);
  return parts
    .map((s) => (s.startsWith('[workspace.package]') ? s.replace(`version = "${from}"`, `version = "${to}"`) : s))
    .join('');
}

/** Cargo.lock: die eigenen Pakete (ohne `source`-Zeile) von `from` auf `to` */
export function setLockVersions(lock, from, to) {
  return lock
    .split('[[package]]')
    .map((block, i) => {
      if (i === 0 || /\nsource = /.test(block)) return block;
      return block.replace(`\nversion = "${from}"\n`, `\nversion = "${to}"\n`);
    })
    .join('[[package]]');
}

function git(...args) {
  return execFileSync('git', args, { encoding: 'utf8' });
}

function main() {
  const toml = readFileSync('Cargo.toml', 'utf8');
  const now = workspaceVersion(toml);
  if (process.argv.includes('--zeige')) {
    console.log(now);
    return;
  }
  const staged = git('diff', '--cached', '--name-only').split('\n').filter(Boolean);
  if (!touchesRust(staged)) return;
  let head = null;
  try {
    head = workspaceVersion(git('show', 'HEAD:Cargo.toml'));
  } catch {
    // erster Commit oder Cargo.toml neu
  }
  if (head !== null && head !== now) {
    console.log(`Rust-Fassung: Version von Hand gesetzt (${head} → ${now})`);
    return;
  }
  // ungestagte Änderungen an Cargo.toml/Cargo.lock würde `git add` unbemerkt mitnehmen
  const dirty = git('diff', '--name-only', '--', 'Cargo.toml', 'Cargo.lock').split('\n').filter(Boolean);
  if (dirty.length) {
    console.error(`Rust-Fassung: ${dirty.join(', ')} hat ungestagte Änderungen – erst stagen oder verwerfen`);
    process.exit(1);
  }
  const next = bumpPatch(now);
  writeFileSync('Cargo.toml', setWorkspaceVersion(toml, now, next));
  writeFileSync('Cargo.lock', setLockVersions(readFileSync('Cargo.lock', 'utf8'), now, next));
  git('add', 'Cargo.toml', 'Cargo.lock');
  console.log(`Rust-Fassung: Version ${now} → ${next}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
