// Stadtleben: Menschen, die nicht nur gehen, sondern an echten Orten etwas tun – abhängig von Ort, Uhrzeit und Wochentag.
//   Haltestellen: Wartende · Bars/Kneipen: Rauchergruppen · Clubs (Fr/Sa-Nacht): Schlange · Späti: Leute mit Flasche
//   Cafés: Gäste an Tischen · Imbiss/Restaurant: Plaudernde · Läden/Kultur: Schaufenstergucker · U-Bahnhof: Straßenmusik
//   Bänke (OSM): Sitzende · Grünflächen: Gruppen auf Decken
// lifeSpots() ist rein rechnerisch und deterministisch (gleicher Ort + gleiche Stunde = gleiche Szene); manageLife()
// setzt daraus echte Passanten (Zustand 'hang') – außer Sicht erzeugt, außer Sicht abgebaut. Werden sie erschreckt,
// fliehen sie wie alle anderen und gehen danach normal weiter.
import { hash01, nearestEdge, inBuilding, onRoad } from './map.js';
import { pointInRings } from './geom.js';
import { AREA_KIND, FURN_KIND } from './citycodes.js';
import { peopleLevel, nightlife, isWeekend } from './rhythm.js';

export const LIFE = { radius: 1500, viewHalfX: 760, viewHalfY: 470, despawn: 1900, every: 0.5, maxHangers: 70 };
export const ACTS = ['wait', 'smoke', 'queue', 'drink', 'sit', 'chat', 'browse', 'music', 'lie'];

const h = (...n) => hash01(n.reduce((a, b) => a * 31 + Math.round(b), 7));
const inHours = (m, from, to) => (from <= to ? m >= from && m < to : m >= from || m < to);

// Was an einem POI gerade los ist: { act, n } oder null
export function activityFor(q, minutes, day) {
  const m = ((minutes % 1440) + 1440) % 1440, hr = Math.floor(m / 60), r = h(q.x, q.y, hr, day);
  const people = peopleLevel(m, day), night = nightlife(m, day);
  switch (q.cat) {
    case 'bus': return inHours(m, 300, 60) ? { act: 'wait', n: Math.floor(people * 3 + r) } : null;
    case 'ubahn': case 'sbahn':
      if (inHours(m, 600, 1200) && r < 0.3) return { act: 'music', n: 1 };
      return inHours(m, 300, 60) ? { act: 'wait', n: Math.floor(people * 2 + r) } : null;
    case 'drink':
      if (q.kind === 'nightclub') return inHours(m, 1380, 360) && night > 0.5 ? { act: 'queue', n: 6 + Math.floor(r * 10) } : null;
      return inHours(m, 1080, 180) ? { act: 'smoke', n: Math.floor(1 + night * 3 + r * 2) } : null;
    case 'supermarket': case 'shop':
      if (q.kind === 'convenience' || q.kind === 'kiosk') return inHours(m, 1020, 180) ? { act: 'drink', n: Math.floor(night * 3 + people + r * 1.5) } : null;
      return inHours(m, 540, 1200) && r < 0.35 ? { act: 'browse', n: 1 } : null;
    case 'cafe': return inHours(m, 480, 1140) ? { act: 'sit', n: Math.floor(people * 3 + r) } : null;
    case 'food': return inHours(m, 660, 1380) ? { act: 'chat', n: Math.floor(people * 2 + r) } : null;
    case 'culture': case 'mall': return inHours(m, 600, 1200) && r < 0.5 ? { act: 'browse', n: 1 + Math.floor(r * 2) } : null;
    default: return null;
  }
}

// Stelle vor dem Laden auf dem Gehweg (zur nächsten Straße hin) und die Blickrichtung dorthin
function frontOf(city, q) {
  if (q._front !== undefined) return q._front;
  const S = city.scale, e = nearestEdge(city, q.x, q.y, 40 * S, (o) => o.cls <= 9);
  let f = null;
  if (e) {
    const dx = q.x - e.x, dy = q.y - e.y, d = Math.hypot(dx, dy) || 1;
    const off = Math.min(d, e.e.w / 2 + 2.2 * S);
    const x = e.x + dx / d * off, y = e.y + dy / d * off;
    if (!inBuilding(city, x, y) && !onRoad(city, x, y)) f = { x, y, ux: e.ux, uy: e.uy, face: Math.atan2(-dy, -dx) }; // Blick zur Straße
  }
  return (q._front = f);
}

// Anordnung einer Gruppe um (x, y): Schlange entlang des Gehwegs, sonst im Kreis (Gesichter zur Mitte)
function arrange(spot, n, act, seed, out, keyBase) {
  for (let i = 0; i < n; i++) {
    let x, y, face;
    if (act === 'queue') { x = spot.x + spot.ux * i * 9; y = spot.y + spot.uy * i * 9; face = Math.atan2(-spot.uy, -spot.ux); }
    else if (act === 'wait' || act === 'browse' || act === 'music') { x = spot.x + spot.ux * (i - (n - 1) / 2) * 12; y = spot.y + spot.uy * (i - (n - 1) / 2) * 12; face = act === 'browse' ? spot.face + Math.PI : spot.face; }
    else { // Gruppe: kleiner Kreis
      const a = seed * 6.28 + i / Math.max(1, n) * Math.PI * 2, r = n === 1 ? 0 : 8 + n;
      x = spot.x + Math.cos(a) * r; y = spot.y + Math.sin(a) * r; face = a + Math.PI;
    }
    out.push({ key: `${keyBase}:${i}`, x, y, face, act, g: keyBase, gx: spot.x, gy: spot.y });
  }
}

// Bank: Ausrichtung parallel zur nächsten Straße (OSM kennt sie meist nicht)
export function benchAngle(city, f) {
  if (f.angle !== undefined) return f.angle;
  const e = nearestEdge(city, f.x, f.y, 25 * city.scale);
  return (f.angle = e ? Math.atan2(e.uy, e.ux) : (f.seed % 628) / 100);
}

// Parklevel: Grünflächen füllen sich nachmittags, am Wochenende mehr
export function parkLevel(minutes, day) {
  const m = ((minutes % 1440) + 1440) % 1440;
  const t = m < 600 || m > 1260 ? 0 : m < 780 ? (m - 600) / 180 : m < 1140 ? 1 : 1 - (m - 1140) / 120;
  return t * (isWeekend(day) ? 1 : 0.55);
}

// Alle gewünschten Plätze im Umkreis
export function lifeSpots(city, cx, cy, minutes, day, radius = LIFE.radius) {
  const out = [], box = { x: cx - radius, y: cy - radius, w: 2 * radius, h: 2 * radius };
  const hr = Math.floor((((minutes % 1440) + 1440) % 1440) / 60);
  for (const q of city.poiHash.query(box, [])) {
    const a = activityFor(q, minutes, day);
    if (!a || a.n <= 0) continue;
    const f = frontOf(city, q);
    if (f) arrange(f, Math.min(a.n, a.act === 'queue' ? 16 : 6), a.act, h(q.x, q.y), out, `p${q.x},${q.y}`);
  }
  // Der Späti aus dem Auftrag: tagsüber ein, zwei Leute davor, abends Stammgäste mit Flasche
  const sp = city.places?.giver;
  if (sp && Math.abs(sp.x - cx) < radius && Math.abs(sp.y - cy) < radius) {
    const m = ((minutes % 1440) + 1440) % 1440, night = nightlife(m, day);
    const n = inHours(m, 1020, 240) ? 2 + Math.round(night * 3) : inHours(m, 480, 1020) ? 1 : 0;
    if (n) {
      const q = { x: sp.x, y: sp.y, cat: 'supermarket', kind: 'convenience' }, f = frontOf(city, q);
      if (f) arrange({ ...f, x: f.x + f.ux * 30, y: f.y + f.uy * 30 }, n, inHours(m, 1020, 240) ? 'drink' : 'chat', 0.3, out, 'spaeti');
    }
  }
  const occ = inHours(minutes, 480, 1200) ? 0.35 : inHours(minutes, 1200, 1380) ? 0.2 : 0.04;
  const park = parkLevel(minutes, day);
  for (const f of city.render.query(box, [])) {
    if (f.layer === 'furn' && f.kind === FURN_KIND.bench) {
      if (h(f.x, f.y, hr, day) >= occ) continue;
      const a = benchAngle(city, f), n = h(f.y, f.x, hr) < 0.4 ? 2 : 1;
      for (let i = 0; i < n; i++) {
        const u = n === 1 ? 0 : (i - 0.5) * 10;
        out.push({ key: `b${f.x},${f.y}:${i}`, x: f.x + Math.cos(a) * u, y: f.y + Math.sin(a) * u, face: a + Math.PI / 2, act: 'sit', g: `b${f.x},${f.y}`, bench: true });
      }
    } else if (f.layer === 'area' && f.kind === AREA_KIND.grass && park > 0) {
      const b = f.bbox ?? null;
      if (!b || b.w * b.h < 400000) continue; // unter ~4 000 m² Hüllfläche: Beet, Mittelstreifen
      const groups = Math.min(5, Math.floor(b.w * b.h / 1500000 + 1) * park + h(b.x, b.y, hr) * 0.9);
      for (let g = 0; g < groups; g++) {
        let spot = null;
        for (let t = 0; t < 12 && !spot; t++) {
          const x = b.x + h(b.x, g, t, 1) * b.w, y = b.y + h(b.y, g, t, 2) * b.h;
          if (pointInRings(x, y, f.rings) && !inBuilding(city, x, y) && !onRoad(city, x, y)) spot = { x, y, ux: 1, uy: 0, face: 0 };
        }
        if (spot) arrange(spot, 2 + Math.floor(h(b.x, b.y, g, day) * 3), 'lie', h(g, b.x), out, `a${Math.round(b.x)},${Math.round(b.y)}:${g}`);
      }
    }
  }
  return out;
}

// Spaziergänger-Varianten (beim Erzeugen gewürfelt): Jogger morgens und abends, Hundehalter tagsüber
export function walkerStyle(minutes, rnd) {
  const m = ((minutes % 1440) + 1440) % 1440;
  const jog = inHours(m, 360, 540) || inHours(m, 1020, 1260) ? 0.14 : inHours(m, 540, 1020) ? 0.04 : 0;
  const dog = inHours(m, 360, 1320) ? 0.09 : 0.02;
  const r = rnd();
  return r < jog ? 'jog' : r < jog + dog ? 'dog' : null;
}
