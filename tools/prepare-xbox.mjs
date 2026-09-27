// Bereitet das UWP-Projekt vor: kopiert web/ nach xbox/GtaBerlin/Web und erzeugt die Paket-Logos (PNG).
// Läuft auf macOS, Linux und Windows (nur Node, keine Abhängigkeiten).
import { cp, rm, mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { deflateSync } from 'node:zlib';

const root = new URL('../', import.meta.url);
const web = fileURLToPath(new URL('web/', root));
const proj = new URL('xbox/GtaBerlin/', root);
const target = fileURLToPath(new URL('Web/', proj));

await rm(target, { recursive: true, force: true });
await cp(web, target, { recursive: true });
console.log(`web/ → ${target}`);

// --- Logos: einfarbige Kachel mit Fernsehturm-Symbol, als PNG ohne Bibliothek ---
function crc32(buf) {
  let c, crc = 0xffffffff;
  for (let n = 0; n < buf.length; n++) {
    c = (crc ^ buf[n]) & 0xff;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc = (crc >>> 8) ^ c;
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
export function png(w, h, pixel) {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 4 + 1)] = 0;
    for (let x = 0; x < w; x++) {
      const [r, g, b, a] = pixel(x, y);
      const o = y * (w * 4 + 1) + 1 + x * 4;
      raw[o] = r; raw[o + 1] = g; raw[o + 2] = b; raw[o + 3] = a;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 6;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
function logo(w, h) {
  const bg = [20, 21, 28, 255], fg = [255, 211, 61, 255];
  const s = Math.min(w, h), cx = w / 2, cy = h / 2;
  return png(w, h, (x, y) => {
    const u = (x - cx) / s, v = (y - cy) / s;
    const shaft = Math.abs(u) < 0.045 && v > -0.3 && v < 0.36;
    const ball = u * u + (v + 0.12) ** 2 < 0.12 ** 2;
    const needle = Math.abs(u) < 0.012 && v > -0.44 && v <= -0.3;
    const ground = v > 0.33 && v < 0.38 && Math.abs(u) < 0.38;
    return shaft || ball || needle || ground ? fg : bg;
  });
}
const assets = fileURLToPath(new URL('Assets/', proj));
await mkdir(assets, { recursive: true });
const sizes = { 'StoreLogo.png': [50, 50], 'Square44x44Logo.png': [44, 44], 'Square150x150Logo.png': [150, 150], 'Wide310x150Logo.png': [310, 150], 'SplashScreen.png': [620, 300] };
for (const [name, [w, h]] of Object.entries(sizes)) await writeFile(assets + name, logo(w, h));
console.log(`Logos → ${assets}`);
console.log('Fertig. Nächster Schritt (Windows): xbox\\GtaBerlin.sln in Visual Studio öffnen, Release|x64, Paket erstellen.');
