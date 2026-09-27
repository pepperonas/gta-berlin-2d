// Welt-Rendering in schräger Draufsicht: Boden flach (Flächen, Wasser, Straßen als Vektoren aus der Karte),
// Gebäude als extrudierte Grundrisse mit sichtbaren Fassaden und leichter Parallaxe (Dach wandert von der
// Bildmitte weg → Seitenwände werden sichtbar).
import { MISSION, RENDER } from './config.js';
import { AREA_KIND, BUILDING_KIND } from './citycodes.js';
import { drawCar, drawPerson, drawTree, shade } from './assets.js';
import { playerCar, speedOf } from './world.js';
import { offsetPolyline } from './geom.js';

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
export const pathOf = (f) => (f._path ??= f.pts ? linePath(f.pts) : ringPath(f.rings));
export { AREA_COLOR, WATER, ASPHALT };

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

export class Renderer {
  constructor(ctx) {
    this.ctx = ctx;
    this.skids = [];
    this.particles = [];
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
    const ctx = this.ctx, cam = world.camera, city = world.city;
    const s = scale * cam.zoom;
    ctx.setTransform(s, 0, 0, s, W / 2 - cam.x * s, H / 2 - cam.y * s);
    const vw = W / s, vh = H / s;
    const v = { x: cam.x - vw / 2, y: cam.y - vh / 2, w: vw, h: vh };
    const t = world.time;
    windowPatterns ??= makeWindowPatterns(ctx);
    ctx.lineDashOffset = 0; // gestrichelte Markierungen stehen fest auf der Straße

    // Sichtbare Kartenobjekte (unten großzügiger: hohe Häuser ragen ins Bild).
    const q = city.render.query({ x: v.x - 60, y: v.y - 60, w: v.w + 120, h: v.h + 420 }, this._q ??= []);
    const areas = [], water = [], edges = [], paths = [], rails = [], buildings = [], trees = [];
    for (const f of q) {
      switch (f.layer) {
        case 'area': areas.push(f); break;
        case 'water': water.push(f); break;
        case 'edge': edges.push(f); break;
        case 'path': paths.push(f); break;
        case 'rail': rails.push(f); break;
        case 'building': buildings.push(f); break;
        case 'tree': trees.push(f); break;
      }
    }

    // 1) Grund: Gehweg/Hof, darauf Flächen (Grün, Plätze, Gleisanlagen)
    ctx.fillStyle = SIDEWALK;
    ctx.fillRect(v.x - 5, v.y - 5, v.w + 10, v.h + 10);
    areas.sort((a, b) => a.kind - b.kind);
    for (const a of areas) { ctx.fillStyle = AREA_COLOR[a.kind]; ctx.fill(pathOf(a), 'evenodd'); }

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
      ctx.restore();
      ctx.strokeStyle = '#6f6a60'; ctx.lineWidth = 3; ctx.stroke(p);
    }

    // 3) Wege (Parks, Fußwege) und ebenerdige Gleise
    ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.strokeStyle = '#b9ab8e'; ctx.lineWidth = 18;
    for (const p of paths) if (!p.bridge) ctx.stroke(pathOf(p));
    this.drawTracks(rails.filter((r) => !r.bridge), false);

    // 4) Straßen: erst Bordstein, dann Asphalt; kleine Straßen zuerst, Brücken zuletzt
    edges.sort((a, b) => (a.bridge - b.bridge) || (b.cls - a.cls));
    for (const e of edges) {
      if (e.bridge) { ctx.strokeStyle = '#7d7a73'; ctx.lineWidth = e.w + 14; ctx.stroke(pathOf(e)); }
      else if (e.cls <= 10) { ctx.strokeStyle = CURB; ctx.lineWidth = e.w + 5; ctx.stroke(pathOf(e)); }
    }
    for (const e of edges) {
      ctx.strokeStyle = e.cls === 10 ? '#a8a296' : e.cls === 11 ? '#8a8272' : ASPHALT;
      ctx.lineWidth = e.w; ctx.stroke(pathOf(e));
    }
    ctx.strokeStyle = 'rgba(245,245,235,0.7)'; ctx.lineWidth = 2; ctx.setLineDash([16, 18]);
    for (const e of edges) if (!e.oneway && e.cls <= 7 && e.w >= 70) ctx.stroke(pathOf(e)); // Mittellinie
    ctx.setLineDash([]);
    ctx.strokeStyle = '#b9ab8e'; ctx.lineWidth = 18;
    for (const p of paths) if (p.bridge) ctx.stroke(pathOf(p));
    ctx.lineCap = 'butt'; ctx.lineJoin = 'miter';

    // 5) Bremsspuren
    ctx.lineWidth = 3; ctx.lineCap = 'round';
    for (const k of this.skids) {
      ctx.strokeStyle = `rgba(20,20,20,${Math.min(0.5, k.life / 8 * 0.5)})`;
      ctx.beginPath(); ctx.moveTo(k.x0, k.y0); ctx.lineTo(k.x1, k.y1); ctx.stroke();
    }
    ctx.lineCap = 'butt';

    // 6) Missionsmarker am Boden
    if (overlayMarkers) this.drawZones(world);

    // 7) Tiefensortierte Objekte
    const list = [];
    const margin = 220;
    const near = (x, y) => x > v.x - margin && x < v.x + v.w + margin && y > v.y - margin && y < v.y + v.h + margin * 1.5;
    for (const b of buildings) list.push({ y: b.bbox.y + b.bbox.h, d: () => this.drawBuilding(b, cam) });
    for (const tr of trees) list.push({ y: tr.y, d: () => drawTree(ctx, tr, t) });
    for (const cr of city.crates) if (near(cr.x, cr.y)) list.push({ y: cr.y + cr.h, d: () => drawCrate(ctx, cr) });
    for (const c of world.cars) if (near(c.x, c.y)) list.push({ y: c.y + 6, d: () => drawCar(ctx, c, t) });
    for (const p of world.peds) if (near(p.x, p.y)) list.push({ y: p.y, d: () => drawPerson(ctx, p, { shirt: p.shirt, skin: p.skin, down: p.state === 'down' }) });
    const pl = world.player;
    if (!pl.inCar) list.push({ y: pl.y, d: () => drawPerson(ctx, pl, { shirt: '#ff7a1a', player: true, down: pl.stun > 0 }) });
    list.sort((a, b) => a.y - b.y);
    for (const it of list) it.d();

    // 8) Hochbahn (U1-Viadukt) und Bahnbrücken über allem, was darunter fährt
    this.drawTracks(rails.filter((r) => r.bridge), true);

    // 9) Partikel
    for (const p of this.particles) {
      const a = p.life / p.max;
      if (p.kind === 'spark') { ctx.fillStyle = `rgba(255,${180 + (a * 75) | 0},60,${a})`; ctx.fillRect(p.x - 1, p.y - 1, 2.5, 2.5); }
      else { ctx.fillStyle = `rgba(70,70,70,${a * 0.45})`; ctx.beginPath(); ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2); ctx.fill(); }
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
    const k = H / 720, taken = [];
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
  drawBuilding(b, cam) {
    const ctx = this.ctx;
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
        faces.push({ x0, y0, ex, ey, L, ny, depth: -((x0 + x1) * dx + (y0 + y1) * dy) });
      }
    }
    faces.sort((a, c) => a.depth - c.depth);
    let front = null;
    for (const f of faces) {
      ctx.fillStyle = col.faces[f.ny > 0.6 ? 0 : f.ny > -0.3 ? 1 : 2];
      ctx.beginPath();
      ctx.moveTo(f.x0, f.y0); ctx.lineTo(f.x0 + f.ex, f.y0 + f.ey);
      ctx.lineTo(f.x0 + f.ex + dx, f.y0 + f.ey + dy); ctx.lineTo(f.x0 + dx, f.y0 + dy); ctx.closePath();
      ctx.fill();
      if (f.L > 24 && H > 24 && b.kind !== BUILDING_KIND.small) {
        ctx.save();
        ctx.transform(f.ex / f.L, f.ey / f.L, dx / H, dy / H, f.x0, f.y0);
        ctx.fillStyle = col.pat;
        ctx.fillRect(4, 2, f.L - 8, H - 4);
        ctx.restore();
      }
      if (!front || f.ny * f.L > front.ny * front.L) front = f;
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
