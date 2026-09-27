// Autopilot für Tests: steuert den Spieler über abstrakte Eingaben (dieselben wie ein Mensch).
// Routen über den echten Straßengraphen (A*, Straßen bis „service“ innerhalb des Gebiets).
import { playerCar, speedOf } from '../../web/src/world.js';
import { nearestEdge } from '../../web/src/map.js';
import { wrapAngle, clamp } from '../../web/src/math.js';
import { offsetPolyline } from '../../web/src/geom.js';

export const idle = () => ({ moveX: 0, moveY: 0, steer: 0, throttle: 0, brake: 0, handbrake: false, sprint: false, horn: false,
  action: false, actionHeld: false, enterExit: false, pause: false, mapToggle: false,
  menuUp: false, menuDown: false, menuLeft: false, menuRight: false, confirm: false, back: false });

const usable = (e) => e.inside && e.cls <= 9 && !(e.cls === 9 && e.w < 40);

// Wegpunkte von from nach to: Einmündung auf die nächste Straße, Straßenzug (rechts der Mitte), Ziel.
export function route(city, from, to) {
  const a = nearestEdge(city, from.x, from.y, 800, usable), b = nearestEdge(city, to.x, to.y, 800, usable);
  const starts = [a.e.a, a.e.b], goals = new Set([b.e.a, b.e.b]);
  const nd = (k) => city.nodes[k];
  const g = new Map(), prev = new Map(), open = [];
  for (const s of starts) { const d = Math.hypot(nd(s).x - from.x, nd(s).y - from.y); g.set(s, d); open.push([d, s]); prev.set(s, null); }
  let goal = null;
  while (open.length) {
    open.sort((p, q) => q[0] - p[0]);
    const [, u] = open.pop();
    if (goals.has(u)) { goal = u; break; }
    for (const k of nd(u).edges) {
      const e = city.edges[k];
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
    p = offsetPolyline(p, e.oneway ? 0 : e.w / 4);
    for (let i = 0; i < p.length; i += 2) pts.push({ x: p[i], y: p[i + 1] });
  }
  // Punkte zusammenfassen, die dicht beieinander liegen (Kreuzungen).
  const out = [];
  for (const p of pts) if (!out.length || Math.hypot(p.x - out[out.length - 1].x, p.y - out[out.length - 1].y) > 30) out.push(p);
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
  input.steer = clamp(diff * 2.5, -1, 1);
  const want = opts.stop ? clamp((d - 6) * 1.2, 0, 200) : Math.abs(diff) > 0.7 ? 70 : opts.cruise ?? 230;
  if (sp < want) { input.throttle = 1; input.brake = 0; } else { input.throttle = 0; input.brake = sp - want > 30 ? 1 : 0.3; }
  if (Math.abs(diff) > 1.8 && d < 60) { input.throttle = 0; input.brake = 1; input.steer = -input.steer; } // rückwärts rangieren
  return d;
}
