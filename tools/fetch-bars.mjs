// Bar-Auslastung aus gostumblr (eigener Server) holen und als Schnappschuss web/data/bars.json ablegen – den liest
// das Spiel, wenn der Dev-Server nicht live durchreicht; so bekommt auch die Xbox-Hülle echte Bars mit.
//   npm run bars:fetch                          (https://app.gostumblr.com/api/v1/bars/busyness)
//   BARS_URL=https://… npm run bars:fetch       oder      npm run bars:fetch -- https://…
// Der Feed wird mit demselben Parser wie im Spiel geprüft (web/src/nightlife.js parseBarFeed).
import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { parseBarFeed } from '../web/src/nightlife.js';
import { fetchBars, GOSTUMBLR } from './bars-source.mjs';

const url = process.argv[2] ?? process.env.BARS_URL ?? GOSTUMBLR;
const out = fileURLToPath(new URL('../web/data/bars.json', import.meta.url));
const doc = await fetchBars(url).catch((err) => { console.error(err.message); process.exit(1); });
const feed = parseBarFeed(doc); // wirft, wenn nichts Brauchbares drin ist
// Nur, was das Spiel braucht (keine Adressen, IDs, Links)
const bars = (doc.bars ?? doc).map((b) => ({ name: b.name, latitude: b.latitude ?? b.lat, longitude: b.longitude ?? b.lon ?? b.lng,
  occupancy_percent: b.occupancy_percent ?? null, usual_percent: b.usual_percent ?? null, last_scraped: b.last_scraped ?? null, trend: b.trend ?? [] }));
await writeFile(out, JSON.stringify({ at: doc.at, source: new URL(url).host, weekly: doc.weekly ?? null, bars }));
const n = (f) => feed.bars.filter(f).length;
console.log(`${feed.bars.length} Bars → ${out} (${n((b) => b.lat !== null)} mit Koordinaten, ${n((b) => b.current !== null)} live, ${n((b) => b.week)} mit Stundenprofil${feed.week ? ', Wochenschnitt dabei' : ''})`);
