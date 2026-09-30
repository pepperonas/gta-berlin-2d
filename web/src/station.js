// U-Bahnhöfe (und unterirdische S-Bahnhöfe) zum Betreten – rein rechnerisch, ohne Canvas.
// Die Karte kennt keine Bahnhofsgrundrisse; ein Bahnhof entsteht aus dem Fahrplan (transit.json): alle unterirdischen
// Halte gleichen Namens, deren Strecke gleich verläuft, bilden einen Bahnsteig. Er liegt genau unter der echten Strecke
// (Mittelpunkt, Richtung), ist so lang wie der längste Zug plus Rand, ein Mittelbahnsteig mit je einem Gleis rechts der
// Fahrtrichtung (Rechtsverkehr), Wände mit Fliesen, Säulen in der Mitte, an beiden Enden eine Treppe. Oben an der
// Straße liegen die Eingänge am nächsten Gehweg über den Treppen. Die Spielfigur behält unten ihre echten Koordinaten
// (Ebene −2); Bewegung und Kollision laufen im Bahnhofsrahmen (u entlang der Strecke, v quer, rechts positiv).
// Die Züge sind die Fahrplanzüge (transit.js): wer hält, steht genau am Bahnsteig; einsteigen mit G am Bahnsteigrand.
import { pointOn, positionAt, TRAIN } from './transit.js';
import { railAt } from './tunnel.js';
import { SpatialHash } from './collision.js';
import { nearestSpot, sidewalkPoint } from './pedestrians.js';
import { hash01, nearestEdge, inBuilding, surfaceAt, T } from './map.js';

export const STATION = {
  half: 46,      // px halbe Bahnsteigbreite (Mittelbahnsteig ≈ 9 m)
  track: 64,     // px Gleismitte quer zur Achse
  wall: 92,      // px Wand quer zur Achse
  margin: 45,    // px Bahnsteig über den Zug hinaus je Ende
  stairL: 80,    // px Treppe (Länge entlang der Achse)
  stairW: 38,    // px Treppe (Breite)
  pillar: 7,     // px Säulenradius
  pillarStep: 110,
  edge: 16,      // px: so nah an der Bahnsteigkante lässt sich einsteigen
  entrance: 18,  // px: Radius des Eingangs an der Straße (hineinlaufen)
  reach: 70,     // px: so nah am Eingang reicht E (Aktion) zum Hinuntergehen, und der Hinweis erscheint
  probe: 150,    // px: Bahnsteig gilt als unterirdisch, wenn in diesem Umkreis kein gleichgerichtetes Gleis sichtbar ist
  poiEntrance: 150, // px: Eingang am Bahnhofssymbol (OSM-Station), wenn keiner der Treppeneingänge so nah liegt
  group: 450,    // px: Halte gleichen Namens so nah → derselbe Bahnhof
  near: 1800,    // px: Bahnhöfe um die Kamera (Eingänge zeichnen/prüfen)
};
const TILE_COLORS = ['#d9c27a', '#7fb3a0', '#c77b5e', '#8aa6c9', '#e3ddcc', '#b58db8', '#9fb86a', '#d69a5a'];
const wrapPi = (a) => { a %= Math.PI; if (a < 0) a += Math.PI; return a; };
const angDiff = (a, b) => { const d = Math.abs(wrapPi(a) - wrapPi(b)); return Math.min(d, Math.PI - d); };

// „U Kottbusser Tor (Berlin)“, „S+U Alexanderplatz Bhf (Berlin)“ → „Kottbusser Tor“, „Alexanderplatz“
export function stationName(n) {
  return String(n ?? '').replace(/\s*\(.*\)\s*$/, '').replace(/^(S\+U|U\+S|U|S)\s+/, '').replace(/\s+Bhf\.?$/, '').trim();
}
const keyOf = (n) => stationName(n).toLowerCase();
// Vergleichsschlüssel Fahrplan ↔ OSM: „Boddinstr.“ = „Boddinstraße“, Groß/klein, Bindestriche und Leerzeichen egal
const matchKey = (n) => keyOf(n).replace(/stra(ß|ss)e\b/g, 'str').replace(/str\./g, 'str').replace(/[^a-zäöüß0-9]/g, '');

// Alle U-/S-Bahn-Halte des Fahrplans in einem Raster-Hash (einmal je Fahrplan)
// hash.byKey: alle Halte je Name – ein Bahnhof entsteht immer aus allen seinen Halten, egal von wo man kommt
function stopIndex(tr) {
  if (tr._stopIdx) return tr._stopIdx;
  const hash = new SpatialHash(1600);
  hash.byKey = new Map();
  for (const p of tr.patterns) {
    if (p.mode !== 'ubahn' && p.mode !== 'sbahn') continue;
    p.stops.forEach((s, i) => {
      const q = pointOn(p, s);
      const it = { p, i, x: q.x, y: q.y, angle: q.angle, name: stationName(p.stopNames[i]), key: keyOf(p.stopNames[i]) };
      if (it.key) { hash.insert(it, { x: q.x, y: q.y, w: 0, h: 0 }); if (!hash.byKey.has(it.key)) hash.byKey.set(it.key, []); hash.byKey.get(it.key).push(it); }
    });
  }
  return (tr._stopIdx = hash);
}

// Länge eines Zugs (px) je Art
export const trainLength = (mode) => { const k = TRAIN[mode]; return k ? k.cars * (k.carL + k.gap) - k.gap : 0; };

// Aus Halten gleichen Namens Bahnsteige bilden (gleiche Richtung, nah beieinander). Liefert Bahnhöfe (einen je Bahnsteig).
function buildPlatforms(city, stops) {
  const groups = [];
  for (const st of stops) {
    let g = groups.find((x) => x.key === st.key && angDiff(x.axis, st.angle) < 0.45 && Math.hypot(x.x - st.x, x.y - st.y) < STATION.group);
    if (!g) groups.push(g = { key: st.key, name: st.name, axis: wrapPi(st.angle), x: st.x, y: st.y, stops: [] });
    g.stops.push(st);
  }
  const out = [];
  for (const g of groups) {
    // unterirdisch? Der ganze Bahnsteig jedes Halts (fünf Punkte entlang des Zugs) ohne sichtbares gleichgerichtetes
    // Gleis im weiten Umkreis – ein einzelner Punkt reicht nicht (in großen Bahnhöfen wie Ostkreuz liegt der Linienweg
    // des Fahrplans gelegentlich neben dem Gleis). Nur mit geladenen Kacheln entscheidbar – sonst später noch einmal.
    const under = platformUnderground(city, g.stops);
    if (under === null) { out.push({ pending: true, key: g.key }); continue; }
    if (!under) continue;
    let axis = g.axis; if (Math.cos(axis) < 0) axis -= Math.PI; // Schrift auf den Schildern nicht kopfüber
    const L = Math.max(...g.stops.map((s) => trainLength(s.p.mode)));
    const HL = L / 2 + STATION.margin;
    const ax = Math.cos(axis), ay = Math.sin(axis);
    // Mitte: dort, wo die haltenden Züge ihre Mitte haben (Spitze am Halt, Zug dahinter) – gemittelt über beide Richtungen
    let cx = 0, cy = 0;
    for (const s of g.stops) { const m = pointOn(s.p, s.p.stops[s.i] - trainLength(s.p.mode) / 2); cx += m.x; cy += m.y; }
    cx /= g.stops.length; cy /= g.stops.length;
    const lines = [...new Set(g.stops.map((s) => s.p.name))].sort();
    const modes = new Set(g.stops.map((s) => s.p.mode));
    const id = `${g.key}|${lines.join(',')}|${Math.round(axis * 100)}`;
    const st = {
      id, key: g.key, name: g.name, lines, sbahn: modes.has('sbahn') && !modes.has('ubahn'), x: cx, y: cy, axis, ax, ay, HL, L,
      color: TILE_COLORS[Math.floor(hash01(g.key.length * 131 + g.key.charCodeAt(0) * 7 + (g.key.charCodeAt(1) || 0)) * TILE_COLORS.length)],
      halts: g.stops.map((s) => ({ pid: s.p.id, i: s.i, dir: Math.cos(s.angle - axis) >= 0 ? 1 : -1 })),
    };
    st.exits = [-1, 1].map((e) => entranceFor(city, st, e));
    // dazu ein Eingang am Bahnhofssymbol (dort sucht man ihn), wenn keiner der beiden in der Nähe liegt
    const poi = mainPoi(city, st);
    if (poi && st.exits.every((ex) => Math.hypot(ex.x - poi.x, ex.y - poi.y) > STATION.poiEntrance)) {
      const l = toLocal(st, poi.x, poi.y);
      st.exits.push({ ...entranceAt(city, st, poi.x, poi.y, l.u < 0 ? -1 : 1, true), main: true });
    }
    out.push(st);
  }
  return out;
}

function platformUnderground(city, stops) {
  for (const s of stops) {
    const L = trainLength(s.p.mode);
    for (let k = 0; k <= 4; k++) {
      const q = pointOn(s.p, s.p.stops[s.i] - L * k / 4);
      if (!city.ready(q.x, q.y, STATION.probe)) return null;
      if (railAt(city, q.x, q.y, Math.cos(q.angle), Math.sin(q.angle), STATION.probe)) return false;
    }
  }
  return true;
}

// Die OSM-Station gleichen Namens beim Bahnsteig (U-/S-Symbol auf Karte und Straße)
function mainPoi(city, st) {
  const r = st.HL + 400;
  let best = null, bd = Infinity;
  for (const q of city.poiHash?.query({ x: st.x - r, y: st.y - r, w: 2 * r, h: 2 * r }, []) ?? []) {
    if ((q.cat !== 'ubahn' && q.cat !== 'sbahn') || matchKey(q.name) !== matchKey(st.name)) continue;
    const d = Math.hypot(q.x - st.x, q.y - st.y) + (q.cat === (st.sbahn ? 'sbahn' : 'ubahn') ? 0 : 1e6); // nur die passende Bahnart (S+U: das U-Symbol zum U-Bahnsteig)
    if (d < bd && d < r) { bd = d; best = q; }
  }
  return best;
}

// Eingang oben für das Treppenende e (−1/1): nächster Gehweg über der Treppe, Blick zur Straße
function entranceFor(city, st, e) {
  const u = e * (st.HL - STATION.stairL / 2);
  return entranceAt(city, st, st.x + st.ax * u, st.y + st.ay * u, e);
}
// Eingang am nächsten Gehweg zu (wx, wy); e = Bahnsteigende, an dem man unten ankommt
// here: der Punkt selbst, wenn man dort gehen kann (Platz, Gehweg – nicht Haus oder Fahrbahn)
function entranceAt(city, st, wx, wy, e, here = false) {
  const walkable = here && !inBuilding(city, wx, wy) && [T.SIDEWALK, T.PLAZA, T.GRASS].includes(surfaceAt(city, wx, wy, 0));
  const spot = walkable ? null : nearestSpot(city, wx, wy);
  let p = spot ? sidewalkPoint(city, spot.edge, spot.side, spot.s) : { x: wx, y: wy };
  if (inBuilding(city, p.x, p.y)) p = { x: wx, y: wy };
  const road = nearestEdge(city, p.x, p.y, 400, (x) => x.cls <= 10);
  return { e, x: p.x, y: p.y, face: road ? Math.atan2(road.y - p.y, road.x - p.x) : st.axis, street: road?.e?.name ?? '' };
}

// Bahnhöfe um (x, y): aus dem Zwischenspeicher der Stadt (je Fahrplan), noch Unentschiedenes wird neu versucht
export function stationsNear(city, x, y, r = STATION.near) {
  const tr = city.transit;
  if (!tr) return [];
  if (city._stations?.tr !== tr) city._stations = { tr, byKey: new Map(), byId: new Map() };
  const cache = city._stations, keys = new Set();
  for (const it of stopIndex(tr).query({ x: x - r, y: y - r, w: 2 * r, h: 2 * r }, [])) keys.add(it.key);
  const out = [];
  for (const k of keys) {
    let list = cache.byKey.get(k);
    if (!list || list.some((s) => s.pending)) {
      list = buildPlatforms(city, stopIndex(tr).byKey.get(k) ?? []);
      if (!list.some((s) => s.pending)) { cache.byKey.set(k, list); for (const s of list) cache.byId.set(s.id, s); }
    }
    for (const s of list) if (!s.pending && Math.hypot(s.x - x, s.y - y) < r + s.HL) out.push(s);
  }
  return out;
}
export const stationById = (city, id) => city._stations?.byId.get(id) ?? null;

// Welt → Bahnhofsrahmen (u entlang, v quer, rechts positiv) und zurück
export function toLocal(st, x, y) { const dx = x - st.x, dy = y - st.y; return { u: dx * st.ax + dy * st.ay, v: -dx * st.ay + dy * st.ax }; }
export function toWorld(st, u, v) { return { x: st.x + st.ax * u - st.ay * v, y: st.y + st.ay * u + st.ax * v }; }

// Säulen in der Bahnsteigmitte (nicht an den Treppen): u-Positionen
export function pillars(st) {
  const out = [], lim = st.HL - STATION.stairL - 30;
  for (let u = -Math.floor(lim / STATION.pillarStep) * STATION.pillarStep; u <= lim; u += STATION.pillarStep) if (Math.abs(u) > 40) out.push(u);
  return out;
}

// Spielfigur im Bahnhof halten: Bahnsteig (Kanten), Säulen; r = Radius. Verändert o (x, y).
export function keepInside(st, o, r) {
  const l = toLocal(st, o.x, o.y);
  l.u = Math.max(-st.HL + r, Math.min(st.HL - r, l.u));
  l.v = Math.max(-STATION.half + r, Math.min(STATION.half - r, l.v));
  for (const pu of pillars(st)) {
    const du = l.u - pu, dv = l.v, d = Math.hypot(du, dv), m = STATION.pillar + r;
    if (d < m) { const k = d > 1e-6 ? m / d : 0; l.u = pu + (d > 1e-6 ? du * k : m); l.v = d > 1e-6 ? dv * k : 0; }
  }
  const w = toWorld(st, l.u, l.v); o.x = w.x; o.y = w.y;
  return l;
}

// Steht die Figur auf einer Treppe? → Ende (−1/1) oder 0
export function stairAt(st, x, y) {
  const l = toLocal(st, x, y);
  if (Math.abs(l.v) > STATION.stairW / 2) return 0;
  if (l.u > st.HL - STATION.stairL * 0.45) return 1;
  if (l.u < -st.HL + STATION.stairL * 0.45) return -1;
  return 0;
}
// Ankunftsplatz unten an der Treppe e (Blick in den Bahnsteig)
export function arrivalAt(st, e) { const w = toWorld(st, e * (st.HL - STATION.stairL - 16), 0); return { ...w, angle: st.axis + (e > 0 ? Math.PI : 0) }; }

// Züge am Bahnsteig: für jeden Halt die Fahrplanzüge in der Nähe, als Wagen im Bahnsteigrahmen
// ([{ ref, pid, dir, dwelling, cars: [{ u, v, L, W }], line, dest, color, mode }]). Die Spitze eines haltenden Zugs steht am
// Bahnsteigende in Fahrtrichtung; ein- und ausfahrende gleiten entlang ihres Gleises.
export function trainsAt(w, st) {
  const tr = w.city.transit, out = [];
  if (!tr || !w.transit) return out;
  for (const h of st.halts) {
    const s = w.transit.tracked.get(h.pid);
    if (!s) continue;
    const p = tr.patterns[h.pid], k = TRAIN[p.mode], L = trainLength(p.mode), stopS = p.stops[h.i];
    for (const v of s.veh) {
      if (v.gone || v.live) continue;
      const pos = positionAt(p, v.tau);
      const ds = pos.s - stopS; // Spitze relativ zum Halt
      if (ds < -st.HL - L || ds > st.HL + L) continue;
      const head = h.dir * (L / 2 + ds);
      const cars = [];
      for (let c = 0; c < k.cars; c++) cars.push({ u: head - h.dir * (c * (k.carL + k.gap) + k.carL / 2), v: h.dir * STATION.track, L: k.carL, W: k.W });
      out.push({ ref: { pid: h.pid, key: v.key }, pid: h.pid, i: h.i, dir: h.dir, dwelling: pos.dwelling && pos.stop === h.i && Math.abs(ds) < 1, cars, line: p.name, dest: stationName(p.stopNames[p.stopNames.length - 1]), color: p.color, mode: p.mode });
    }
  }
  return out;
}

// Der Zug, in den man an (x, y) einsteigen kann: hält, auf der eigenen Seite, Figur am Rand neben einem Wagen → { train, car }
export function boardable(w, st, x, y) {
  const l = toLocal(st, x, y);
  if (Math.abs(l.v) < STATION.half - STATION.edge) return null;
  const side = Math.sign(l.v);
  for (const t of trainsAt(w, st)) {
    if (!t.dwelling || t.dir !== side) continue;
    const car = t.cars.findIndex((c) => Math.abs(c.u - l.u) <= c.L / 2 + 4);
    if (car >= 0) return { train: t, car };
  }
  return null;
}

// Nächste Abfahrten je Richtung (Fahrgastinfo): [{ line, dest, min, dir }], je Richtung die zwei nächsten
export function departures(w, st) {
  const tr = w.city.transit, out = [];
  if (!tr || !w.transit) return out;
  for (const h of st.halts) {
    const s = w.transit.tracked.get(h.pid);
    if (!s) continue;
    // Ankunft wie positionAt: Halt i erreicht, wenn die Fahrzeit des Abschnitts ohne den Aufenthalt vorbei ist
    const p = tr.patterns[h.pid], last = h.i === p.off.length - 1;
    const arrive = h.i === 0 ? 0 : p.off[h.i] - (last ? 0 : Math.min(p.dwell, (p.off[h.i] - p.off[h.i - 1]) * 0.4));
    for (const v of s.veh) {
      if (v.gone || v.live) continue;
      const sec = arrive - v.tau;
      if (sec < -p.dwell || sec > 1200) continue;
      out.push({ line: p.name, dest: stationName(p.stopNames[p.stopNames.length - 1]), sec: Math.max(0, sec), dir: h.dir, color: p.color });
    }
  }
  out.sort((a, b) => a.sec - b.sec);
  const per = { '-1': 0, 1: 0 };
  return out.filter((d) => per[d.dir]++ < 2);
}

// Wartende auf dem Bahnsteig (nur Darstellung, aus dem Bahnhof und der Stunde): [{ id, u, v, angle }]
export function waiting(st, hour) {
  const n = 5 + Math.floor(hash01(st.x * 3 + st.y + hour) * 9), out = [];
  for (let k = 0; k < n; k++) {
    const h = (a) => hash01(st.x * 7 + st.y * 13 + k * 101 + a);
    const side = h(1) < 0.5 ? -1 : 1, u = (h(2) * 2 - 1) * (st.HL - STATION.stairL - 40);
    out.push({ id: Math.floor(h(4) * 1e6), u, v: side * (STATION.half - 12 - h(3) * 14), side, angle: side > 0 ? Math.PI / 2 : -Math.PI / 2 });
  }
  return out;
}

// Nächster Eingang (an der Straße) im Umkreis r um (x, y) aus einer Bahnhofsliste: { stn, ex, d } oder null
export function entranceNear(stations, x, y, r = STATION.reach) {
  let best = null;
  for (const stn of stations ?? []) for (const ex of stn.exits) {
    const d = Math.hypot(ex.x - x, ex.y - y);
    if (d < r && (!best || d < best.d)) best = { stn, ex, d };
  }
  return best;
}
