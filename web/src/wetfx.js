// Wetter und Nacht in der Darstellung: Wolkenschatten, Regen mit Spritzern, nasser Asphalt mit Pfützen, Nebel und
// Leuchtreklame vor Kneipen, Clubs, Spätis und Imbissen. Reine Darstellung – Positionen kommen aus Ort-Hashes,
// Bewegung aus der Spielzeit; nichts davon berührt die Simulation.
import { hash01, inBuilding } from './map.js';
import { pointAlong } from './geom.js';

const h = (...n) => hash01(n.reduce((a, b) => a * 31 + Math.round(b), 5));

// --- Wolkenschatten: große weiche Flecken auf einem Weltraster, die mit dem Wind ziehen ---------------------------
const CLOUD_CELL = 2600;
let cloudSprite = null;
function cloudBlob() {
  if (cloudSprite !== null) return cloudSprite;
  cloudSprite = false;
  try {
    const c = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(128, 128) : typeof document !== 'undefined' ? Object.assign(document.createElement('canvas'), { width: 128, height: 128 }) : null;
    const g = c?.getContext('2d');
    if (!g?.createRadialGradient) return cloudSprite;
    for (const [x, y, r] of [[64, 64, 52], [40, 58, 34], [88, 70, 36], [62, 44, 30]]) {
      const gr = g.createRadialGradient(x, y, 0, x, y, r);
      gr.addColorStop(0, 'rgba(20,28,45,0.55)'); gr.addColorStop(1, 'rgba(20,28,45,0)');
      g.fillStyle = gr; g.fillRect(0, 0, 128, 128);
    }
    cloudSprite = c;
  } catch { cloudSprite = false; }
  return cloudSprite;
}

// Wolken im Rechteck v (Weltkoordinaten): [{ x, y, r }] – wie viele, hängt an der Bewölkung
export function cloudShadows(v, wx, t, seed = 1) {
  const out = [], ox = wx.wind.x * t, oy = wx.wind.y * t;
  const x0 = Math.floor((v.x - ox) / CLOUD_CELL) - 1, x1 = Math.floor((v.x + v.w - ox) / CLOUD_CELL) + 1;
  const y0 = Math.floor((v.y - oy) / CLOUD_CELL) - 1, y1 = Math.floor((v.y + v.h - oy) / CLOUD_CELL) + 1;
  for (let i = x0; i <= x1; i++) for (let j = y0; j <= y1; j++) {
    for (let k = 0; k < 2; k++) {
      if (h(i, j, k, seed) > wx.cloud * 0.9) continue;
      const r = CLOUD_CELL * (0.35 + h(j, i, k) * 0.45);
      const x = (i + h(i, k, j, 2)) * CLOUD_CELL + ox, y = (j + h(k, j, i, 3)) * CLOUD_CELL + oy;
      if (x + r < v.x || x - r > v.x + v.w || y + r < v.y || y - r > v.y + v.h) continue;
      out.push({ x, y, r });
    }
  }
  return out;
}

// Bedeckter Himmel: die ganze Szene etwas grauer (tags; nachts übernimmt die Lichtkarte)
export function drawOvercast(ctx, v, wx, dark) {
  const a = 0.14 * wx.cloud * (1 - dark);
  if (a < 0.01) return;
  ctx.fillStyle = `rgba(70,80,96,${a})`; ctx.fillRect(v.x, v.y, v.w, v.h);
}

export function drawCloudShadows(ctx, v, wx, t, sunStrength) {
  const alpha = 0.55 * sunStrength * Math.min(1, wx.cloud * 1.4) * (1 - wx.rain * 0.6);
  if (alpha < 0.02) return 0;
  const spr = cloudBlob();
  const list = cloudShadows(v, wx, t);
  ctx.save(); ctx.globalAlpha = alpha;
  for (const c of list) {
    if (spr) ctx.drawImage(spr, c.x - c.r, c.y - c.r, c.r * 2, c.r * 2);
    else { ctx.fillStyle = 'rgba(20,28,45,0.3)'; ctx.beginPath(); ctx.arc(c.x, c.y, c.r * 0.6, 0, Math.PI * 2); ctx.fill(); }
  }
  ctx.restore();
  return list.length;
}

// --- Regen: fallende Striche mit dem Wind geneigt, Spritzer am Boden -----------------------------------------------
export function rainDrops(v, wx, t, count) {
  const n = Math.round(count * wx.rain), out = [];
  const fall = 900, tilt = wx.wind.x * 0.02;
  for (let i = 0; i < n; i++) {
    const period = 0.55 + h(i, 1) * 0.35, ph = (t / period + h(i, 2)) % 1, cyc = Math.floor(t / period + h(i, 2));
    const x = v.x + h(i, cyc, 3) * v.w, y = v.y + (h(i, cyc, 4) * 1.2 - 0.1) * v.h + ph * fall * 0.25;
    out.push({ x, y, len: 14 + h(i, 5) * 12, tilt, splash: ph > 0.92 });
  }
  return out;
}

export function drawRain(ctx, v, wx, t, scale) {
  if (wx.rain < 0.03) return 0;
  const drops = rainDrops(v, wx, t, 420);
  ctx.save();
  ctx.strokeStyle = 'rgba(205,218,238,0.6)'; ctx.lineWidth = 1.4 / scale;
  ctx.beginPath();
  for (const d of drops) { ctx.moveTo(d.x, d.y); ctx.lineTo(d.x - d.tilt * d.len, d.y - d.len); }
  ctx.stroke();
  ctx.strokeStyle = 'rgba(220,230,245,0.5)'; ctx.lineWidth = 0.8 / scale;
  for (const d of drops) if (d.splash) { ctx.beginPath(); ctx.ellipse(d.x, d.y, 3.5, 1.8, 0, 0, Math.PI * 2); ctx.stroke(); }
  // Regenschleier: kühler, grauer
  ctx.fillStyle = `rgba(70,85,105,${0.12 * wx.rain})`; ctx.fillRect(v.x, v.y, v.w, v.h);
  ctx.restore();
  return drops.length;
}

// --- Nasse Straße: dunkler Glanz und Pfützen am Fahrbahnrand -----------------------------------------------------
const tmp = { x: 0, y: 0, ux: 1, uy: 0 };
export function edgePuddles(city, e) {
  if (e._puddles) return e._puddles;
  const out = [];
  if (e.cls <= 8 && !e.bridge && !e.passage && e.len > 60) { // unter Häusern (Durchfahrten, Überbauungen) regnet es nicht
    const S = city.scale, n = Math.floor(e.len / (18 * S) + h(e.id, 7));
    for (let k = 0; k < n; k++) {
      if (h(e.id, k, 9) > 0.6) continue;
      const s = (0.1 + 0.8 * h(e.id, k, 1)) * e.len, side = h(k, e.id) < 0.5 ? -1 : 1;
      pointAlong(e.pts, s, tmp);
      const off = side * (e.w / 2 - (0.6 + h(e.id, k, 2) * 1.2) * S); // an der Rinne, wo das Wasser steht
      const px = tmp.x - tmp.uy * off, py = tmp.y + tmp.ux * off;
      if (inBuilding(city, px, py)) continue;
      out.push({ x: px, y: py, rx: (0.8 + h(e.id, k, 3) * 1.6) * S, ry: (0.5 + h(e.id, k, 4) * 0.7) * S, a: Math.atan2(tmp.uy, tmp.ux) });
    }
  }
  return (e._puddles = out);
}

export function drawWetRoads(ctx, edges, junctions, pathOf, city, wet, L) {
  if (wet < 0.02) return 0;
  ctx.save();
  ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  ctx.strokeStyle = `rgba(12,20,34,${0.24 * wet})`;
  for (const e of edges) if (!e.bridge && e.cls <= 10) { ctx.lineWidth = e.w; ctx.stroke(pathOf(e)); }
  ctx.fillStyle = `rgba(12,20,34,${0.24 * wet})`;
  for (const j of junctions) if (!j.bridge) { ctx.beginPath(); ctx.arc(j.x, j.y, j.r, 0, Math.PI * 2); ctx.fill(); }
  // Pfützen spiegeln den Himmel: tags hell-graublau, nachts fast schwarz (die Lichter spiegeln sich über die Lichtkarte)
  const day = 1 - L.dark;
  ctx.fillStyle = `rgba(${Math.round(60 + 110 * day)},${Math.round(70 + 120 * day)},${Math.round(90 + 125 * day)},${0.55 * wet})`;
  let n = 0;
  for (const e of edges) for (const p of edgePuddles(city, e)) { ctx.beginPath(); ctx.ellipse(p.x, p.y, p.rx * wet, p.ry * wet, p.a, 0, Math.PI * 2); ctx.fill(); n++; }
  ctx.restore();
  return n;
}

// --- Nebel: Dunst über allem, um die Bildmitte etwas lichter ------------------------------------------------------
export function drawFog(ctx, v, wx) {
  if (wx.fog < 0.03) return false;
  const cx = v.x + v.w / 2, cy = v.y + v.h / 2, R = Math.hypot(v.w, v.h) / 2;
  const a = 0.62 * wx.fog;
  if (ctx.createRadialGradient) {
    const gr = ctx.createRadialGradient(cx, cy, R * 0.12, cx, cy, R);
    gr.addColorStop(0, `rgba(214,218,222,${a * 0.35})`); gr.addColorStop(0.6, `rgba(214,218,222,${a * 0.85})`); gr.addColorStop(1, `rgba(214,218,222,${a})`);
    ctx.fillStyle = gr;
  } else ctx.fillStyle = `rgba(214,218,222,${a * 0.7})`;
  ctx.fillRect(v.x, v.y, v.w, v.h);
  return true;
}

// --- Leuchtreklame ------------------------------------------------------------------------------------------------
const NEON_COLORS = ['#ff3cac', '#3cf0ff', '#57ff6b', '#ffae3c', '#b76bff', '#ff4b4b', '#fff04a'];
// Schriftzug einer Leuchtreklame für einen POI oder null (nur Nachtleben, Spätis, Imbisse, Hotels)
export function neonText(q) {
  const name = String(q.name ?? '').trim();
  const short = (s) => s.toUpperCase().replace(/\s+/g, ' ').slice(0, 14);
  if (q.cat === 'drink') return q.kind === 'nightclub' ? (name ? short(name) : 'CLUB') : name && h(q.x, q.y, 1) < 0.6 ? short(name) : q.kind === 'pub' ? 'KNEIPE' : 'BAR';
  if (q.kind === 'convenience' || q.kind === 'kiosk') return 'SPÄTI';
  if (q.cat === 'food') {
    if (/d[öo]ner|kebab/i.test(name)) return 'DÖNER';
    if (/pizz/i.test(name)) return 'PIZZA';
    if (q.kind === 'fast_food') return h(q.x, q.y, 2) < 0.5 ? 'IMBISS' : name ? short(name) : 'IMBISS';
    return h(q.x, q.y, 3) < 0.35 && name ? short(name) : null;
  }
  if (q.cat === 'hotel') return 'HOTEL';
  return null;
}
export const neonColor = (q) => NEON_COLORS[Math.floor(h(q.x, q.y, 4) * NEON_COLORS.length)];
// flackernde Röhren (jede achte), sonst an
export const neonOn = (q, t) => h(q.x, q.y, 5) > 0.12 || h(q.x, q.y, Math.floor(t * 9)) > 0.3;

export function drawNeon(ctx, signs, t, k) {
  ctx.save();
  ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  ctx.font = 'bold 9px system-ui, sans-serif';
  for (const s of signs) {
    const on = neonOn(s.q, t);
    ctx.globalAlpha = on ? k : 0.35 * k;
    ctx.lineJoin = 'round';
    if (on) { ctx.strokeStyle = s.color; ctx.globalAlpha = 0.3 * k; ctx.lineWidth = 5; ctx.strokeText(s.text, s.x, s.y); ctx.globalAlpha = k; }
    ctx.fillStyle = on ? '#ffffff' : s.color; ctx.lineWidth = 2; ctx.strokeStyle = s.color;
    ctx.strokeText(s.text, s.x, s.y); ctx.fillText(s.text, s.x, s.y);
  }
  ctx.restore();
}
