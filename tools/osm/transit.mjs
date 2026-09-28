// ÖPNV aus dem VBB-Fahrplan (GTFS, CC BY 3.0 „VBB Verkehrsverbund Berlin-Brandenburg GmbH“) → web/data/berlin/transit.json.
// Je Linie und Fahrtverlauf („Muster“: Linie + Linienweg + Haltestellenfolge) bleiben: der Weg (projiziert wie die Karte,
// vereinfacht, auf das Kartengebiet gekürzt), die Halte mit ihrer Lage auf dem Weg, die Fahrzeiten zwischen den Halten
// (Median über alle Fahrten) und die Abfahrtszeiten an einem Werktag, Samstag und Sonntag. Das Spiel rechnet daraus den
// Takt je Uhrzeit; einzelne Fahrten braucht es nicht (die Spieluhr läuft 60× schneller als die Fahrzeuge).
//   node tools/osm/transit.mjs [data/raw/gtfs.zip] [web/data/berlin]
import { readFileSync, writeFileSync } from 'node:fs';
import { listZip, zipCsv } from './zip.mjs';
import { makeProjection, simplify, delta } from './geo.mjs';
import { pointInRing } from '../../web/src/geom.js';

export const TRANSIT_ATTRIBUTION = 'Fahrplandaten: VBB Verkehrsverbund Berlin-Brandenburg GmbH (CC BY 3.0)';
// GTFS-Verkehrsmittel (auch die erweiterten Typen des VBB) → Spielart
export function modeOf(type) {
  const t = +type;
  if (t === 3 || (t >= 700 && t < 800)) return 'bus';
  if (t === 0 || t === 900) return 'tram';
  if (t === 109) return 'sbahn';
  if (t === 1 || t === 400) return 'ubahn';
  return null; // Regionalbahn, Fähre, Rufbus u. a. bleiben draußen
}

// Projektion wie der Karten-Build (tools/osm/build.mjs) aus index.json meta.origin
export function projectionFromMeta(meta) {
  const { lat0, lon0, bbox: [s, w, n, e] } = meta.origin, S = meta.scale;
  const proj = makeProjection(lat0, lon0);
  const corners = [proj(s, w), proj(s, e), proj(n, w), proj(n, e)];
  const minX = Math.min(...corners.map((c) => c[0])), maxY = Math.max(...corners.map((c) => c[1]));
  return (lat, lon) => { const [x, y] = proj(lat, lon); return [Math.round((x - minX) * S), Math.round((maxY - y) * S)]; };
}

const secs = (t) => { const [h, m, s] = t.split(':').map(Number); return h * 3600 + m * 60 + (s || 0); };
const ymd = (d) => `${d.getUTCFullYear()}${String(d.getUTCMonth() + 1).padStart(2, '0')}${String(d.getUTCDate()).padStart(2, '0')}`;
const parseYmd = (s) => new Date(Date.UTC(+s.slice(0, 4), +s.slice(4, 6) - 1, +s.slice(6, 8)));
const WEEKDAYS = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday'];

// Kandidaten für Stichtage: die ersten vier Dienstage, Samstage und Sonntage ab einer Woche nach Fahrplanbeginn
export function candidateDates(calendar) {
  let start = '99999999';
  for (const c of calendar) if (c.start_date < start) start = c.start_date;
  const d0 = parseYmd(start === '99999999' ? '20260101' : start);
  d0.setUTCDate(d0.getUTCDate() + 7);
  const four = (dow) => { const d = new Date(d0), out = []; while (d.getUTCDay() !== dow) d.setUTCDate(d.getUTCDate() + 1); for (let i = 0; i < 4; i++) { out.push(ymd(d)); d.setUTCDate(d.getUTCDate() + 7); } return out; };
  return { w: four(2), a: four(6), u: four(0) };
}

// Stichtag je Tagesart: der „normalste“ Kandidat = der mit den meisten Fahrten (Feiertage und Bauarbeiten dünnen aus)
export function pickDates(candidates, calendar, calendarDates, tripsPerService) {
  const out = {};
  for (const [k, list] of Object.entries(candidates)) {
    let best = list[0], bestN = -1;
    for (const date of list) {
      const act = serviceDays(calendar, calendarDates, { [k]: date });
      let n = 0;
      for (const [id, c] of tripsPerService) if (act.get(id)?.has(k)) n += c;
      if (n > bestN) { bestN = n; best = date; }
    }
    out[k] = best;
  }
  return out;
}
// (für Tests und einfache Fälle) nur die ersten Kandidaten
export function sampleDates(calendar) { const c = candidateDates(calendar); return { w: c.w[0], a: c.a[0], u: c.u[0] }; }

export function serviceDays(calendar, calendarDates, dates) {
  const active = new Map(); // service_id → Set('w','a','u')
  const set = (id, k, on) => { let s = active.get(id); if (!s) active.set(id, s = new Set()); on ? s.add(k) : s.delete(k); };
  for (const c of calendar) for (const [k, date] of Object.entries(dates)) {
    const dow = WEEKDAYS[parseYmd(date).getUTCDay()];
    if (c[dow] === '1' && date >= c.start_date && date <= c.end_date) set(c.service_id, k, true);
  }
  for (const x of calendarDates) for (const [k, date] of Object.entries(dates)) if (x.date === date) set(x.service_id, k, x.exception_type === '1');
  return active;
}

// Lage der Halte auf dem Weg (fortschreitend, damit Schleifen nicht zurückspringen)
export function stopDistances(shape, stops) {
  const cum = [0];
  for (let i = 2; i < shape.length; i += 2) cum.push(cum[cum.length - 1] + Math.hypot(shape[i] - shape[i - 2], shape[i + 1] - shape[i - 1]));
  const out = [];
  let seg = 0;
  for (const [x, y] of stops) {
    let best = Infinity, bestD = cum[seg] ?? 0, bestSeg = seg;
    for (let i = seg; i < cum.length - 1; i++) {
      const ax = shape[2 * i], ay = shape[2 * i + 1], dx = shape[2 * i + 2] - ax, dy = shape[2 * i + 3] - ay, L2 = dx * dx + dy * dy || 1;
      const t = Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / L2));
      const d2 = (ax + dx * t - x) ** 2 + (ay + dy * t - y) ** 2;
      if (d2 < best) { best = d2; bestD = cum[i] + t * Math.sqrt(L2); bestSeg = i; }
      if (best < 400 && i > bestSeg + 40) break; // gefunden und weit genug gesucht
    }
    out.push(Math.max(Math.round(bestD), out.length ? out[out.length - 1] : 0)); seg = bestSeg; // nie hinter den vorigen Halt
  }
  return { dists: out, length: cum[cum.length - 1] };
}

// Teilstück eines Linienzugs zwischen den Bogenlängen s0 und s1
function cut(pts, s0, s1) {
  const out = [];
  let acc = 0;
  for (let i = 0; i < pts.length - 2; i += 2) {
    const ax = pts[i], ay = pts[i + 1], bx = pts[i + 2], by = pts[i + 3], L = Math.hypot(bx - ax, by - ay);
    const a = Math.max(s0, acc), b = Math.min(s1, acc + L);
    if (b > a && L > 0) {
      if (!out.length) out.push(Math.round(ax + (bx - ax) * (a - acc) / L), Math.round(ay + (by - ay) * (a - acc) / L));
      out.push(Math.round(ax + (bx - ax) * (b - acc) / L), Math.round(ay + (by - ay) * (b - acc) / L));
    }
    acc += L;
  }
  return out;
}

// Abfahrtslisten: eindimensional differenzkodiert (geom.delta ist für x/y-Paare)
export const delta1 = (a) => a.map((v, i) => (i ? v - a[i - 1] : v));
export const undelta1 = (a) => { const o = []; for (let i = 0; i < a.length; i++) o.push(i ? o[i - 1] + a[i] : a[i]); return o; };
const median = (a) => { const s = a.slice().sort((x, y) => x - y); return s[s.length >> 1]; };

export async function buildTransit(zip, index, { log = () => {} } = {}) {
  const entries = listZip(zip), toPx = projectionFromMeta(index.meta), S = index.meta.scale;
  const W = index.meta.width, H = index.meta.height;
  const border = index.border.map((r) => { const o = []; let x = 0, y = 0; for (let i = 0; i < r.length; i += 2) { x += r[i]; y += r[i + 1]; o.push(x, y); } return o; });
  const inBerlin = (x, y) => x >= 0 && y >= 0 && x <= W && y <= H && border.some((r) => pointInRing(x, y, r));
  const rows = async (name) => { const out = []; for await (const r of zipCsv(zip, name, entries)) out.push(r); return out; };

  const routes = new Map();
  for (const r of await rows('routes.txt')) { const mode = modeOf(r.route_type); if (mode) routes.set(r.route_id, { name: r.route_short_name || r.route_long_name, mode, color: r.route_color || '' }); }
  const calendar = entries.has('calendar.txt') ? await rows('calendar.txt') : [];
  const calendarDates = entries.has('calendar_dates.txt') ? await rows('calendar_dates.txt') : [];
  const allTrips = [], perService = new Map();
  for await (const t of zipCsv(zip, 'trips.txt', entries)) {
    if (!routes.has(t.route_id)) continue;
    allTrips.push(t); perService.set(t.service_id, (perService.get(t.service_id) ?? 0) + 1);
  }
  const cands = candidateDates(calendar.length ? calendar : calendarDates.map((c) => ({ start_date: c.date })));
  const dates = pickDates(cands, calendar, calendarDates, perService);
  const service = serviceDays(calendar, calendarDates, dates);
  const trips = new Map();
  for (const t of allTrips) {
    const days = service.get(t.service_id);
    if (!days || !days.size) continue;
    trips.set(t.trip_id, { route: t.route_id, shape: t.shape_id, days, times: [], stops: [] });
  }
  log(`  ${routes.size} Linien, ${trips.size} Fahrten an den Stichtagen ${Object.values(dates).join('/')}`);
  const stops = new Map();
  for await (const s of zipCsv(zip, 'stops.txt', entries)) {
    const [x, y] = toPx(+s.stop_lat, +s.stop_lon);
    stops.set(s.stop_id, { x, y, name: s.stop_name });
  }
  for await (const st of zipCsv(zip, 'stop_times.txt', entries)) {
    const t = trips.get(st.trip_id);
    if (!t) continue;
    t.stops.push([+st.stop_sequence, st.stop_id, secs(st.departure_time || st.arrival_time)]);
  }
  // Muster bilden
  const patterns = new Map();
  for (const t of trips.values()) {
    if (t.stops.length < 2) continue;
    t.stops.sort((a, b) => a[0] - b[0]);
    const key = `${t.route}|${t.shape}|${t.stops.map((s) => s[1]).join(',')}`;
    let p = patterns.get(key);
    if (!p) patterns.set(key, p = { route: t.route, shape: t.shape, stops: t.stops.map((s) => s[1]), rel: t.stops.map(() => []), deps: { w: [], a: [], u: [] } });
    const t0 = t.stops[0][2];
    t.stops.forEach((s, i) => p.rel[i].push(s[2] - t0));
    for (const d of t.days) p.deps[d].push(Math.round(t0 / 60));
  }
  // Nur Muster mit mindestens zwei Halten in Berlin; Wege dazu einlesen
  const want = new Map();
  for (const p of patterns.values()) {
    const inside = p.stops.map((id) => { const s = stops.get(id); return s ? inBerlin(s.x, s.y) : false; });
    const first = inside.indexOf(true), last = inside.lastIndexOf(true);
    if (first < 0 || last - first < 1) { p.drop = true; continue; }
    p.first = first; p.last = last;
    want.set(p.shape, null);
  }
  for await (const r of zipCsv(zip, 'shapes.txt', entries)) {
    if (!want.has(r.shape_id)) continue;
    let a = want.get(r.shape_id);
    if (!a) want.set(r.shape_id, a = []);
    a.push([+r.shape_pt_sequence, ...toPx(+r.shape_pt_lat, +r.shape_pt_lon)]);
  }
  // Ausgabe
  const lines = [], lineIx = new Map(), shapes = [], shapeIx = new Map(), names = [], nameIx = new Map(), out = [];
  const nameOf = (n) => { if (!nameIx.has(n)) { nameIx.set(n, names.length); names.push(n); } return nameIx.get(n); };
  for (const p of patterns.values()) {
    if (p.drop) continue;
    const raw = want.get(p.shape);
    const stopPts = p.stops.map((id) => { const s = stops.get(id); return [s.x, s.y]; });
    let full;
    if (raw && raw.length >= 2) { raw.sort((a, b) => a[0] - b[0]); full = raw.flatMap((q) => [q[1], q[2]]); }
    else full = stopPts.flat(); // ohne Linienweg: gerade von Halt zu Halt
    const { dists } = stopDistances(full, stopPts);
    const s0 = dists[p.first], s1 = dists[p.last];
    if (s1 - s0 < 20 * S) continue;
    const path = simplify(cut(full, s0, s1), 1.5 * S);
    // vereinfachter Weg ist etwas kürzer: Halte anteilig mitziehen, damit der letzte nicht hinter dem Wegende liegt
    let plen = 0; for (let i = 2; i < path.length; i += 2) plen += Math.hypot(path[i] - path[i - 2], path[i + 1] - path[i - 1]);
    const k = plen / (s1 - s0);
    const key = path.join(',');
    if (!shapeIx.has(key)) { shapeIx.set(key, shapes.length); shapes.push(delta(path)); }
    const r = routes.get(p.route), lk = `${r.name}|${r.mode}|${r.color}`;
    if (!lineIx.has(lk)) { lineIx.set(lk, lines.length); lines.push([r.name, r.mode, r.color]); }
    const idx = []; for (let i = p.first; i <= p.last; i++) idx.push(i);
    const base = median(p.rel[p.first]);
    const off = idx.map((i) => Math.max(0, median(p.rel[i]) - base));
    for (let i = 1; i < off.length; i++) off[i] = Math.max(off[i], off[i - 1]); // Fahrzeiten nie rückwärts
    const shift = Math.round(base / 60);
    const dep = (k) => delta1(p.deps[k].map((m) => m + shift).sort((a, b) => a - b));
    out.push({ l: lineIx.get(lk), s: shapeIx.get(key), st: idx.map((i) => Math.floor((dists[i] - s0) * k)), sn: idx.map((i) => nameOf(stops.get(p.stops[i]).name)), off, d: [dep('w'), dep('a'), dep('u')] });
  }
  out.sort((a, b) => a.l - b.l || a.s - b.s || (a.d[0][0] ?? 0) - (b.d[0][0] ?? 0));
  log(`  ${out.length} Fahrtverläufe, ${shapes.length} Linienwege, ${lines.length} Linien`);
  return { v: 1, attribution: TRANSIT_ATTRIBUTION, dates, lines, shapes, names, patterns: out };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const zip = process.argv[2] ?? 'data/raw/gtfs.zip', dir = process.argv[3] ?? 'web/data/berlin';
  const index = JSON.parse(readFileSync(`${dir}/index.json`, 'utf8'));
  const t0 = Date.now();
  const t = await buildTransit(zip, index, { log: console.log });
  const json = JSON.stringify(t);
  writeFileSync(`${dir}/transit.json`, json);
  console.log(`transit.json: ${(json.length / 1e6).toFixed(1)} MB in ${((Date.now() - t0) / 1000).toFixed(0)} s`);
}
