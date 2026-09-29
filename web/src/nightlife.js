// Nachtleben (rein rechnerisch): wie voll eine Bar, Kneipe oder ein Club gerade ist und was man davon an der Kamera hört.
// Grundlage sind die Lokale aus OpenStreetMap (POI-Kategorie 'drink': bar, pub, biergarten, nightclub) mit einem
// typischen Verlauf nach Uhrzeit und Wochentag (rhythm.js nightlife). Liegt ein Auslastungs-Feed vor (gostumblr, vom
// eigenen Server, s. README „Nachtleben“), zählt für die dort genannten Bars dessen Wochenprofil bzw. die gemeldete
// Auslastung – vor genau diesen Bars ist das Nachtleben am deutlichsten zu hören, auch wenn OSM sie nicht kennt.
// Alles hier ist Darstellung: world.rng bleibt unberührt, nur life.js setzt vor vollen Bars mehr Raucher auf den Gehweg.
import { nightlife } from './rhythm.js';

export const NIGHT = {
  hear: 700,        // px (70 m): so weit trägt das Stimmengewirr vor einer vollen Bar
  hearFeed: 900,    // px: Bars aus dem Feed (meist größer, Leute stehen draußen)
  match: 1500,      // px: Feed-Bar mit gleichem Namen darf so weit vom OSM-Punkt liegen
  near: 300,        // px: Feed-Bar ohne Namensgleichheit gilt als dieselbe, wenn sie so nah am OSM-Punkt liegt
  stale: 6 * 3600,  // s: ältere Live-Werte zählen nicht mehr (Wochenprofil bleibt)
  refresh: 300,     // s: so oft holt main.js den Feed neu
};

const wrap = (m) => ((m % 1440) + 1440) % 1440;
const clamp01 = (v) => Math.max(0, Math.min(1, v));
const DAY_NAMES = [['monday', 'montag', 'mo', 'mon'], ['tuesday', 'dienstag', 'di', 'tue'], ['wednesday', 'mittwoch', 'mi', 'wed'],
  ['thursday', 'donnerstag', 'do', 'thu'], ['friday', 'freitag', 'fr', 'fri'], ['saturday', 'samstag', 'sa', 'sat'], ['sunday', 'sonntag', 'so', 'sun']];

// Name vergleichbar machen: klein, ohne Akzente/Satzzeichen, ohne Allerweltswörter („Bar“, „Berlin“, „Kneipe“ …)
export function normName(s) {
  return String(s ?? '').toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ß/g, 'ss')
    .replace(/[^a-z0-9]+/g, ' ').split(' ').filter((w) => w && !['bar', 'the', 'berlin', 'kneipe', 'pub', 'club', 'cafe', 'die', 'der', 'das', 'zum', 'zur'].includes(w)).join(' ');
}

// Auslastung 0..1 aus 0..1 oder Prozent (0..100)
const level = (v) => { const n = Number(v); return Number.isFinite(n) ? clamp01(n > 1 ? n / 100 : n) : null; };
const pick = (o, keys) => { for (const k of keys) if (o?.[k] !== undefined && o[k] !== null) return o[k]; return undefined; };
const dayIndex = (name) => DAY_NAMES.findIndex((a) => a.includes(String(name).toLowerCase().slice(0, 9).trim()));

// Wochenprofil: 7 Tage (Mo zuerst) × 24 Stunden, Werte 0..1. Versteht die gängigen Formen:
// [{ name: 'Monday', data: [24] }] (Google-„Stoßzeiten“), { mo: [24], … }, [[24] × 7] und ein einzelnes [24] für alle Tage.
export function parseWeek(v) {
  if (!v) return null;
  const hours = (a) => (Array.isArray(a) && a.length >= 24 ? a.slice(0, 24).map((x) => level(x) ?? 0) : null);
  const week = new Array(7).fill(null);
  if (Array.isArray(v) && hours(v) && typeof v[0] !== 'object') return week.map(() => hours(v));
  if (Array.isArray(v)) v.forEach((d, i) => {
    if (Array.isArray(d)) week[i] = hours(d);
    else if (d && typeof d === 'object') { const di = d.name !== undefined || d.day !== undefined ? dayIndex(d.name ?? d.day) : i; if (di >= 0 && di < 7) week[di] = hours(d.data ?? d.hours ?? d.values); }
  });
  else if (typeof v === 'object') for (const [k, d] of Object.entries(v)) { const di = /^\d$/.test(k) ? +k : dayIndex(k); if (di >= 0 && di < 7) week[di] = hours(d); }
  return week.some(Boolean) ? week : null;
}

// Feed → { at (s, Unix), bars: [{ name, key, lat, lon, current, week }] }. Nimmt ein Array oder { bars | venues | data | … }.
// Unbrauchbare Einträge (ohne Namen und ohne Koordinaten) fallen weg; wirft nur, wenn gar nichts Brauchbares drin ist.
export function parseBarFeed(json) {
  const list = Array.isArray(json) ? json : pick(json, ['bars', 'venues', 'data', 'results', 'items', 'places', 'features']);
  if (!Array.isArray(list)) throw new Error('Bar-Feed: keine Liste gefunden (erwartet Array oder { bars: [...] })');
  const stamp = (v) => { if (v === undefined || v === null) return null; const n = typeof v === 'number' ? v : Date.parse(v) / 1000; return Number.isFinite(n) ? (n > 1e11 ? n / 1000 : n) : null; };
  const at = stamp(pick(json, ['at', 'updated', 'updated_at', 'updatedAt', 'timestamp', 'time'])) ?? null;
  const bars = [];
  for (const raw of list) {
    if (!raw || typeof raw !== 'object') continue;
    const b = raw.properties ? { ...raw.properties, coordinates: raw.geometry?.coordinates } : raw; // GeoJSON-Feature
    const name = String(pick(b, ['name', 'title', 'bar', 'venue', 'label']) ?? '').trim();
    const pos = pick(b, ['coordinates', 'location', 'geo', 'position', 'coords']) ?? b;
    let lat = Number(pick(pos, ['lat', 'latitude'])), lon = Number(pick(pos, ['lon', 'lng', 'long', 'longitude']));
    if (Array.isArray(pos) && pos.length >= 2) { lon = Number(pos[0]); lat = Number(pos[1]); } // GeoJSON-Reihenfolge
    const hasPos = Number.isFinite(lat) && Number.isFinite(lon) && Math.abs(lat) <= 90 && Math.abs(lon) <= 180 && (lat !== 0 || lon !== 0);
    if (!name && !hasPos) continue;
    const current = level(pick(b, ['current_popularity', 'currentPopularity', 'current', 'live', 'occupancy', 'auslastung', 'load', 'busy', 'busyness', 'level', 'value']));
    const week = parseWeek(pick(b, ['populartimes', 'popular_times', 'popularTimes', 'week', 'weekly', 'profile', 'hours', 'woche']));
    const t = stamp(pick(b, ['at', 'updated', 'updated_at', 'updatedAt', 'timestamp', 'time'])) ?? at;
    bars.push({ name, key: normName(name), lat: hasPos ? lat : null, lon: hasPos ? lon : null, current, week, at: t });
  }
  if (!bars.length) throw new Error('Bar-Feed: keine Bar mit Namen oder Koordinaten');
  return { at, bars };
}

// Feed an die Karte hängen: Koordinaten in px (toPx aus projection.js geoToPx), Namensindex. gen zählt hoch, damit
// zwischengespeicherte Zuordnungen an den POIs neu berechnet werden.
export function attachBars(city, feed, toPx, now = Date.now() / 1000) {
  const bars = feed.bars.map((b, i) => {
    const [x, y] = b.lat !== null && toPx ? toPx(b.lat, b.lon) : [null, null];
    return { ...b, id: i, x, y };
  });
  const byName = new Map();
  for (const b of bars) if (b.key) { if (!byName.has(b.key)) byName.set(b.key, []); byName.get(b.key).push(b); }
  city.bars = { gen: (city.bars?.gen ?? 0) + 1, list: bars, byName, fetched: now, at: feed.at };
  return city.bars;
}

// Die Feed-Bar zu einem OSM-Lokal (oder null). Gleicher Name in der Nähe, sonst eine Feed-Bar direkt daneben.
export function feedBarFor(city, q) {
  const B = city.bars;
  if (!B) return null;
  if (q._barGen === B.gen) return q._bar;
  let best = null, bd = Infinity;
  for (const b of B.byName.get(normName(q.name)) ?? []) {
    const d = b.x === null ? NIGHT.match - 1 : Math.hypot(b.x - q.x, b.y - q.y);
    if (d < NIGHT.match && d < bd) { bd = d; best = b; }
  }
  if (!best) for (const b of B.list) {
    if (b.x === null) continue;
    const d = Math.hypot(b.x - q.x, b.y - q.y);
    if (d < NIGHT.near && d < bd) { bd = d; best = b; }
  }
  if (best) best.osm = true;
  q._barGen = B.gen; q._bar = best;
  return best;
}

// Typischer Verlauf je Lokalart (ohne Feed): Kneipen schon ab dem Feierabend, Biergärten am Abend (im Sommerhalbjahr
// sind sie im Spiel immer offen), Clubs erst ab 23 Uhr und vor allem Freitag-/Samstagnacht
export function typicalLevel(kind, minutes, day) {
  const m = wrap(minutes), night = nightlife(m, day);
  const evening = m >= 1020 && m < 1260 ? (m - 1020) / 240 : m >= 1260 ? 1 : 0; // 17–21 Uhr ansteigend
  switch (kind) {
    case 'nightclub': return m >= 1380 || m < 360 ? clamp01((night - 0.3) / 0.7) : 0;
    case 'pub': return Math.max(night * 0.8, evening * 0.45);
    case 'biergarten': return m >= 720 && m < 1380 ? Math.max(0.25, evening * 0.7) * (day === 5 || day === 6 ? 1 : 0.7) : 0;
    default: return Math.max(night * 0.9, evening * 0.3); // bar
  }
}

// Wochenprofil an Minute m, Tag day (linear zwischen den vollen Stunden)
export function weekLevel(week, minutes, day) {
  const m = wrap(minutes), h = Math.floor(m / 60), f = (m % 60) / 60;
  const a = week[((day % 7) + 7) % 7]?.[h], nd = h === 23 ? (day + 1) % 7 : day, b = week[((nd % 7) + 7) % 7]?.[(h + 1) % 24];
  if (a === undefined || a === null) return null;
  return a + ((b ?? a) - a) * f;
}

// Wie voll ist das Lokal gerade (0..1)? bar = Feed-Eintrag oder null. Mit Wochenprofil gilt es zur Spielzeit; nur eine
// Live-Auslastung (Echtzeit) macht die Bar gegenüber ihresgleichen voller oder leerer, der Tagesverlauf bleibt.
export function barLevel(kind, bar, minutes, day, now = null) {
  const typ = typicalLevel(kind ?? 'bar', minutes, day);
  if (!bar) return typ;
  const wk = bar.week ? weekLevel(bar.week, minutes, day) : null;
  if (wk !== null) return clamp01(wk);
  const fresh = bar.current !== null && (now === null || bar.at === null || now - bar.at < NIGHT.stale);
  const base = Math.max(typ, typicalLevel('bar', minutes, day));
  return clamp01(base * (fresh ? 0.45 + 1.1 * bar.current : 1.1));
}

// Was man vom Nachtleben an (x, y) hört: crowd (Stimmengewirr), music (Bass aus Clubs/Kneipen, gedämpft), pan (−1 links
// … 1 rechts, lauteste Quelle), feed (Anteil, der von Feed-Bars kommt), sources (die hörbaren Lokale, lauteste zuerst).
// front(q) → Stelle vor dem Lokal auf dem Gehweg (life.js frontOf) oder null (dann der POI selbst).
export function nightlifeAt(city, x, y, minutes, day, { front = null, now = null, weather = null } = {}) {
  const R = NIGHT.hearFeed, out = [], seen = new Set();
  const add = (q, kind, bar, hear) => {
    const lvl = barLevel(kind, bar, minutes, day, now);
    if (lvl <= 0.01) return;
    const f = (front && front(q)) ?? q, d = Math.hypot(f.x - x, f.y - y);
    if (d >= hear) return;
    const fall = (1 - d / hear) ** 1.6;
    out.push({ name: q.name ?? bar?.name ?? '', kind, x: f.x, y: f.y, d, lvl, feed: !!bar, gain: lvl * fall * (bar ? 1.35 : 1), club: kind === 'nightclub' });
  };
  for (const q of city.poiHash.query({ x: x - R, y: y - R, w: 2 * R, h: 2 * R }, [])) {
    if (q.cat !== 'drink') continue;
    const bar = feedBarFor(city, q);
    if (bar) seen.add(bar.id);
    add(q, q.kind || 'bar', bar, bar ? NIGHT.hearFeed : NIGHT.hear);
  }
  // Feed-Bars, die OSM (noch) nicht kennt oder deren Kachel noch fehlt: direkt an ihrer Koordinate
  for (const b of city.bars?.list ?? []) {
    if (b.x === null || seen.has(b.id) || Math.abs(b.x - x) > R || Math.abs(b.y - y) > R) continue;
    if (!b._q) b._q = { x: b.x, y: b.y, name: b.name, cat: 'drink', kind: 'bar' };
    add(b._q, 'bar', b, NIGHT.hearFeed);
  }
  out.sort((a, b) => b.gain - a.gain);
  // Regen und Kälte treiben die Leute nach drinnen: draußen wird es leiser, die Musik bleibt
  const wet = clamp01(weather?.rain ?? 0), snow = clamp01(weather?.snow ?? 0), inside = 1 - 0.55 * wet - 0.35 * snow;
  let crowd = 0, music = 0, feed = 0;
  for (const s of out) { crowd += s.gain * (s.club ? 0.6 : 1); music += s.gain * (s.club ? 1 : 0.35); if (s.feed) feed += s.gain; }
  const top = out[0], pan = top ? Math.max(-1, Math.min(1, (top.x - x) / 500)) : 0;
  return { crowd: clamp01(crowd * 0.8 * inside), music: clamp01(music * 0.7), pan, feed: crowd > 0 ? clamp01(feed / crowd) : 0, sources: out };
}
