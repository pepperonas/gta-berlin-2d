// Welt-Rendering in schräger Draufsicht: Boden flach (Flächen, Wasser, Straßen als Vektoren aus der Karte),
// Gebäude als extrudierte Grundrisse mit sichtbaren Fassaden und leichter Parallaxe (Dach wandert von der
// Bildmitte weg → Seitenwände werden sichtbar).
import { MISSION, RENDER } from './config.js';
import { AREA_KIND, BUILDING_KIND } from './citycodes.js';
import { drawCar, drawPerson, drawTree, shade } from './assets.js';
import { playerCar, speedOf } from './world.js';
import { signalState } from './signals.js';
import { offsetPolyline, polylineLength } from './geom.js';
import { laneOffsets, parkingStrip } from './street.js';
import { cutPolyline } from './roadgraph.js';
import { PARK, SURFACE } from './citycodes.js';
import { lightAt } from './daylight.js';
import { Lighting, casterBox, makeCanvas } from './lighting.js';
import { edgeLamps } from './lamps.js';
import { nearestEdge, surfaceAt, T as SURF } from './map.js';
import { texture } from './textures.js';
import { edgeDecals } from './decals.js';

const AREA_COLOR = {
  [AREA_KIND.rail]: '#7b756c', [AREA_KIND.plaza]: '#8e8b85', [AREA_KIND.allotments]: '#6c9851',
  [AREA_KIND.cemetery]: '#5b8a47', [AREA_KIND.grass]: '#5d9340', [AREA_KIND.pitch]: '#4d8c3c',
  [AREA_KIND.sand]: '#d6c48d', [AREA_KIND.wood]: '#3e7631',
};
const WALLS = {
  [BUILDING_KIND.house]: ['#c9b79c', '#d6c7a1', '#c4a484', '#b8a488', '#d9c9b3', '#c7a9a0', '#b3aa9a', '#d4bfa0', '#a89080', '#e0d4bd'],
  [BUILDING_KIND.public]: ['#a3b1a0', '#9aa3ab', '#b0aaa0', '#c2b8a6'],
  [BUILDING_KIND.industrial]: ['#8f9aa6', '#9aa0a3', '#858d93', '#a3a8a0'],
  [BUILDING_KIND.church]: ['#a0674e', '#8f5a45'],
  [BUILDING_KIND.small]: ['#9d968c', '#8c877f'],
  [BUILDING_KIND.spaeti]: ['#d9c46a'],
  [BUILDING_KIND.warehouse]: ['#7f8a93'],
};
// POI-Darstellung: Farbe und Kurzzeichen je Kategorie (Haltestellen wie im Berliner Liniennetz: U blau, S grün, H gelb).
export const POI_STYLE = {
  ubahn: { bg: '#1f5bb5', fg: '#fff', glyph: 'U', prio: 0 }, sbahn: { bg: '#2f8f3f', fg: '#fff', glyph: 'S', prio: 0 },
  bahn: { bg: '#d0312d', fg: '#fff', glyph: 'DB', prio: 0 }, bus: { bg: '#f2c500', fg: '#1d4f2a', glyph: 'H', prio: 3 },
  mall: { bg: '#8e44ad', fg: '#fff', glyph: '▣', prio: 1 }, supermarket: { bg: '#e67e22', fg: '#fff', glyph: '€', prio: 2 },
  shop: { bg: '#9b59b6', fg: '#fff', glyph: '▪', prio: 4 }, food: { bg: '#c0392b', fg: '#fff', glyph: '⚬', prio: 2 },
  drink: { bg: '#b8327a', fg: '#fff', glyph: '▾', prio: 2 }, cafe: { bg: '#7b4a2a', fg: '#fff', glyph: '◡', prio: 3 },
  service: { bg: '#34627f', fg: '#fff', glyph: 'i', prio: 4 }, culture: { bg: '#16806f', fg: '#fff', glyph: '★', prio: 3 },
  hotel: { bg: '#2c3e8f', fg: '#fff', glyph: 'Z', prio: 4 },
};

// Gleismaße in px (10 px = 1 m).
export const TRACK = { gauge: 14.35, rail: 1.6, sleeper: 26, sleeperDash: [2.5, 3.5], bed: 36, deck: 46 };

const SIDEWALK = '#9d9990', ASPHALT = '#3b3e43', CURB = '#c3bfb5', WATER = '#2c6c98';

export function ringPath(rings) {
  const p = new Path2D();
  for (const r of rings) {
    p.moveTo(r[0], r[1]);
    for (let i = 2; i < r.length; i += 2) p.lineTo(r[i], r[i + 1]);
    p.closePath();
  }
  return p;
}
export function linePath(pts) {
  const p = new Path2D();
  p.moveTo(pts[0], pts[1]);
  for (let i = 2; i < pts.length; i += 2) p.lineTo(pts[i], pts[i + 1]);
  return p;
}
// Path2D je Kartenobjekt einmal erzeugen und am Objekt merken.
// Uferlinie: bei an der Kachelgrenze abgeschnittenen Wasserflächen ohne die Schnittkanten (sonst Kaimauer quer im Wasser).
function shoreOf(wa) {
  if (!wa.clip) return pathOf(wa);
  if (wa._shore) return wa._shore;
  const { x0, y0, x1, y1 } = wa.clip, p = new Path2D();
  const on = (ax, ay, bx, by) => (ax === x0 && bx === x0) || (ax === x1 && bx === x1) || (ay === y0 && by === y0) || (ay === y1 && by === y1);
  for (const r of wa.rings) {
    const n = r.length;
    for (let i = 0; i < n; i += 2) {
      const ax = r[i], ay = r[i + 1], bx = r[(i + 2) % n], by = r[(i + 3) % n];
      if (on(ax, ay, bx, by)) continue;
      p.moveTo(ax, ay); p.lineTo(bx, by);
    }
  }
  return (wa._shore = p);
}
export const pathOf = (f) => (f._path ??= f.pts ? linePath(f.pts) : ringPath(f.rings));
export { AREA_COLOR, WATER, ASPHALT };

const PARK_COLOR = 'rgba(96,98,104,0.42)'; // Parkstreifen: helleres Grau über dem Asphalt (Textur scheint durch)
const AREA_TEXTURE = {
  [AREA_KIND.rail]: 'rail', [AREA_KIND.plaza]: 'plaza', [AREA_KIND.allotments]: 'allotments', [AREA_KIND.cemetery]: 'cemetery',
  [AREA_KIND.grass]: 'grass', [AREA_KIND.pitch]: 'pitch', [AREA_KIND.sand]: 'sand', [AREA_KIND.wood]: 'wood',
};
const GUTTER = '#55575b';

// Decals einer Kante als Pfade je Art (einmal gebaut, Weltkoordinaten)
function decalPaths(e, city) {
  if (e._decalPaths !== undefined) return e._decalPaths;
  const list = edgeDecals(city, e);
  if (!list.length) return (e._decalPaths = null);
  const P = { frame: new Path2D(), grate: new Path2D(), lid: new Path2D(), lidIn: new Path2D(), patch: new Path2D(), crack: new Path2D(), oil: new Path2D() };
  const rect = (path, d, l, w) => {
    const c = Math.cos(d.a), s = Math.sin(d.a), hx = l / 2, hy = w / 2;
    path.moveTo(d.x + c * hx - s * hy, d.y + s * hx + c * hy); path.lineTo(d.x - c * hx - s * hy, d.y - s * hx + c * hy);
    path.lineTo(d.x - c * hx + s * hy, d.y - s * hx - c * hy); path.lineTo(d.x + c * hx + s * hy, d.y + s * hx - c * hy); path.closePath();
  };
  for (const d of list) {
    if (d.t === 'gully') { rect(P.frame, d, d.l, d.w); rect(P.grate, d, d.l * 0.72, d.w * 0.6); }
    else if (d.t === 'manhole') { P.lid.moveTo(d.x + d.l / 2, d.y); P.lid.arc(d.x, d.y, d.l / 2, 0, Math.PI * 2); P.lidIn.moveTo(d.x + d.l * 0.36, d.y); P.lidIn.arc(d.x, d.y, d.l * 0.36, 0, Math.PI * 2); }
    else if (d.t === 'patch') rect(P.patch, d, d.l, d.w);
    else if (d.t === 'oil') { P.oil.moveTo(d.x + d.l / 2, d.y); P.oil.ellipse(d.x, d.y, d.l / 2, d.w / 2, d.a, 0, Math.PI * 2); }
    else if (d.t === 'crack' && d.pts) {
      const c = Math.cos(d.a), s = Math.sin(d.a);
      for (let i = 0; i < d.pts.length; i += 2) {
        const x = d.x + c * d.pts[i] - s * d.pts[i + 1], y = d.y + s * d.pts[i] + c * d.pts[i + 1];
        if (i) P.crack.lineTo(x, y); else P.crack.moveTo(x, y);
      }
    }
  }
  return (e._decalPaths = P);
}

// Baumscheibe unter Straßenbäumen (Baum steht auf dem Gehweg nahe einer Fahrbahn); einmal je Baum bestimmt
function treePit(city, tr) {
  if (tr._pit !== undefined) return tr._pit;
  const onWalk = surfaceAt(city, tr.x, tr.y) === SURF.SIDEWALK;
  return (tr._pit = onWalk && !!nearestEdge(city, tr.x, tr.y, 6 * city.scale + 60, (o) => o.cls <= 8));
}
const FENCE_STYLE = { 0: ['#8a8f95', 1.5, []], 1: ['#a08f78', 3.5, []], 2: ['#3d6e2f', 7, []], 3: ['#3a3d42', 4, [1, 16]] };

// Zufahrten einer Ampelkreuzung: je Kante die Haltelinie vor der Kreuzung (Mitte bis rechter Bordstein, in Fahrtrichtung).
function signalApproaches(city, n) {
  const nd = city.nodes.get(n), out = [];
  if (!nd) return out;
  const es = nd.edges.map((k) => city.edges.get(k)).filter((e) => e && e.cls <= 8);
  if (!es.length) return out;
  const r = Math.max(...es.map((e) => e.w / 2)) + 2 * city.scale;
  for (const e of es) {
    const incoming = e.b === n ? e.oneway !== -1 : e.oneway !== 1; // darf man auf dieser Kante zur Kreuzung fahren?
    if (!incoming) continue;
    const p = e.pts, i = e.b === n ? p.length - 4 : 2;
    const [x0, y0, x1, y1] = e.b === n ? [p[i], p[i + 1], p[i + 2], p[i + 3]] : [p[i], p[i + 1], p[i - 2], p[i - 1]];
    const L = Math.hypot(x1 - x0, y1 - y0) || 1, ux = (x1 - x0) / L, uy = (y1 - y0) / L; // Richtung zur Kreuzung
    const d = Math.min(r + 4, L * 0.9);
    const heading = Math.atan2(uy, ux);
    const from = e.oneway ? -e.w / 2 : 0;
    out.push({ x: nd.x - ux * d, y: nd.y - uy * d, heading, from, to: e.w / 2 });
  }
  return out;
}
let cobblePattern = null;
function makeCobblePattern(ctx) {
  const c = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(12, 12) : Object.assign(document.createElement('canvas'), { width: 12, height: 12 });
  const g = c.getContext('2d');
  g.fillStyle = '#4d4f53'; g.fillRect(0, 0, 12, 12);
  g.fillStyle = '#5c5e62';
  for (const [x, y] of [[0.5, 0.5], [6.5, 0.5], [3.5, 6.5], [9.5, 6.5], [-2.5, 6.5]]) g.fillRect(x, y, 5, 5);
  return ctx.createPattern(c, 'repeat') ?? '#4d4f53';
}

// Markierungen einer Kante einmalig als Pfade (Ränder an Kreuzungen gekürzt).
function buildMarks(e, city) {
  if (e.cls > 8 || e.bridge && e.w < 50) return null;
  const cs = e.cs, trim = (n) => {
    const nd = city.nodes.get(n);
    if (!nd || nd.edges.length < 3) return 0;
    let r = 0; for (const k of nd.edges) { const o = city.edges.get(k); if (o && o !== e) r = Math.max(r, o.w / 2); }
    return r + 10;
  };
  const L = e.len, s0 = trim(e.a), s1 = L - trim(e.b);
  if (s1 - s0 < 30) return null;
  const line = (off) => linePath(cutPolyline(offsetPolyline(e.pts, off), s0, s1));
  const m = { strips: [], solid: [], dashed: [], fine: [] };
  for (const side of [-1, 1]) {
    const ps = parkingStrip(cs, side);
    if ((ps.kind === PARK.lane || ps.kind === PARK.half) && ps.depth > 5) m.strips.push({ path: line(ps.offset), w: ps.depth });
    const sd = side < 0 ? cs.left : cs.right;
    if (sd.cycle > 5) {
      m.solid.push(line(side * (cs.width / 2 - sd.parkW - sd.cycle))); // Radstreifen: durchgezogen zur Fahrbahn
      if (sd.parkW > 5) m.fine.push(line(side * (cs.width / 2 - sd.parkW))); // Trennung zum Parkstreifen
    }
  }
  const lo = laneOffsets(cs, city.scale);
  if (cs.fwd && cs.bwd && !lo.narrow && cs.width >= 55) m.dashed.push(line(lo.center)); // Mittellinie bei Gegenverkehr
  for (let i = 1; i < cs.fwd; i++) m.dashed.push(line(lo.center + i * lo.laneW));
  for (let j = 1; j < cs.bwd; j++) m.dashed.push(line(lo.center - j * lo.laneW));
  return m;
}

let windowPatterns = null;
function makeWindowPatterns(ctx) {
  const mk = (lit) => {
    const c = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(14, 16) : Object.assign(document.createElement('canvas'), { width: 14, height: 16 });
    const g = c.getContext('2d');
    g.fillStyle = lit ? '#f1dc98' : '#2d3440'; g.fillRect(4, 4, 6, 8);
    return ctx.createPattern(c, 'repeat');
  };
  return [mk(false), mk(false), mk(true)];
}

// Nachtfenster: 4 × 4 Fenster je Kachel, je Variante ein anderer Anteil erleuchtet (deterministisch).
// Zwei deckungsgleiche Fassungen: fürs Bild (hell/dunkel) und für die Lichtkarte (nur die hellen Fenster, sonst leer).
export const NIGHT_DENSITY = [0.2, 0.4, 0.6, 0.8];
const litAt = (k, i) => hash01n(i * 31 + k * 977 + 5) < NIGHT_DENSITY[k];
function hash01n(n) { const x = Math.sin(n * 12.9898 + 78.233) * 43758.5453; return x - Math.floor(x); }
const nightPatternCache = new WeakMap();
function nightWindowPatterns(ctx) {
  let pats = nightPatternCache.get(ctx);
  if (pats) return pats;
  const mk = (k, forLight) => {
    const c = makeCanvas(56, 64), g = c.getContext('2d');
    for (let i = 0; i < 16; i++) {
      const x = (i % 4) * 14 + 4, y = Math.floor(i / 4) * 16 + 4;
      if (litAt(k, i)) { g.fillStyle = forLight ? '#fff3d2' : '#ffd98a'; g.fillRect(x, y, 6, 8); }
      else if (!forLight) { g.fillStyle = '#262c38'; g.fillRect(x, y, 6, 8); }
    }
    return ctx.createPattern(c, 'repeat');
  };
  pats = { img: NIGHT_DENSITY.map((_, k) => mk(k, false)), light: NIGHT_DENSITY.map((_, k) => mk(k, true)) };
  nightPatternCache.set(ctx, pats);
  return pats;
}
const darkPatternCache = new WeakMap();
function dark3(ctx) {
  let p = darkPatternCache.get(ctx);
  if (!p) {
    const c = makeCanvas(14, 16), g = c.getContext('2d');
    g.fillStyle = '#262c38'; g.fillRect(4, 4, 6, 8);
    p = ctx.createPattern(c, 'repeat');
    darkPatternCache.set(ctx, p);
  }
  return p;
}
// Welche Nachtfenster-Variante ein Haus zeigt (-1 = alles dunkel). Streut je Haus um den Tagesanteil.
export function nightVariant(b, windowsLit) {
  if (windowsLit <= 0) return -1;
  const f = windowsLit + (hash01n(b.seed % 100003) - 0.5) * 0.6;
  return f < 0.1 ? -1 : Math.min(NIGHT_DENSITY.length - 1, Math.floor(f * NIGHT_DENSITY.length));
}

const SIGNAL_RGB = { red: '255,60,48', yellow: '255,204,0', green: '52,199,89' };
const SHOP_GLOW = new Set(['mall', 'supermarket', 'shop', 'food', 'drink', 'cafe', 'hotel', 'ubahn', 'sbahn']);

// Schaufenster-Schein: vom Laden aus zum Gehweg der nächsten Straße (zwischengespeichert am POI)
function shopGlowPoint(city, q) {
  if (q._glow !== undefined) return q._glow;
  const e = nearestEdge(city, q.x, q.y, 40 * city.scale, (o) => o.cls <= 8 && !o.bridge);
  let gp = null;
  if (e) {
    const dx = q.x - e.x, dy = q.y - e.y, d = Math.hypot(dx, dy);
    if (d > 1) { const k = (e.e.w / 2 + 2.2 * city.scale) / d; if (k < 1) gp = [e.x + dx * k, e.y + dy * k]; }
  }
  q._glow = gp;
  return gp;
}

// Straßenlaterne: Mast (schräge Ansicht wie die Häuser: Höhe wächst nach oben), Ausleger zur Fahrbahn, Leuchte
const LAMP_H = 20; // 8 m Masthöhe × heightScale / 2 wie die Dachverschiebung
export function lampHead(lp) { return lp.gas ? [lp.x, lp.y - LAMP_H] : [lp.x + lp.nx * 9, lp.y - LAMP_H + lp.ny * 9]; }
function drawLamp(ctx, lp, on) {
  const [hx, hy] = lampHead(lp);
  ctx.fillStyle = 'rgba(0,0,0,0.25)'; ctx.beginPath(); ctx.arc(lp.x + 1, lp.y + 1, 2.2, 0, Math.PI * 2); ctx.fill();
  ctx.strokeStyle = lp.gas ? '#2f3a33' : '#4a5058'; ctx.lineWidth = 1.8; ctx.lineCap = 'round';
  ctx.beginPath(); ctx.moveTo(lp.x, lp.y); ctx.lineTo(lp.x, lp.y - LAMP_H);
  if (!lp.gas) ctx.lineTo(hx, hy);
  ctx.stroke(); ctx.lineCap = 'butt';
  const lit = on ? (lp.gas ? '#ffd9a0' : '#fff6de') : '#c9ccd1';
  if (lp.gas) { // Berliner Gaslaterne: Laterne mit dunklem Dach
    ctx.fillStyle = lit; ctx.fillRect(hx - 2.6, hy - 3, 5.2, 5);
    ctx.fillStyle = '#2f3a33'; ctx.beginPath(); ctx.moveTo(hx - 3.6, hy - 3); ctx.lineTo(hx + 3.6, hy - 3); ctx.lineTo(hx, hy - 6.5); ctx.fill();
  } else {
    ctx.save(); ctx.translate(hx, hy); ctx.rotate(Math.atan2(lp.ny, lp.nx));
    ctx.fillStyle = '#3a4048'; ctx.fillRect(-3.5, -2.2, 7, 4.4);
    ctx.fillStyle = lit; ctx.fillRect(-2.5, -1.2, 5, 2.4);
    ctx.restore();
  }
}

// Qualitätsstufe aus der gemessenen Zeichenzeit (Median über RENDER.sampleFrames Bilder), mit Hysterese.
export function nextQuality(current, medianMs) {
  if (current === 'high' && medianMs > RENDER.budgetMs) return 'low';
  if (current === 'low' && medianMs < RENDER.recoverMs) return 'high';
  return current;
}

export class Renderer {
  constructor(ctx) {
    this.ctx = ctx;
    this.skids = [];
    this.particles = [];
    this.lighting = new Lighting();
    this.stats = { shadows: 0, lights: 0, ms: 0 };
    this.quality = 'high';
    this._times = [];
  }

  // Ereignisse der Simulation in Effekte übersetzen.
  handleEvents(events) {
    for (const e of events) {
      if (e.type === 'crash') for (let i = 0; i < 6 + e.strength * 14; i++) {
        const a = Math.random() * Math.PI * 2, v = 60 + Math.random() * 160 * e.strength;
        this.particles.push({ kind: 'spark', x: e.x, y: e.y, vx: Math.cos(a) * v, vy: Math.sin(a) * v, life: 0.35, max: 0.35 });
      }
      if (e.type === 'wreck') for (let i = 0; i < 20; i++) {
        this.particles.push({ kind: 'smoke', x: e.x + (Math.random() - 0.5) * 20, y: e.y, vx: (Math.random() - 0.5) * 30, vy: -20 - Math.random() * 30, life: 1.6, max: 1.6, r: 6 });
      }
    }
  }

  update(world, dt) {
    for (const c of world.cars) {
      const hard = c.skid > 0.2 || (c.controls.handbrake && speedOf(c) > 60) || (c.controls.brake > 0.8 && speedOf(c) > 150 && c.driver);
      if (hard) {
        const s = Math.sin(c.angle), co = Math.cos(c.angle);
        for (const side of [-1, 1]) {
          const x = c.x - co * (c.hw - 6) - s * side * (c.hh - 3), y = c.y - s * (c.hw - 6) + co * side * (c.hh - 3);
          const key = `${c.id}${side}`;
          const last = this._lastSkid?.[key];
          if (last && Math.hypot(last.x - x, last.y - y) < 30) this.skids.push({ x0: last.x, y0: last.y, x1: x, y1: y, life: 8 });
          (this._lastSkid ??= {})[key] = { x, y };
        }
      } else if (this._lastSkid) { delete this._lastSkid[`${c.id}-1`]; delete this._lastSkid[`${c.id}1`]; }
      if ((c.health < 35 || c.wrecked) && Math.random() < (c.wrecked ? 0.35 : 0.15)) {
        this.particles.push({ kind: 'smoke', x: c.x + Math.cos(c.angle) * c.hw * 0.7, y: c.y + Math.sin(c.angle) * c.hw * 0.7, vx: (Math.random() - 0.5) * 10, vy: -18, life: 1.4, max: 1.4, r: 4 });
      }
    }
    if (this.skids.length > 600) this.skids.splice(0, this.skids.length - 600);
    for (const s of this.skids) s.life -= dt;
    this.skids = this.skids.filter((s) => s.life > 0);
    for (const p of this.particles) {
      p.life -= dt; p.x += p.vx * dt; p.y += p.vy * dt;
      if (p.kind === 'spark') { p.vx *= 0.9; p.vy *= 0.9; } else p.r += dt * 8;
    }
    this.particles = this.particles.filter((p) => p.life > 0);
  }


  draw(world, W, H, scale, overlayMarkers = true) {
    const t0 = performance.now();
    this.drawFrame(world, W, H, scale, overlayMarkers);
    const ts = this._times;
    ts.push(performance.now() - t0);
    if (ts.length >= RENDER.sampleFrames) {
      ts.sort((a, b) => a - b);
      this.stats.ms = ts[ts.length >> 1];
      this.quality = nextQuality(this.quality, this.stats.ms);
      ts.length = 0;
    }
  }

  drawFrame(world, W, H, scale, overlayMarkers) {
    const ctx = this.ctx, cam = world.camera, city = world.city;
    const s = scale * cam.zoom;
    ctx.setTransform(s, 0, 0, s, W / 2 - cam.x * s, H / 2 - cam.y * s);
    const vw = W / s, vh = H / s;
    const v = { x: cam.x - vw / 2, y: cam.y - vh / 2, w: vw, h: vh };
    const t = world.time;
    const L = this.light = lightAt(world.clock ?? 780);
    const tf = [s, W / 2 - cam.x * s, H / 2 - cam.y * s];
    windowPatterns ??= makeWindowPatterns(ctx);
    cobblePattern ??= makeCobblePattern(ctx);
    ctx.lineDashOffset = 0; // gestrichelte Markierungen stehen fest auf der Straße

    // Sichtbare Kartenobjekte (unten großzügiger: hohe Häuser ragen ins Bild).
    const q = city.render.query({ x: v.x - 60, y: v.y - 60, w: v.w + 120, h: v.h + 420 }, this._q ??= []);
    const areas = [], water = [], edges = [], paths = [], rails = [], buildings = [], trees = [], junctions = [], crossings = [], barriers = [], fences = [];
    for (const f of q) {
      switch (f.layer) {
        case 'area': areas.push(f); break;
        case 'water': water.push(f); break;
        case 'edge': edges.push(f); break;
        case 'path': paths.push(f); break;
        case 'rail': rails.push(f); break;
        case 'building': buildings.push(f); break;
        case 'tree': trees.push(f); break;
        case 'junction': junctions.push(f); break;
        case 'crossing': crossings.push(f); break;
        case 'barrier': barriers.push(f); break;
        case 'fence': fences.push(f); break;
      }
    }

    // 1) Grund: Gehweg/Hof, darauf Flächen (Grün, Plätze, Gleisanlagen)
    const tex = (k, fallback) => texture(ctx, k) ?? fallback;
    ctx.fillStyle = tex('sidewalk', SIDEWALK);
    ctx.fillRect(v.x - 5, v.y - 5, v.w + 10, v.h + 10);
    areas.sort((a, b) => a.kind - b.kind);
    for (const a of areas) { ctx.fillStyle = tex(AREA_TEXTURE[a.kind], AREA_COLOR[a.kind]); ctx.fill(pathOf(a), 'evenodd'); }

    // 2) Wasser mit Wellen und Kaikante
    for (const wa of water) {
      const p = pathOf(wa);
      ctx.fillStyle = WATER; ctx.fill(p, 'evenodd');
      ctx.save(); ctx.clip(p, 'evenodd');
      ctx.strokeStyle = 'rgba(255,255,255,0.12)'; ctx.lineWidth = 1.5;
      const x0 = Math.max(v.x, wa.bbox.x), x1 = Math.min(v.x + v.w, wa.bbox.x + wa.bbox.w);
      const y0 = Math.max(v.y, wa.bbox.y), y1 = Math.min(v.y + v.h, wa.bbox.y + wa.bbox.h);
      for (let y = Math.floor(y0 / 22) * 22 + 10; y < y1; y += 22) {
        ctx.beginPath();
        for (let x = x0; x <= x1 + 12; x += 12) ctx.lineTo(x, y + Math.sin(x * 0.05 + t * 1.5 + y) * 2);
        ctx.stroke();
      }
      // zum Ufer hin dunkler (Kaimauer wirft Schatten ins Wasser)
      ctx.strokeStyle = 'rgba(8,30,55,0.22)'; ctx.lineWidth = 60; ctx.stroke(shoreOf(wa));
      ctx.lineWidth = 24; ctx.stroke(shoreOf(wa));
      ctx.restore();
      ctx.strokeStyle = '#6f6a60'; ctx.lineWidth = 3; ctx.stroke(shoreOf(wa));
    }

    // 3) Wege (Parks, Fußwege) und ebenerdige Gleise
    ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.strokeStyle = '#b9ab8e'; ctx.lineWidth = 18;
    for (const p of paths) if (!p.bridge) ctx.stroke(pathOf(p));
    this.drawTracks(rails.filter((r) => !r.bridge), false);

    // 4) Straßen: erst Bordstein, dann Asphalt; kleine Straßen zuerst, Brücken zuletzt
    edges.sort((a, b) => (a.bridge - b.bridge) || (b.cls - a.cls));
    // Bordstein (heller Stein), davor der dunkle Rinnstein, dann die Fahrbahn
    for (const e of edges) {
      if (e.bridge) { ctx.strokeStyle = '#7d7a73'; ctx.lineWidth = e.w + 14; ctx.stroke(pathOf(e)); }
      else if (e.cls <= 10) { ctx.strokeStyle = CURB; ctx.lineWidth = e.w + 5; ctx.stroke(pathOf(e)); }
    }
    for (const e of edges) if (!e.bridge && e.cls <= 8) { ctx.strokeStyle = GUTTER; ctx.lineWidth = e.w + 1.4; ctx.stroke(pathOf(e)); }
    for (const j of junctions) if (!j.bridge) {
      ctx.fillStyle = CURB; ctx.beginPath(); ctx.arc(j.x, j.y, j.r + 2.5, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = GUTTER; ctx.beginPath(); ctx.arc(j.x, j.y, j.r + 0.7, 0, Math.PI * 2); ctx.fill();
    }
    const asphalt = tex('asphalt', ASPHALT), cobble = tex('cobble', cobblePattern);
    for (const j of junctions) { ctx.fillStyle = j.cobble ? cobble : asphalt; ctx.beginPath(); ctx.arc(j.x, j.y, j.r, 0, Math.PI * 2); ctx.fill(); }
    for (const e of edges) {
      ctx.strokeStyle = e.cls === 10 ? '#a8a296' : e.cls === 11 ? '#8a8272' : e.cs.surface === SURFACE.cobble ? cobble : asphalt;
      ctx.lineWidth = e.w; ctx.stroke(pathOf(e));
    }
    if (this.quality === 'high') this.drawDecals(edges, city);
    this.drawStreetMarkings(edges, world.city);
    this.drawCrossings(crossings);
    this.drawSignals(world, v);
    ctx.strokeStyle = '#b9ab8e'; ctx.lineWidth = 18;
    for (const p of paths) if (p.bridge) ctx.stroke(pathOf(p));
    ctx.lineCap = 'butt'; ctx.lineJoin = 'miter';

    // Zäune, Mauern, Hecken, Poller
    ctx.lineCap = 'round';
    for (const f of fences) {
      const st = FENCE_STYLE[f.kind] ?? FENCE_STYLE[0];
      ctx.strokeStyle = st[0]; ctx.lineWidth = st[1]; ctx.setLineDash(st[2]); ctx.stroke(pathOf(f)); ctx.setLineDash([]);
    }
    for (const b of barriers) {
      ctx.fillStyle = b.kind ? '#c0392b' : '#3a3d42'; ctx.beginPath(); ctx.arc(b.x, b.y, 2.6, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = b.kind ? '#f2f2f2' : '#9aa0a8'; ctx.beginPath(); ctx.arc(b.x - 0.6, b.y - 0.8, 1.1, 0, Math.PI * 2); ctx.fill();
    }
    ctx.lineCap = 'butt';

    // Baumscheiben der Straßenbäume
    ctx.fillStyle = '#5e4c3b'; ctx.strokeStyle = '#8c8578'; ctx.lineWidth = 1.2;
    for (const tr of trees) if (treePit(city, tr)) { const h = 7 + tr.r; ctx.fillRect(tr.x - h, tr.y - h, 2 * h, 2 * h); ctx.strokeRect(tr.x - h, tr.y - h, 2 * h, 2 * h); }

    // 5) Bremsspuren
    ctx.lineWidth = 3; ctx.lineCap = 'round';
    for (const k of this.skids) {
      ctx.strokeStyle = `rgba(20,20,20,${Math.min(0.5, k.life / 8 * 0.5)})`;
      ctx.beginPath(); ctx.moveTo(k.x0, k.y0); ctx.lineTo(k.x1, k.y1); ctx.stroke();
    }
    ctx.lineCap = 'butt';

    // 5b) Schattenwurf von Häusern und Bäumen (Sonnenstand aus der Spieluhr)
    this.stats.shadows = 0;
    if (L.sun.strength >= 0.02) {
      const cq = city.render.query(casterBox(v, L.sun), this._cq ??= []);
      const casters = cq.filter((f) => f.layer === 'building');
      this.stats.shadows = this.lighting.drawShadows(ctx, W, H, tf, L.sun, casters, this.quality === 'high' ? trees : []);
    }

    // 6) Missionsmarker am Boden
    if (overlayMarkers) this.drawZones(world);

    // 7) Tiefensortierte Objekte
    const list = [];
    const margin = 220;
    const near = (x, y) => x > v.x - margin && x < v.x + v.w + margin && y > v.y - margin && y < v.y + v.h + margin * 1.5;
    for (const b of buildings) list.push({ y: b.bbox.y + b.bbox.h, b, d: () => this.drawBuilding(b, cam) });
    for (const tr of trees) list.push({ y: tr.y, tr, d: () => drawTree(ctx, tr, t, L.sun) });
    const lamps = this._lamps ??= [];
    lamps.length = 0;
    for (const e of edges) for (const lp of edgeLamps(city, e)) if (near(lp.x, lp.y)) { lamps.push(lp); list.push({ y: lp.y, lp, d: () => drawLamp(ctx, lp, L.lampsOn) }); }
    for (const cr of city.crates) if (near(cr.x, cr.y)) list.push({ y: cr.y + cr.h, d: () => drawCrate(ctx, cr) });
    for (const c of world.cars) if (near(c.x, c.y)) list.push({ y: c.y + 6, d: () => drawCar(ctx, c, t, L.sun) });
    for (const p of world.peds) if (near(p.x, p.y)) list.push({ y: p.y, d: () => drawPerson(ctx, p, { shirt: p.shirt, skin: p.skin, down: p.state === 'down', sun: L.sun }) });
    const pl = world.player;
    if (!pl.inCar) list.push({ y: pl.y, d: () => drawPerson(ctx, pl, { shirt: '#ff7a1a', player: true, down: pl.stun > 0, sun: L.sun }) });
    list.sort((a, b) => a.y - b.y);
    for (const it of list) it.d();
    this._depth = list;

    // 8) Hochbahn (U1-Viadukt) und Bahnbrücken über allem, was darunter fährt
    this.drawTracks(rails.filter((r) => r.bridge), true);

    // 9) Partikel
    for (const p of this.particles) {
      const a = p.life / p.max;
      if (p.kind === 'spark') { ctx.fillStyle = `rgba(255,${180 + (a * 75) | 0},60,${a})`; ctx.fillRect(p.x - 1, p.y - 1, 2.5, 2.5); }
      else { ctx.fillStyle = `rgba(70,70,70,${a * 0.45})`; ctx.beginPath(); ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2); ctx.fill(); }
    }
    // 9b) Dämmerung/Nacht: Lichtkarte über die Welt legen
    this.stats.lights = 0;
    if (L.dark > 0.02) {
      const fill = `rgb(${Math.round(L.ambient[0] * 255)},${Math.round(L.ambient[1] * 255)},${Math.round(L.ambient[2] * 255)})`;
      this.stats.lights = this.lighting.drawLightmap(ctx, W, H, tf, L.ambient, this.collectLights(world, v, L, overlayMarkers),
        (g) => this.lightOccluders(g, cam, fill, L));
    }

    // Spieler-Markierung über dem Dach, falls er hinter einem Haus verschwindet
    if (!pl.inCar) {
      ctx.fillStyle = 'rgba(255,122,26,0.9)';
      ctx.beginPath(); ctx.moveTo(pl.x, pl.y - 12); ctx.lineTo(pl.x - 4, pl.y - 19); ctx.lineTo(pl.x + 4, pl.y - 19); ctx.fill();
    }

    // 10) POI-Beschriftungen (in Bildschirmkoordinaten, damit die Schrift scharf und gleich groß bleibt)
    if (overlayMarkers) this.drawPois(world, v, s, W, H);

    // 11) Außerhalb des Spielgebiets abdunkeln, Grenze markieren
    const outside = new Path2D();
    outside.rect(v.x - 10, v.y - 10, v.w + 20, v.h + 20);
    outside.addPath(city._borderPath ??= ringPath(city.border));
    ctx.fillStyle = 'rgba(12,14,22,0.5)'; ctx.fill(outside, 'evenodd');
    ctx.strokeStyle = 'rgba(255,211,61,0.35)'; ctx.lineWidth = 4; ctx.setLineDash([24, 16]); ctx.stroke(city._borderPath); ctx.setLineDash([]);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
  }

  // Zweiter Durchgang der Lichtkarte, in derselben Tiefenfolge wie das Bild: Häuser und Baumkronen decken mit dem
  // Umgebungslicht ab, was hinter ihnen am Boden leuchtet (kein Laternenschein auf Dächern), erleuchtete Fenster und
  // Laternenköpfe leuchten selbst.
  lightOccluders(g, cam, fill, L) {
    const k = L.dark;
    const occlude = this.quality === 'high';
    for (const it of this._depth ?? []) {
      if (it.b) { if (occlude) this.drawBuilding(it.b, cam, g, { fill }); }
      else if (it.tr && occlude) {
        const r = it.tr.size, cy = it.tr.y - Math.min(r * 0.5, 30);
        g.fillStyle = fill; g.beginPath(); g.arc(it.tr.x, cy, r * 0.8, 0, Math.PI * 2); g.fill();
      } else if (it.lp && L.lampsOn) {
        const [hx, hy] = lampHead(it.lp);
        g.globalCompositeOperation = 'lighter'; g.globalAlpha = 0.9 * k;
        g.drawImage(this.lighting.glow(it.lp.rgb), hx - 16, hy - 16, 32, 32);
        g.globalCompositeOperation = 'source-over'; g.globalAlpha = 1;
      }
    }
  }

  // Lichtquellen im Bild (Weltkoordinaten) für die Lichtkarte.
  collectLights(world, v, L, markers) {
    const out = [], pad = 250;
    const inView = (x, y) => x > v.x - pad && x < v.x + v.w + pad && y > v.y - pad && y < v.y + v.h + pad;
    const k = L.dark;
    // Straßenlaternen: Lichtfleck auf Gehweg und Fahrbahnrand unter dem Kopf
    if (L.lampsOn) for (const lp of this._lamps ?? []) {
      out.push({ x: lp.x + lp.nx * 18, y: lp.y + lp.ny * 18, r: lp.main ? 150 : 125, rgb: lp.rgb, a: (lp.gas ? 0.55 : 0.7) * k });
    }
    // Ampeln: farbiger Schein
    const city = world.city;
    for (const s of city._signalList ?? []) {
      if (!inView(s.x, s.y)) continue;
      for (const a of s.app) {
        const state = signalState(city, s.n, a.heading, world.time), py = a.to + 3 + (state === 'red' ? 4 : state === 'yellow' ? 9 : 14);
        out.push({ x: a.x - Math.sin(a.heading) * py, y: a.y + Math.cos(a.heading) * py, r: 34, rgb: SIGNAL_RGB[state], a: 0.8 * k });
      }
    }
    // Läden, Lokale und Bahnhöfe: warmer Schein aus dem Schaufenster auf den Gehweg
    for (const q of city.poiHash?.query({ x: v.x - pad, y: v.y - pad, w: v.w + 2 * pad, h: v.h + 2 * pad }, this._pq ??= []) ?? []) {
      if (!SHOP_GLOW.has(q.cat)) continue;
      const gp = shopGlowPoint(city, q);
      if (gp) out.push({ x: gp[0], y: gp[1], r: 70, rgb: '255,210,150', a: 0.45 * k });
    }
    for (const c of world.cars) {
      if (c.wrecked || !c.driver || !inView(c.x, c.y)) continue; // geparkte Autos ohne Fahrer bleiben dunkel
      const ca = Math.cos(c.angle), sa = Math.sin(c.angle);
      const fx = c.x + ca * c.hw, fy = c.y + sa * c.hw, bx = c.x - ca * c.hw, by = c.y - sa * c.hw;
      out.push({ x: fx - ca * 4, y: fy - sa * 4, r: 230, rgb: '255,236,196', a: 0.85 * k, cone: c.angle });
      out.push({ x: fx, y: fy, r: 34, rgb: '255,240,210', a: 0.6 * k });
      const braking = c.controls?.brake > 0.1;
      out.push({ x: bx, y: by, r: braking ? 46 : 26, rgb: '255,50,36', a: (braking ? 0.9 : 0.45) * k });
    }
    const pl = world.player;
    if (!pl.inCar) out.push({ x: pl.x, y: pl.y, r: 70, rgb: '255,210,170', a: 0.35 * k });
    if (markers) {
      const m = world.mission, p = world.city.places;
      const spots = m.state === 'toPickup' ? [p.pickup] : m.state === 'toDropoff' ? [p.dropoff] : m.state === 'idle' || m.state === 'briefing' ? [p.giver] : [];
      for (const z of spots) if (z && inView(z.x, z.y)) out.push({ x: z.x, y: z.y, r: 120, rgb: '255,211,61', a: 0.7 * k });
    }
    return out;
  }

  drawZones(world) {
    const ctx = this.ctx, m = world.mission, p = world.city.places, t = world.time;
    const ring = (pt, color, r) => {
      const pulse = 1 + Math.sin(t * 4) * 0.06;
      ctx.fillStyle = color.replace('A', '0.18'); ctx.beginPath(); ctx.arc(pt.x, pt.y, r * pulse, 0, Math.PI * 2); ctx.fill();
      ctx.strokeStyle = color.replace('A', '0.9'); ctx.lineWidth = 2.5; ctx.setLineDash([10, 7]); ctx.lineDashOffset = -t * 20;
      ctx.beginPath(); ctx.arc(pt.x, pt.y, r * pulse, 0, Math.PI * 2); ctx.stroke(); ctx.setLineDash([]);
      ctx.lineDashOffset = 0; // sonst „laufen“ im nächsten Bild die Mittellinien und Gleise mit
    };
    if (m.state === 'available') ring(p.giver, 'rgba(255,210,0,A)', MISSION.giverRadius);
    if (m.state === 'toPickup') ring(p.pickup, 'rgba(255,210,0,A)', MISSION.zoneRadius);
    if (m.state === 'toDropoff') ring(p.dropoff, 'rgba(80,220,120,A)', MISSION.zoneRadius);
  }


  // Zebrastreifen (Balken in Fahrtrichtung über die ganze Fahrbahn), Ampel- und markierte Furten (zwei Linien).
  drawCrossings(crossings) {
    const ctx = this.ctx;
    ctx.fillStyle = 'rgba(245,245,238,0.85)';
    for (const z of crossings) {
      const w = z.edge.w;
      ctx.save();
      ctx.transform(z.ux, z.uy, -z.uy, z.ux, z.x, z.y); // lokale Achsen: x entlang der Straße, y quer
      if (z.kind === 'zebra') for (let y = -w / 2 + 3; y < w / 2 - 3; y += 10) ctx.fillRect(-20, y, 40, 5);
      else { ctx.fillRect(-20, -w / 2, 2, w); ctx.fillRect(18, -w / 2, 2, w); }
      ctx.restore();
    }
  }

  // Ampeln: Haltelinie auf der Zufahrtsseite und ein Signal am rechten Fahrbahnrand, Farbe nach aktuellem Umlauf.
  drawSignals(world, v) {
    const ctx = this.ctx, city = world.city;
    if (city._signalGen !== city.gen) { // nach jedem Nachladen neu (Kreuzungen am Rand des Geladenen sind erst dann vollständig)
      city._signalGen = city.gen;
      city._signalList = [...city.signals].map((n) => { const nd = city.nodes.get(n); return nd && { n, x: nd.x, y: nd.y, app: signalApproaches(city, n) }; }).filter(Boolean);
    }
    const list = city._signalList;
    for (const s of list) {
      if (s.x < v.x - 150 || s.x > v.x + v.w + 150 || s.y < v.y - 150 || s.y > v.y + v.h + 150) continue;
      for (const a of s.app) {
        const state = signalState(city, s.n, a.heading, world.time);
        ctx.save();
        ctx.transform(Math.cos(a.heading), Math.sin(a.heading), -Math.sin(a.heading), Math.cos(a.heading), a.x, a.y);
        ctx.fillStyle = 'rgba(245,245,238,0.9)'; ctx.fillRect(-3, a.from, 4, a.to - a.from); // Haltelinie
        ctx.fillStyle = '#1d1f22'; ctx.fillRect(-4, a.to + 3, 8, 18);                      // Signalgeber
        const c = { red: '#ff3b30', yellow: '#ffcc00', green: '#34c759' }[state];
        ctx.fillStyle = c; ctx.beginPath(); ctx.arc(0, a.to + 3 + (state === 'red' ? 4 : state === 'yellow' ? 9 : 14), 3, 0, Math.PI * 2); ctx.fill();
        ctx.restore();
      }
    }
  }

  // Straßenraum nach Querschnitt: Parkstreifen, Radfahrstreifen, Mittellinie, Spurtrennlinien.
  // An Kreuzungen enden die Markierungen am Rand der Querstraße.
  drawDecals(edges, city) {
    const ctx = this.ctx, ps = [];
    for (const e of edges) { const p = decalPaths(e, city); if (p) ps.push(p); }
    ctx.fillStyle = 'rgba(18,20,24,0.38)'; for (const p of ps) ctx.fill(p.patch);
    ctx.strokeStyle = 'rgba(0,0,0,0.18)'; ctx.lineWidth = 0.8; for (const p of ps) ctx.stroke(p.patch);
    ctx.fillStyle = 'rgba(8,8,10,0.26)'; for (const p of ps) ctx.fill(p.oil);
    ctx.strokeStyle = 'rgba(12,12,14,0.55)'; ctx.lineWidth = 0.9; ctx.lineJoin = 'round'; for (const p of ps) ctx.stroke(p.crack);
    ctx.fillStyle = '#6a6c70'; for (const p of ps) ctx.fill(p.frame);
    ctx.fillStyle = '#17181b'; for (const p of ps) ctx.fill(p.grate);
    ctx.fillStyle = '#5c5e62'; for (const p of ps) ctx.fill(p.lid);
    ctx.fillStyle = '#44464a'; for (const p of ps) ctx.fill(p.lidIn);
  }

  drawStreetMarkings(edges, city) {
    const ctx = this.ctx;
    const marks = edges.map((e) => (e._marks ??= buildMarks(e, city))).filter(Boolean);
    ctx.lineCap = 'butt';
    for (const m of marks) for (const st of m.strips) { ctx.strokeStyle = PARK_COLOR; ctx.lineWidth = st.w; ctx.stroke(st.path); }
    ctx.strokeStyle = 'rgba(245,245,235,0.85)'; ctx.lineWidth = 2;
    for (const m of marks) for (const p of m.solid) ctx.stroke(p);
    ctx.setLineDash([16, 18]);
    for (const m of marks) for (const p of m.dashed) ctx.stroke(p);
    ctx.setLineDash([4, 6]); ctx.lineWidth = 1.5;
    for (const m of marks) for (const p of m.fine) ctx.stroke(p);
    ctx.setLineDash([]);
    ctx.lineCap = 'round';
  }

  // Gleise maßstäblich: jeder OSM-Weg ist ein Gleis (Spurweite 1435 mm, Schwellen 2,6 m im Abstand von 0,6 m).
  // Ebenen nacheinander über ALLE Gleise zeichnen (Bett/Viadukt → Schwellen → Schienen), damit parallele Gleise
  // und Weichen sauber ineinander übergehen statt sich gegenseitig zu übermalen.
  drawTracks(tracks, elevated) {
    if (!tracks.length) return;
    const ctx = this.ctx;
    ctx.save();
    ctx.lineCap = 'butt'; ctx.lineJoin = 'round';
    ctx.strokeStyle = elevated ? '#4a433d' : '#7a7266'; ctx.lineWidth = elevated ? TRACK.deck : TRACK.bed;
    for (const r of tracks) ctx.stroke(pathOf(r));
    ctx.strokeStyle = elevated ? '#6b5f52' : '#5a4a3c'; ctx.lineWidth = TRACK.sleeper; ctx.setLineDash(TRACK.sleeperDash);
    for (const r of tracks) ctx.stroke(pathOf(r));
    ctx.setLineDash([]);
    ctx.strokeStyle = '#c9cdd2'; ctx.lineWidth = TRACK.rail;
    for (const r of tracks) {
      r._rails ??= [linePath(offsetPolyline(r.pts, TRACK.gauge / 2)), linePath(offsetPolyline(r.pts, -TRACK.gauge / 2))];
      ctx.stroke(r._rails[0]); ctx.stroke(r._rails[1]);
    }
    ctx.restore();
  }

  drawPois(world, v, s, W, H) {
    const ctx = this.ctx, cam = world.camera;
    const pois = world.city.poiHash.query(v, this._pq ??= []);
    if (!pois.length) return;
    const list = pois.slice().sort((a, b) => POI_STYLE[a.cat].prio - POI_STYLE[b.cat].prio);
    const k = Math.min(W / 1280, H / 720), taken = []; // wie das HUD: 16:9-Grundformat (hud.js BASE)
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.font = `600 ${Math.round(12 * k)}px Segoe UI, system-ui, sans-serif`;
    ctx.textBaseline = 'middle';
    let shown = 0;
    for (const q of list) {
      if (shown >= 28) break;
      const st = POI_STYLE[q.cat];
      const x = (q.x - cam.x) * s + W / 2, y = (q.y - cam.y) * s + H / 2 - 40 * k; // über dem Dach
      const r = 9 * k, label = q.cat === 'bus' ? '' : q.name;
      const tw = label ? ctx.measureText(label).width + 10 * k : 0;
      const box = { x: x - r, y: y - r, w: 2 * r + tw, h: 2 * r };
      if (taken.some((b) => b.x < box.x + box.w && box.x < b.x + b.w && b.y < box.y + box.h && box.y < b.y + b.h)) continue;
      taken.push(box); shown++;
      ctx.fillStyle = 'rgba(0,0,0,0.25)'; ctx.beginPath(); ctx.moveTo(x - 3 * k, y + r - 2 * k); ctx.lineTo(x + 3 * k, y + r - 2 * k); ctx.lineTo(x, y + r + 6 * k); ctx.fill();
      if (label) {
        ctx.fillStyle = 'rgba(15,17,24,0.72)';
        ctx.beginPath(); ctx.roundRect(x, y - 8 * k, tw + r, 16 * k, 8 * k); ctx.fill();
        ctx.fillStyle = '#f2f2f2'; ctx.textAlign = 'left'; ctx.fillText(label, x + r + 3 * k, y + 0.5);
      }
      ctx.fillStyle = st.bg;
      if (q.cat === 'ubahn' || q.cat === 'bus' || q.cat === 'bahn') ctx.fillRect(x - r, y - r, 2 * r, 2 * r);
      else { ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.fill(); }
      ctx.fillStyle = st.fg; ctx.textAlign = 'center';
      ctx.font = `800 ${Math.round((st.glyph.length > 1 ? 9 : 12) * k)}px Segoe UI, system-ui, sans-serif`;
      ctx.fillText(st.glyph, x, y + 0.5);
      ctx.font = `600 ${Math.round(12 * k)}px Segoe UI, system-ui, sans-serif`;
    }
    ctx.restore();
  }

  // Gebäude: sichtbare Fassaden (Kanten, deren Außennormale vom Dachversatz weg zeigt), dann das Dach.
  // night: null (normales Bild) oder { light: true, fill: Umgebungsfarbe } für die Lichtkarte.
  drawBuilding(b, cam, ctx = this.ctx, night = null) {
    const L = this.light;
    const variant = L && L.windowsLit > 0 ? nightVariant(b, L.windowsLit) : -2; // -2: Tagesfenster
    const H = Math.max(18, b.height * RENDER.heightScale);
    const dx = (b.cx - cam.x) * H * 0.0005;
    const dy = -H * 0.5 + (b.cy - cam.y) * H * 0.00025;
    if (!b._col) {
      const pal = WALLS[b.kind] ?? WALLS[0];
      const wall = pal[b.seed % pal.length];
      b._col = { roof: shade(wall, b.kind === BUILDING_KIND.warehouse ? -0.05 : 0.08), line: shade(wall, -0.3),
        faces: [shade(wall, -0.12), shade(wall, -0.24), shade(wall, -0.36)], pat: windowPatterns[b.seed % 3] };
    }
    const col = b._col;
    const faces = this._faces ??= [];
    faces.length = 0;
    for (let ri = 0; ri < b.rings.length; ri++) {
      const r = b.rings[ri], n = r.length;
      const out = b.sign[ri] * (ri === 0 ? 1 : -1);
      for (let i = 0; i < n; i += 2) {
        const x0 = r[i], y0 = r[i + 1], x1 = r[(i + 2) % n], y1 = r[(i + 3) % n];
        const ex = x1 - x0, ey = y1 - y0, L = Math.hypot(ex, ey);
        if (L < 1) continue;
        const nx = out * ey / L, ny = -out * ex / L;
        if (nx * dx + ny * dy >= 0) continue;
        faces.push({ x0, y0, ex, ey, L, ny, ri, ei: i / 2, depth: -((x0 + x1) * dx + (y0 + y1) * dy) });
      }
    }
    faces.sort((a, c) => a.depth - c.depth);
    let front = null;
    const lightMode = !!night;
    const winPat = variant === -2 ? col.pat : variant >= 0 ? nightWindowPatterns(ctx)[lightMode ? 'light' : 'img'][variant] : lightMode ? null : dark3(ctx);
    for (const f of faces) {
      ctx.fillStyle = lightMode ? night.fill : col.faces[f.ny > 0.6 ? 0 : f.ny > -0.3 ? 1 : 2];
      ctx.beginPath();
      ctx.moveTo(f.x0, f.y0); ctx.lineTo(f.x0 + f.ex, f.y0 + f.ey);
      ctx.lineTo(f.x0 + f.ex + dx, f.y0 + f.ey + dy); ctx.lineTo(f.x0 + dx, f.y0 + dy); ctx.closePath();
      ctx.fill();
      if (winPat && f.L > 24 && H > 24 && b.kind !== BUILDING_KIND.small) {
        ctx.save();
        ctx.transform(f.ex / f.L, f.ey / f.L, dx / H, dy / H, f.x0, f.y0);
        ctx.fillStyle = winPat;
        ctx.fillRect(4, 2, f.L - 8, H - 4);
        ctx.restore();
      }
      if (lightMode) continue;
      if (b.doors) for (const [dr, de, dt] of b.doors) if (dr === f.ri && de === f.ei) { // Hauseingang
        ctx.save();
        ctx.transform(f.ex / f.L, f.ey / f.L, dx / H, dy / H, f.x0, f.y0);
        const u = dt / 1000 * f.L;
        ctx.fillStyle = '#3b2a1e'; ctx.fillRect(u - 6, 0, 12, Math.min(22, H * 0.4));
        ctx.restore();
      }
      if (!front || f.ny * f.L > front.ny * front.L) front = f;
    }
    if (lightMode) { // Lichtkarte: Dach verdeckt, was dahinter am Boden leuchtet
      ctx.save(); ctx.translate(dx, dy); ctx.fillStyle = night.fill; ctx.fill(pathOf(b), 'evenodd'); ctx.restore();
      return;
    }
    if (b.kind === BUILDING_KIND.spaeti && front) {
      ctx.save();
      ctx.transform(front.ex / front.L, front.ey / front.L, dx / H, dy / H, front.x0, front.y0);
      const w = Math.min(front.L, 120), u0 = (front.L - w) / 2;
      ctx.fillStyle = '#9fd3ff'; ctx.fillRect(u0 + 6, 2, w - 30, 13);
      ctx.fillStyle = '#e03b3b'; ctx.fillRect(u0, 15, w, 12);
      ctx.save(); ctx.scale(1, -1); ctx.fillStyle = '#fff'; ctx.font = 'bold 10px Segoe UI, system-ui, sans-serif'; ctx.textAlign = 'center';
      ctx.fillText('SPÄTI 24/7', u0 + w / 2, -17); ctx.restore();
      ctx.restore();
    }
    ctx.save();
    ctx.translate(dx, dy);
    const p = pathOf(b);
    ctx.fillStyle = col.roof; ctx.fill(p, 'evenodd');
    ctx.strokeStyle = col.line; ctx.lineWidth = 2; ctx.stroke(p);
    if (b.kind === BUILDING_KIND.warehouse) {
      ctx.fillStyle = '#e8e8e8'; ctx.font = 'bold 18px Segoe UI, system-ui, sans-serif'; ctx.textAlign = 'center';
      ctx.fillText('LAGER 7', b.cx, b.cy + 6);
    }
    ctx.restore();
  }
}

function drawCrate(ctx, c) {
  ctx.fillStyle = 'rgba(0,0,0,0.3)'; ctx.fillRect(c.x + 3, c.y + 3, c.w, c.h);
  ctx.fillStyle = '#8a5f2b'; ctx.fillRect(c.x, c.y + c.h - 8, c.w, 8);
  ctx.fillStyle = '#b07a3a'; ctx.fillRect(c.x, c.y - 8, c.w, c.h);
  ctx.strokeStyle = '#6e4a1c'; ctx.lineWidth = 1.5; ctx.strokeRect(c.x + 1, c.y - 7, c.w - 2, c.h - 2);
  ctx.beginPath(); ctx.moveTo(c.x + 1, c.y - 7); ctx.lineTo(c.x + c.w - 1, c.y + c.h - 9); ctx.stroke();
}

export { playerCar, speedOf };
