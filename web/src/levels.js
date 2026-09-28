// Ebene je Objekt (rein, ohne Zufall): Autos, Personen, Räder und die Spielfigur tragen `lvl` (0 = Boden, ≥ 1 Brücke,
// < 0 offene Unterführung). Die Ebene wechselt nur in Portalen – Knoten, die Wege verschiedener Ebenen teilen
// (Brückenkopf, Rampe, Treppe). Wer eine Brücke nur unterquert oder überquert, bleibt auf seiner Ebene.
//  – Im Portal gilt die Straße bzw. der Weg, auf dem man tatsächlich ist: der Lotfußpunkt liegt innerhalb des Stücks
//    (nicht an dessen Ende). Auf der eigenen Fahrbahn bleibt man, wo man ist.
//  – Außerhalb von Portalen bleibt die Ebene; nur wenn unter einem Objekt auf Brücke/Unterführung weit und breit keine
//    Fläche seiner Ebene mehr liegt (Sicherheitsnetz), fällt es auf den Boden zurück.
import { AREA_KIND } from './citycodes.js';
import { pointInRings } from './geom.js';

export const LEVEL = { pathHalf: 12, margin: 5, check: 10, lost: 30, slack: 25, maxHalf: 100 }; // px (10 px = 1 m) bzw. Schritte
const q = [];
const box = { x: 0, y: 0, w: 0, h: 0 };
const around = (x, y, r) => { box.x = x - r; box.y = y - r; box.w = 2 * r; box.h = 2 * r; return box; };

export const sameLvl = (a, b) => (a?.lvl ?? 0) === (b?.lvl ?? 0);

// Reichweite einer Straße zu einer Seite (+ rechts der Kantenrichtung): Fahrbahn + Lücke zur Gegenfahrbahn + Radweg
function reach(e, side) {
  const tr = side > 0 ? e.cs?.right?.track : e.cs?.left?.track;
  return e.w / 2 + (e.fill && Math.sign(e.fill) === side ? Math.abs(e.fill) : 0) + (tr ? 4 + tr : 0);
}

// Flächenstücke um (x, y): { lvl, d (Abstand zur Achse), t (Lotfußpunkt 0…1), half (Reichweite), angle }
function pieces(city, x, y, r, out) {
  out.length = 0;
  for (const s of city.edgeSegs.query(around(x, y, r), q)) {
    const e = s.e;
    if (e.junction) continue;
    const dx = s.bx - s.ax, dy = s.by - s.ay, L2 = dx * dx + dy * dy || 1;
    const t = ((x - s.ax) * dx + (y - s.ay) * dy) / L2, tc = Math.max(0, Math.min(1, t));
    const d = Math.hypot(s.ax + dx * tc - x, s.ay + dy * tc - y), side = Math.sign((x - s.ax) * -dy + (y - s.ay) * dx) || 1;
    out.push({ lvl: e.lvl ?? 0, d, t, half: reach(e, side), angle: Math.atan2(dy, dx), ends: e.pts });
  }
  for (const f of city.render.query(around(x, y, r), q)) {
    if (f.layer !== 'path' || !f.lvl) continue; // Wege am Boden zählen nicht als eigene Fläche
    const p = f.pts;
    for (let i = 0; i < p.length - 2; i += 2) {
      const dx = p[i + 2] - p[i], dy = p[i + 3] - p[i + 1], L2 = dx * dx + dy * dy || 1;
      const t = ((x - p[i]) * dx + (y - p[i + 1]) * dy) / L2, tc = Math.max(0, Math.min(1, t));
      const d = Math.hypot(p[i] + dx * tc - x, p[i + 1] + dy * tc - y);
      if (d < r) out.push({ lvl: f.lvl, d, t, half: LEVEL.pathHalf, angle: Math.atan2(dy, dx), path: p });
    }
  }
  return out;
}
const tmpPieces = [], tmpPortals = [];

// Liegt unter (x, y) eine Fläche der Ebene lvl (Straße, Brückenweg, Kreuzungsscheibe, Brückendeck)? r = Toleranz
export function levelHere(city, x, y, lvl, r = LEVEL.margin) {
  if (lvl === 0) return true; // Boden ist überall
  for (const p of pieces(city, x, y, 60 + r, tmpPieces)) if (p.lvl === lvl && p.d <= p.half + r) return true;
  for (const s of city.edgeSegs.query(around(x, y, 1), q)) {
    const j = s.e.junction;
    if (j && lvl >= (j.lo ?? 0) && lvl <= (j.hi ?? 0) && Math.hypot(x - j.x, y - j.y) <= j.r + r) return true;
  }
  for (const f of city.polys.query(around(x, y, 1), q)) if (f.kind === AREA_KIND.bridge && (f.lvl || 1) === lvl && pointInRings(x, y, f.rings)) return true;
  return false;
}

// Ebene beim Erzeugen/Teleport: die Straße, auf der man steht und die in Blickrichtung verläuft (angle optional);
// sonst Boden, wenn Boden-Fläche da ist; sonst die einzige vorhandene Ebene.
export function initialLevel(city, x, y, angle = null, r = 30) {
  let best = null, ground = false;
  for (const p of pieces(city, x, y, 60 + r, tmpPieces)) {
    if (p.d > p.half + r) continue;
    if (p.lvl === 0) ground = true;
    let da = 0;
    if (angle !== null) { da = Math.abs(((angle - p.angle) % Math.PI + Math.PI) % Math.PI); da = Math.min(da, Math.PI - da); }
    const score = da * 100 + p.d;
    if (!best || score < best.score) best = { lvl: p.lvl, score, aligned: da < 0.35 };
  }
  if (!best) return 0;
  if (angle !== null && best.aligned) return best.lvl;
  return ground ? 0 : best.lvl;
}

// Ebene eines Objekts einen Schritt weiterführen (obj.x, obj.y, obj.lvl; obj._lvlT zählt fürs Sicherheitsnetz)
// Führt ein Stück durch das Portal (Kante endet dort, Weg hat dort einen Punkt)? Nur solche zählen im Portal –
// eine Straße, die bloß darunter oder darüber hindurchführt (A 100 unter dem Kaiserdamm), entscheidet nichts.
function through(p, portals) {
  for (const portal of portals) {
    if (p.lvl < portal.lo || p.lvl > portal.hi) continue;
    const near = (x, y) => Math.abs(x - portal.x) < 3 && Math.abs(y - portal.y) < 3;
    if (p.ends) { const e = p.ends; if (near(e[0], e[1]) || near(e[e.length - 2], e[e.length - 1])) return true; continue; }
    if (!p.path) return true; // ohne Geometrie (Testattrappe): zählt
    for (let i = 0; i < p.path.length; i += 2) if (near(p.path[i], p.path[i + 1])) return true;
  }
  return false;
}

const aligned = (a, b) => { let d = Math.abs(((a - b) % Math.PI + Math.PI) % Math.PI); d = Math.min(d, Math.PI - d); return d < 0.3; }; // bis 17°: man fährt auf ihr entlang

export function stepLevel(city, obj) {
  const L = obj.lvl ?? 0;
  // nur Portale, die die eigene Ebene verbinden (wer auf der Brücke über das Portal einer tieferen Brücke fährt, bleibt
  // oben); liegen mehrere übereinander (Richtungsfahrbahnen), zählen alle
  const portals = tmpPortals; portals.length = 0;
  for (const p of city.portals.query(around(obj.x, obj.y, 1), q)) if (L >= p.lo && L <= p.hi && Math.hypot(obj.x - p.x, obj.y - p.y) <= p.r) portals.push(p);
  if (portals.length) {
    const reachR = Math.max(...portals.map((p) => p.r));
    // Im Portal zählt die Fläche, auf der man ist oder der man am nächsten ist (Lotfußpunkt innerhalb des Stücks):
    // je Ebene der kleinste Überstand über die Breite (d − Reichweite, ≤ 0 = drauf). Auf der eigenen bleibt man, solange
    // man auf ihr ist; sonst gilt die nächste bis LEVEL.slack daneben – wer am Rand der Brücke auffährt oder mit einer
    // Radseite neben der schmalen Fahrbahn, kommt trotzdem hinauf. Auf keiner (Gehweg, Treppenfuß daneben): nichts.
    // Liegen beide Flächen unter einem (breite Straße am Brückenkopf), gilt die, deren Achse man deutlich näher ist
    // (relativer Abstand d/Reichweite unter der Hälfte) – sofern sie in Fahrtrichtung verläuft (wer auf der breiten
    // Heerstraße geradeaus fährt, streift den Anfang der Abfahrt, bleibt aber oben).
    let own = Infinity, ownRel = Infinity, best = null, bestEx = Infinity, bestRel = Infinity, bestAligned = false;
    const heading = Number.isFinite(obj.angle) ? obj.angle : null;
    for (const p of pieces(city, obj.x, obj.y, reachR + 60, tmpPieces)) {
      // Lotfußpunkt im Stück nötig – außer für ein Stück der eigenen Ebene, das nicht am Portal endet: am Knoten zwischen
      // zwei solchen Stücken fällt der Lotfußpunkt innen in der Biegung aus beiden heraus, man ist aber drauf
      const thru = through(p, portals);
      if ((p.lvl !== L || thru) && (p.t <= 0.001 || p.t >= 0.999)) continue;
      // Eigene Ebene: nur Straßen, auf denen man entlangfährt (bis 17°; ohne Fahrtrichtung jede durchs Portal) – auch
      // wenn sie nicht durchs Portal führt (A 100 an der Auffahrt vorbei); eine, die man nur quert oder die schräg
      // abzweigt (A 100 unter dem Kaiserdamm, Zufahrt an der Kiefholzstraße), hält niemanden unten.
      // Andere Ebene: nur Stücke, die durchs Portal führen (die Brücke nebenan zählt nicht).
      if (p.lvl === L ? (heading !== null ? !aligned(heading, p.angle) : !thru) : !thru) continue;
      const half = Math.min(p.half, LEVEL.maxHalf); // unplausible Breiten (OSM: 30 m auf 7 m Länge) nicht über den Brückenkopf
      const ex = p.d - half, rel = p.d / (half || 1);
      if (p.lvl === L) { own = Math.min(own, ex); ownRel = Math.min(ownRel, rel); }
      else if (ex < bestEx) { bestEx = ex; bestRel = rel; best = p; bestAligned = heading === null || aligned(heading, p.angle); }
    }
    if (best && bestEx <= LEVEL.slack && ((own > 0 && bestEx < own) || (bestEx <= 0 && bestAligned && bestRel < 0.5 * ownRel))) obj.lvl = best.lvl;
    obj._lvlT = 0;
    return obj.lvl ?? 0;
  }
  if (L !== 0 && (obj._lvlT = (obj._lvlT ?? 0) + 1) >= LEVEL.check) { // Sicherheitsnetz: keine Fläche der eigenen Ebene mehr in der Nähe
    obj._lvlT = 0;
    if (!levelHere(city, obj.x, obj.y, L, LEVEL.lost)) obj.lvl = 0;
  }
  return obj.lvl ?? 0;
}

// Können sich zwei Objekte berühren (Stoß, Kontakt, Hindernis für die KI, Schuss)? Auf derselben Ebene immer; auf
// verschiedenen nur, wenn beide im selben Portal stehen, das beide Ebenen verbindet (am Rampenfuß wechseln zwei dicht
// hintereinander fahrende Autos nicht im selben Moment die Ebene – sie dürfen dort nicht durcheinander hindurchfahren).
export function touch(city, a, b) {
  const la = a.lvl ?? 0, lb = b.lvl ?? 0;
  if (la === lb) return true;
  if (!city?.portals) return false;
  for (const p of city.portals.query(around(a.x, a.y, 1), qTouch)) {
    if (la < p.lo || la > p.hi || lb < p.lo || lb > p.hi) continue;
    if (Math.hypot(a.x - p.x, a.y - p.y) <= p.r + 30 && Math.hypot(b.x - p.x, b.y - p.y) <= p.r + 30) return true;
  }
  return false;
}
const qTouch = [];
