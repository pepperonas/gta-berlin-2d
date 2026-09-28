// Öffentlicher Verkehr aus dem VBB-Fahrplan (transit.json, gebaut von tools/osm/transit.mjs).
// Die Spieluhr läuft 60× schneller als die Fahrzeuge fahren. Deshalb bestimmt der Fahrplan nur den TAKT je Uhrzeit
// (Abfahrten pro Stunde an diesem Wochentag); die Fahrzeuge selbst fahren in Echtzeit mit den Fahr- und Haltezeiten
// des Fahrplans. Jeder Fahrtverlauf („Muster“) in der Nähe der Kamera führt eine Liste virtueller Fahrzeuge, die nur
// aus ihrer Fahrzeit τ bestehen – so laufen sie auch dort, wo die Karte gerade nicht geladen ist. Busse werden in der
// Nähe zu echten KI-Fahrzeugen (world.js/services.js), Straßenbahnen und Züge bleiben an ihrem Linienweg.
import { SpatialHash } from './collision.js';
import { undelta, pointAlong } from './geom.js';
import { hash01 } from './map.js';

export const TRANSIT = { track: 12000, every: 1, busLive: 2200, dwell: { bus: 12, tram: 15, sbahn: 25, ubahn: 20 } };
// Fahrzeuglängen in px (10 px = 1 m) und Aufteilung in Wagen
export const TRAIN = {
  tram: { cars: 3, carL: 100, W: 24, gap: 4 },
  sbahn: { cars: 6, carL: 180, W: 30, gap: 6 },
  ubahn: { cars: 6, carL: 160, W: 26, gap: 5 },
};
export const BUS = { L: 120, W: 25 };

const undelta1 = (a) => { const o = []; for (let i = 0; i < a.length; i++) o.push(i ? o[i - 1] + a[i] : a[i]); return o; };

// JSON → Laufzeitstruktur: Linienwege mit Bogenlängen, Halte, Fahrplan, räumlicher Index
export function prepareTransit(json) {
  const shapes = json.shapes.map((d) => {
    const pts = undelta(d), cum = [0];
    for (let i = 2; i < pts.length; i += 2) cum.push(cum[cum.length - 1] + Math.hypot(pts[i] - pts[i - 2], pts[i + 1] - pts[i - 1]));
    return { pts, cum, len: cum[cum.length - 1] };
  });
  const hash = new SpatialHash(3200);
  const patterns = json.patterns.map((p, id) => {
    const [name, mode, color] = json.lines[p.l];
    const deps = p.d.map(undelta1);
    const pat = { id, name, mode, color, shape: shapes[p.s], stops: p.st, stopNames: p.sn.map((i) => json.names[i]), off: p.off, deps };
    pat.dwell = TRANSIT.dwell[mode] ?? 15;
    pat.duration = p.off[p.off.length - 1];
    return pat;
  });
  const byShape = new Map();
  patterns.forEach((p) => { let a = byShape.get(p.shape); if (!a) byShape.set(p.shape, a = []); a.push(p); });
  for (const [sh, list] of byShape) {
    const pts = sh.pts;
    for (let i = 0; i < pts.length - 2; i += 2) {
      const x0 = Math.min(pts[i], pts[i + 2]), y0 = Math.min(pts[i + 1], pts[i + 3]);
      hash.insert({ list }, { x: x0, y: y0, w: Math.abs(pts[i + 2] - pts[i]), h: Math.abs(pts[i + 3] - pts[i + 1]) });
    }
  }
  return { patterns, shapes, hash, attribution: json.attribution, lines: json.lines };
}

// Tagesart aus dem Wochentag (0 = Mo): Mo–Fr, Sa, So
export const dayType = (day) => (day === 5 ? 1 : day === 6 ? 2 : 0);

// Abfahrten je Stunde um die Uhrzeit (±30 min; Fahrten nach Mitternacht zählen zum Vortag, GTFS-Zeiten > 24:00)
export function departuresPerHour(p, minutes, day) {
  const m = ((minutes % 1440) + 1440) % 1440, dt = dayType(day), prev = dayType((day + 6) % 7);
  let n = 0;
  for (const d of p.deps[dt]) if (d >= m - 30 && d < m + 30) n++;
  for (const d of p.deps[prev]) if (d - 1440 >= m - 30 && d - 1440 < m + 30) n++;
  return n;
}

// Lage zur Fahrzeit τ (s seit Abfahrt am ersten Halt): { s: Bogenlänge, stop: Index des nächsten/aktuellen Halts, dwelling }
// Zwischen zwei Halten fährt das Fahrzeug gleichmäßig; die letzten dwell Sekunden vor der Abfahrt steht es am Halt.
export function positionAt(p, tau) {
  const off = p.off, st = p.stops, n = off.length;
  if (tau <= 0) return { s: st[0], stop: 0, dwelling: true };
  if (tau >= off[n - 1]) return { s: st[n - 1], stop: n - 1, dwelling: true, done: true };
  let i = 1;
  while (i < n - 1 && off[i] <= tau) i++;
  const dep = off[i - 1], next = off[i], run = Math.max(1, next - dep - (i < n - 1 ? Math.min(p.dwell, (next - dep) * 0.4) : 0));
  const u = (tau - dep) / run;
  if (u >= 1) return { s: st[i], stop: i, dwelling: true };
  return { s: st[i - 1] + (st[i] - st[i - 1]) * u, stop: i, dwelling: false };
}

const tmp = { x: 0, y: 0, ux: 1, uy: 0 };
export function pointOn(p, s, out = {}) {
  pointAlong(p.shape.pts, Math.max(0, Math.min(p.shape.len, s)), tmp);
  out.x = tmp.x; out.y = tmp.y; out.angle = Math.atan2(tmp.uy, tmp.ux);
  return out;
}

// Muster, deren Weg nahe (x, y) verläuft
export function patternsNear(tr, x, y, r) {
  const set = new Set();
  for (const e of tr.hash.query({ x: x - r, y: y - r, w: 2 * r, h: 2 * r }, [])) for (const p of e.list) set.add(p);
  return set;
}

// Virtuelle Fahrzeuge eines Musters beim ersten Verfolgen: gleichmäßig im aktuellen Takt verteilt (Phase fest je Muster)
export function initialVehicles(p, perHour, seed = 0) {
  if (!perHour) return [];
  const H = 3600 / perHour, ph = hash01(p.id * 13 + seed) * H, out = [];
  for (let tau = ph; tau < p.duration; tau += H) out.push({ tau, delay: 0, key: `${p.id}:${Math.round(tau)}:${seed}` });
  return out;
}

// Verfolgte Muster um die Kamera fortschreiben (jeden Schritt): Fahrzeuge rücken vor, neue fahren im Takt ab, am
// Endhalt fallen sie weg. blocked(p, veh) → true hält ein Fahrzeug an (Straßenbahn vor einem Hindernis).
export function stepTransit(state, tr, cam, minutes, day, dt, t, blocked = null) {
  if (t - (state.lastScan ?? -99) >= TRANSIT.every) {
    state.lastScan = t;
    const near = patternsNear(tr, cam.x, cam.y, TRANSIT.track);
    for (const [id] of state.tracked) if (!near.has(tr.patterns[id])) state.tracked.delete(id);
    for (const p of near) if (!state.tracked.has(p.id)) {
      const perHour = departuresPerHour(p, minutes, day);
      state.tracked.set(p.id, { veh: initialVehicles(p, perHour, state.seed ?? 0), acc: hash01(p.id * 7 + 1), n: 0 });
    }
  }
  for (const [id, s] of state.tracked) {
    const p = tr.patterns[id], perHour = departuresPerHour(p, minutes, day);
    s.acc += dt * perHour / 3600;
    if (s.acc >= 1) { s.acc -= 1; s.n++; s.veh.push({ tau: 0, delay: 0, key: `${id}:n${s.n}` }); }
    for (const v of s.veh) {
      if (v.live) continue; // als echtes Fahrzeug unterwegs (Bus)
      if (blocked && blocked(p, v)) { v.delay += dt; continue; }
      v.tau += dt;
    }
    s.veh = s.veh.filter((v) => !v.gone && v.tau <= p.duration + p.dwell);
  }
}

// Lage eines Zugs/einer Bahn als Wagenfolge entlang des Weges (Spitze bei s): [{ x, y, angle, L, W }]
export function trainCars(p, s) {
  const k = TRAIN[p.mode], out = [];
  for (let i = 0; i < k.cars; i++) {
    const mid = s - k.carL / 2 - i * (k.carL + k.gap);
    const a = pointOn(p, mid), f = pointOn(p, mid + k.carL / 2), b = pointOn(p, mid - k.carL / 2);
    out.push({ x: a.x, y: a.y, angle: Math.atan2(f.y - b.y, f.x - b.x), L: k.carL, W: k.W, first: i === 0, last: i === k.cars - 1 });
  }
  return out;
}
