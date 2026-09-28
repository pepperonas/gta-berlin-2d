// Mitfahren (rein): Lage eines Nahverkehrsfahrzeugs aus seiner Referenz, Fahrzeuge in Reichweite, Plätze zum Aussteigen.
// Referenzen: { pid, key } = Fahrplan-Fahrzeug (transit.js), { carId } = Bus als KI-Auto, { playerTrain: true } = Zug,
// den der Spieler führt (playertrain.js). Verschwindet ein Fahrzeug, liefert vehicleState null (world.js steigt dann aus).
import { positionAt, pointOn, trainCars, BUS } from './transit.js';
import { circleVsObb, circleVsSegment, circleVsCircle, circleVsRect } from './collision.js';
import { blocks } from './car.js';
import { undergroundAtS } from './tunnel.js';
import { nearestPoi } from './map.js';
import { nearestSpot, sidewalkPoint } from './pedestrians.js';

export const RIDE = { reach: 25, cab: 30, hopOn: 25 / 0.36, hopOff: 10 / 0.36, hurtFrom: 40 / 0.36, stun: 1.2, hurt: 10 };

// Tempo aus dem Fahrplan: Abstand der Halte durch die Fahrzeit ohne Haltezeit (wie transitlive.js)
export function speedOfPattern(p, tau) {
  const pos = positionAt(p, tau);
  if (pos.dwelling) return 0;
  const i = pos.stop;
  return (p.stops[i] - p.stops[i - 1]) / Math.max(1, p.off[i] - p.off[i - 1] - Math.min(p.dwell, (p.off[i] - p.off[i - 1]) * 0.4));
}

function busCars(c) { return [{ x: c.x, y: c.y, angle: c.angle, L: BUS.L, W: BUS.W, first: true, last: true }]; }

export function vehicleState(w, ref) {
  const tr = w.city.transit;
  if (!tr || !ref) return null;
  if (ref.carId !== undefined) {
    const c = w.cars.find((x) => x.id === ref.carId && x.duty?.bus && !x.wrecked);
    if (!c) return null;
    const p = tr.patterns[c.duty.pid];
    return { mode: 'bus', p, s: c.duty.s, speed: Math.hypot(c.vx, c.vy), cars: busCars(c), dwelling: !!c.duty.boarding, stop: c.duty.stop, underground: false };
  }
  if (ref.playerTrain) {
    const t = w.playerTrain;
    if (!t) return null;
    const p = tr.patterns[t.pid];
    return { mode: p.mode, p, s: t.s, speed: t.v, cars: trainCars(p, t.s), dwelling: t.v < 3, stop: t.nextStop ?? 0, underground: undergroundAtS(w.city, p, t.s) };
  }
  const s = w.transit?.tracked.get(ref.pid);
  const v = s?.veh.find((x) => x.key === ref.key && !x.gone);
  if (!v || v.live) return null;
  const p = tr.patterns[ref.pid];
  if (p.mode === 'bus') return null; // Busse fahren nur als KI-Auto ({ carId }, s. o.) – TRAIN[p.mode] wäre undefined
  const pos = positionAt(p, v.tau);
  if (pos.done) return null;
  return { mode: p.mode, p, s: pos.s, speed: v.blockedT > 0 ? 0 : speedOfPattern(p, v.tau), cars: trainCars(p, pos.s), dwelling: pos.dwelling, stop: pos.stop, underground: undergroundAtS(w.city, p, pos.s) };
}

// Abstand Punkt → Wagen-Rechteck (0 innen)
function distToCar(x, y, c) {
  const dx = x - c.x, dy = y - c.y, ca = Math.cos(c.angle), sa = Math.sin(c.angle);
  const lx = Math.abs(dx * ca + dy * sa) - c.L / 2, ly = Math.abs(-dx * sa + dy * ca) - c.W / 2;
  return Math.hypot(Math.max(0, lx), Math.max(0, ly));
}

export function transitNear(w, x, y, r) {
  const tr = w.city.transit, out = [];
  if (!tr) return out;
  const consider = (ref) => {
    const st = vehicleState(w, ref);
    if (!st || st.underground) return;
    st.cars.forEach((c, i) => {
      const d = distToCar(x, y, c);
      if (d > r) return;
      const f = st.cars[0], a = f.angle, fx = f.x + Math.cos(a) * f.L / 2, fy = f.y + Math.sin(a) * f.L / 2;
      out.push({ ref, mode: st.mode, dist: d, car: i, front: Math.hypot(x - fx, y - fy) });
    });
  };
  for (const [pid, s] of w.transit?.tracked ?? []) {
    const p = tr.patterns[pid];
    if (p.mode === 'bus') continue;
    for (const v of s.veh) if (!v.gone && !v.live) {
      const head = pointOn(p, positionAt(p, v.tau).s);
      if (Math.abs(head.x - x) < 2200 && Math.abs(head.y - y) < 2200) consider({ pid, key: v.key });
    }
  }
  for (const c of w.cars) if (c.duty?.bus && c.driver === 'npc' && Math.abs(c.x - x) < 200 && Math.abs(c.y - y) < 200) consider({ carId: c.id });
  if (w.playerTrain) consider({ playerTrain: true });
  // Nur je Fahrzeug der nächste Wagen
  const best = new Map();
  for (const h of out) { const k = JSON.stringify(h.ref); if (!best.has(k) || best.get(k).dist > h.dist) best.set(k, h); }
  return [...best.values()].sort((a, b) => a.dist - b.dist);
}

// Platz frei? Wie world.js spotFree (dort nicht exportiert): feste Hindernisse jeder Form – Segment (Wand/Zaun/Ufer/
// Gebietsgrenze), Kreis (Baum, Poller) oder Rechteck (Kiste) – levelbewusst und ohne umgefahrene Poller (car.js
// blocks(), dieselbe kanonische Prüfung wie collideCarWorld/pushCircleOutOfWorld/spotFree), dazu parkende/fahrende Autos.
export function spotFreeHere(w, x, y, r, lvl) {
  const box = { x: x - r, y: y - r, w: 2 * r, h: 2 * r };
  for (const s of w.solids.query(box, [])) {
    if (!blocks(w, s, lvl)) continue;
    const m = s.seg ? circleVsSegment(x, y, r, s) : s.r !== undefined ? circleVsCircle(x, y, r, s.x, s.y, s.r) : circleVsRect(x, y, r, s);
    if (m) return false;
  }
  return w.cars.every((o) => !circleVsObb(x, y, r, o));
}

// Freier Platz neben Wagen i: rechts in Fahrtrichtung zuerst, dann links, dann hinter dem letzten Wagen
export function alightSpot(w, st, i) {
  const c = st.cars[Math.min(i, st.cars.length - 1)], nx = -Math.sin(c.angle), ny = Math.cos(c.angle), d = c.W / 2 + 12;
  const cands = [[c.x + nx * d, c.y + ny * d], [c.x - nx * d, c.y - ny * d]];
  const last = st.cars[st.cars.length - 1];
  cands.push([last.x - Math.cos(last.angle) * (last.L / 2 + 14), last.y - Math.sin(last.angle) * (last.L / 2 + 14)]);
  const lvl = w.player.lvl ?? 0;
  for (const [x, y] of cands) {
    if (spotFreeHere(w, x, y, 8, lvl)) return { x, y };
  }
  return null;
}

// Straßenausgang eines Bahnhofs: Bahnhofs-POI mit passendem Namen in der Nähe, sonst nächster Gehweg
export function stationExit(w, p, stopIndex) {
  const at = pointOn(p, p.stops[stopIndex]), name = (p.stopNames[stopIndex] ?? '').replace(/^[SU]\s+/, '').replace(/\s*\(.*\)$/, '');
  const poi = nearestPoi(w.city, at.x, at.y, 500, (q) => (q.cat === 'ubahn' || q.cat === 'sbahn' || q.cat === 'bahn') && (!name || q.name.includes(name)))
    ?? nearestPoi(w.city, at.x, at.y, 500, (q) => q.cat === 'ubahn' || q.cat === 'sbahn' || q.cat === 'bahn');
  const base = poi ?? at;
  const spot = nearestSpot(w.city, base.x, base.y);
  return spot ? sidewalkPoint(w.city, spot.edge, spot.side, spot.s) : { x: base.x, y: base.y };
}
