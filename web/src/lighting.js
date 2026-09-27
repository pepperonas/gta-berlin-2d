// Schattenwurf und Lichtkarte (nur Darstellung). Beide arbeiten in eigenen Bildschirm-Canvases:
//  – Schatten: Häuser und Bäume werden deckend schwarz in eine Schattenebene gezeichnet und einmal halbtransparent
//    aufgetragen – so dunkeln sich überlappende Schatten nicht doppelt ab.
//  – Licht: die Lichtkarte ist mit dem Umgebungslicht gefüllt, Lichtquellen kommen additiv dazu, dann wird sie per
//    „multiply“ über die Welt gelegt (dunkle Nacht, helle Lichtkegel). Bei Tag entfällt sie.
import { RENDER } from './config.js';

export function makeCanvas(w, h) {
  if (typeof OffscreenCanvas !== 'undefined') return new OffscreenCanvas(w, h);
  return Object.assign(document.createElement('canvas'), { width: w, height: h });
}

function sized(c, w, h) {
  if (!c) return makeCanvas(w, h);
  if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
  return c;
}

export const SHADOW_ALPHA = 0.3;       // Deckkraft der Hausschatten bei voller Sonne
const SHADOW_COLOR = '#0e1330';        // leicht bläulich, wirkt natürlicher als reines Schwarz
const MAX_REACH = 900;                 // längster gezeichneter Schatten (px)

// Hausgrundriss-Höhe wie in drawBuilding (schräge Extrusion)
export const buildingHeight = (b) => Math.max(18, b.height * RENDER.heightScale);

// Versatz, um den ein Schatten der Höhe h wandert
export function shadowOffset(sun, h) {
  const k = Math.min(MAX_REACH, h * sun.len);
  return [sun.dx * k, sun.dy * k];
}

// Wo können Häuser stehen, deren Schatten ins Bild fällt? (Sichtrechteck gegen die Sonne verlängert)
export function casterBox(v, sun, maxH = 400) {
  const [ox, oy] = shadowOffset(sun, maxH);
  return { x: v.x - Math.max(0, ox) - 20, y: v.y - Math.max(0, oy) - 20, w: v.w + Math.abs(ox) + 40, h: v.h + Math.abs(oy) + 40 };
}

// Schattenfläche eines Hauses: jede Wand überstreicht beim Verschieben um (ox, oy) ein Viereck. Die Vierecke aller Wände
// (auch der Innenhöfe) ergeben zusammen mit dem Grundriss den ganzen Schatten des Prismas – Innenhöfe bleiben
// korrekt nur dort beschattet, wo die Wände hineinwerfen. Alle Vierecke gleich orientiert, damit „nonzero“ vereinigt.
export function addBuildingShadow(g, b, ox, oy) {
  for (const r of b.rings) {
    const n = r.length;
    for (let i = 0; i < n; i += 2) {
      const x0 = r[i], y0 = r[i + 1], x1 = r[(i + 2) % n], y1 = r[(i + 3) % n];
      const cross = (x1 - x0) * oy - (y1 - y0) * ox;
      if (Math.abs(cross) < 1e-6) continue;
      if (cross > 0) { g.moveTo(x0, y0); g.lineTo(x1, y1); g.lineTo(x1 + ox, y1 + oy); g.lineTo(x0 + ox, y0 + oy); }
      else { g.moveTo(x0, y0); g.lineTo(x0 + ox, y0 + oy); g.lineTo(x1 + ox, y1 + oy); g.lineTo(x1, y1); }
      g.closePath();
    }
  }
}

export class Lighting {
  constructor() { this.shadow = null; this.light = null; this.sprites = new Map(); }

  // Schattenebene zeichnen und auftragen. tf: [s, tx, ty] Welt→Bildschirm.
  drawShadows(ctx, W, H, tf, sun, buildings, trees) {
    if (sun.strength < 0.02) return 0;
    this.shadow = sized(this.shadow, W, H);
    const g = this.shadow.getContext('2d');
    g.setTransform(1, 0, 0, 1, 0, 0); g.clearRect(0, 0, W, H);
    g.setTransform(tf[0], 0, 0, tf[0], tf[1], tf[2]);
    g.fillStyle = SHADOW_COLOR;
    g.beginPath();
    for (const b of buildings) { const [ox, oy] = shadowOffset(sun, buildingHeight(b)); addBuildingShadow(g, b, ox, oy); }
    g.fill('nonzero');
    // Bäume: Krone als Scheibe, entlang der Sonne um die Stammhöhe versetzt
    g.beginPath();
    for (const t of trees) {
      const h = Math.min(t.size * 0.5, 30) + t.size * 0.8;
      const [ox, oy] = shadowOffset(sun, h * 0.6);
      g.moveTo(t.x + ox + t.size * 0.9, t.y + oy);
      g.arc(t.x + ox, t.y + oy, t.size * 0.9, 0, Math.PI * 2);
    }
    g.fill('nonzero');
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalAlpha = SHADOW_ALPHA * sun.strength;
    ctx.drawImage(this.shadow, 0, 0);
    ctx.restore();
    return buildings.length;
  }

  // Weiche runde Lichtquelle (Farbe als "r,g,b"), einmal gerendert und zwischengespeichert.
  glow(rgb) {
    const key = 'g' + rgb;
    let c = this.sprites.get(key);
    if (!c) {
      c = makeCanvas(128, 128);
      const g = c.getContext('2d');
      const gr = g.createRadialGradient(64, 64, 0, 64, 64, 64);
      gr.addColorStop(0, `rgba(${rgb},1)`); gr.addColorStop(0.35, `rgba(${rgb},0.55)`); gr.addColorStop(1, `rgba(${rgb},0)`);
      g.fillStyle = gr; g.fillRect(0, 0, 128, 128);
      this.sprites.set(key, c);
    }
    return c;
  }

  // Scheinwerferkegel: zeigt nach rechts (+x), Ursprung links mittig.
  cone(rgb) {
    const key = 'c' + rgb;
    let c = this.sprites.get(key);
    if (!c) {
      c = makeCanvas(256, 160);
      const g = c.getContext('2d');
      const gr = g.createRadialGradient(0, 80, 0, 0, 80, 256);
      gr.addColorStop(0, `rgba(${rgb},0.95)`); gr.addColorStop(0.5, `rgba(${rgb},0.4)`); gr.addColorStop(1, `rgba(${rgb},0)`);
      g.fillStyle = gr;
      g.beginPath(); g.moveTo(0, 70); g.lineTo(256, 0); g.lineTo(256, 160); g.lineTo(0, 90); g.closePath(); g.fill();
      this.sprites.set(key, c);
    }
    return c;
  }

  // Lichtkarte: ambient [r,g,b] (0…1), lights: [{ x, y, r, rgb, a, cone?: angle }]
  drawLightmap(ctx, W, H, tf, ambient, lights) {
    this.light = sized(this.light, W, H);
    const g = this.light.getContext('2d');
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.globalCompositeOperation = 'source-over'; g.globalAlpha = 1;
    g.fillStyle = `rgb(${Math.round(ambient[0] * 255)},${Math.round(ambient[1] * 255)},${Math.round(ambient[2] * 255)})`;
    g.fillRect(0, 0, W, H);
    g.setTransform(tf[0], 0, 0, tf[0], tf[1], tf[2]);
    g.globalCompositeOperation = 'lighter';
    for (const l of lights) {
      g.globalAlpha = Math.min(1, l.a);
      if (l.cone !== undefined) {
        g.save(); g.translate(l.x, l.y); g.rotate(l.cone);
        g.drawImage(this.cone(l.rgb), 0, -l.r * 0.31, l.r, l.r * 0.62);
        g.restore();
      } else g.drawImage(this.glow(l.rgb), l.x - l.r, l.y - l.r, l.r * 2, l.r * 2);
    }
    g.globalAlpha = 1; g.globalCompositeOperation = 'source-over';
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalCompositeOperation = 'multiply';
    ctx.drawImage(this.light, 0, 0);
    ctx.restore();
    return lights.length;
  }
}
