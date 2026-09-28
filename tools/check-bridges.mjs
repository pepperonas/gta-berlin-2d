// Brückenprüfung: jede Brückenfahrbahn (Ebene ≥ 1, Straße bis Klasse 8) wird in jeder erlaubten Richtung auf dem
// rechten Fahrstreifen abgefahren – 40 m Anlauf auf der anschließenden Straße, die Brücke, 40 m Auslauf. Das Fahrzeug
// führt seine Ebene wie im Spiel (initialLevel/stepLevel) und stößt gegen alles, was es auf seiner Ebene sperrt
// (car.js blocks; Poller zählen nicht, die fährt das Auto um). Gemeldet werden Sperren und Ebenen, die nicht passen.
//
//   node tools/check-bridges.mjs            # ganz Berlin (blockweise, ~1 GB)
//   node tools/check-bridges.mjs 52.4965 13.4585 1500   # nur Brücken im Umkreis (m) eines Punkts
import { blocks } from '../web/src/car.js';
import { initialLevel, stepLevel } from '../web/src/levels.js';
import { obbVsSegment, circleVsObb, obbVsRect, obbBounds } from '../web/src/collision.js';
import { laneOffsets } from '../web/src/street.js';
import { offsetPolyline } from '../web/src/geom.js';

const CAR = { hw: 21, hh: 9 }, STEP = 4, RUN = 400; // px: Fahrzeug, Schrittweite, Anlauf/Auslauf (40 m)
const tmp = [];

// Stück einer Polylinie (x,y-Paare) bis zur Länge len ab dem Anfang
function head(pts, len) {
  const out = [pts[0], pts[1]];
  let acc = 0;
  for (let i = 0; i < pts.length - 2; i += 2) {
    const dx = pts[i + 2] - pts[i], dy = pts[i + 3] - pts[i + 1], L = Math.hypot(dx, dy);
    if (acc + L >= len) { const f = (len - acc) / (L || 1); out.push(pts[i] + dx * f, pts[i + 1] + dy * f); return out; }
    acc += L; out.push(pts[i + 2], pts[i + 3]);
  }
  return out;
}
const rev = (p) => { const o = []; for (let i = p.length - 2; i >= 0; i -= 2) o.push(p[i], p[i + 1]); return o; };

// Liegt (x, y) quer neben der Brückenachse (Lotfußpunkt nicht vor dem ersten oder hinter dem letzten Punkt)? Am
// Innenrand einer scharfen Kurve beginnt der versetzte Fahrweg vor dem Brückenanfang – dort ist man noch nicht drauf.
function besideAxis(bp, x, y) {
  let best = Infinity, inside = false;
  for (let i = 0; i < bp.length - 2; i += 2) {
    const dx = bp[i + 2] - bp[i], dy = bp[i + 3] - bp[i + 1], L2 = dx * dx + dy * dy || 1;
    const t = ((x - bp[i]) * dx + (y - bp[i + 1]) * dy) / L2, tc = Math.max(0, Math.min(1, t));
    const d = Math.hypot(bp[i] + dx * tc - x, bp[i + 1] + dy * tc - y);
    if (d < best) { best = d; inside = !((i === 0 && t < 0) || (i === bp.length - 4 && t > 1)); }
  }
  return inside;
}

// Anschluss an einem Knoten: die Straße (Klasse ≤ 8, nicht die Brücke selbst), die am geradesten weiterführt;
// dir = Fahrtrichtung am Knoten (Einheitsvektor). Liefert Punkte, die am Knoten beginnen und von ihm wegführen.
function continuation(city, e, node, dir) {
  let best = null;
  for (const k of city.nodes.get(node)?.edges ?? []) {
    const o = city.edges.get(k);
    if (!o || o === e || o.cls > 8 || o.passage) continue;
    const pts = o.a === node ? o.pts : rev(o.pts);
    const dx = pts[2] - pts[0], dy = pts[3] - pts[1], L = Math.hypot(dx, dy) || 1;
    const straight = (dx * dir[0] + dy * dir[1]) / L;
    if (!best || straight > best.straight) best = { o, pts, straight };
  }
  return best;
}

// Eine Fahrt über die Brücke e von Knoten from nach to im Querversatz off (px, + rechts; null = rechter Fahrstreifen)
function drive(city, world, e, forward, offset = null, log = null) {
  const bp = forward ? e.pts : rev(e.pts), from = forward ? e.a : e.b, to = forward ? e.b : e.a;
  const inDir = [bp[0] - bp[2], bp[1] - bp[3]], inL = Math.hypot(...inDir) || 1;
  const n = bp.length, outDir = [bp[n - 2] - bp[n - 4], bp[n - 1] - bp[n - 3]], outL = Math.hypot(...outDir) || 1;
  const before = continuation(city, e, from, [inDir[0] / inL, inDir[1] / inL]);
  const after = continuation(city, e, to, [outDir[0] / outL, outDir[1] / outL]);
  const lead = before ? rev(head(before.pts, RUN)) : [];
  const tail = after ? head(after.pts, RUN) : [];
  const path = [...lead.slice(0, -2), ...bp, ...tail.slice(2)];
  // rechter Fahrstreifen der Brücke (Versatz + rechts der Fahrtrichtung)
  const lo = laneOffsets(e.cs, city.scale);
  const lanes = forward ? lo.fwd : lo.bwd.map((x) => -x);
  const off = offset ?? (lanes.length ? Math.max(...lanes) : 0);
  const pts = offsetPolyline(path, off);
  const startBridge = lead.length ? lead.length - 2 : 0, endBridge = startBridge + bp.length - 2; // Punktindizes der Brücke
  const found = [];
  const car = { x: pts[0], y: pts[1], angle: Math.atan2(pts[3] - pts[1], pts[2] - pts[0]), ...CAR, lvl: undefined };
  car.lvl = before ? (before.o.lvl ?? 0) : initialLevel(city, car.x, car.y, car.angle); // wer von dort kommt, ist auf deren Ebene
  let wrongLvl = 0, onBridgeSteps = 0;
  // Wegpunkte in Schritten; die Fahrzeugachse folgt der Sehne 1,5 m davor bis 1,5 m danach (so lenkt ein Fahrer durch
  // eine Biegung – ein harter Knick am Polylinienpunkt ließe die Ecke über den Bordstein schwenken)
  const samples = [];
  for (let i = 0; i < pts.length - 2; i += 2) {
    const ax = pts[i], ay = pts[i + 1], dx = pts[i + 2] - ax, dy = pts[i + 3] - ay, L = Math.hypot(dx, dy);
    const steps = Math.max(1, Math.ceil(L / STEP)), onBridge = i >= startBridge && i < endBridge;
    for (let k = 1; k <= steps; k++) samples.push([ax + dx * k / steps, ay + dy * k / steps, onBridge]);
  }
  const look = Math.max(1, Math.round(15 / STEP));
  for (let j = 0; j < samples.length; j++) {
    const [x, y, onBridge] = samples[j], a = samples[Math.max(0, j - look)], b = samples[Math.min(samples.length - 1, j + look)];
    car.x = x; car.y = y; car.angle = Math.atan2(b[1] - a[1], b[0] - a[0]);
    const was = car.lvl; stepLevel(city, car);
    if (log && was !== car.lvl) log.push(`Ebene ${was} → ${car.lvl} bei ${Math.round(car.x)},${Math.round(car.y)}${onBridge ? ' (Brücke)' : ''}`);
    if (onBridge && besideAxis(bp, car.x, car.y)) { onBridgeSteps++; if (car.lvl !== e.lvl) wrongLvl++; }
    if (!onBridge) continue; // An- und Abfahrt: Kanten, Bordsteine dort sind nicht Sache der Brücke
    for (const sol of world.solids.query(obbBounds(car), tmp)) {
      if (sol.layer === 'barrier' || sol.kind === 'border' || !blocks(world, sol, car.lvl)) continue; // Stadtgrenze: Kartenende, keine Sperre
      const m = sol.seg ? obbVsSegment(car, sol) : sol.r !== undefined ? circleVsObb(sol.x, sol.y, sol.r, car) : obbVsRect(car, sol);
      if (!m || m.depth < 1.5) continue;
      found.push({ x: Math.round(car.x), y: Math.round(car.y), lvl: car.lvl, what: sol.seg ? (sol.kind === 'wall' ? wallName(city, sol) : sol.kind) : sol.r !== undefined ? 'Baum' : 'Kiste', onBridge });
      break;
    }
    if (found.length) break;
  }
  // falsche Ebene zählt ab 3 m Strecke (ein Wechsel 1 m hinter dem Knoten ist auf einem 10-m-Stück schon 10 %)
  return { hit: found[0] ?? null, wrongLvl: onBridgeSteps ? wrongLvl / onBridgeSteps : 0, wrongM: wrongLvl * STEP / city.scale, before: before?.o, after: after?.o };
}
function wallName(city, s) { return `Wand(Ebene ${s.lvl ?? 0})`; }

// Eine Fahrt mit Protokoll der Ebenenwechsel (zur Fehlersuche)
export function driveTrace(city, e, forward, offset) { const log = []; const r = drive(city, { solids: city.solids, knocked: new Map() }, e, forward, offset, log); log.push(JSON.stringify(r.hit)); return { ...r, log }; }

// Alle Brücken im Rechteck prüfen: [{ e, forward, hit, wrongLvl }]
export function checkBridges(city, box = null) {
  const world = { solids: city.solids, knocked: new Map() }, out = [];
  for (const e of city.edges.values()) {
    if (!(e.lvl >= 1) || e.cls > 8 || e.passage || !e.cs) continue;
    const mx = (e.pts[0] + e.pts[e.pts.length - 2]) / 2, my = (e.pts[1] + e.pts[e.pts.length - 1]) / 2;
    if (box && (mx < box[0] || mx > box[2] || my < box[1] || my > box[3])) continue;
    const dirs = e.oneway === 1 ? [true] : e.oneway === -1 ? [false] : [true, false];
    // über die ganze Breite: Fahrbahn samt Lücke zur Gegenfahrbahn, alle 1,5 m (das Auto bleibt mit der Karosserie darauf)
    const S = city.scale, half = e.w / 2, lo = -half - (e.fill < 0 ? -e.fill : 0), hi = half + (e.fill > 0 ? e.fill : 0);
    for (const forward of dirs) {
      const offs = [null];
      for (let o = lo + CAR.hh; o <= hi - CAR.hh + 0.1; o += 1.5 * S) offs.push(forward ? o : -o);
      if (hi - lo < 2 * CAR.hh) offs.push(0);
      for (const off of offs) {
        const r = drive(city, world, e, forward, off);
        if (r.hit || (r.wrongLvl > 0.2 && r.wrongM > 3)) { out.push({ e, forward, off, ...r }); break; }
      }
    }
  }
  return out;
}

// Ganz Berlin in Blöcken (passt so in den normalen Speicher): Block samt Rand laden, dessen Brücken prüfen, entladen
export function checkAllBridges(city, block = 8) {
  const T = city.tile, out = [];
  let count = 0;
  for (let bx = 0; bx * block * T < city.width; bx++) for (let by = 0; by * block * T < city.height; by++) {
    const x0 = bx * block * T, y0 = by * block * T, x1 = x0 + block * T, y1 = y0 + block * T;
    const before = new Set(city.pinned);
    city.loadArea(x0 - T, y0 - T, x1 + T, y1 + T, { pin: true });
    for (const e of city.edges.values()) { const mx = (e.pts[0] + e.pts[e.pts.length - 2]) / 2, my = (e.pts[1] + e.pts[e.pts.length - 1]) / 2; if (e.lvl >= 1 && e.cls <= 8 && !e.passage && mx >= x0 && mx < x1 && my >= y0 && my < y1) count++; }
    out.push(...checkBridges(city, [x0, y0, x1, y1]));
    for (const k of [...city.pinned]) if (!before.has(k)) city.unload(k);
  }
  out.count = count;
  return out;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const { openRealCity, realIndex, geoToPx } = await import('../tests/helpers/city.js');
  const city = openRealCity(), meta = realIndex().meta;
  let box = null;
  if (process.argv[2]) {
    const [x, y] = geoToPx(meta, +process.argv[2], +process.argv[3]), r = (+process.argv[4] || 1500) * city.scale;
    box = [x - r, y - r, x + r, y + r];
    city.loadArea(x - r - 2000, y - r - 2000, x + r + 2000, y + r + 2000);
  }
  const t0 = Date.now(), bad = box ? checkBridges(city, box) : checkAllBridges(city);
  const n = bad.count ?? [...city.edges.values()].filter((e) => e.lvl >= 1 && e.cls <= 8 && !e.passage).length;
  for (const b of bad) console.log(`${b.e.name || '(ohne Namen)'} #${b.e.id} [${Math.round(b.e.pts[0])},${Math.round(b.e.pts[1])}] ${b.forward ? 'vor' : 'zurück'} Versatz ${b.off === null ? 'Spur' : Math.round(b.off / city.scale * 10) / 10 + ' m'}: ${b.hit ? `${b.hit.what} bei ${b.hit.x},${b.hit.y} auf Ebene ${b.hit.lvl}${b.hit.onBridge ? ' (auf der Brücke)' : ' (An-/Abfahrt)'}` : ''}${b.wrongLvl > 0.2 ? ` falsche Ebene ${Math.round(b.wrongLvl * 100)} % (${Math.round(b.wrongM)} m)` : ''}`);
  console.log(`${bad.length} Befunde bei ${n} Brückenkanten in ${Date.now() - t0} ms`);
}
