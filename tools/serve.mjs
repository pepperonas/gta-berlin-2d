// Minimaler statischer Server für die Entwicklung (ES-Module brauchen http://, nicht file://).
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { fetchBars, GOSTUMBLR } from './bars-source.mjs';

const root = fileURLToPath(new URL('../web/', import.meta.url));
const port = Number(process.env.PORT ?? 8080);
const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.ogg': 'audio/ogg', '.wav': 'audio/wav', '.mp3': 'audio/mpeg', '.css': 'text/css' };

// Nachtleben: der Server reicht die Bar-Auslastung aus gostumblr (tools/bars-source.mjs) unter /data/bars.json live
// durch (höchstens alle 2 min abgefragt, wie die gostumblr-App selbst) – so braucht gostumblr kein CORS für localhost.
// BARS_URL=https://… nimmt eine andere Quelle, BARS_URL=aus schaltet ab (dann gilt der Schnappschuss, falls vorhanden).
const barsUrl = process.env.BARS_URL === 'aus' ? null : (process.env.BARS_URL ?? GOSTUMBLR);
let barsCache = null;
async function liveBars() {
  if (barsCache && Date.now() - barsCache.t < 120000) return barsCache.body;
  barsCache = { t: Date.now(), body: Buffer.from(JSON.stringify(await fetchBars(barsUrl))) };
  return barsCache.body;
}

createServer(async (req, res) => {
  try {
    if (barsUrl && new URL(req.url, 'http://x').pathname === '/data/bars.json') {
      try { const body = await liveBars(); if (!body) throw new Error('zuletzt nicht erreichbar'); res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-cache' }); res.end(body); return; }
      catch (err) { barsCache = { t: Date.now(), body: null }; console.warn(`Bar-Feed ${barsUrl}: ${err.message} – nehme den Schnappschuss`); }
    }
    let path = normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname)).replace(/^(\.\.[/\\])+/, '');
    let file = join(root, path);
    if (!file.startsWith(root)) throw new Error('outside');
    if ((await stat(file)).isDirectory()) file = join(file, 'index.html');
    const body = await readFile(file);
    res.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-cache' });
    res.end(body);
  } catch {
    res.writeHead(404); res.end('Not found');
  }
}).listen(port, () => console.log(`GTA Berlin läuft auf http://localhost:${port}${barsUrl ? ` (Bar-Feed: ${barsUrl})` : ''}`));
