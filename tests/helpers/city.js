// Echte Karte (web/data/berlin/) für Tests: Kacheln werden synchron von der Platte geladen.
// realCity() hält das frühere Kerngebiet (Ortsteile Kreuzberg und Neukölln) fest geladen – einmal pro Testprozess;
// Welten laden ringsum wie im Spiel nach (city.focus).
import { readFileSync } from 'node:fs';
import { openCity } from '../../web/src/map.js';
import { prepareTransit } from '../../web/src/transit.js';
import { existsSync } from 'node:fs';

const dir = new URL('../../web/data/berlin/', import.meta.url);
export const realIndex = () => JSON.parse(readFileSync(new URL('index.json', dir)));
export const realOverview = () => JSON.parse(readFileSync(new URL('overview.json', dir)));
export const tileLoader = () => (k) => JSON.parse(readFileSync(new URL(`tiles/${k}.json`, dir)));
export const openRealCity = () => openCity(realIndex(), tileLoader());
// Fahrplan (transit.json), einmal je Testprozess vorbereitet
let transitCache;
export function realTransit() {
  if (transitCache !== undefined) return transitCache;
  const f = new URL('transit.json', dir);
  return (transitCache = existsSync(f) ? prepareTransit(JSON.parse(readFileSync(f))) : null);
}

// Hüllrechteck von Ortsteilen (px)
export function districtBox(city, names) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const d of city.districts) if (names.includes(d.name)) for (const r of d.rings) for (let i = 0; i < r.length; i += 2) {
    x0 = Math.min(x0, r[i]); x1 = Math.max(x1, r[i]); y0 = Math.min(y0, r[i + 1]); y1 = Math.max(y1, r[i + 1]);
  }
  return [x0, y0, x1, y1];
}

let cached = null;
export function realCity() {
  if (cached) return cached;
  cached = openRealCity();
  cached.transit = realTransit();
  cached.loadArea(...districtBox(cached, ['Kreuzberg', 'Neukölln']), { pin: true });
  return cached;
}

// Geokoordinaten → Karten-px (dieselbe Projektion wie der Build, aus index.json meta)
import { makeProjection } from '../../tools/osm/geo.mjs';
export function geoToPx(meta, lat, lon) {
  const [s, w, n, e] = meta.origin.bbox, S = meta.scale;
  const proj = makeProjection(meta.origin.lat0, meta.origin.lon0);
  const c = [proj(s, w), proj(s, e), proj(n, w), proj(n, e)];
  const minX = Math.min(...c.map((q) => q[0])), maxY = Math.max(...c.map((q) => q[1]));
  const [x, y] = proj(lat, lon);
  return [Math.round((x - minX) * S), Math.round((maxY - y) * S)];
}

// Erreichbarkeit auf einem Raster (Zellweite cell px): Welche Zellen kann ein Kreis mit Radius r von start aus
// erreichen, ohne Hauswände, Zäune, Ufer, Bäume oder Poller zu berühren? Liefert eine Abfrage (x, y) → true/false.
// solidFilter(s) → false lässt ein Hindernis weg (z. B. umfahrbare Poller).
export function reachability(city, box, start, r, { cell = 5, solidFilter = () => true } = {}) {
  const [x0, y0, x1, y1] = box, nx = Math.ceil((x1 - x0) / cell), ny = Math.ceil((y1 - y0) / cell);
  const blocked = new Uint8Array(nx * ny);
  const mark = (bx0, by0, bx1, by1, dist) => {
    const i0 = Math.max(0, Math.floor((bx0 - r - x0) / cell)), i1 = Math.min(nx - 1, Math.floor((bx1 + r - x0) / cell));
    const j0 = Math.max(0, Math.floor((by0 - r - y0) / cell)), j1 = Math.min(ny - 1, Math.floor((by1 + r - y0) / cell));
    for (let j = j0; j <= j1; j++) for (let i = i0; i <= i1; i++) {
      const k = j * nx + i;
      if (!blocked[k] && dist(x0 + (i + 0.5) * cell, y0 + (j + 0.5) * cell) < r) blocked[k] = 1;
    }
  };
  for (const s of city.solids.query({ x: x0, y: y0, w: x1 - x0, h: y1 - y0 }, [])) {
    if (!solidFilter(s)) continue;
    if (s.seg) {
      const dx = s.bx - s.ax, dy = s.by - s.ay, L2 = dx * dx + dy * dy || 1;
      mark(Math.min(s.ax, s.bx), Math.min(s.ay, s.by), Math.max(s.ax, s.bx), Math.max(s.ay, s.by), (x, y) => {
        const t = Math.max(0, Math.min(1, ((x - s.ax) * dx + (y - s.ay) * dy) / L2));
        return Math.hypot(s.ax + dx * t - x, s.ay + dy * t - y);
      });
    } else if (s.r !== undefined) mark(s.x - s.r, s.y - s.r, s.x + s.r, s.y + s.r, (x, y) => Math.hypot(x - s.x, y - s.y) - s.r);
    else if (s.w !== undefined) mark(s.x, s.y, s.x + s.w, s.y + s.h, (x, y) => Math.hypot(Math.max(s.x - x, 0, x - s.x - s.w), Math.max(s.y - y, 0, y - s.y - s.h)));
  }
  const seen = new Uint8Array(nx * ny), q = new Int32Array(nx * ny);
  const idx = (x, y) => { const i = Math.floor((x - x0) / cell), j = Math.floor((y - y0) / cell); return i < 0 || j < 0 || i >= nx || j >= ny ? -1 : j * nx + i; };
  const s0 = idx(start[0], start[1]);
  let head = 0, tail = 0;
  if (s0 >= 0 && !blocked[s0]) { seen[s0] = 1; q[tail++] = s0; }
  while (head < tail) {
    const k = q[head++], i = k % nx, j = (k - i) / nx;
    for (const [di, dj] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
      const a = i + di, b = j + dj;
      if (a < 0 || b < 0 || a >= nx || b >= ny) continue;
      const m = b * nx + a;
      if (!seen[m] && !blocked[m]) { seen[m] = 1; q[tail++] = m; }
    }
  }
  const at = (x, y) => { const k = idx(x, y); return k >= 0 && seen[k] === 1; };
  at.count = tail;
  at.blockedAt = (x, y) => { const k = idx(x, y); return k >= 0 && blocked[k] === 1; };
  return at;
}
