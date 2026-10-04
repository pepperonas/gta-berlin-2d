// Badges für die README: zählt Codezeilen und Tests, liest Versionen aus den Projektdateien und schreibt je Badge
// eine SVG-Datei nach docs/badges/ (dazu badges.json mit allen Werten). Das Repo ist privat, darum keine
// Shields.io-Endpunkte: die Bilder liegen im Repo und werden relativ eingebunden.
//
//   node tools/badges.mjs           Badges neu schreiben (macht der Commit-Hook tools/githooks/pre-commit)
//   node tools/badges.mjs --check   nur prüfen; Rückgabewert 1, wenn ein Badge veraltet ist
//
// Gezählt wird statisch, ohne die Tests auszuführen: Rust `#[test]`, JS `test(`/`it(` am Zeilenanfang in tests/.
// Das deckt sich mit den Läufen (npm test: 471, cargo test: alle `#[test]` inkl. ignorierter).
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
export const OUT = 'docs/badges';

// Sprachen nach Dateiendung; versionierte und neue, nicht ignorierte Dateien (erzeugte fallen über .gitignore heraus)
const LANGS = { '.rs': 'rust', '.wgsl': 'wgsl', '.js': 'js', '.mjs': 'js', '.cs': 'csharp' };
const SKIP = [/^web\/data\//, /^temp\//, /^data\//];

// Codezeilen: ohne Leerzeilen und ohne reine Kommentarzeilen (Zeilen- und Blockkommentare).
export function countLoc(text) {
  let n = 0;
  let block = false;
  for (const raw of text.split('\n')) {
    const l = raw.trim();
    if (block) {
      if (l.includes('*/')) {
        block = false;
        if (l.slice(l.indexOf('*/') + 2).trim()) n++;
      }
      continue;
    }
    if (!l || l.startsWith('//')) continue;
    if (l.startsWith('/*')) {
      if (!l.includes('*/', 2)) block = true;
      else if (l.slice(l.indexOf('*/', 2) + 2).trim()) n++;
      continue;
    }
    n++;
  }
  return n;
}

/** Tests in einer Datei: Rust `#[test]`, JS `test(`/`it(` am Zeilenanfang. */
export function countTests(text, lang) {
  const re = lang === 'rust' ? /^\s*#\[test\]/gm : /^\s*(?:test|it)\(/gm;
  return (text.match(re) || []).length;
}

/** Ganze Zahl lesbar: 12345 → „12,3k“, 812 → „812“. */
export function short(n) {
  if (n < 1000) return String(n);
  const k = n / 1000;
  return (k < 100 ? k.toFixed(1).replace('.', ',') : Math.round(k)) + 'k';
}

// Breite eines Texts in Verdana 11 px (wie Shields.io), grob nach Zeichenklassen
export function textWidth(s) {
  let w = 0;
  for (const c of s) {
    if (' ,.:;\'|!iljI'.includes(c)) w += 3.6;
    else if ('ftr()[]'.includes(c)) w += 4.6;
    else if ('mwMW'.includes(c)) w += 10.3;
    else if (/[A-ZÄÖÜ]/.test(c)) w += 7.6;
    else if (/[0-9]/.test(c)) w += 7;
    else if ('·×≥–'.includes(c)) w += 7;
    else w += 6.6;
  }
  return Math.ceil(w);
}

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** Flaches Badge im Stil von Shields.io. */
export function svg(label, value, color) {
  const lw = textWidth(label) + 12;
  const vw = textWidth(value) + 12;
  const w = lw + vw;
  const t = (x, s, len) =>
    `<text x="${x * 10}" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="${len * 10}">${esc(s)}</text>` +
    `<text x="${x * 10}" y="140" transform="scale(.1)" textLength="${len * 10}">${esc(s)}</text>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="20" role="img" aria-label="${esc(label)}: ${esc(value)}">` +
    `<title>${esc(label)}: ${esc(value)}</title>` +
    `<linearGradient id="s" x2="0" y2="100%"><stop offset="0" stop-color="#bbb" stop-opacity=".1"/><stop offset="1" stop-opacity=".1"/></linearGradient>` +
    `<clipPath id="r"><rect width="${w}" height="20" rx="3" fill="#fff"/></clipPath>` +
    `<g clip-path="url(#r)"><rect width="${lw}" height="20" fill="#555"/><rect x="${lw}" width="${vw}" height="20" fill="${color}"/><rect width="${w}" height="20" fill="url(#s)"/></g>` +
    `<g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110">` +
    t(lw / 2, label, lw - 12) + t(lw + vw / 2, value, vw - 12) +
    `</g></svg>\n`;
}

/** Alle Werte aus dem Arbeitsbaum. */
export function collect(root = ROOT) {
  const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { cwd: root, encoding: 'utf8' })
    .split('\n')
    .filter((f) => f && !SKIP.some((r) => r.test(f)));
  const loc = { rust: 0, wgsl: 0, js: 0, csharp: 0 };
  const tests = { rust: 0, js: 0 };
  for (const f of files) {
    const lang = LANGS[extname(f)];
    if (!lang) continue;
    let text;
    try {
      text = readFileSync(join(root, f), 'utf8');
    } catch {
      continue; // gelöscht, aber noch nicht aus dem Index
    }
    loc[lang] += countLoc(text);
    if (lang === 'rust') tests.rust += countTests(text, 'rust');
    if (lang === 'js' && f.startsWith('tests/') && f.endsWith('.test.js')) tests.js += countTests(text, 'js');
  }
  const read = (f) => readFileSync(join(root, f), 'utf8');
  const lock = read('Cargo.lock');
  const crate = (name) => (lock.match(new RegExp(`name = "${name}"\\nversion = "([^"]+)"`)) || [])[1] || '?';
  const cargo = read('Cargo.toml');
  const pkg = JSON.parse(read('package.json'));
  const index = JSON.parse(read('web/data/berlin/index.json'));
  const m = index.meta;
  return {
    version: pkg.version,
    loc,
    tests,
    edition: (cargo.match(/edition = "(\d+)"/) || [])[1] || '?',
    msrv: (cargo.match(/rust-version = "([^"]+)"/) || [])[1] || '?',
    crates: (cargo.match(/members = \[([^\]]*)\]/) || ['', ''])[1].split(',').filter((s) => s.trim()).length,
    wgpu: crate('wgpu'),
    winit: crate('winit'),
    cpal: crate('cpal'),
    jsDeps: Object.keys(pkg.dependencies || {}).length,
    node: (pkg.engines?.node || '?').replace('>=', '≥ '),
    tiles: Array.isArray(index.tiles) ? index.tiles.length : Object.keys(index.tiles).length,
    areaKm: [Math.round(m.width / m.scale / 1000), Math.round(m.height / m.scale / 1000)],
    osm: (m.osmBase || '').slice(0, 10),
  };
}

/** Badges in README-Reihenfolge: [Datei, Beschriftung, Wert, Farbe]. */
export function badges(v) {
  const code = v.loc.rust + v.loc.wgsl + v.loc.js + v.loc.csharp;
  return [
    ['version', 'Version', v.version, '#0b57d0'],
    ['status', 'Status', 'Prototyp (0.x)', '#e67e22'],
    ['loc', 'Codezeilen', short(code), '#2e7d32'],
    ['loc-rust', 'Rust', short(v.loc.rust) + ' LoC', '#b7410e'],
    ['loc-js', 'JavaScript', short(v.loc.js) + ' LoC', '#c9a800'],
    ['loc-wgsl', 'WGSL', short(v.loc.wgsl) + ' LoC', '#5c6bc0'],
    ['tests', 'Unit-Tests', String(v.tests.rust + v.tests.js), '#2e7d32'],
    ['tests-rust', 'Rust-Tests', String(v.tests.rust), '#b7410e'],
    ['tests-js', 'JS-Tests', String(v.tests.js), '#c9a800'],
    ['rust', 'Rust', `${v.edition} · MSRV ${v.msrv}`, '#b7410e'],
    ['crates', 'Crates', String(v.crates), '#795548'],
    ['wgpu', 'wgpu', v.wgpu, '#40739e'],
    ['winit', 'winit', v.winit, '#40739e'],
    ['cpal', 'Audio', `cpal ${v.cpal}`, '#40739e'],
    ['graphics', 'Grafik', 'Metal | DX12', '#6a1b9a'],
    ['platform', 'Plattform', 'macOS | Windows', '#455a64'],
    ['xbox', 'Xbox-Hülle', 'nicht gebaut', '#9e9e9e'],
    ['node', 'Node', v.node, '#3c873a'],
    ['js-deps', 'JS-Abhängigkeiten', String(v.jsDeps), v.jsDeps ? '#e67e22' : '#2e7d32'],
    ['map', 'Karte', 'OpenStreetMap', '#7ebc6f'],
    ['area', 'Berlin', `1:1 · ${v.areaKm[0]} × ${v.areaKm[1]} km`, '#d32f2f'],
    ['tiles', 'Kacheln', String(v.tiles), '#7ebc6f'],
    ['osm', 'OSM-Stand', v.osm, '#7ebc6f'],
    ['semver', 'Versionierung', 'SemVer', '#3f51b5'],
    ['changelog', 'Changelog', 'Keep a Changelog', '#e05735'],
    ['lang', 'Sprache', 'Deutsch', '#000000'],
  ];
}

/** Dateiinhalte, die in docs/badges/ stehen sollen. */
export function render(v) {
  const out = { 'badges.json': JSON.stringify(v, null, 2) + '\n' };
  for (const [f, label, value, color] of badges(v)) out[f + '.svg'] = svg(label, value, color);
  return out;
}

function main() {
  const want = render(collect());
  const dir = join(ROOT, OUT);
  const check = process.argv.includes('--check');
  let have = [];
  try {
    have = readdirSync(dir);
  } catch {}
  const stale = Object.keys(want).filter((f) => {
    try {
      return readFileSync(join(dir, f), 'utf8') !== want[f];
    } catch {
      return true;
    }
  });
  const extra = have.filter((f) => !(f in want));
  if (check) {
    if (stale.length || extra.length) {
      console.error(`Badges veraltet: ${[...stale, ...extra].join(', ')} — node tools/badges.mjs`);
      process.exit(1);
    }
    return;
  }
  mkdirSync(dir, { recursive: true });
  for (const f of stale) writeFileSync(join(dir, f), want[f]);
  for (const f of extra) rmSync(join(dir, f));
  if (stale.length || extra.length) console.log(`Badges aktualisiert: ${stale.length + extra.length} Datei(en)`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
