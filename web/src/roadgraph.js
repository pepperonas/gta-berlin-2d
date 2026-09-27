// Fahrspurgraph für den KI-Verkehr aus dem Straßengraphen der Karte.
// Rechtsverkehr: je Richtung eine Spur, bei Gegenverkehr um ein Viertel der Fahrbahnbreite nach rechts versetzt.
// An Kreuzungen werden die Spuren gekürzt; Abbiegeverbinder sind kubische Bézierkurven.
import { offsetPolyline, polylineLength } from './geom.js';
import { SpatialHash } from './collision.js';
import { TRAFFIC_MAX_CLASS } from './citycodes.js';

// Reisetempo je Straßenklasse in px/s (10 px = 1 m).
const CRUISE = { 1: 190, 2: 165, 3: 150, 4: 140, 5: 125, 6: 105, 7: 100, 8: 55 };

export const drivable = (e) => e.inside && e.cls <= TRAFFIC_MAX_CLASS && e.len > 5;

function cutPolyline(pts, s0, s1) { // Teilstück zwischen den Bogenlängen s0 und s1
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

export function buildLaneGraph(city) {
  if (city.lanes) return city.lanes;
  const S = city.scale;
  const edges = city.edges.filter(drivable);
  const deg = new Map();
  const radius = new Map();
  for (const e of edges) for (const n of [e.a, e.b]) {
    deg.set(n, (deg.get(n) ?? 0) + 1);
    radius.set(n, Math.max(radius.get(n) ?? 0, e.w / 2));
  }
  const trimAt = (n) => (deg.get(n) >= 3 ? radius.get(n) + 1 * S : 0);
  const lanes = [], out = new Map();
  for (const e of edges) {
    for (const dir of [1, -1]) {
      if ((e.oneway === 1 && dir === -1) || (e.oneway === -1 && dir === 1)) continue;
      const base = dir === 1 ? e.pts : reverse(e.pts);
      const off = e.oneway ? 0 : e.w / 4;
      const from = dir === 1 ? e.a : e.b, to = dir === 1 ? e.b : e.a;
      const raw = offsetPolyline(base, off);
      const L = polylineLength(raw);
      let s0 = trimAt(from), s1 = L - trimAt(to);
      if (s1 - s0 < L * 0.3) { const m = L / 2; s0 = Math.min(s0, m - L * 0.15); s1 = Math.max(s1, m + L * 0.15); }
      const pts = cutPolyline(raw, s0, s1);
      if (pts.length < 4) continue;
      const lane = { id: lanes.length, edge: e, dir, from, to, pts, len: polylineLength(pts), cruise: CRUISE[e.cls] ?? 100, next: null };
      lanes.push(lane);
      (out.get(from) ?? out.set(from, []).get(from)).push(lane);
    }
  }
  for (const l of lanes) {
    const cand = (out.get(l.to) ?? []).filter((m) => m.edge !== l.edge);
    l.next = cand.length ? cand : (out.get(l.to) ?? []).filter((m) => m !== l);
  }
  const hash = new SpatialHash(256);
  for (const l of lanes) for (let i = 0; i < l.pts.length - 2; i += 2) {
    const ax = l.pts[i], ay = l.pts[i + 1], bx = l.pts[i + 2], by = l.pts[i + 3];
    hash.insert({ lane: l, i, ax, ay, bx, by }, { x: Math.min(ax, bx), y: Math.min(ay, by), w: Math.abs(bx - ax), h: Math.abs(by - ay) });
  }
  city.lanes = { lanes, hash, out };
  return city.lanes;
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
export function nearestLane(graph, x, y, angle, radius = 400) {
  const box = { x: x - radius, y: y - radius, w: 2 * radius, h: 2 * radius };
  let best = null;
  const ca = Math.cos(angle), sa = Math.sin(angle);
  for (const s of graph.hash.query(box, [])) {
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
