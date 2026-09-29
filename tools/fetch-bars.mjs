// Bar-Auslastung (gostumblr) vom eigenen Server holen und als Schnappschuss web/data/bars.json ablegen – den liest das
// Spiel, wenn kein Live-Feed (?bars=URL, Konsole „bars URL“) gesetzt ist; auch die Xbox-Hülle bekommt ihn so mit.
//   BARS_URL=https://mein-vps/… npm run bars:fetch      oder      npm run bars:fetch -- https://mein-vps/…
// Der Feed wird mit demselben Parser wie im Spiel geprüft (web/src/nightlife.js parseBarFeed) und normalisiert
// gespeichert: { at, source, bars: [{ name, lat, lon, current, week }] }.
import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { parseBarFeed } from '../web/src/nightlife.js';

const url = process.argv[2] ?? process.env.BARS_URL;
if (!url) { console.error('Aufruf: BARS_URL=https://… npm run bars:fetch   (oder URL als Argument)'); process.exit(1); }
const out = fileURLToPath(new URL('../web/data/bars.json', import.meta.url));
const headers = { accept: 'application/json', ...(process.env.BARS_TOKEN ? { authorization: `Bearer ${process.env.BARS_TOKEN}` } : {}) };
const res = await fetch(url, { headers });
if (!res.ok) { console.error(`${url}: HTTP ${res.status}`); process.exit(1); }
const feed = parseBarFeed(await res.json());
const bars = feed.bars.map(({ name, lat, lon, current, week, at }) => ({ name, lat, lon, current, week, at }));
await writeFile(out, JSON.stringify({ at: feed.at ?? Math.round(Date.now() / 1000), source: new URL(url).host, bars }));
const withPos = bars.filter((b) => b.lat !== null).length, withWeek = bars.filter((b) => b.week).length;
console.log(`${bars.length} Bars → ${out} (${withPos} mit Koordinaten, ${withWeek} mit Wochenprofil, ${bars.filter((b) => b.current !== null).length} mit Live-Wert)`);
