// Fahrspurgraph für den KI-Verkehr aus dem Straßengraphen der Karte.
// Rechtsverkehr: Spuren je Richtung aus dem Straßenquerschnitt (street.js laneOffsets).
// An Kreuzungen werden die Spuren gekürzt; Abbiegeverbinder sind kubische Bézierkurven.
import { offsetPolyline, polylineLength } from './geom.js';
import { SpatialHash } from './collision.js';
import { TRAFFIC_MAX_CLASS, ROAD_CLASS } from './citycodes.js';
import { laneOffsets } from './street.js';

// Reisetempo aus dem Tempolimit (km/h → px/s bei 10 px = 1 m), Spielstraßen nicht unter 30 px/s.
export const cruiseFor = (kmh) => Math.max(30, kmh / 3.6 * 10);

export const drivable = (e) => e.inside && (e.cls <= TRAFFIC_MAX_CLASS || e.cls === ROAD_CLASS.busway) && e.len > 5 && !e.blocked && !e.passage;

export function cutPolyline(pts, s0, s1) { // Teilstück zwischen den Bogenlängen s0 und s1
  const out = [];
  let acc = 0;
  for (let i = 0; i < pts.length - 2; i += 2) {
    const ax = pts[i], ay = pts[i + 1], bx = pts[i + 2], by = pts[i + 3];
    const L = Math.hypot(bx - ax, by - ay);
    const a = Math.max(s0, acc), b = Math.min(s1, acc + L);
    if (b > a && L > 0) {
      const ta = (a - acc) / L, tb = (b - acc) / L;
      if (!out.length) out.push(ax + (bx - ax) * ta, ay + (by - ay) * ta);
      out.push(ax + (bx - ax) * tb, ay + (by - ay) * tb);
    }
    acc += L;
  }
  return out;
}

// Spurgraph, mitwachsend mit den nachgeladenen Kacheln: Spuren entstehen, wenn ihre Kante geladen wird, und
// verschwinden mit ihr. Nachfolger (lane.next) werden bei Bedarf berechnet und bis zum nächsten Nachladen gemerkt.
export function buildLaneGraph(city) {
  if (city.lanes) return city.lanes;
  const g = { lanes: new Set(), hash: new SpatialHash(320), out: new Map(), byEdge: new Map() };
  city.lanes = g;
  const add = (e) => {
    if (!drivable(e)) return;
    const made = makeLanes(city, e, g);
    if (!made.length) return;
    g.byEdge.set(e.id, made);
    for (const l of made) {
      g.lanes.add(l);
      addOut(g.out, l.from, l);
      l.cells = [];
      for (let i = 0; i < l.pts.length - 2; i += 2) {
        const ax = l.pts[i], ay = l.pts[i + 1], bx = l.pts[i + 2], by = l.pts[i + 3];
        const sg = { lane: l, i, ax, ay, bx, by };
        l.cells.push(sg, g.hash.insert(sg, { x: Math.min(ax, bx), y: Math.min(ay, by), w: Math.abs(bx - ax), h: Math.abs(by - ay) }));
      }
    }
  };
  const remove = (e) => {
    const made = g.byEdge.get(e.id);
    if (!made) return;
    g.byEdge.delete(e.id);
    for (const l of made) {
      g.lanes.delete(l);
      const list = g.out.get(l.from);
      if (list) { list.splice(list.indexOf(l), 1); if (!list.length) g.out.delete(l.from); }
      for (let i = 0; i < l.cells.length; i += 2) g.hash.remove(l.cells[i], l.cells[i + 1]);
      l.removed = true;
    }
  };
  city.hooks.edgeAdd.push(add);
  city.hooks.edgeRemove.push(remove);
  for (const e of [...city.edges.values()].sort((a, b) => a.id - b.id)) add(e);
  return g;
}

// Spurlisten je Knoten nach Kante und Spur sortiert: gleiche Nachfolger, egal in welcher Folge Kacheln laden.
function addOut(out, n, l) {
  let list = out.get(n);
  if (!list) out.set(n, list = []);
  let i = list.length;
  while (i > 0 && (list[i - 1].edge.id > l.edge.id || (list[i - 1].edge.id === l.edge.id && list[i - 1].key > l.key))) i--;
  list.splice(i, 0, l);
}

let laneId = 0;
function makeLanes(city, e, g) {
  const S = city.scale, lo = laneOffsets(e.cs, S), made = [];
  const trimAt = (n) => city.nodes.get(n)?.trim ?? 0; // vom Karten-Build: größte halbe Breite + 1 m an Kreuzungen
  for (const dir of [1, -1]) {
    const n = dir === 1 ? e.cs.fwd : e.cs.bwd;
    if (!n) continue;
    const base = dir === 1 ? e.pts : reverse(e.pts);
    const from = dir === 1 ? e.a : e.b, to = dir === 1 ? e.b : e.a;
    for (let k = 0; k < n; k++) {
      // Querlage aus dem Querschnitt (rechts positiv in Kantenrichtung; bei Gegenrichtung gespiegelt)
      const off = dir === 1 ? lo.fwd[k] : -lo.bwd[k];
      const raw = offsetPolyline(base, off);
      const L = polylineLength(raw);
      let s0 = trimAt(from), s1 = L - trimAt(to);
      if (s1 - s0 < L * 0.3) { const m = L / 2; s0 = Math.min(s0, m - L * 0.15); s1 = Math.max(s1, m + L * 0.15); }
      const pts = cutPolyline(raw, s0, s1);
      if (pts.length < 4) continue;
      // eng: Gegenverkehr teilt sich die Fahrbahnmitte (Engstelle, man muss einander durchlassen)
      made.push(new Lane(city, g, { id: laneId++, key: (dir === 1 ? 0 : 100) + k, edge: e, dir, k, n, from, to, pts, len: polylineLength(pts), cruise: cruiseFor(e.cs.maxspeed), narrow: lo.narrow && e.cs.fwd > 0 && e.cs.bwd > 0, busOnly: e.cls === ROAD_CLASS.busway }));
    }
  }
  // Gegenbusspur in Einbahnstraßen (oneway:bus=no): eigene Spur nur für Busse am linken Fahrbahnrand
  if (e.cs.busContra && (e.cs.fwd === 0) !== (e.cs.bwd === 0)) {
    const dir = e.cs.fwd ? -1 : 1, base = dir === 1 ? e.pts : reverse(e.pts);
    const from = dir === 1 ? e.a : e.b, to = dir === 1 ? e.b : e.a;
    const raw = offsetPolyline(base, Math.max(1.4 * S, e.cs.width / 2 - 1.6 * S));
    const L = polylineLength(raw);
    let s0 = trimAt(from), s1 = L - trimAt(to);
    if (s1 - s0 < L * 0.3) { const m = L / 2; s0 = Math.min(s0, m - L * 0.15); s1 = Math.max(s1, m + L * 0.15); }
    const pts = cutPolyline(raw, s0, s1);
    if (pts.length >= 4) made.push(new Lane(city, g, { id: laneId++, key: dir === 1 ? 50 : 150, edge: e, dir, k: 0, n: 1, from, to, pts, len: polylineLength(pts), cruise: cruiseFor(e.cs.maxspeed), narrow: false, busOnly: true }));
  }
  return made;
}

class Lane {
  constructor(city, g, props) { Object.assign(this, props); this._city = city; this._g = g; this._gen = -1; this._next = null; this._genB = -1; this._nextB = null; }
  // Nachfolger für den allgemeinen Verkehr (ohne Busspuren); auf einer Busspur selbst sind alle erlaubt
  get next() {
    if (this._gen !== this._city.gen) { this._next = nextLanes(this, this._g, this._city.turnBans, !!this.busOnly); this._gen = this._city.gen; }
    return this._next;
  }
  // Nachfolger für Linienbusse (mit Busspuren und Gegenbusspuren)
  get nextBus() {
    if (this._genB !== this._city.gen) { this._nextB = nextLanes(this, this._g, this._city.turnBans, true); this._genB = this._city.gen; }
    return this._nextB;
  }
}

function nextLanes(l, g, banned, withBus = false) {
  const at = (g.out.get(l.to) ?? []).filter((m) => withBus || !m.busOnly);
  const all = at.filter((m) => m.edge !== l.edge && !banned.has(`${l.edge.id}>${l.to}>${m.edge.id}`));
  // Rechts abbiegen nur von der äußersten, links nur von der innersten Spur; geradeaus spurtreu.
  const ok = all.filter((m) => {
    const a = turnAngle(l, m);
    if (a > 0.5) return l.k === l.n - 1 && m.k === m.n - 1;
    if (a < -0.5) return l.k === 0 && m.k === 0;
    return m.k === Math.min(l.k, m.n - 1);
  });
  if (ok.length) return ok;
  if (all.length) return all;
  const any = at.filter((m) => m !== l && !banned.has(`${l.edge.id}>${l.to}>${m.edge.id}`));
  return any.length ? any : at.filter((m) => m !== l);
}

function reverse(pts) { const r = []; for (let i = pts.length - 2; i >= 0; i -= 2) r.push(pts[i], pts[i + 1]); return r; }

export function laneDir(l, atEnd) {
  const p = l.pts, n = p.length;
  const [ax, ay, bx, by] = atEnd ? [p[n - 4], p[n - 3], p[n - 2], p[n - 1]] : [p[0], p[1], p[2], p[3]];
  const L = Math.hypot(bx - ax, by - ay) || 1;
  return [(bx - ax) / L, (by - ay) / L];
}

// Abbiegewinkel (Radiant, 0 = geradeaus) von Spur a auf Spur b.
export function turnAngle(a, b) {
  const [ux, uy] = laneDir(a, true), [vx, vy] = laneDir(b, false);
  return Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
}

// Verbinder-Punkte vom Ende von a zum Anfang von b (ohne Endpunkte).
export function connector(a, b) {
  const p = a.pts, n = p.length;
  const x0 = p[n - 2], y0 = p[n - 1], x3 = b.pts[0], y3 = b.pts[1];
  const d = Math.hypot(x3 - x0, y3 - y0);
  if (d < 4) return [];
  const [ux, uy] = laneDir(a, true), [vx, vy] = laneDir(b, false);
  const k = Math.max(d * 0.45, 12);
  const x1 = x0 + ux * k, y1 = y0 + uy * k, x2 = x3 - vx * k, y2 = y3 - vy * k;
  const pts = [];
  const steps = Math.max(2, Math.min(8, Math.round(d / 25)));
  for (let s = 1; s < steps; s++) {
    const t = s / steps, u = 1 - t;
    pts.push(u * u * u * x0 + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t * t * t * x3,
      u * u * u * y0 + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t * t * t * y3);
  }
  return pts;
}

// Nächste Spur wählen: lieber geradeaus, Wenden nur in Sackgassen.
export function chooseNext(lane, rng) {
  const opts = lane.next;
  if (!opts.length) return null;
  let total = 0;
  const w = opts.map((m) => { const c = Math.cos(turnAngle(lane, m)); const x = 0.4 + Math.max(0, c) * 2.2; total += x; return x; });
  let r = rng() * total;
  for (let i = 0; i < opts.length; i++) if ((r -= w[i]) <= 0) return opts[i];
  return opts[opts.length - 1];
}

// Nächste Spur zu einer Position; Richtung zählt mit (Winkelabweichung in px bewertet).
export function nearestLane(graph, x, y, angle, radius = 400, allowBus = false) {
  const box = { x: x - radius, y: y - radius, w: 2 * radius, h: 2 * radius };
  let best = null;
  const ca = Math.cos(angle), sa = Math.sin(angle);
  for (const s of graph.hash.query(box, [])) {
    if (s.lane.busOnly && !allowBus) continue;
    const dx = s.bx - s.ax, dy = s.by - s.ay, L2 = dx * dx + dy * dy, L = Math.sqrt(L2) || 1;
    let t = L2 ? ((x - s.ax) * dx + (y - s.ay) * dy) / L2 : 0;
    t = t < 0 ? 0 : t > 1 ? 1 : t;
    const px = s.ax + dx * t, py = s.ay + dy * t;
    const align = angle === undefined ? 1 : (ca * dx + sa * dy) / L;
    const score = Math.hypot(px - x, py - y) + (1 - align) * 60;
    if (!best || score < best.score) best = { score, lane: s.lane, i: s.i, t, x: px, y: py };
  }
  return best;
}
