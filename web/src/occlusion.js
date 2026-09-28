// Verdeckung der Spielfigur bzw. ihres Autos (rein, ohne Canvas): Wird sie im Bild von etwas übermalt, das in der
// Zeichenfolge nach ihr kommt – eine Baumkrone, ein Haus (Dach oder Fassade, auch in der Tordurchfahrt) oder ein
// Viadukt/eine Bahnbrücke –, zeichnet der Renderer ihre Umrisse obendrauf.
import { pointInRings, segDist2 } from './geom.js';

// Dachversatz eines Hauses in der Schrägansicht (wie render.js drawBuilding)
export function roofOffset(b, cam, heightScale) {
  const H = Math.max(18, b.height * heightScale);
  return { H, dx: (b.cx - cam.x) * H * 0.0005, dy: -H * 0.5 + (b.cy - cam.y) * H * 0.00025 };
}

// Krone eines Baums (wie assets.js drawTree): Mittelpunkt über dem Stamm, Radius = Größe
export const crownOf = (tr) => ({ x: tr.x, y: tr.y - Math.min(tr.size * 0.5, 30), r: tr.size });

// target: { x, y, key (Tiefenschlüssel wie in der Zeichenliste) } → 'tree' | 'building' | 'bridge' | null
export function coverOf(target, { trees = [], buildings = [], bridges = [], deck = 46, cam, heightScale }) {
  const { x, y, key } = target;
  for (const f of bridges) { // über allem gezeichnet
    const p = f.pts;
    for (let i = 0; i < p.length - 2; i += 2) if (segDist2(x, y, p[i], p[i + 1], p[i + 2], p[i + 3]) < (deck / 2) ** 2) return 'bridge';
  }
  for (const b of buildings) {
    if (b.bbox.y + b.bbox.h <= key) continue; // vorher gezeichnet
    const { dx, dy } = roofOffset(b, cam, heightScale);
    const bb = b.bbox;
    if (x < bb.x + Math.min(0, dx) - 2 || x > bb.x + bb.w + Math.max(0, dx) + 2 || y < bb.y + Math.min(0, dy) - 2 || y > bb.y + bb.h + Math.max(0, dy) + 2) continue;
    for (const k of [1, 0.8, 0.6, 0.4, 0.2, 0.08]) if (pointInRings(x - dx * k, y - dy * k, b.rings)) return 'building';
  }
  for (const tr of trees) {
    if (tr.y <= key) continue;
    const c = crownOf(tr);
    if (Math.hypot(x - c.x, y - c.y) < c.r * 0.9) return 'tree';
  }
  return null;
}

// Alle Verdecker eines Objekts (auch teilweise): t = { x, y, R (Umkreis), key, pts? (Stichpunkte der Grundfläche) }.
// → [{ kind: 'tree', x, y, r } | { kind: 'building', b, dx, dy } | { kind: 'bridge', pts }]. Genau zugeschnitten wird
// später beim Zeichnen (Silhouette ∩ Verdecker); hier reicht, dass der Verdecker das Objekt wirklich berührt.
export function occludersOf(t, { trees = [], buildings = [], bridges = [], deck = 46, cam, heightScale }) {
  const out = [], { x, y, R, key } = t, pts = t.pts ?? [[x, y]];
  for (const f of bridges) {
    const p = f.pts;
    for (let i = 0; i < p.length - 2; i += 2) if (segDist2(x, y, p[i], p[i + 1], p[i + 2], p[i + 3]) < (deck / 2 + R) ** 2) { out.push({ kind: 'bridge', pts: p }); break; }
  }
  for (const b of buildings) {
    if ((b._depthY ?? b.bbox.y + b.bbox.h) <= key) continue; // vorher gezeichnet (Tiefenschlüssel wie render.js buildingDepth)
    const { dx, dy } = roofOffset(b, cam, heightScale), bb = b.bbox;
    if (x + R < bb.x + Math.min(0, dx) || x - R > bb.x + bb.w + Math.max(0, dx) || y + R < bb.y + Math.min(0, dy) || y - R > bb.y + bb.h + Math.max(0, dy)) continue;
    let hit = false;
    for (const [px, py] of pts) {
      for (const k of [1, 0.8, 0.6, 0.4, 0.2, 0.08]) if (pointInRings(px - dx * k, py - dy * k, b.rings)) { hit = true; break; }
      if (hit) break;
    }
    if (hit) out.push({ kind: 'building', b, dx, dy });
  }
  for (const tr of trees) {
    if (tr.y <= key) continue;
    const c = crownOf(tr);
    if (Math.hypot(x - c.x, y - c.y) < c.r * 0.95 + R * 0.8) out.push({ kind: 'tree', x: c.x, y: c.y, r: c.r * 0.95 });
  }
  return out;
}

// Stichpunkte einer Figur (Mitte) bzw. eines Fahrzeugs (Mitte, Ecken, Kantenmitten) für occludersOf
export function samplePoints(o, hw, hh, angle) {
  if (!hw) return [[o.x, o.y]];
  const c = Math.cos(angle), s = Math.sin(angle), out = [];
  for (const u of [-1, 0, 1]) for (const v of [-1, 0, 1]) out.push([o.x + c * u * hw - s * v * hh, o.y + s * u * hw + c * v * hh]);
  return out;
}

// --- Ebenen: Flächen über einem Objekt (Brückenfahrbahn, Brückenweg, Kreuzungsscheibe, Brückendeck) -------------
const PATH_HALF = 9; // halbe Breite eines gezeichneten Wegs (18 px)
const roadHalf = (e) => {
  const tr = Math.max(e.cs?.left?.track ? 4 + e.cs.left.track : 0, e.cs?.right?.track ? 4 + e.cs.right.track : 0);
  return e.w / 2 + (e.fill ? Math.abs(e.fill) : 0) + tr;
};
const grow = (bb, r) => ({ x: bb.x - r, y: bb.y - r, w: bb.w + 2 * r, h: bb.h + 2 * r });

// Flächen der Ebenen ≥ minLvl aus den sichtbaren Listen: [{ kind, lvl, bbox, … }]
export function levelSurfaces({ edges = [], paths = [], junctions = [], decks = [] }, minLvl = 1) {
  const out = [];
  for (const e of edges) if ((e.lvl ?? 0) >= minLvl) { const half = roadHalf(e); out.push({ kind: 'road', lvl: e.lvl ?? 0, pts: e.pts, half, bbox: grow(e.bbox, half) }); }
  for (const p of paths) if ((p.lvl ?? 0) >= minLvl) out.push({ kind: 'road', lvl: p.lvl ?? 0, pts: p.pts, half: PATH_HALF, bbox: grow(p.bbox, PATH_HALF) });
  for (const j of junctions) { const l = j.hi ?? j.lvl ?? 0; if (l >= minLvl) out.push({ kind: 'disc', lvl: l, x: j.x, y: j.y, r: j.r, bbox: { x: j.x - j.r, y: j.y - j.r, w: 2 * j.r, h: 2 * j.r } }); }
  for (const a of decks) { const l = Math.max(1, a.lvl ?? 0); if (l >= minLvl) out.push({ kind: 'deck', lvl: l, rings: a.rings, bbox: a.bbox }); }
  return out;
}

// Liegt (x, y) auf der Fläche s?
export function onSurface(s, x, y) {
  const b = s.bbox;
  if (x < b.x || x > b.x + b.w || y < b.y || y > b.y + b.h) return false;
  if (s.kind === 'disc') return (x - s.x) ** 2 + (y - s.y) ** 2 <= s.r * s.r;
  if (s.kind === 'deck') return pointInRings(x, y, s.rings);
  const p = s.pts, h2 = s.half * s.half;
  for (let i = 0; i < p.length - 2; i += 2) if (segDist2(x, y, p[i], p[i + 1], p[i + 2], p[i + 3]) <= h2) return true;
  return false;
}

// Flächen höher als lvl, die einen der Stichpunkte überdecken. inPortal(x, y): dort (Rampenfuß, Treppe) geht es
// hinauf – da liegt nichts darüber.
export function surfacesOver(pts, lvl, surfaces, inPortal = () => false, out = []) {
  out.length = 0;
  for (const [x, y] of pts) {
    if (inPortal(x, y)) continue;
    for (const s of surfaces) if (s.lvl > lvl && !out.includes(s) && onSurface(s, x, y)) out.push(s);
  }
  return out;
}

// Ebene eines Punkts auf Schienen/Linienweg (Straßenbahn): oben, wenn eine höhere Fläche darunter liegt und keine
// Fläche der Ebene 0; liegen beide übereinander (Brücke kreuzt Straße), gilt prev (die Ebene davor auf dem Weg).
export function trackLevel(x, y, upper, ground, prev = 0) {
  let up = 0;
  for (const s of upper) if (s.lvl > up && onSurface(s, x, y)) up = s.lvl;
  if (!up) return 0;
  for (const s of ground) if (onSurface(s, x, y)) return prev;
  return up;
}
