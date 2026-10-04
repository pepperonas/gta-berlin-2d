// README-Badges (tools/badges.mjs): Zählregeln und das gerenderte SVG.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { countLoc, countTests, short, svg, badges, collect } from '../tools/badges.mjs';

test('Codezeilen: Leer- und Kommentarzeilen zählen nicht, Code hinter einem Blockkommentar schon', () => {
  const src = [
    '// Kopf',
    '/// Doku',
    '',
    'let a = 1; // dahinter zählt',
    '/* Block',
    ' * Mitte',
    ' */',
    '/* kurz */ let b = 2;',
    '/* einzeilig */',
    'let c = 3;',
  ].join('\n');
  assert.equal(countLoc(src), 3);
});

test('Tests: Rust #[test], JS test(/it( am Zeilenanfang, nicht im Text', () => {
  assert.equal(countTests('#[test]\nfn a() {}\n    #[test]\nfn b() {}\n// #[test] nur erwähnt', 'rust'), 2);
  assert.equal(countTests("test('a', () => {});\n  it('b', () => {});\nconst x = test;", 'js'), 2);
});

test('Zahlen kurz und deutsch', () => {
  assert.equal(short(812), '812');
  assert.equal(short(26201), '26,2k');
  assert.equal(short(150000), '150k');
});

test('SVG: Beschriftung und Wert stehen drin, Sonderzeichen sind maskiert', () => {
  const s = svg('Tests', '<1 & 2>', '#2e7d32');
  assert.match(s, /^<svg [^>]*width="\d+" height="20"/);
  assert.ok(s.includes('Tests: &lt;1 &amp; 2&gt;'));
  assert.ok(!s.includes('<1 &'));
});

test('Werte aus dem Repo: Version wie package.json, alle Badges eindeutig benannt', () => {
  const v = collect();
  assert.ok(v.tests.rust > 100 && v.tests.js > 100, JSON.stringify(v.tests));
  assert.ok(v.loc.rust > 10000 && v.loc.js > 10000);
  const b = badges(v);
  assert.equal(new Set(b.map((x) => x[0])).size, b.length);
  assert.equal(b.find((x) => x[0] === 'version')[2], v.version);
  assert.equal(b.find((x) => x[0] === 'tests')[2], String(v.tests.rust + v.tests.js));
});
