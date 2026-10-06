// Bereitet das UWP-Projekt vor: kopiert web/ nach xbox/GtaBerlin/Web (die Paket-Logos kommen aus build_icon.py).
// Läuft auf macOS, Linux und Windows (nur Node, keine Abhängigkeiten).
import { cp, rm } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const root = new URL('../', import.meta.url);
const web = fileURLToPath(new URL('web/', root));
const proj = new URL('xbox/GtaBerlin/', root);
const target = fileURLToPath(new URL('Web/', proj));

await rm(target, { recursive: true, force: true });
await cp(web, target, { recursive: true });
console.log(`web/ → ${target}`);

// Die Paket-Logos liegen fertig in xbox/GtaBerlin/Assets/ (erzeugt von tools/gfx/build_icon.py, dasselbe Motiv wie
// das App-Icon) und werden hier nicht mehr überschrieben.
console.log('Fertig. Nächster Schritt (Windows): xbox\\GtaBerlin.sln in Visual Studio öffnen, Release|x64, Paket erstellen.');
