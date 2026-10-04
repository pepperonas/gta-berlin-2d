// Verkehr: Rechtsverkehr auf dem echten Straßennetz. KI-Fahrer folgen ihrer Spur (Pure Pursuit),
// wählen an Kreuzungen die nächste Spur, bremsen vor Kurven und Hindernissen, lösen Blockaden,
// fahren sich frei und suchen nach einem Unfall die nächste passende Spur.
import { touch } from './levels.js';
import { near } from './grid.js';
import { clamp, wrapAngle } from './math.js';
import { forwardSpeed } from './car.js';
import { buildLaneGraph, chooseNext, connector, turnAngle, nearestLane, laneDir } from './roadgraph.js';
import { signalState } from './signals.js';
import { pointAlong } from './geom.js';
import { DRY } from './traction.js';

const LOOKAHEAD = 420; // so viel Route (px) hält die KI im Voraus
const GAP_PX = 57;     // Halteabstand Mitte zu Mitte: Autolänge 42 px + 1,5 m
const TRAM_YIELD = 160; // px: so nah vor einer entgegenkommenden Straßenbahn setzt ein Auto zurück

// Kurventempo aus dem Abbiegewinkel: geradeaus unbegrenzt, rechtwinklig ~ 55 px/s.
function turnSpeed(angle) {
  const a = Math.abs(angle);
  if (a < 0.25) return Infinity;
  return clamp(150 - a * 70, 38, 140);
}

function appendLane(ai, lane, fromS = 0) {
  const p = lane.pts;
  (ai.segs ??= []).push({ lane, k0: ai.route.length / 2 }); // ab diesem Routenpunkt gehört die Route zu dieser Spur
  let acc = 0;
  if (fromS > 0) { // Einstieg mitten auf der Spur: Route beginnt am Einstiegspunkt, nicht erst am nächsten Stützpunkt
    const q = pointAlong(p, Math.min(fromS, lane.len));
    ai.route.push(q.x, q.y); ai.cap.push(lane.cruise);
  }
  for (let i = 0; i < p.length; i += 2) {
    if (i > 0) acc += Math.hypot(p[i] - p[i - 2], p[i + 1] - p[i - 1]);
    if (acc <= fromS && fromS > 0) continue;
    ai.route.push(p[i], p[i + 1]); ai.cap.push(lane.cruise);
  }
  ai.lane = lane;
  // Haltelinie am Spurende einer Ampelkreuzung merken (Index des letzten Spurpunkts in der Route)
  if (ai.signals?.has(lane.to)) {
    const [ux, uy] = laneDir(lane, true);
    ai.stops.push({ k: ai.route.length / 2 - 1, v: lane.to, heading: Math.atan2(uy, ux) });
  }
}

function extendRoute(ai, rng, forced = null) {
  const next = forced ?? (ai.field ? towardGoal(ai) : ai.follow ? followLine(ai) : null) ?? chooseNext(ai.lane, rng);
  if (!next) return false;
  const v = Math.min(turnSpeed(turnAngle(ai.lane, next)), next.cruise, ai.lane.cruise);
  const con = connector(ai.lane, next);
  ai.segs[ai.segs.length - 1].kEnd = ai.route.length / 2 - 1; // letzter Punkt vor der Kreuzung
  if (ai.cap.length) ai.cap[ai.cap.length - 1] = Math.min(ai.cap[ai.cap.length - 1], v);
  for (let i = 0; i < con.length; i += 2) { ai.route.push(con[i], con[i + 1]); ai.cap.push(v); }
  appendLane(ai, next);
  return true;
}

// Zielfahrt: Entfernungsfeld über den Spurgraph (Dijkstra rückwärts vom Zielspurstück, begrenzt auf ein Rechteck um
// Start und Ziel). Wer ein Ziel hat, nimmt an jeder Kreuzung die Nachfolgespur mit der kleinsten Restentfernung.
export function goalField(city, fromX, fromY, x, y, margin = 6000, angle = undefined) {
  const g = buildLaneGraph(city);
  // mit Richtung (Haltestelle): die Spur in Fahrtrichtung, nicht die Gegenfahrbahn
  const goal = nearestLane(g, x, y, angle, 400, true) ?? nearestLane(g, x, y, undefined, 1500, true);
  if (!goal) return null;
  const x0 = Math.min(fromX, x) - margin, y0 = Math.min(fromY, y) - margin, x1 = Math.max(fromX, x) + margin, y1 = Math.max(fromY, y) + margin;
  const inBox = (l) => { const p = l.pts; return p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1; };
  const prev = new Map();
  for (const l of g.lanes) {
    if (!inBox(l)) continue;
    for (const n of l.next) { let a = prev.get(n); if (!a) prev.set(n, a = []); a.push(l); }
  }
  // Zielstück zählt bis zum Zielpunkt; alle anderen mit ihrer Länge (einfache Liste statt Heap: wenige Tausend Spuren)
  const dist = new Map([[goal.lane, 0]]), open = [goal.lane];
  while (open.length) {
    let bi = 0; for (let i = 1; i < open.length; i++) if (dist.get(open[i]) < dist.get(open[bi])) bi = i;
    const l = open[bi]; open[bi] = open[open.length - 1]; open.pop();
    const d = dist.get(l);
    for (const p of prev.get(l) ?? []) {
      const nd = d + p.len;
      if (nd < (dist.get(p) ?? Infinity)) { if (!dist.has(p)) open.push(p); dist.set(p, nd); }
    }
  }
  return { dist, goal: goal.lane, x, y };
}
function towardGoal(ai) {
  let best = null, bd = Infinity;
  for (const n of ai.lane.next) { const d = ai.field.dist.get(n); if (d !== undefined && d < bd) { bd = d; best = n; } }
  return best;
}
// Linienweg folgen (Bus): an jeder Kreuzung die Nachfolgespur, deren Ende am dichtesten am Weg liegt und dabei auf
// ihm vorankommt. ai.follow = { pts, s } (s = bisherige Lage auf dem Weg); null, wenn keine Spur zum Weg passt.
export function followLine(ai) {
  const f = ai.follow;
  let best = null, bd = Infinity, bs = f.s;
  for (const n of ai.lane.nextBus) {
    // mittlerer Abstand einiger Punkte der Spur (erste 60 m) zum Weg; die Spur muss auf ihm vorankommen
    let sum = 0, cnt = 0, last = f.s;
    for (const u of [0.25, 0.5, 0.75, 1]) {
      const pt = pointAlong(n.pts, Math.min(n.len, 600) * u);
      const q = projectNear(f.pts, f.cum, pt.x, pt.y, last);
      sum += q.d; cnt++; last = Math.max(last, q.s);
    }
    const d = sum / cnt;
    if (last <= f.s + 5 || d > 140) continue;
    if (d < bd) { bd = d; best = n; bs = last; }
  }
  if (!best) { // keine Spur liegt am Weg (verwinkelte Kreuzung): die, die dem Wegpunkt 60 m voraus am nächsten kommt
    const aim = pointAlong(f.pts, Math.min(f.s + 600, f.cum[f.cum.length - 1]));
    const cur = ai.lane.pts, k = cur.length - 2, d0 = Math.hypot(cur[k] - aim.x, cur[k + 1] - aim.y);
    for (const n of ai.lane.nextBus) {
      const p = n.pts, j = p.length - 2, d = Math.hypot(p[j] - aim.x, p[j + 1] - aim.y);
      if (d < d0 && d < bd) { bd = d; best = n; }
    }
  }
  if (best) f.s = bs;
  return best;
}
// Nächster Punkt auf dem Weg ab Bogenlänge s0 (nur ein Fenster voraus – Schleifen im Weg sollen nicht zurückspringen)
export function projectNear(pts, cum, x, y, s0, ahead = 4000) {
  let best = { s: s0, d: Infinity };
  let i = 0;
  while (i < cum.length - 2 && cum[i + 1] < s0 - 50) i++;
  for (; i < cum.length - 1 && cum[i] <= s0 + ahead; i++) {
    const ax = pts[2 * i], ay = pts[2 * i + 1], dx = pts[2 * i + 2] - ax, dy = pts[2 * i + 3] - ay, L2 = dx * dx + dy * dy || 1;
    const t = Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / L2));
    const d = Math.hypot(ax + dx * t - x, ay + dy * t - y);
    if (d < best.d) best = { s: cum[i] + t * Math.sqrt(L2), d };
  }
  return best;
}

// Ziel setzen (Einsatzort). Die schon geplanten ~400 px Route bleiben (dort können Kreuzungen reserviert sein),
// ab dann wählt die Route jeden Nachfolger zum Ziel hin.
export function setGoal(car, city, x, y, angle = undefined) {
  const f = car.ai && goalField(city, car.x, car.y, x, y, 6000, angle);
  if (!f) return false;
  car.ai.field = f;
  return true;
}
export const goalDistance = (car) => { const ai = car.ai; return ai?.field ? ai.field.dist.get(ai.lane) ?? Infinity : Infinity; };

function remaining(ai, car) {
  const r = ai.route;
  let L = Math.hypot(r[2 * ai.i + 2] - car.x, r[2 * ai.i + 3] - car.y);
  for (let k = ai.i + 1; k < r.length / 2 - 1 && L < LOOKAHEAD; k++) L += Math.hypot(r[2 * k + 2] - r[2 * k], r[2 * k + 3] - r[2 * k + 1]);
  return L;
}

export function initAi(car, lane, s, rng, city) {
  car.ai = {
    route: [], cap: [], i: 0, lane: null, stops: [], signals: city?.signals ?? null,
    cruiseK: 0.85 + rng() * 0.3, segs: [], claims: [],
    blockedT: 0, stuckT: 0, reverseT: 0, hornT: 0,
  };
  appendLane(car.ai, lane, s);
  if (car.ai.route.length < 4) extendRoute(car.ai, rng);
}

// Setzt ein Auto auf eine Spur (Bogenlänge s) und richtet es aus.
export function placeOnLane(car, city, lane, s, rng) {
  const p = lane.pts;
  let acc = 0;
  for (let i = 0; i < p.length - 2; i += 2) {
    const L = Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]);
    if (acc + L >= s || i === p.length - 4) {
      const t = L ? clamp((s - acc) / L, 0, 1) : 0;
      car.x = p[i] + (p[i + 2] - p[i]) * t; car.y = p[i + 1] + (p[i + 3] - p[i + 1]) * t;
      car.angle = Math.atan2(p[i + 3] - p[i + 1], p[i + 2] - p[i]);
      break;
    }
    acc += L;
  }
  car.vx = car.vy = car.angVel = 0;
  initAi(car, lane, s, rng, city);
}

// Zufällige Spur mit Abstand minR…maxR zu (cx, cy).
export function spawnSpot(city, rng, cx, cy, minR, maxR) {
  const g = buildLaneGraph(city);
  const segs = g.hash.query({ x: cx - maxR, y: cy - maxR, w: 2 * maxR, h: 2 * maxR }, []);
  for (let tries = 0; tries < 40 && segs.length; tries++) {
    const sg = segs[Math.floor(rng() * segs.length)];
    if (sg.lane.busOnly) continue;
    const t = rng();
    const x = sg.ax + (sg.bx - sg.ax) * t, y = sg.ay + (sg.by - sg.ay) * t;
    const d = Math.hypot(x - cx, y - cy);
    if (d < minR || d > maxR) continue;
    let s = 0;
    const p = sg.lane.pts;
    for (let i = 0; i < sg.i; i += 2) s += Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]);
    s += Math.hypot(x - sg.ax, y - sg.ay);
    return { lane: sg.lane, s, x, y };
  }
  return null;
}

// Nächste Spur zum Auto finden (nach Unfall / Abdrängen / Übernahme durch die KI).
export function replan(car, city, rng) {
  const g = buildLaneGraph(city);
  const bus = car.kind === 'bus';
  const hit = nearestLane(g, car.x, car.y, car.angle, 600, bus) ?? nearestLane(g, car.x, car.y, car.angle, 3000, bus);
  if (!hit) { car.ai = null; return; }
  let s = 0;
  const p = hit.lane.pts;
  for (let i = 0; i < hit.i; i += 2) s += Math.hypot(p[i + 2] - p[i], p[i + 3] - p[i + 1]);
  s += Math.hypot(hit.x - p[hit.i], hit.y - p[hit.i + 1]) + 30;
  const keep = car.ai;
  initAi(car, hit.lane, s, rng, city);
  // Auftrag überlebt die Neuplanung: Tempo, Zielfeld (Einsatz), Linienweg (Bus), Sondersignal, Halt
  if (keep) Object.assign(car.ai, { cruiseK: keep.cruiseK, field: keep.field ?? null, follow: keep.follow ?? null, urgent: keep.urgent, hold: keep.hold });
}

// Zebrastreifen um (x, y) (Obermenge des 400-px-Felds): je 200-px-Zelle einmal aus dem Zeichen-Index gesucht und bis zum
// nächsten Kachelwechsel (city.gen) gemerkt – statt für jedes Auto in jedem Schritt alle Kartenobjekte abzufragen
function zebrasNear(world, x, y) {
  const city = world.city, c = world._zebra?.gen === city.gen ? world._zebra : (world._zebra = { gen: city.gen, cells: new Map() });
  const cx = Math.floor(x / 200), cy = Math.floor(y / 200), key = cx * 65536 + cy;
  let list = c.cells.get(key);
  if (!list) {
    list = [];
    for (const z of city.render.query({ x: cx * 200 - 200, y: cy * 200 - 200, w: 600, h: 600 }, world._zq ??= [])) if (z.layer === 'crossing' && z.kind === 'zebra') list.push(z);
    c.cells.set(key, list);
  }
  return list;
}

// Abstand zum nächsten Zebrastreifen voraus, an dem ein Fußgänger steht oder geht (Infinity = keiner).
function zebraAhead(car, world) {
  const c = Math.cos(car.angle), s = Math.sin(car.angle);
  let best = Infinity;
  for (const z of zebrasNear(world, car.x, car.y)) {
    const rx = z.x - car.x, ry = z.y - car.y, along = rx * c + ry * s;
    if (along <= 0 || along > 200 || Math.abs(-rx * s + ry * c) > z.edge.w / 2 + 20) continue;
    const reach = z.edge.w / 2 + 25;
    if (world.peds.some((p) => p.state !== 'down' && Math.hypot(p.x - z.x, p.y - z.y) < reach && touch(world.city, car, p))) best = Math.min(best, along);
  }
  return best;
}

// Hindernisabstand vor dem Auto (Kegel), getrennt nach KI-Autos und „ehrlichen“ Hindernissen.
// Gemessen wird entlang des Wegs, den das Auto gleich fährt (Route ab der Fahrzeugmitte), nicht entlang seiner Achse:
// mitten im Abbiegen zeigt die Achse noch schräg, und ein Auto, das auf der Gegenspur korrekt wartet, läge sonst „im Weg“
// – beide warteten dann ewig aufeinander.
export function obstacleAhead(car, world) {
  const c = Math.cos(car.angle), s = Math.sin(car.angle);
  let dCar = Infinity, dOther = Infinity, playerBlock = false, blocker = null, pedBlock = false;
  const path = aheadPath(car);
  // Längere/breitere Fahrzeuge (LKW, Müllauto): Abstände gelten zwischen den Stoßstangen wie bei zwei Pkw (42 × 20 px)
  const myL = car.hw - 21, myW = car.hh - 10;
  const check = (ox, oy, lat, isAiCar, isPlayer, obj = null, isPed = false) => {
    const rx = ox - car.x, ry = oy - car.y;
    const oL = obj?.hw !== undefined ? obj.hw - 21 : 0, ext = myL + oL;
    if (rx * rx + ry * ry > (110 + ext) ** 2) return; // zuerst der billige Abstandstest, dann die Ebenen
    if (obj && !touch(world.city, car, obj)) return; // auf der Brücke bremst niemand für den Verkehr darunter
    lat += myW + (obj?.hh !== undefined ? obj.hh - 10 : 0);
    let along, side;
    if (path) {
      along = Infinity; side = Infinity;
      for (let k = 0, acc = 0; k + 3 < path.length; k += 2) {
        const ax = path[k], ay = path[k + 1], dx = path[k + 2] - ax, dy = path[k + 3] - ay, L = Math.hypot(dx, dy) || 1;
        const u = Math.max(0, Math.min(L, ((ox - ax) * dx + (oy - ay) * dy) / L));
        const d = Math.hypot(ax + dx * u / L - ox, ay + dy * u / L - oy);
        if (d < side && !(k === 0 && u === 0)) { side = d; along = acc + u; }
        acc += L;
      }
    } else { along = rx * c + ry * s; side = Math.abs(-rx * s + ry * c); }
    if (along <= 0 || along > 110 + ext) return;
    if (side > lat) return;
    along = Math.max(1, along - ext);
    if (isAiCar) { if (along < dCar) { dCar = along; blocker = obj; } }
    else { if (along < dOther) { dOther = along; pedBlock = isPed; } if (isPlayer) playerBlock = true; }
  };
  // Kandidaten: mit Raster (world.js während der KI-Schleife) nur die Nachbarn – in derselben Reihenfolge wie die Listen
  const R = 110 + myL + 60, grid = world._gridOn && world._gCars?.list === world.cars && world._gPeds?.list === world.peds;
  const cars = grid ? near(world._gCars, car.x, car.y, R, world._nbC ??= []) : null, peds = grid ? near(world._gPeds, car.x, car.y, R, world._nbP ??= []) : null;
  const nc = cars ? cars.length : world.cars.length, np = peds ? peds.length : world.peds.length;
  for (let k = 0; k < nc; k++) {
    const o = cars ? world.cars[cars[k]] : world.cars[k];
    if (o === car) continue;
    // Stehende Autos ohne Fahrer (Parker, abgestelltes Spielerauto, Missionsauto) wie Parker: nur wenn sie wirklich in der Spur stehen
    const parkedLike = o.driver === null && Math.abs(o.vx) + Math.abs(o.vy) < 10;
    check(o.x, o.y, parkedLike ? 18 : 22, o.driver === 'npc' && !o.wrecked, o.driver === 'player', o);
  }
  for (let k = 0; k < np; k++) { const p = peds ? world.peds[peds[k]] : world.peds[k]; if (p.state !== 'gone' && p.state !== 'dead') check(p.x, p.y, 16, false, false, p, true); } // über Tote fahren (sonst stünde der Verkehr ewig)
  for (const b of world.bikes ?? []) if (b.state === 'ride') check(b.x, b.y, 14, false, false, b, true); // Radfahrer: dahinter bleiben
  for (const o of world.railObs ?? []) check(o.x, o.y, 22, false, false, o); // Straßenbahnwagen: warten, bis sie vorbei sind
  const pl = world.player;
  if (!pl.inCar && touch(world.city, car, pl)) check(pl.x, pl.y, 17, false, true);
  return { dCar, dOther, playerBlock, blocker, pedBlock };
}

// Wie weit (px) vor dem Kopf einer entgegenkommenden Straßenbahn dieses Auto in ihrem Weg steht (Infinity: gar nicht).
// Dieselbe Geometrie wie die Hindernisprüfung der Bahn (transitlive.js obstacleAt: Punkte auf der Strecke vor dem Kopf,
// Umkreis 24 + 0,4 · halbe Autolänge), nur weiter voraus – so weicht ein Auto genau dann, wenn die Bahn auf es wartet.
// Geradlinig statt entlang der Route – die hilft beim Zurücksetzen nicht (sonst pendelt das Auto vor und zurück).
export function tramHeadOn(car, world) {
  const r = 24 + 0.4 * car.hw + 4;
  let best = Infinity;
  for (const o of world.railObs ?? []) {
    if (Math.cos(o.angle - car.angle) > -0.5) continue;
    const c = Math.cos(o.angle), s = Math.sin(o.angle);
    for (let d = 10; d <= TRAM_YIELD && d < best; d += 15) {
      const px = o.x + c * (o.hw + d), py = o.y + s * (o.hw + d);
      if (Math.hypot(car.x - px, car.y - py) < r) { best = d; break; }
    }
  }
  return best;
}

// Weg der nächsten ~120 px ab der Fahrzeugmitte entlang der Route (null ohne Route).
function aheadPath(car) {
  const ai = car.ai, r = ai?.route;
  if (!r || r.length < 4) return null;
  const out = ai._ahead ??= [];
  out.length = 0; out.push(car.x, car.y);
  let acc = 0, x = car.x, y = car.y;
  for (let k = ai.i + 1; 2 * k + 1 < r.length && acc < 150 + car.hw; k++) {
    const nx = r[2 * k], ny = r[2 * k + 1];
    acc += Math.hypot(nx - x, ny - y); out.push(nx, ny); x = nx; y = ny;
  }
  return out.length >= 4 ? out : null;
}

export function driveAi(car, world, dt) {
  let ai = car.ai;
  const city = world.city, ctl = car.controls;
  if (car.wrecked) return;
  if (!ai || ai.route.length < 4) { replan(car, city, world.rng); ai = car.ai; if (!ai) return; claimNarrow(world, car, ai.segs[0].lane); }
  if (world.nresGen !== city.gen) rekeyNarrow(world);
  const r = ai.route;

  // Fortschritt auf der Route: zum Segment weiterschalten, das vor dem Auto liegt.
  let t = 0, px = car.x, py = car.y;
  for (;;) {
    const ax = r[2 * ai.i], ay = r[2 * ai.i + 1], bx = r[2 * ai.i + 2], by = r[2 * ai.i + 3];
    if (bx === undefined) break;
    const dx = bx - ax, dy = by - ay, L2 = dx * dx + dy * dy || 1;
    t = ((car.x - ax) * dx + (car.y - ay) * dy) / L2;
    px = ax + dx * clamp(t, 0, 1); py = ay + dy * clamp(t, 0, 1);
    if (t > 1 || Math.hypot(bx - car.x, by - car.y) < 10) { ai.i++; if (2 * ai.i + 3 >= r.length) { if (!extendRoute(ai, world.rng)) break; } continue; }
    break;
  }
  if (ai.i > 24) {
    const n = ai.i - 2; r.splice(0, 2 * n); ai.cap.splice(0, n); ai.i = 2;
    for (const st of ai.stops) st.k -= n;
    for (const sg of ai.segs) { sg.k0 -= n; if (sg.kEnd !== undefined) sg.kEnd -= n; }
    while (ai.segs.length > 1 && ai.segs[1].k0 <= 0 && ai.segs[0].kEnd !== undefined && ai.segs[0].kEnd < 0) ai.segs.shift();
  }

  while (remaining(ai, car) < LOOKAHEAD) if (!extendRoute(ai, world.rng)) break;
  if (Math.hypot(px - car.x, py - car.y) > 260) { replan(car, city, world.rng); if (car.ai) claimNarrow(world, car, car.ai.segs[0].lane); return; }

  // Zielpunkt voraus auf der Route (Pure Pursuit), plus Tempolimit aus Kurven voraus.
  const vf = forwardSpeed(car);
  const look = clamp(Math.abs(vf) * 0.35, 36, 90);
  let aimX = px, aimY = py, acc = 0, found = false;
  let target = (ai.cap[ai.i] ?? 100) * ai.cruiseK;
  const tr = car.traction ?? DRY, kb = tr.brake; // Wetter (traction.js): langsamer, früher bremsen, mehr Abstand
  target *= 0.6 + 0.4 * kb;
  let x0 = px, y0 = py;
  for (let k = ai.i + 1; k < r.length / 2; k++) {
    const x1 = r[2 * k], y1 = r[2 * k + 1], L = Math.hypot(x1 - x0, y1 - y0);
    if (!found && acc + L >= look) { const u = (look - acc) / (L || 1); aimX = x0 + (x1 - x0) * u; aimY = y0 + (y1 - y0) * u; found = true; }
    acc += L;
    const cap = ai.cap[k];
    if (acc < 220 && cap < target) target = Math.min(target, Math.sqrt(cap * cap + 2 * 260 * kb * Math.max(0, acc - 20)));
    x0 = x1; y0 = y1;
    if (acc > 240 && found) break;
  }
  if (!found) { aimX = x0; aimY = y0; }
  const dx = aimX - car.x, dy = aimY - car.y;

  if (ai.reverseT > 0) {
    ai.reverseT -= dt;
    ctl.throttle = 0; ctl.brake = 1; ctl.handbrake = false;
    ctl.steer = -clamp(wrapAngle(Math.atan2(dy, dx) - car.angle) * 2, -1, 1);
    return;
  }

  const diff = wrapAngle(Math.atan2(dy, dx) - car.angle);
  ctl.steer = clamp(diff * 2.4, -1, 1);
  if (Math.abs(diff) > 0.6) target = Math.min(target, 55 * tr.lat);

  // Ampel: bei Rot (und bei Gelb, wenn noch Bremsweg bleibt) an der Haltelinie halten
  // Haltelinie erst verwerfen, wenn das Auto sie wirklich überfahren hat (vorzeichenbehafteter Abstand in Fahrtrichtung)
  const along = (q) => (r[2 * q.k] - car.x) * Math.cos(q.heading) + (r[2 * q.k + 1] - car.y) * Math.sin(q.heading);
  while (ai.stops.length && (ai.stops[0].k < ai.i - 3 || along(ai.stops[0]) < -8)) ai.stops.shift();
  const st = ai.stops[0];
  if (st) {
    let dist = along(st);
    if (st.k > ai.i + 1) { dist = Math.hypot(r[2 * (ai.i + 1)] - car.x, r[2 * (ai.i + 1) + 1] - car.y); for (let k = ai.i + 1; k < st.k && dist < 400; k++) dist += Math.hypot(r[2 * k + 2] - r[2 * k], r[2 * k + 3] - r[2 * k + 1]); }
    if (dist < 400) {
      const light = signalState(city, st.v, st.heading, world.time);
      const brakeDist = vf * vf / (2 * 300 * kb);
      // Blaulicht mit Sondersignal fährt über Rot, aber langsam in die Kreuzung
      if (ai.urgent) { if (light !== 'green' && dist < 60) target = Math.min(target, 70); }
      else if (light === 'red' || (light === 'yellow' && dist > brakeDist + 10)) target = Math.min(target, Math.sqrt(2 * 90 * kb * Math.max(0, dist - 15))); // Bremsweg v²/2a mit a ≈ 0,9 m/s² (Regler bremst träge)
      ai.light = light; ai.lightDist = dist;
    }
  }
  // Zebrastreifen: vor wartenden oder querenden Fußgängern halten
  const zc = zebraAhead(car, world);
  if (zc < 200) target = Math.min(target, Math.sqrt(2 * 90 * kb * Math.max(0, zc - 30)));

  const { dCar, dOther, playerBlock, blocker, pedBlock } = obstacleAhead(car, world);
  const railHead = tramHeadOn(car, world);
  // Die Straßenbahn hat Vorrang: kommt sie frontal im eigenen Weg entgegen (ihr Linienweg liegt stellenweise in der
  // Gegenspur), setzt das Auto zurück, solange sie nah ist – sonst warteten beide ewig aufeinander. Wer direkt hinter
  // einem so zurücksetzenden Auto steht, setzt mit zurück (sonst führe der Vordermann auf).
  ai.tramYield = railHead < TRAM_YIELD || (blocker?.ai?.tramYield && blocker.ai.reverseT > 0 && dCar < 80);
  if (ai.tramYield) ai.reverseT = Math.max(ai.reverseT, 0.3);
  const d = Math.min(dOther, dCar);
  // Vorfahrt an der nächsten Kreuzung: erst einfahren, wenn sie frei ist und dahinter Platz ist (sonst an der Linie warten).
  // Reserviert wird nur, wer auch losfahren kann (niemand steht direkt davor).
  const gate = entryGate(car, world, d > 70 && vf > -2, dt);
  ai.blink = blinkFor(ai, car);
  // Halt deutlich vor dem Linienpunkt: 10 px davor schaltet die Route schon auf „in der Kreuzung“ weiter
  if (gate < Infinity) target = Math.min(target, Math.sqrt(2 * 90 * kb * Math.max(0, gate - GATE_STOP)));
  releaseClaims(car, world, Math.abs(vf) < 5 ? (ai.stillT = (ai.stillT ?? 0) + dt) : (ai.stillT = 0));
  // Selbstheilung: wer auf einer Engstelle fährt, hält sie auch (egal über welchen Weg er hineinkam)
  const here = ai.segs[currentSeg(ai)];
  if (here?.lane.narrow && !ai.claims.some((cl) => cl.kind === 'n' && cl.edge === narrowKey(city, here.lane.edge))) claimNarrow(world, car, here.lane, here);
  if (vf > 40) ai.headOn = 0; // fährt wieder frei
  // Abstand halten: Mitte zu Mitte eine Autolänge (42 px) + 1,5 m Lücke; darunter stehen bleiben
  if (d < 125 / kb) target = Math.min(target, Math.max(0, (d - GAP_PX) * 2.2 * kb)); // Stillstandsabstand bleibt, in Fahrt wächst er mit 1/kb
  // Arbeitshalt (Paket, Mülltonnen, Einsatzort): stehen bleiben, der Verkehr dahinter wartet
  if (ai.hold > 0) { ai.hold -= dt; target = 0; }

  if (target < 1 && Math.abs(vf) < 6) {
    ai.blockedT += dt;
    // In der Schlange (Vordermann fährt in dieselbe Richtung) einfach warten. Nur wenn sich zwei Autos frontal oder quer
    // gegenseitig blockieren, setzt eines von beiden (das mit der höheren Nummer) kurz zurück – nie beide.
    if (blocker && dCar <= dOther && ai.blockedT > 3.5 && Math.cos(blocker.angle - car.angle) < 0.5) {
      const mine = ai.claims.some((cl) => cl.kind === 'n'), theirs = blocker.ai?.claims.some((cl) => cl.kind === 'n');
      if (theirs && !mine ? true : mine && !theirs ? false : car.id > blocker.id) { ai.reverseT = 1.2; ai.blockedT = 0; ai.headOn = (ai.headOn ?? 0) + 1; }
    }
    // Spieler oder Fußgänger im Weg: hupen (Fußgänger weichen dann aus, siehe world.js)
    if ((playerBlock && ai.blockedT > 1.5 || pedBlock && dOther < dCar && ai.blockedT > 2.5) && ai.hornT <= 0) {
      world.events.push({ type: 'horn', x: car.x, y: car.y, npc: true, heading: car.angle });
      ai.hornT = 3;
    }
  } else ai.blockedT = 0;
  ai.hornT -= dt;

  if (target < 3) { ctl.throttle = 0; ctl.brake = vf > 1 ? 1 : 0; ctl.handbrake = false; if (Math.abs(vf) < 4) { car.vx = 0; car.vy = 0; car.angVel = 0; } return; } // halten (Ampel, Stau), ohne nachzukriechen
  if (vf < target - 8) { ctl.throttle = clamp((target - vf) / 60, 0.25, 1); ctl.brake = 0; }
  else if (vf > target + 8) { ctl.throttle = 0; ctl.brake = clamp((vf - target) / 70, 0.25, 1); }
  else { ctl.throttle = 0.15; ctl.brake = 0; }
  ctl.handbrake = false;

  // Festgefahren (Gas, aber keine Bewegung, kein Hindernis) → kurz zurücksetzen.
  if (ctl.throttle > 0.3 && Math.abs(vf) < 8 && d >= 110) {
    ai.stuckT += dt;
    if (ai.stuckT > 1.8) { ai.reverseT = 1.0; ai.stuckT = 0; }
  } else ai.stuckT = 0;
}

// --- Vorfahrt: Kreuzungen und Engstellen reservieren -------------------------------------------------
// world.jres: Knoten → { approach, cars, since, moves (Auto → Bewegung) }   (Kreuzung ohne Ampel: nur eine Zufahrt zur Zeit, Kolonne darf nachrücken)
// world.nres: Kante → { dir, cars }          (enge Straße mit Gegenverkehr: nur eine Richtung zur Zeit)
const GATE_STOP = 24; // px vor dem Linienpunkt anhalten, wenn die Einfahrt verweigert ist
const NARROW_WAIT = 50; // zusätzlicher Abstand vor einer Engstelle (px)
const ENTRY_LOOK = 100;   // ab hier vor dem Spurende um Einfahrt bitten (px)
const CLAIM_AT = 45;      // so nah an der Linie wird reserviert
const PLATOON_S = 6;      // so lange darf eine Kolonne aus derselben Zufahrt nachrücken
const HOLD_STILL_S = 2;
const REROUTE_S = 3;       // so lange an einer verweigerten Einfahrt warten, dann anders abbiegen   // wer mit Reservierung, aber noch vor der Kreuzung so lange steht, gibt sie frei

export function currentSeg(ai) {
  let j = 0;
  for (let k = 0; k < ai.segs.length; k++) if (ai.segs[k].k0 <= ai.i) j = k;
  return j;
}

function routeDist(ai, car, k) {
  const r = ai.route;
  let d = Math.hypot(r[2 * (ai.i + 1)] - car.x, r[2 * (ai.i + 1) + 1] - car.y);
  for (let q = ai.i + 1; q < k && d < 400; q++) d += Math.hypot(r[2 * q + 2] - r[2 * q], r[2 * q + 3] - r[2 * q + 1]);
  return d;
}

// Blinker (nur Anzeige): vor einem Abbiegen bis in die Kreuzung hinein; +1 rechts, -1 links, 0 geradeaus.
export const BLINK_AHEAD = 110;
export function blinkFor(ai, car) {
  if (!ai.segs?.length) return 0;
  const j = currentSeg(ai), cur = ai.segs[j], next = ai.segs[j + 1];
  if (!next || cur.kEnd === undefined) return 0;
  if (cur.kEnd >= ai.i && routeDist(ai, car, cur.kEnd) > BLINK_AHEAD) return 0;
  const a = turnAngle(cur.lane, next.lane);
  return a > 0.5 ? 1 : a < -0.5 ? -1 : 0;
}

// Reservierung gilt nur, solange ihr Auto noch fährt und sie selbst noch führt (nach Umplanen, Unfall, Übernahme weg).
function prune(world, set, owns) {
  for (const id of set) { const c = world.cars.find((o) => o.id === id); if (!c || c.driver !== 'npc' || c.wrecked || !c.ai?.claims.some(owns)) set.delete(id); }
  return set;
}
const ownsJ = (v) => (cl) => cl.kind === 'j' && cl.v === v;

// Zusammenhängende enge Abschnitte derselben Straße sind EINE Engstelle (sonst treffen sich zwei Autos an der Naht,
// jedes mit „seiner“ Hälfte reserviert). Schlüssel = kleinste Kantennummer der Gruppe; neu bestimmt nach jedem Nachladen.
export function narrowKey(city, edge) {
  if (edge._nk && edge._nk.gen === city.gen) return edge._nk.key;
  const g = buildLaneGraph(city), isNarrow = (e) => (g.byEdge.get(e.id) ?? []).some((l) => l.narrow);
  const seen = new Set([edge.id]), todo = [edge];
  let key = edge.id;
  while (todo.length) {
    const e = todo.pop();
    for (const v of [e.a, e.b]) for (const k of city.nodes.get(v)?.edges ?? []) {
      if (seen.has(k)) continue;
      const o = city.edges.get(k);
      if (!o || o.name !== edge.name || !edge.name || !isNarrow(o)) continue;
      seen.add(k); todo.push(o); key = Math.min(key, k);
    }
  }
  for (const k of seen) { const o = city.edges.get(k); if (o) o._nk = { gen: city.gen, key }; }
  return key;
}
const ownsN = (e) => (cl) => cl.kind === 'n' && cl.edge === e;

// Nach dem Nachladen von Kacheln kann eine Engstellen-Gruppe wachsen: neuer Schlüssel, neue Bezugskante, damit auch
// eine andere Vorzeichen-Konvention für die Richtung. Die Belegung deshalb aus den Reservierungen der Autos neu aufbauen –
// sonst verwaisen alte Einträge und Gegenverkehr bekommt die Engstelle ein zweites Mal.
function rekeyNarrow(world) {
  const city = world.city;
  world.nresGen = city.gen;
  world.nres = new Map();
  for (const c of world.cars) {
    if (c.driver !== 'npc' || c.wrecked || !c.ai) continue;
    for (const cl of c.ai.claims) {
      if (cl.kind !== 'n' || !cl.lane) continue;
      cl.edge = narrowKey(city, cl.lane.edge);
      let t = world.nres.get(cl.edge);
      if (!t) world.nres.set(cl.edge, t = { dir: narrowDir(city, cl.lane), cars: new Set() });
      t.cars.add(c.id);
    }
  }
}

// Abstand bis zur Haltelinie, wenn das Auto vor der nächsten Kreuzung warten muss (sonst Infinity).
function entryGate(car, world, canGo, dt) {
  const ai = car.ai;
  if (!ai.segs?.length) return Infinity;
  const j = currentSeg(ai), cur = ai.segs[j], next = ai.segs[j + 1];
  if (!next || cur.kEnd === undefined) return Infinity;
  if (cur.kEnd < ai.i) {
    // schon über die Linie gerollt (im Pulk hinter einem Vordermann, der die Reservierung nie zuließ): jetzt ist man
    // drin – die Reservierung nachholen, damit die anderen es sehen; umkehren geht ohnehin nicht mehr.
    if (!ai.claims.some((cl) => cl.seg === next)) claim(world, car, cur.lane, next);
    return Infinity;
  }
  const dist = routeDist(ai, car, cur.kEnd);
  if (dist > ENTRY_LOOK) return Infinity;
  if (ai.claims.some((cl) => cl.seg === next)) return Infinity; // schon reserviert
  if (!mayEnter(world, car, cur.lane, next.lane)) {
    // Einfahrt verweigert (Kreuzung/Engstelle belegt, kein Platz dahinter): nach einer Weile woanders abbiegen,
    // wie echte Fahrer – das löst auch Kreis-Verklemmungen (zwei Kolonnen warten gegenseitig auf ihre Straßen).
    ai.deniedT = (ai.deniedT ?? 0) + dt;
    if (ai.deniedT > REROUTE_S && dist < 60) {
      const alt = cur.lane.next.filter((l) => l !== next.lane && mayEnter(world, car, cur.lane, l));
      if (alt.length) { rerouteAt(ai, j, alt[Math.floor(world.rng() * alt.length)], world.rng); ai.deniedT = 0; return Infinity; }
    }
    // vor einer Engstelle weiter zurück warten: der Gegenverkehr schwenkt an ihrem Ende aus der Mitte auf seine Spur
    return next.lane.narrow ? dist - NARROW_WAIT : dist;
  }
  ai.deniedT = 0;
  // reservieren, wenn es losgehen kann – spätestens aber beim Überfahren der Linie (im Pulk rollt man sonst ohne hinein)
  if ((dist < CLAIM_AT && canGo) || dist < 15) claim(world, car, cur.lane, next);
  return Infinity;
}

// Route ab dem Ende von Stück j verwerfen und über die Spur alt neu aufbauen.
function rerouteAt(ai, j, alt, rng) {
  const cur = ai.segs[j], k = cur.kEnd;
  ai.route.length = 2 * (k + 1); ai.cap.length = k + 1;
  ai.segs.length = j + 1; delete cur.kEnd;
  ai.stops = ai.stops.filter((st) => st.k <= k);
  ai.claims = ai.claims.filter((cl) => ai.segs.includes(cl.seg));
  ai.lane = cur.lane;
  extendRoute(ai, rng, alt);
}

function isJunction(world, v) { const nd = world.city.nodes.get(v); return !!nd && nd.edges.length >= 3; }

// Bewegung durch eine Kreuzung als Sehne vom Spurende zum Anfang der Zielspur.
function moveOf(from, to) {
  const p = from.pts;
  return { ax: p[p.length - 2], ay: p[p.length - 1], bx: to.pts[0], by: to.pts[1], to };
}
// Kreuzen sich zwei Bewegungen (oder kommen sich näher als eine Autobreite, oder münden in dieselbe Spur)?
export function movesConflict(m, n) {
  if (m.to === n.to) return true;
  const cr = (ax, ay, bx, by, cx, cy) => (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
  const d1 = cr(m.ax, m.ay, m.bx, m.by, n.ax, n.ay), d2 = cr(m.ax, m.ay, m.bx, m.by, n.bx, n.by);
  const d3 = cr(n.ax, n.ay, n.bx, n.by, m.ax, m.ay), d4 = cr(n.ax, n.ay, n.bx, n.by, m.bx, m.by);
  if (((d1 > 0) !== (d2 > 0)) && ((d3 > 0) !== (d4 > 0))) return true;
  const ps = (px, py, ax, ay, bx, by) => {
    const dx = bx - ax, dy = by - ay, L2 = dx * dx + dy * dy || 1, u = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / L2));
    return Math.hypot(ax + dx * u - px, ay + dy * u - py);
  };
  return Math.min(ps(n.ax, n.ay, m.ax, m.ay, m.bx, m.by), ps(n.bx, n.by, m.ax, m.ay, m.bx, m.by),
    ps(m.ax, m.ay, n.ax, n.ay, n.bx, n.by), ps(m.bx, m.by, n.ax, n.ay, n.bx, n.by)) < 20;
}

export function mayEnter(world, car, from, to) {
  const v = from.to;
  // Dahinter Platz? Kein stehendes Auto kurz hinter der Einmündung (sonst blockiert man die Kreuzung)
  const sx = to.pts[0], sy = to.pts[1], ux = to.pts[2] - sx, uy = to.pts[3] - sy, L = Math.hypot(ux, uy) || 1;
  for (const o of world.cars) {
    if (o === car || o.wrecked || !touch(world.city, car, o)) continue;
    const rx = o.x - sx, ry = o.y - sy, along = (rx * ux + ry * uy) / L, lat = Math.abs(-rx * uy + ry * ux) / L;
    if (along > -10 && along < 55 && lat < 14 && Math.hypot(o.vx, o.vy) < 25) return false;
  }
  if (!world.city.signals.has(v) && isJunction(world, v)) {
    const h = world.jres?.get(v);
    if (h && prune(world, h.cars, ownsJ(v)).size && !h.cars.has(car.id)) {
      // Wessen Weg die Reservierenden nicht kreuzt, darf mit hinein (Gegenverkehr geradeaus, zweimal rechts …)
      const mine = moveOf(from, to);
      const clear = [...h.cars].every((id) => { const m = h.moves?.get(id); return m && !movesConflict(m, mine); });
      if (!clear) {
        if (h.approach !== `${from.edge.id}:${from.dir}`) return false;         // andere Zufahrt ist dran
        if (world.time - h.since > PLATOON_S) return false;                     // Kolonne lang genug, jetzt die anderen
      }
    }
  }
  if (to.narrow) {
    const nk = narrowKey(world.city, to.edge), t = world.nres?.get(nk);
    // Gegenrichtung belegt – oder eine Sackgasse: wer drin ist, wendet am Ende und kommt zurück, also nur einzeln hinein
    if (t && prune(world, t.cars, ownsN(nk)).size && !t.cars.has(car.id) && (t.dir !== narrowDir(world.city, to) || deadEndNarrow(world.city, to))) return false;
  }
  return true;
}

// next: das Routenstück (Spur) hinter der Kreuzung – die Reservierung hängt an genau diesem Stück.
function claim(world, car, from, next) {
  const v = from.to, to = next.lane;
  car.ai.claims.push({ kind: 'mark', seg: next });
  if (!world.city.signals.has(v) && isJunction(world, v)) {
    world.jres ??= new Map();
    let h = world.jres.get(v);
    car.ai.claims.push({ kind: 'j', v, seg: next });
    if (!h || !prune(world, h.cars, ownsJ(v)).size) world.jres.set(v, h = { approach: `${from.edge.id}:${from.dir}`, cars: new Set(), since: world.time, moves: new Map() });
    h.cars.add(car.id);
    h.moves.set(car.id, moveOf(from, to));
  }
  if (to.narrow) claimNarrow(world, car, to, next);
}

// Darf auf dieser Spur (einer Engstelle) gerade ein Auto erzeugt werden? Nicht gegen die belegte Richtung.
// Endet diese Engstelle (in Fahrtrichtung) in einer Sackgasse, an der nur Wenden bleibt?
function deadEndNarrow(city, lane) {
  if (lane._de && lane._de.gen === city.gen) return lane._de.v;
  const nk = narrowKey(city, lane.edge);
  let l = lane, v = false;
  for (let k = 0; k < 30 && l; k++) {
    const nx = l.next;
    if (!nx.length || nx.every((m) => m.edge === l.edge)) { v = true; break; }
    l = nx.find((m) => m.narrow && m.edge !== l.edge && narrowKey(city, m.edge) === nk);
  }
  lane._de = { gen: city.gen, v };
  return v;
}

// Darf ein frisch platziertes Auto hier entstehen? Wer näher als der Haltabstand an seiner ersten Linie geboren wird,
// ist im ersten Schritt schon darüber – entryGate trägt ihn dann ungeprüft ein (gedacht für den Pulk). Deshalb dort nur,
// wenn die Einfahrt ohnehin frei wäre (sonst Gegenverkehr auf einer Engstelle, zweite Zufahrt in eine Kreuzung).
export function spawnAllowed(world, car) {
  const ai = car.ai;
  if (!ai?.segs?.length) return true;
  const cur = ai.segs[0], next = ai.segs[1];
  // ohne nächstes Stück wählt erst der erste Fahrschritt (per Zufall) – dann muss jeder mögliche Nachfolger frei sein
  const end = cur.kEnd ?? ai.route.length / 2 - 1;
  const d = end < ai.i ? 0 : routeDist(ai, car, end);
  if (d >= GATE_STOP) return true;
  const outs = next ? [next.lane] : cur.lane.next.filter((l) => car.kind === 'bus' || !l.busOnly);
  return outs.every((l) => mayEnter(world, car, cur.lane, l));
}

export function narrowFree(world, lane) {
  if (!lane.narrow) return true;
  const nk = narrowKey(world.city, lane.edge), t = world.nres?.get(nk);
  return !t || !prune(world, t.cars, ownsN(nk)).size || (t.dir === narrowDir(world.city, lane) && !deadEndNarrow(world.city, lane));
}

// Fahrtrichtung auf der Engstelle als Himmelsrichtung (die Kanten einer Gruppe können unterschiedlich orientiert sein):
// +1, wenn die Spur ungefähr in Richtung der Gruppenkante mit dem kleinsten Schlüssel zeigt.
export function narrowDir(city, lane) {
  const ref = city.edges.get(narrowKey(city, lane.edge)) ?? lane.edge;
  const ax = ref.pts[ref.pts.length - 2] - ref.pts[0], ay = ref.pts[ref.pts.length - 1] - ref.pts[1];
  const p = lane.pts, bx = p[p.length - 2] - p[0], by = p[p.length - 1] - p[1];
  return ax * bx + ay * by >= 0 ? 1 : -1;
}

export function claimNarrow(world, car, lane, seg = car.ai?.segs?.[0]) {
  if (!lane.narrow || !car.ai) return;
  world.nres ??= new Map();
  const nk = narrowKey(world.city, lane.edge);
  let t = world.nres.get(nk);
  car.ai.claims.push({ kind: 'n', edge: nk, seg, lane });
  if (!t || !prune(world, t.cars, ownsN(nk)).size) world.nres.set(nk, t = { dir: narrowDir(world.city, lane), cars: new Set() });
  t.cars.add(car.id);
}

// Reservierungen freigeben: Kreuzung, sobald das Auto hindurch ist; Engstelle, sobald es die Kante verlassen hat;
// alles, was vor der Linie steht und nicht losfahren kann (stillT), damit andere nicht umsonst warten.
function releaseClaims(car, world, stillT = 0) {
  const ai = car.ai;
  if (!ai.claims.length) return;
  const j = currentSeg(ai), beforeLine = ai.segs[j]?.kEnd !== undefined && ai.segs[j].kEnd >= ai.i;
  // Erst sammeln, dann austragen: ein Auto hält oft mehrere Reservierungen derselben Engstelle/Kreuzung (je Abschnitt);
  // fällt eine davon weg, steckt es womöglich noch drin – austragen nur, wenn keine mehr übrig ist.
  const dropped = [];
  const drop = (cl) => { dropped.push(cl); return false; };
  ai.claims = ai.claims.filter((cl) => {
    const idx = ai.segs.indexOf(cl.seg);
    if (idx < 0) return drop(cl);                                    // Routenstück schon abgeräumt: längst vorbei
    if (idx === j + 1 && beforeLine && stillT > HOLD_STILL_S) return drop(cl); // steht vor der Linie
    if (cl.kind === 'mark') return idx > j;                           // bis das Auto auf dem Stück ist
    if (cl.kind === 'n') {                                            // bis es die Engstelle (alle Abschnitte) verlassen hat
      if (idx >= j) return true;
      const cur = ai.segs[j]?.lane;
      return (cur?.narrow && narrowKey(world.city, cur.edge) === cl.edge) || drop(cl);
    }
    if (idx > j) return true;                                         // noch vor oder in der Kreuzung
    if (idx < j) return drop(cl);                                     // schon auf einem späteren Stück
    const nd = world.city.nodes.get(cl.v);
    return (!nd || Math.hypot(car.x - nd.x, car.y - nd.y) <= (nd.trim ?? 0) + 30) || drop(cl); // auf dem Stück: bis das Heck aus der Kreuzung ist (dicht aufeinanderfolgende Knoten dürfen sich nicht gegenseitig halten)
  });
  for (const cl of dropped) {
    if (cl.kind === 'j' && !ai.claims.some(ownsJ(cl.v))) world.jres?.get(cl.v)?.cars.delete(car.id);
    if (cl.kind === 'n' && !ai.claims.some(ownsN(cl.edge))) world.nres?.get(cl.edge)?.cars.delete(car.id);
  }
}

// Auto aus allen Reservierungen nehmen (beim Abbauen/Umplanen).
export function dropClaims(car, world) {
  for (const cl of car.ai?.claims ?? []) {
    if (cl.kind === 'j') world.jres?.get(cl.v)?.cars.delete(car.id);
    if (cl.kind === 'n') world.nres?.get(cl.edge)?.cars.delete(car.id);
  }
  if (car.ai) car.ai.claims = [];
}
