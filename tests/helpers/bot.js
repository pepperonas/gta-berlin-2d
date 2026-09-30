// Autopilot für Tests: steuert den Spieler über abstrakte Eingaben (dieselben wie ein Mensch).
// Routen über den echten Straßengraphen (A*, Straßen bis „service“ innerhalb des Gebiets).
import { specOf } from '../../web/src/carmodels.js';
import { DYN } from '../../web/src/dynamics.js';
import { playerCar, speedOf } from '../../web/src/world.js';
import { nearestEdge } from '../../web/src/map.js';
import { wrapAngle, clamp } from '../../web/src/math.js';
import { idleInput } from '../../web/src/idle.js';
import { offsetPolyline } from '../../web/src/geom.js';
import { laneOffsets } from '../../web/src/street.js';

export const idle = () => ({ ...idleInput });

const usable = (e) => e.inside && e.cls <= 9 && !(e.cls === 9 && e.w < 40) && !e.blocked && !e.passage;

// Wegpunkte von from nach to: Einmündung auf die nächste Straße, Straßenzug (rechts der Mitte), Ziel.
export function route(city, from, to) {
  const a = nearestEdge(city, from.x, from.y, 800, usable), b = nearestEdge(city, to.x, to.y, 800, usable);
  const starts = [a.e.a, a.e.b], goals = new Set([b.e.a, b.e.b]);
  const nd = (k) => city.nodes.get(k);
  const g = new Map(), prev = new Map(), open = [];
  for (const s of starts) { const d = Math.hypot(nd(s).x - from.x, nd(s).y - from.y); g.set(s, d); open.push([d, s]); prev.set(s, null); }
  let goal = null;
  while (open.length) {
    open.sort((p, q) => q[0] - p[0]);
    const [, u] = open.pop();
    if (goals.has(u)) { goal = u; break; }
    for (const k of nd(u).edges) {
      const e = city.edges.get(k);
      if (!usable(e)) continue;
      const v = e.a === u ? e.b : e.a, dv = g.get(u) + e.len;
      if (dv < (g.get(v) ?? Infinity)) {
        g.set(v, dv); prev.set(v, [u, e]);
        open.push([dv + Math.hypot(nd(v).x - to.x, nd(v).y - to.y), v]);
      }
    }
  }
  if (goal === null) throw new Error('keine Route');
  const legs = [];
  for (let cur = goal; prev.get(cur); cur = prev.get(cur)[0]) legs.unshift(prev.get(cur));
  const pts = [];
  for (const [u, e] of legs) {
    const fwd = e.a === u;
    let p = fwd ? e.pts : rev(e.pts);
    // Fahrstreifen wie die KI: rechte Spur der eigenen Richtung laut Querschnitt (neben Park-/Radstreifen)
    // (bei Fahrt gegen die Kantenrichtung ist die Polylinie umgedreht, rechts = −Versatz der Rückwärtsspur)
    const lo = laneOffsets(e.cs, city.scale), lanes = fwd ? lo.fwd : lo.bwd.map((o) => -o);
    p = offsetPolyline(p, lanes.length ? lanes[lanes.length - 1] : 0);
    for (let i = 0; i < p.length; i += 2) pts.push({ x: p[i], y: p[i + 1] });
  }
  // Punkte zusammenfassen, die dicht beieinander liegen (Kreuzungen), lange Stücke alle 25 px unterteilen,
  // damit der Autopilot der Spur folgt statt Ecken abzuschneiden.
  const out = [];
  for (const p of pts) {
    const q = out[out.length - 1];
    if (q && Math.hypot(p.x - q.x, p.y - q.y) <= 3) continue; // nur Doppelpunkte (Spurversatz an Nahtstellen bleibt kurz)
    if (q) { const n = Math.floor(Math.hypot(p.x - q.x, p.y - q.y) / 25); for (let k = 1; k < n; k++) out.push({ x: q.x + (p.x - q.x) * k / n, y: q.y + (p.y - q.y) * k / n }); }
    out.push(p);
  }
  // Haarnadeln entfernen: wo zwei versetzte Spuren an einer Kreuzung aneinanderstoßen, entsteht ein kurzer Zickzack.
  for (let changed = true; changed;) {
    changed = false;
    for (let i = 1; i < out.length - 1; i++) {
      const a = out[i - 1], b = out[i], c = out[i + 1];
      const t1 = Math.atan2(b.y - a.y, b.x - a.x), t2 = Math.atan2(c.y - b.y, c.x - b.x);
      if (Math.abs(wrapAngle(t2 - t1)) > 2.0) { out.splice(i, 1); changed = true; break; }
    }
  }
  // Auf die Straße einfädeln: erst den nächsten Straßenpunkt ansteuern.
  out.unshift({ x: a.x, y: a.y });
  out.push({ x: b.x, y: b.y });
  return out;
}

function rev(p) { const r = []; for (let i = p.length - 2; i >= 0; i -= 2) r.push(p[i], p[i + 1]); return r; }

export function driveTo(w, target, input, opts = {}) {
  const car = playerCar(w);
  const dx = target.x - car.x, dy = target.y - car.y, d = Math.hypot(dx, dy);
  const diff = wrapAngle(Math.atan2(dy, dx) - car.angle);
  const sp = speedOf(car);
  // Mit Fahrdynamik (dynamics.js): Pure Pursuit – Krümmung 2·sin(Winkel)/Abstand, Lenkwinkel über den Radstand;
  // ohne (Arcade): Winkelfehler × Verstärkung
  if (car.dyn || car.driver === 'player') {
    const wb = specOf(car).wb * 10, kappa = 2 * Math.sin(diff) / Math.max(d, 20);
    input.steer = Math.abs(diff) > Math.PI / 2 ? Math.sign(diff) : clamp(Math.atan(kappa * wb) / (specOf(car).steerMax * DYN.fun.steer), -1, 1); // Ziel hinter dem Auto: voller Einschlag
  } else input.steer = clamp(diff * 2.5, -1, 1);
  let want = opts.stop ? clamp((d - 6) * 1.2, 0, 200) : Math.abs(diff) > 0.7 ? 70 : opts.cruise ?? 160;
  if (opts.next && !opts.stop) { // vor Kurven abbremsen: Winkel am nächsten Wegpunkt
    const turn = Math.abs(wrapAngle(Math.atan2(opts.next.y - target.y, opts.next.x - target.x) - Math.atan2(dy, dx)));
    if (turn > 0.35) want = Math.min(want, 60 + d * 0.9 - turn * 20);
  }
  if (opts.cap !== undefined) want = Math.min(want, opts.cap); // vorausschauend: Kurve im Bremsweg (followRoute)
  if (sp < want) { input.throttle = 1; input.brake = 0; } else { input.throttle = 0; input.brake = sp - want > 30 || (opts.cap !== undefined && sp > opts.cap + 6) ? 1 : 0.3; }
  if (Math.abs(diff) > (opts.reverseAt ?? (d < 60 ? 1.8 : 2.4))) { input.throttle = 0; input.brake = 1; input.steer = -input.steer; } // rückwärts rangieren (auch Wenden in engen Gassen)
  return d;
}

// Folgt einer Wegpunktliste per Pure Pursuit (Zielpunkt 45 px voraus auf der Linie) und hält am letzten Punkt.
// Liefert true, sobald das Auto am Ziel steht. state wird zwischen Aufrufen weitergereicht.
// Tempo, mit dem man die Kurven voraus (bis 70 m) mit echter Bremsleistung (4,5 m/s²) noch schafft:
// √(v_Kurve² + 2·a·Abstand); scharfe Ecken ≈ 20 km/h, sanfte schneller.
const BRAKE_PX = 45;
function cornerCap(car, pts, i0) {
  let cap = Infinity, dist = Math.hypot(pts[Math.min(i0 + 1, pts.length - 1)].x - car.x, pts[Math.min(i0 + 1, pts.length - 1)].y - car.y);
  for (let i = i0 + 1; i < pts.length - 1 && dist < 700; i++) {
    const a = pts[i - 1], b = pts[i], c = pts[i + 1];
    const turn = Math.abs(wrapAngle(Math.atan2(c.y - b.y, c.x - b.x) - Math.atan2(b.y - a.y, b.x - a.x)));
    if (turn > 0.3) {
      const vc = turn > 1.2 ? 45 : turn > 0.7 ? 75 : 120;
      cap = Math.min(cap, Math.sqrt(vc * vc + 2 * BRAKE_PX * dist));
    }
    dist += Math.hypot(c.x - b.x, c.y - b.y);
  }
  return cap;
}

export function followRoute(w, pts, input, state) {
  const car = playerCar(w);
  state.i ??= 0;
  // Fortschritt: auf das Segment weiterschalten, auf dem das Auto gerade ist
  while (state.i < pts.length - 2) {
    const a = pts[state.i], b = pts[state.i + 1], dx = b.x - a.x, dy = b.y - a.y, L2 = dx * dx + dy * dy || 1;
    if (((car.x - a.x) * dx + (car.y - a.y) * dy) / L2 < 1) break;
    state.i++;
  }
  // Schleifen und Zacken in der Linie (Spurversatz an Knoten) überspringen: ist ein späterer Punkt schon nah, dorthin
  for (let j = Math.min(pts.length - 2, state.i + 8); j > state.i; j--) if (Math.hypot(pts[j].x - car.x, pts[j].y - car.y) < 22) { state.i = j; break; }
  const a = pts[state.i], b = pts[Math.min(state.i + 1, pts.length - 1)];
  const dx = b.x - a.x, dy = b.y - a.y, L = Math.hypot(dx, dy) || 1;
  const t = Math.max(0, ((car.x - a.x) * dx + (car.y - a.y) * dy) / (L * L));
  let rest = 36 + speedOf(car) * 0.2, px = a.x + dx * Math.min(1, t), py = a.y + dy * Math.min(1, t), k = state.i + 1;
  let aim = { x: px, y: py };
  while (k < pts.length) {
    const seg = Math.hypot(pts[k].x - px, pts[k].y - py);
    if (seg >= rest) { aim = { x: px + (pts[k].x - px) * rest / seg, y: py + (pts[k].y - py) * rest / seg }; break; }
    rest -= seg; px = pts[k].x; py = pts[k].y; aim = pts[k]; k++;
  }
  const goal = pts[pts.length - 1];
  const dGoal = Math.hypot(goal.x - car.x, goal.y - car.y);
  const last = k >= pts.length || dGoal < 60;
  const next = pts[Math.min(k + 2, pts.length - 1)];
  driveTo(w, last ? goal : aim, input, { stop: last, next: last ? null : next, reverseAt: last ? undefined : 2.6, cruise: state.cruise ?? 200, cap: last ? undefined : cornerCap(car, pts, state.i) });
  // Festgefahren (Gas, aber kein Tempo – Nase an Wand oder Bordstein, echter Wendekreis): kurz mit Gegenlenkung zurück
  const sp = speedOf(car);
  if (state.back > 0) { state.back -= 1 / 60; input.throttle = 0; input.brake = 1; input.handbrake = false; input.steer = -state.backSteer; }
  else if (input.throttle > 0 && sp < 8) { if ((state.stuck = (state.stuck ?? 0) + 1 / 60) > 1.2) { state.back = 1; state.backSteer = input.steer || 1; state.stuck = 0; } }
  else if (sp > 20) state.stuck = 0;
  // Kreisen ohne Fortschritt (Ziel liegt im Wendekreis): nach 4 s auf demselben Wegstück zurücksetzen
  if (state.i !== state.lastI) { state.lastI = state.i; state.sameT = 0; }
  else if (!last && (state.sameT = (state.sameT ?? 0) + 1 / 60) > 4 && !(state.back > 0)) { state.back = 1.5; state.backSteer = -(input.steer || 1); state.sameT = 0; }
  return last && dGoal < 20 && sp < 10;
}
