// Liste der Rust-Abhängigkeiten mit Lizenz für die Seite „Über das Spiel“: liest `cargo metadata` und schreibt
// crates/game/src/thirdparty.tsv (Name, Version, Lizenz je Zeile, sortiert). Ein Rust-Test prüft, dass die Liste
// zu Cargo.lock passt; nach einem Update von Abhängigkeiten: node tools/thirdparty.mjs
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = 'crates/game/src/thirdparty.tsv';

const meta = JSON.parse(
  execFileSync('cargo', ['metadata', '--format-version', '1', '--locked'], {
    cwd: ROOT,
    encoding: 'utf8',
    maxBuffer: 256 << 20,
  }),
);
const own = new Set(meta.workspace_members);
const rows = meta.packages
  .filter((p) => !own.has(p.id))
  .map((p) => [p.name, p.version, (p.license || 'siehe Paket').replace(/\s+/g, ' ')])
  .sort((a, b) => a[0].localeCompare(b[0]) || a[1].localeCompare(b[1], undefined, { numeric: true }));
writeFileSync(join(ROOT, OUT), rows.map((r) => r.join('\t')).join('\n') + '\n');
console.log(`${OUT}: ${rows.length} Pakete`);
