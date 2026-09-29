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

// Berliner Ortszeit eines Unix-Zeitpunkts (s): Wochentag (0 = Mo) und Stunde. Die Spieluhr ist Berliner Zeit, gostumblr
// rechnet seine Stundenwerte ebenfalls in Ortszeit.
const BERLIN = (() => { try { return new Intl.DateTimeFormat('en-GB', { timeZone: 'Europe/Berlin', weekday: 'short', hour: '2-digit', hourCycle: 'h23' }); } catch { return null; } })();
const WD = { Mon: 0, Tue: 1, Wed: 2, Thu: 3, Fri: 4, Sat: 5, Sun: 6 };
export function berlinSlot(epoch) {
  const d = new Date(epoch * 1000);
  if (BERLIN) {
    const parts = Object.fromEntries(BERLIN.formatToParts(d).map((p) => [p.type, p.value]));
    return { dow: WD[parts.weekday], hour: Number(parts.hour) % 24 };
  }
  const u = new Date(d.getTime() + 3600e3); // ohne Intl: MEZ
  return { dow: (u.getUTCDay() + 6) % 7, hour: u.getUTCHours() };
}

// Wochenschnitt aller Bars. gostumblr liefert ihn je Wochentag (GET /api/v1/bars/busyness/weekly?dow=N, 0 = Sonntag):
// [{ dow, hours: [{ hour, avg_occupancy }] }]; im Schnappschuss steht er schon als 7 × 24 (Mo zuerst, 0..1).
export function parseGlobalWeek(v) {
  if (!v) return null;
  if (Array.isArray(v) && v.some((d) => d && typeof d === 'object' && !Array.isArray(d) && d.dow !== undefined)) {
    const week = new Array(7).fill(null);
    for (const d of v) {
      if (!d || !Array.isArray(d.hours)) continue;
      const di = (Number(d.dow) + 6) % 7; // Sonntag zuerst → Montag zuerst
      const row = new Array(24).fill(null);
      for (const h of d.hours) if (h && h.hour >= 0 && h.hour < 24) row[h.hour] = level(h.avg_occupancy);
      if (row.some((x) => x !== null)) week[di] = row;
    }
    return week.some(Boolean) ? week : null;
  }
  return parseWeek(v);
}

// Wochenprofil einer gostumblr-Bar: Form des Wochenschnitts, skaliert mit ihrer Beliebtheit (Googles „üblich“ bzw. der
// Live-Wert gegenüber dem Schnitt zur selben Stunde), darüber die echten Messungen der letzten 24 h (trend) je Stunde.
function barWeek(b, global) {
  let week = null;
  if (global) {
    let ratio = 1;
    const ref = b.usual ?? b.current, slot = b.at !== null ? berlinSlot(b.at) : null, avg = slot ? global[slot.dow]?.[slot.hour] : null;
    if (ref !== null && ref !== undefined && avg) ratio = Math.max(0.3, Math.min(2.5, ref / avg));
    week = global.map((row) => (row ? row.map((v) => (v === null ? null : clamp01(v * ratio))) : null));
  }
  if (b.trend?.length) {
    const sum = new Map();
    for (const [t, pct] of b.trend) {
      const v = level(pct);
      if (v === null || !Number.isFinite(t)) continue;
      const { dow, hour } = berlinSlot(t), k = dow * 24 + hour, o = sum.get(k) ?? [0, 0];
      o[0] += v; o[1]++; sum.set(k, o);
    }
    if (sum.size) {
      week ??= new Array(7).fill(null);
      for (const [k, [a, n]] of sum) { const d = Math.floor(k / 24); week[d] = week[d] ? [...week[d]] : new Array(24).fill(null); week[d][k % 24] = a / n; }
    }
  }
  return week;
}

// Feed → { at (s, Unix), week, bars: [{ name, key, lat, lon, current, usual, week, at }] }. Versteht die Antwort von
// gostumblr (GET /api/v1/bars/busyness: { bars: [{ name, latitude, longitude, occupancy_percent, usual_percent,
// last_scraped, trend: [[epoch, pct]] }] }, dazu optional weekly = Wochenschnitt) und allgemein eine Liste oder
// { bars | venues | data | … } bzw. GeoJSON. Unbrauchbare Einträge (ohne Namen und ohne Koordinaten) fallen weg; wirft
// nur, wenn gar nichts Brauchbares drin ist.
export function parseBarFeed(json) {
  const list = Array.isArray(json) ? json : pick(json, ['bars', 'venues', 'data', 'results', 'items', 'places', 'features']);
  if (!Array.isArray(list)) throw new Error('Bar-Feed: keine Liste gefunden (erwartet Array oder { bars: [...] })');
  const stamp = (v) => { if (v === undefined || v === null) return null; const n = typeof v === 'number' ? v : Date.parse(v) / 1000; return Number.isFinite(n) ? (n > 1e11 ? n / 1000 : n) : null; };
  const at = stamp(pick(json, ['at', 'updated', 'updated_at', 'updatedAt', 'timestamp', 'time'])) ?? null;
  const global = Array.isArray(json) ? null : parseGlobalWeek(pick(json, ['week', 'weekly']));
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
    const current = level(pick(b, ['occupancy_percent', 'current_popularity', 'currentPopularity', 'current', 'live', 'occupancy', 'auslastung', 'load', 'busy', 'busyness', 'level', 'value']));
    const usual = level(pick(b, ['usual_percent', 'usual']));
    const t = stamp(pick(b, ['last_scraped', 'at', 'updated', 'updated_at', 'updatedAt', 'timestamp', 'time'])) ?? at;
    const trend = Array.isArray(b.trend) ? b.trend.filter((p) => Array.isArray(p) && p.length >= 2).map(([e, v]) => [stamp(e), Number(v)]) : null;
    const bar = { name, key: normName(name), lat: hasPos ? lat : null, lon: hasPos ? lon : null, current, usual, at: t, trend };
    bar.week = parseWeek(pick(b, ['populartimes', 'popular_times', 'popularTimes', 'week', 'weekly', 'profile', 'woche'])) ?? barWeek(bar, global);
    bars.push(bar);
  }
  if (!bars.length) throw new Error('Bar-Feed: keine Bar mit Namen oder Koordinaten');
  return { at, week: global, bars };
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
  const live = bar.current ?? bar.usual ?? null; // gostumblr: Live-Wert, sonst Googles „üblich“ zur Messzeit
  const fresh = live !== null && (now === null || bar.at === null || now - bar.at < NIGHT.stale);
  const base = Math.max(typ, typicalLevel('bar', minutes, day));
  return clamp01(base * (fresh ? 0.45 + 1.1 * live : 1.1));
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
