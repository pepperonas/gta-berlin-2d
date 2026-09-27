// Sichtprüfung: rendert web/data/city.json als SVG (Ausschnitt optional).
//   node tools/osm/preview.mjs out.svg [x y w h]   (px-Koordinaten der Karte)
import { readFile, writeFile } from 'node:fs/promises';
import { undelta } from './geo.mjs';

const [out = 'preview.svg', ...rect] = process.argv.slice(2);
const c = JSON.parse(await readFile(new URL('../../web/data/city.json', import.meta.url)));
const [X, Y, Wd, Ht] = rect.length === 4 ? rect.map(Number) : [0, 0, c.meta.width, c.meta.height];
const k = 1600 / Wd;
const d = (pts, close) => { let s = ''; for (let i = 0; i < pts.length; i += 2) s += (i ? 'L' : 'M') + ((pts[i] - X) * k).toFixed(1) + ' ' + ((pts[i + 1] - Y) * k).toFixed(1); return s + (close ? 'Z' : ''); };
const vis = (pts) => { for (let i = 0; i < pts.length; i += 2) if (pts[i] > X && pts[i] < X + Wd && pts[i + 1] > Y && pts[i + 1] < Y + Ht) return true; return false; };
const parts = [`<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="${Math.round(Ht * k)}" style="background:#a19d95">`];
const AREA = ['#7c7a78', '#8f8c86', '#8fae6e', '#7aa35e', '#5d9340', '#6aa84f', '#d8c98c', '#3f7a32'];
for (const [kind, rings] of c.areas) { const ds = rings.map(([, r]) => undelta(r)).filter(vis).map((r) => d(r, true)).join(''); if (ds) parts.push(`<path d="${ds}" fill="${AREA[kind]}" fill-rule="evenodd"/>`); }
for (const rings of c.water) { const ds = rings.map(([, r]) => undelta(r)).filter(vis).map((r) => d(r, true)).join(''); if (ds) parts.push(`<path d="${ds}" fill="#2c6c98" fill-rule="evenodd"/>`); }
const V = c.vertices;
for (const [a, b, cls, w, , , br, inside, p] of c.edges) {
  const pts = [V[2 * a], V[2 * a + 1], ...undelta(p), V[2 * b], V[2 * b + 1]];
  if (!vis(pts)) continue;
  parts.push(`<path d="${d(pts)}" stroke="${br ? '#777' : inside ? '#3b3e43' : '#555'}" stroke-width="${Math.max(0.6, w * k)}" fill="none" stroke-linecap="round"/>`);
}
for (const [, p] of c.paths) { const pts = undelta(p); if (vis(pts)) parts.push(`<path d="${d(pts)}" stroke="#c9b99a" stroke-width="${Math.max(0.4, 20 * k)}" fill="none"/>`); }
for (const [br, sub, p] of c.rails) { const pts = undelta(p); if (vis(pts)) parts.push(`<path d="${d(pts)}" stroke="${br ? '#4a4038' : '#6b5e50'}" stroke-width="${Math.max(0.6, (sub ? 80 : 50) * k)}" fill="none"/>`); }
for (const [h, kind, rings] of c.buildings) {
  const rs = rings.map(undelta); if (!vis(rs[0])) continue;
  const col = ['#c9b79c', '#a3b1a0', '#8f9aa6', '#c4a484', '#b3aa9a', '#e0c030', '#ff6600'][kind];
  parts.push(`<path d="${rs.map((r) => d(r, true)).join('')}" fill="${col}" fill-rule="evenodd" stroke="#555" stroke-width="0.3"/>`);
}
for (const wl of c.walls) { const pts = undelta(wl); if (vis(pts)) parts.push(`<path d="${d(pts)}" stroke="#f0f" stroke-width="1" fill="none"/>`); }
for (const b of c.border) parts.push(`<path d="${d(undelta(b), true)}" stroke="#e00" stroke-width="2" fill="none"/>`);
for (const [key, p] of Object.entries(c.places)) if (p && p.x !== undefined) parts.push(`<circle cx="${(p.x - X) * k}" cy="${(p.y - Y) * k}" r="4" fill="#ff0"/><text x="${(p.x - X) * k + 5}" y="${(p.y - Y) * k}" font-size="10">${key}</text>`);
parts.push('</svg>');
await writeFile(out, parts.join('\n'));
console.log(out, (parts.join('').length / 1e6).toFixed(1), 'MB');
