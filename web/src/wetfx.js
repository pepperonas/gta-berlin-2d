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
export function drawOvercast(ctx, v, wx, dark, cover = 0) {
  const a = 0.14 * wx.cloud * (1 - dark) * (1 - 0.6 * cover);
  if (a < 0.01) return;
  ctx.fillStyle = `rgba(70,80,96,${a})`; ctx.fillRect(v.x, v.y, v.w, v.h);
}

export function drawCloudShadows(ctx, v, wx, t, sunStrength) {
  // unter geschlossener Decke (bedeckt, Schneefall) gibt es keine einzelnen Wolkenschatten mehr, nur graues Licht
  const closed = Math.max(0, Math.min(1, (0.97 - wx.cloud) / 0.4));
  const alpha = 0.55 * sunStrength * Math.min(1, wx.cloud * 1.4) * (1 - Math.min(1, wx.rain) * 0.6) * closed * (1 - Math.min(1, wx.snow ?? 0));
  if (alpha < 0.02) return 0;
  const list = cloudShadows(v, wx, t);
  // Wolkenformen aus fraktalem Rauschen (ausgefranst, verschieden groß), ziehen mit dem Wind; Kreise nur ohne Canvas
  const pat = cloudPattern(ctx, wx.cloud);
  if (pat) {
    ctx.save(); ctx.globalAlpha = alpha; ctx.fillStyle = scrolled(pat, CLOUD_SCALE, wx.wind.x * t, wx.wind.y * t);
    ctx.fillRect(v.x, v.y, v.w, v.h); ctx.restore();
    return list.length;
  }
  const spr = cloudBlob();
  ctx.save(); ctx.globalAlpha = alpha;
  for (const c of list) {
    if (spr) ctx.drawImage(spr, c.x - c.r, c.y - c.r, c.r * 2, c.r * 2);
    else { ctx.fillStyle = 'rgba(20,28,45,0.3)'; ctx.beginPath(); ctx.arc(c.x, c.y, c.r * 0.6, 0, Math.PI * 2); ctx.fill(); }
  }
  ctx.restore();
  return list.length;
}

// --- Regen: fallende Striche mit dem Wind geneigt, Spritzer am Boden -----------------------------------------------
// Tropfen: layer 0 fern (kurz, blass) … 2 nah (lang, hell); Richtung (tx, ty) = fallend plus Windversatz (Böen)
export function rainDrops(v, wx, t, count, gust = 1) {
  const n = Math.round(count * wx.rain), out = [];
  const fall = 900, tilt = wx.wind.x * 0.02;
  const storm = wx.storm ?? 0, wl = Math.hypot(wx.wind.x, wx.wind.y) || 1;
  const lean = Math.min(1.6, 0.25 + storm * 1.2 * gust), tx = wx.wind.x / wl * lean, ty = 1 + wx.wind.y / wl * lean * 0.5;
  for (let i = 0; i < n; i++) {
    const period = 0.55 + h(i, 1) * 0.35, ph = (t / period + h(i, 2)) % 1, cyc = Math.floor(t / period + h(i, 2));
    const x = v.x + h(i, cyc, 3) * v.w, y = v.y + (h(i, cyc, 4) * 1.2 - 0.1) * v.h + ph * fall * 0.25;
    const layer = i % 3;
    out.push({ x, y, len: (14 + h(i, 5) * 12) * [0.55, 0.85, 1.2][layer], tilt, tx: storm > 0.05 ? tx : tilt, ty: storm > 0.05 ? ty : 1, layer, splash: ph > 0.92 });
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
  // am Rand des geladenen Gebiets fehlen evtl. Häuser (inBuilding): dann berechnen, aber nicht merken
  const complete = !city.ready || city.ready(e.bbox.x + e.bbox.w / 2, e.bbox.y + e.bbox.h / 2, Math.max(e.bbox.w, e.bbox.h) / 2);
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
  if (complete) e._puddles = out;
  return out;
}

// Nässe auf der Fahrbahn. layer(fn) (render.js): zeichnet deckend in eine eigene Ebene und trägt sie einmal auf – sonst
// dunkeln Überlappungen (Kreuzungsscheiben über den Straßen) zu Kreisen nach. Ohne layer direkt (Tests, Fallback).
export function drawWetRoads(ctx, edges, junctions, pathOf, city, wet, L, { layer = null, t = 0, rain = 0 } = {}) {
  if (wet < 0.02) return 0;
  const paint = (g, col) => {
    g.lineCap = 'round'; g.lineJoin = 'round'; g.strokeStyle = col; g.fillStyle = col;
    for (const e of edges) if (!e.bridge && e.cls <= 10) { g.lineWidth = e.w; g.stroke(pathOf(e)); }
    for (const j of junctions) if (!j.bridge) { g.beginPath(); g.arc(j.x, j.y, j.r, 0, Math.PI * 2); g.fill(); }
  };
  // nasser Asphalt: deutlich dunkler und leicht bläulich (Wasserfilm), tags mit mattem Himmelsglanz
  if (!layer || !layer((g) => paint(g, '#0b121d'), 0.34 * wet)) { ctx.save(); paint(ctx, `rgba(12,20,34,${0.24 * wet})`); ctx.restore(); }
  const day = 1 - L.dark;
  if (layer && day > 0.2) layer((g) => paint(g, '#8fa2bb'), 0.05 * wet * day);
  return drawPuddles(ctx, edges, city, wet, L, t, rain);
}

// Pfützen: dunkler, nasser Rand, darin der Himmel gespiegelt (tags hell-graublau, zur Mitte heller; nachts fast
// schwarz – Lichter spiegeln sich über die Lichtkarte); bei Regen Ringe, die sich ausbreiten
export function drawPuddles(ctx, edges, city, wet, L, t = 0, rain = 0) {
  const day = 1 - L.dark, list = [];
  for (const e of edges) for (const p of edgePuddles(city, e)) list.push(p);
  if (!list.length) return 0;
  ctx.save();
  ctx.fillStyle = `rgba(8,12,18,${0.35 * wet})`;
  ctx.beginPath();
  for (const p of list) { ctx.moveTo(p.x + p.rx * wet * 1.15, p.y); ctx.ellipse(p.x, p.y, p.rx * wet * 1.15, p.ry * wet * 1.2, p.a, 0, Math.PI * 2); }
  ctx.fill();
  const sky = (k, a) => `rgba(${Math.round(55 + 115 * day * k)},${Math.round(64 + 124 * day * k)},${Math.round(82 + 130 * day * k)},${a})`;
  for (const p of list) {
    const rx = p.rx * wet, ry = p.ry * wet;
    if (ctx.createRadialGradient && rx > 2) {
      const gr = ctx.createRadialGradient(p.x - rx * 0.2, p.y - ry * 0.3, 0, p.x, p.y, rx);
      gr.addColorStop(0, sky(1.08, 0.7 * wet)); gr.addColorStop(0.7, sky(0.9, 0.62 * wet)); gr.addColorStop(1, sky(0.65, 0.5 * wet));
      ctx.fillStyle = gr;
    } else ctx.fillStyle = sky(1, 0.55 * wet);
    ctx.beginPath(); ctx.ellipse(p.x, p.y, rx, ry, p.a, 0, Math.PI * 2); ctx.fill();
  }
  // Regenringe in den Pfützen: je Pfütze einige Ringe mit eigener Phase (aus der Lage), wachsen und verblassen
  if (rain > 0.05) {
    ctx.lineWidth = 0.7;
    const per = Math.min(4, 1 + Math.round(rain * 2.5));
    for (const p of list) for (let k = 0; k < per; k++) {
      const life = 0.7 + h(p.x, p.y, k) * 0.5, ph = (t / life + h(p.y, p.x, k)) % 1, cyc = Math.floor(t / life + h(p.y, p.x, k));
      const u = h(p.x, k, cyc) * 2 - 1, v2 = h(p.y, k, cyc) * 2 - 1;
      if (u * u + v2 * v2 > 0.7) continue;
      const c = Math.cos(p.a), s2 = Math.sin(p.a), ox = u * p.rx * wet * 0.8, oy = v2 * p.ry * wet * 0.8;
      const x = p.x + ox * c - oy * s2, y = p.y + ox * s2 + oy * c, r = 0.6 + ph * 4.5;
      ctx.strokeStyle = `rgba(225,232,242,${(0.55 * (1 - ph) * Math.min(1, rain)).toFixed(3)})`;
      ctx.beginPath(); ctx.ellipse(x, y, r, r * 0.8, 0, 0, Math.PI * 2); ctx.stroke();
    }
  }
  ctx.restore();
  return list.length;
}

// Nasser Boden abseits der Straßen (Gehwege, Höfe, Grün): etwas dunkler – vor Straßen und Häusern gezeichnet
export function drawWetGround(ctx, v, wet) {
  if (wet < 0.03) return false;
  ctx.fillStyle = `rgba(16,22,32,${(0.16 * wet).toFixed(3)})`; ctx.fillRect(v.x - 5, v.y - 5, v.w + 10, v.h + 10);
  return true;
}

// Bodennebel: liegt auf Straßen und Höfen, bevor Autos, Menschen und Häuser gezeichnet werden – die Dächer ragen so
// heraus (sie sind dem Blick näher). Schwaden aus Rauschen ziehen langsam mit dem Wind.
export function drawGroundFog(ctx, v, wx, t) {
  const fog = wx.fog ?? 0;
  if (fog < 0.03) return false;
  const k = Math.min(1, fog), dense = clamp01((fog - 1) / 0.7);
  ctx.save();
  ctx.fillStyle = `rgba(210,215,220,${(0.28 * k + 0.2 * dense).toFixed(3)})`; ctx.fillRect(v.x, v.y, v.w, v.h);
  const pat = fogPattern(ctx);
  if (pat) {
    ctx.globalAlpha = 0.45 * k + 0.3 * dense;
    ctx.fillStyle = scrolled(pat, 7, wx.wind.x * t * 0.4, wx.wind.y * t * 0.4); ctx.fillRect(v.x, v.y, v.w, v.h);
    ctx.globalAlpha = 0.3 * k;
    ctx.fillStyle = scrolled(pat, 3.1, -wx.wind.y * t * 0.25 + 900, wx.wind.x * t * 0.25 + 400); ctx.fillRect(v.x, v.y, v.w, v.h);
  }
  ctx.restore();
  return true;
}

// Aufschlagringe des Regens am Boden (wachsen und verblassen), im ersten Moment ein Spritzer (crown)
export function rainRipples(v, wx, t, count) {
  const n = Math.round(count * Math.min(1.6, wx.rain ?? 0)), out = [];
  for (let i = 0; i < n; i++) {
    const life = 0.35 + h(i, 61) * 0.3, ph = (t / life + h(i, 62)) % 1, cyc = Math.floor(t / life + h(i, 62));
    out.push({ x: v.x + h(i, cyc, 63) * v.w, y: v.y + h(i, cyc, 64) * v.h, r: 0.8 + ph * (3 + h(i, 65) * 3), a: 1 - ph, crown: ph < 0.18 });
  }
  return out;
}

// --- Nebel: Dunst über allem, um die Bildmitte etwas lichter ------------------------------------------------------
export function drawFog(ctx, v, wx) {
  if (wx.fog < 0.03) return false;
  const cx = v.x + v.w / 2, cy = v.y + v.h / 2, R = Math.hypot(v.w, v.h) / 2;
  const dense = Math.max(0, Math.min(1, (wx.fog - 1) / 0.7)); // dichter Nebel: kaum 50 m Sicht, Mitte kaum lichter
  const a = Math.min(0.85, 0.42 * Math.min(1, wx.fog) + 0.33 * dense); // Rest der Trübung liegt als Bodennebel darunter
  if (ctx.createRadialGradient) {
    const gr = ctx.createRadialGradient(cx, cy, R * (0.12 - 0.08 * dense), cx, cy, R * (1 - 0.35 * dense));
    gr.addColorStop(0, `rgba(214,218,222,${a * (0.35 + 0.3 * dense)})`); gr.addColorStop(0.6, `rgba(214,218,222,${a * 0.85})`); gr.addColorStop(1, `rgba(214,218,222,${a})`);
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

// ================================================================================================================
// Starkregen, Sturm, Gewitter, dichter Nebel, Schneefall und Schneedecke
// ================================================================================================================
const clamp01 = (x) => Math.max(0, Math.min(1, x));
const makeC = (w, hgt) => {
  try {
    if (typeof OffscreenCanvas !== 'undefined') return new OffscreenCanvas(w, hgt);
    if (typeof document !== 'undefined') return Object.assign(document.createElement('canvas'), { width: w, height: hgt });
  } catch { /* kein Canvas */ }
  return null;
};

// Wertrauschen auf einem kachelbaren Gitter (period Zellen), bilinear mit weicher Kurve – 0…1
export function tileNoise(x, y, period, seed = 1) {
  const xi = Math.floor(x), yi = Math.floor(y), fx = x - xi, fy = y - yi;
  const v = (i, j) => hash01((((i % period) + period) % period) * 7919 + (((j % period) + period) % period) * 104729 + seed * 31);
  const sx = fx * fx * (3 - 2 * fx), sy = fy * fy * (3 - 2 * fy);
  const a = v(xi, yi) + (v(xi + 1, yi) - v(xi, yi)) * sx, b = v(xi, yi + 1) + (v(xi + 1, yi + 1) - v(xi, yi + 1)) * sx;
  return a + (b - a) * sy;
}
// fraktales Rauschen (drei Oktaven), kachelbar über N px
export function snowNoise(px, py, N) {
  let s = 0, amp = 0.55, per = 4, norm = 0;
  for (let o = 0; o < 3; o++) { s += amp * tileNoise(px / N * per, py / N * per, per, o + 1); norm += amp; amp *= 0.5; per *= 2; }
  return s / norm;
}

// --- Rauschmuster (kachelbar, je Zeichenfläche einmal erzeugt): Wolkenschatten, Nebel, Regenschleier ----------------
const NOISE_N = 256, noisePats = new WeakMap();
const smooth = (u) => { const x = clamp01(u); return x * x * (3 - 2 * x); };
// build(n, x, y) → [r, g, b, a] (0…255) für das Rauschen n an der Stelle (x, y); null ohne Canvas
export function noisePattern(ctx, key, build) {
  let per = noisePats.get(ctx);
  if (!per) noisePats.set(ctx, per = new Map());
  if (per.has(key)) return per.get(key);
  let pat = null;
  const c = makeC(NOISE_N, NOISE_N), g = c?.getContext('2d');
  if (g?.createImageData && ctx.createPattern) {
    const img = g.createImageData(NOISE_N, NOISE_N), d = img?.data;
    if (!d) { per.set(key, null); return null; } // Test-Attrappe ohne Bilddaten
    for (let y = 0; y < NOISE_N; y++) for (let x = 0; x < NOISE_N; x++) {
      const px = build(snowNoise(x, y, NOISE_N), x, y), i = (y * NOISE_N + x) * 4;
      d[i] = px[0]; d[i + 1] = px[1]; d[i + 2] = px[2]; d[i + 3] = px[3];
    }
    g.putImageData(img, 0, 0);
    pat = ctx.createPattern(c, 'repeat');
  }
  per.set(key, pat);
  return pat;
}
// Muster in Weltkoordinaten verschieben (Wind) und vergrößern (eine Kachel = 256 × scale px)
export function scrolled(pat, scale, ox, oy, sy = scale, rot = 0) {
  try { pat.setTransform?.(new DOMMatrix().translate(ox, oy).rotate(rot * 180 / Math.PI).scale(scale, sy)); } catch { /* ohne DOMMatrix: unverschoben */ }
  return pat;
}
const CLOUD_SCALE = 24; // eine Kachel ≈ 6 km: einzelne Wolken 300 m – 1,5 km
// Wolkenschatten je Bewölkungsstufe: Rauschen über einer Schwelle, die mit der Bewölkung sinkt, weicher Rand
function cloudPattern(ctx, cloud) {
  const k = Math.max(1, Math.min(8, Math.round(cloud * 8)));
  return noisePattern(ctx, 'cloud' + k, (n) => {
    const edge = 0.74 - k / 8 * 0.36, a = smooth((n - edge) / 0.12);
    return [18, 26, 42, Math.round(a * 150)];
  });
}
// Nebel: weiche, ungleichmäßige Schwaden (hell, halbtransparent)
const fogPattern = (ctx) => noisePattern(ctx, 'fog', (n) => [214, 219, 224, Math.round(smooth((n - 0.25) / 0.6) * 255)]);
// Regenschleier: Streifen dichteren Regens (wird entlang der Fallrichtung gestreckt)
const rainPattern = (ctx) => noisePattern(ctx, 'rain', (n) => [190, 202, 218, Math.round(smooth((n - 0.45) / 0.35) * 255)]);

// Deckkraft der Schneedecke an einer Rauschstelle n (0…1) bei Schneehöhe depth: dünn = Flecken (nur in den Senken,
// wo der Wind den Schnee hintreibt), tief = geschlossen
export function snowCoverAlpha(n, depth) {
  if (depth <= 0) return 0;
  const edge = 0.72 - depth * 0.75; // Schwelle wandert mit der Höhe durch das Rauschen (gemessene Verteilung 0,28…0,82)
  const u = clamp01((n - edge) / 0.18);
  return clamp01(u * u * (3 - 2 * u) * (0.55 + 0.45 * clamp01(depth * 1.6)));
}

// Schneetextur je Höhenstufe (0…9): kachelbares Bild 256 px mit Flecken, leichtem Blauschimmer in den Mulden und Glitzern
const SNOW_N = 256, snowPats = new WeakMap();
export function snowPattern(ctx, depth) {
  const k = Math.max(1, Math.min(10, Math.round(depth * 10)));
  let per = snowPats.get(ctx);
  if (!per) snowPats.set(ctx, per = new Map());
  if (per.has(k)) return per.get(k);
  let pat = null;
  const c = makeC(SNOW_N, SNOW_N), g = c?.getContext('2d');
  if (g?.createImageData && ctx.createPattern) {
    const img = g.createImageData(SNOW_N, SNOW_N), d = img.data, dep = k / 10;
    for (let y = 0; y < SNOW_N; y++) for (let x = 0; x < SNOW_N; x++) {
      const n = snowNoise(x, y, SNOW_N), a = snowCoverAlpha(n, dep), i = (y * SNOW_N + x) * 4;
      const shade = 0.84 + 0.16 * snowNoise(x * 3 + 17, y * 3 + 5, SNOW_N); // Wellen und Verwehungen der Oberfläche
      const sparkle = hash01(x * 131 + y * 977 + 7) > 0.996 ? 12 : 0;
      d[i] = Math.min(255, 236 * shade + 18 + sparkle); d[i + 1] = Math.min(255, 240 * shade + 16 + sparkle); d[i + 2] = Math.min(255, 250 * shade + 12 + sparkle);
      d[i + 3] = Math.round(255 * a);
    }
    g.putImageData(img, 0, 0);
    pat = ctx.createPattern(c, 'repeat');
    try { pat?.setTransform?.(new DOMMatrix().scale(2.2)); } catch { /* ohne DOMMatrix: 1:1 */ }
  }
  per.set(k, pat);
  return pat;
}

// Schneedecke auf dem Boden (Gehwege, Höfe, Grün) – vor Wasser und Straßen gezeichnet
export function drawSnowGround(ctx, v, depth) {
  if (depth < 0.02) return false;
  const pat = snowPattern(ctx, depth);
  ctx.save();
  if (pat) { ctx.fillStyle = pat; ctx.fillRect(v.x - 5, v.y - 5, v.w + 10, v.h + 10); }
  else { ctx.fillStyle = `rgba(232,237,245,${0.85 * depth})`; ctx.fillRect(v.x - 5, v.y - 5, v.w + 10, v.h + 10); }
  ctx.restore();
  return true;
}

// Schnee auf Straßen: Matsch über die ganze Breite, festgefahrene Reifenspuren je Fahrstreifen, Schneewälle am
// Bordstein. Je mehr Verkehr (niedrige Klasse), desto freier die Spur. laneOffsets(cs, unit) aus street.js.
export function snowRoadPaths(e, unit, laneOffsets, linePath, offsetPolyline) {
  if (e._snowRoad) return e._snowRoad;
  const tracks = [], berms = [];
  const lo = e.cs ? laneOffsets(e.cs, unit) : null;
  const centers = lo ? [...new Set([...lo.fwd, ...lo.bwd])] : [0];
  for (const c of centers) if (Number.isFinite(c)) for (const s of [-1, 1]) tracks.push(linePath(offsetPolyline(e.pts, c + s * 0.85 * unit)));
  for (const s of [-1, 1]) berms.push(linePath(offsetPolyline(e.pts, s * (e.w / 2 - 0.35 * unit))));
  return (e._snowRoad = { tracks, berms });
}
export const roadSnowAlpha = (cls, depth) => clamp01(depth * (cls <= 3 ? 0.5 : cls <= 5 ? 0.62 : cls <= 8 ? 0.78 : 0.9));

// --- Schneefall: Flocken in drei Tiefen (nah = groß, schnell, unscharf; fern = klein, langsam), Wind und Taumeln --
export function snowFlakes(v, wx, t, count, gust = 1) {
  const n = Math.round(count * clamp01(wx.snow ?? 0)), out = [];
  const wl = Math.hypot(wx.wind.x, wx.wind.y) || 1, ux = wx.wind.x / wl, uy = wx.wind.y / wl;
  const drift = (20 + (wx.storm ?? 0) * 260) * gust;
  for (let i = 0; i < n; i++) {
    const layer = i % 3, sp = [0.45, 0.75, 1.15][layer];
    const life = 2.2 + h(i, 1) * 1.8, ph = (t / life + h(i, 2)) % 1, cyc = Math.floor(t / life + h(i, 2));
    const bx = v.x + h(i, cyc, 3) * v.w, by = v.y + h(i, cyc, 4) * v.h;
    const move = ph * life * drift * sp, sway = Math.sin(t * (1.1 + h(i, 5)) + i) * 7 * sp;
    out.push({
      x: bx + ux * move - uy * sway, y: by + uy * move + ux * sway + ph * 14 * sp, layer,
      r: [0.6, 1, 1.7][layer] * (0.7 + h(i, 6) * 0.6), a: Math.min(1, Math.min(ph, 1 - ph) * 6) * [0.55, 0.75, 0.9][layer],
    });
  }
  return out;
}

let flakeSpr = null;
function flakeSprite() {
  if (flakeSpr !== null) return flakeSpr;
  flakeSpr = false;
  const c = makeC(32, 32), g = c?.getContext('2d');
  if (!g?.createRadialGradient) return flakeSpr;
  const gr = g.createRadialGradient(16, 16, 0, 16, 16, 16);
  gr.addColorStop(0, 'rgba(255,255,255,1)'); gr.addColorStop(0.35, 'rgba(250,252,255,0.85)'); gr.addColorStop(1, 'rgba(240,245,255,0)');
  g.fillStyle = gr; g.fillRect(0, 0, 32, 32);
  return (flakeSpr = c);
}
export function drawSnowfall(ctx, v, wx, t, scale, gust = 1) {
  const snow = wx.snow ?? 0;
  if (snow < 0.03) return 0;
  const flakes = snowFlakes(v, wx, t, 1700, gust);
  const storm = wx.storm ?? 0;
  ctx.save();
  const wl = Math.hypot(wx.wind.x, wx.wind.y) || 1, sx = wx.wind.x / wl, sy = wx.wind.y / wl;
  // fern und mittel: kleine Punkte; nah: weiche, leicht unscharfe Flocken (dem Blick nah, also größer und verwischt)
  const soft = flakeSprite();
  for (let layer = 0; layer < 3; layer++) {
    if (layer === 2 && soft) {
      for (const f of flakes) if (f.layer === 2 && f.a > 0.05) { ctx.globalAlpha = f.a; const r = f.r * 2.2; ctx.drawImage(soft, f.x - r, f.y - r, 2 * r, 2 * r); }
      ctx.globalAlpha = 1;
      continue;
    }
    ctx.fillStyle = layer === 2 ? 'rgba(250,252,255,0.9)' : layer === 1 ? 'rgba(244,247,252,0.75)' : 'rgba(236,241,250,0.55)';
    ctx.beginPath();
    for (const f of flakes) if (f.layer === layer && f.a > 0.05) { ctx.moveTo(f.x + f.r, f.y); ctx.arc(f.x, f.y, f.r, 0, Math.PI * 2); }
    ctx.fill();
  }
  if (storm > 0.2) { // Schneesturm: Flocken ziehen zu Strichen
    ctx.strokeStyle = `rgba(245,248,255,${0.35 * storm})`; ctx.lineWidth = 1.1 / scale; ctx.beginPath();
    const len = 18 * storm * gust;
    for (const f of flakes) if (f.layer === 2) { ctx.moveTo(f.x, f.y); ctx.lineTo(f.x - sx * len, f.y - sy * len); }
    ctx.stroke();
  }
  // Dunst aus Schnee: weißlich, bei Sturm dichter (Sicht nimmt ab); im Sturm fegen Schneeschleier quer durchs Bild
  ctx.fillStyle = `rgba(222,228,238,${0.1 * snow + 0.18 * storm * snow})`; ctx.fillRect(v.x, v.y, v.w, v.h);
  const pat = storm > 0.2 ? fogPattern(ctx) : null;
  if (pat) {
    const sp = (180 + 380 * storm) * gust;
    ctx.globalAlpha = Math.min(0.55, 0.4 * storm * snow);
    ctx.fillStyle = scrolled(pat, 3, sx * sp * t, sy * sp * t, 8, Math.atan2(sy, sx)); ctx.fillRect(v.x, v.y, v.w, v.h);
    ctx.globalAlpha = 1;
  }
  ctx.restore();
  return flakes.length;
}

// --- Regen in drei Tiefen, bei Starkregen dichter und mit Gischtschleier; Sturm treibt ihn schräg ------------------
export function drawRainLayers(ctx, v, wx, t, scale, gust = 1) {
  const rain = wx.rain ?? 0;
  if (rain < 0.03) return 0;
  const drops = rainDrops(v, wx, t, 420, gust);
  const heavy = clamp01(rain - 1);
  ctx.save();
  // Aufschläge am Boden zuerst (die Tropfen fallen darüber): Ringe, im ersten Moment eine kleine Krone
  const rip = rainRipples(v, wx, t, 260);
  ctx.lineWidth = 0.6 / scale; ctx.strokeStyle = 'rgba(215,225,240,0.24)';
  ctx.beginPath();
  for (const r of rip) if (r.a > 0.35) { ctx.moveTo(r.x + r.r, r.y); ctx.ellipse(r.x, r.y, r.r, r.r * 0.75, 0, 0, Math.PI * 2); }
  ctx.stroke();
  ctx.strokeStyle = 'rgba(215,225,240,0.1)'; ctx.beginPath();
  for (const r of rip) if (r.a <= 0.35) { ctx.moveTo(r.x + r.r, r.y); ctx.ellipse(r.x, r.y, r.r, r.r * 0.75, 0, 0, Math.PI * 2); }
  ctx.stroke();
  ctx.fillStyle = 'rgba(232,240,250,0.35)'; ctx.beginPath(); // im ersten Moment ein heller Tupfer (Spritzer)
  for (const r of rip) if (r.crown) { ctx.moveTo(r.x + 0.9, r.y); ctx.arc(r.x, r.y, 0.9, 0, Math.PI * 2); }
  ctx.fill();
  // Tropfen: blasser, langer Schweif (Bewegungsunschärfe) und kurzer heller Kopf; nah = dicker und heller
  for (let layer = 0; layer < 3; layer++) {
    ctx.strokeStyle = ['rgba(170,186,208,0.18)', 'rgba(190,205,226,0.26)', 'rgba(210,222,240,0.34)'][layer];
    ctx.lineWidth = [0.7, 1.0, 1.5][layer] / scale;
    ctx.beginPath();
    for (const d of drops) if (d.layer === layer) { ctx.moveTo(d.x, d.y); ctx.lineTo(d.x - d.tx * d.len, d.y - d.ty * d.len); }
    ctx.stroke();
    ctx.strokeStyle = ['rgba(200,212,230,0.42)', 'rgba(220,230,244,0.6)', 'rgba(238,244,252,0.8)'][layer];
    ctx.lineWidth = [0.9, 1.3, 1.9][layer] / scale;
    ctx.beginPath();
    for (const d of drops) if (d.layer === layer) { ctx.moveTo(d.x, d.y); ctx.lineTo(d.x - d.tx * d.len * 0.3, d.y - d.ty * d.len * 0.3); }
    ctx.stroke();
  }
  // Regenschleier: kühler, grauer; Starkregen nimmt Sicht (Gischt); dichtere Regenwände ziehen mit dem Wind durchs Bild
  ctx.fillStyle = `rgba(70,85,105,${0.12 * Math.min(1, rain) + 0.14 * heavy})`; ctx.fillRect(v.x, v.y, v.w, v.h);
  const pat = rainPattern(ctx), storm = wx.storm ?? 0;
  if (pat && (rain > 0.5 || storm > 0.2)) {
    const wl = Math.hypot(wx.wind.x, wx.wind.y) || 1, sp = (140 + 320 * storm) * gust;
    ctx.globalAlpha = Math.min(0.5, 0.1 * Math.min(1, rain) + 0.22 * heavy + 0.14 * storm);
    ctx.fillStyle = scrolled(pat, 4, wx.wind.x / wl * sp * t, wx.wind.y / wl * sp * t + t * 60, 12, Math.atan2(wx.wind.y, wx.wind.x)); // Wände quer zum Wind
    ctx.fillRect(v.x, v.y, v.w, v.h);
  }
  ctx.restore();
  return drops.length;
}

// --- dichter Nebel: Schwaden, die mit dem Wind ziehen, über dem gleichmäßigen Dunst --------------------------------
let fogSprite = null;
function fogBlob() {
  if (fogSprite !== null) return fogSprite;
  fogSprite = false;
  const c = makeC(128, 128), g = c?.getContext('2d');
  if (!g?.createRadialGradient) return fogSprite;
  for (const [x, y, r] of [[64, 64, 60], [36, 70, 40], [92, 58, 42], [60, 40, 34], [70, 90, 36]]) {
    const gr = g.createRadialGradient(x, y, 0, x, y, r);
    gr.addColorStop(0, 'rgba(222,226,230,0.5)'); gr.addColorStop(1, 'rgba(222,226,230,0)');
    g.fillStyle = gr; g.fillRect(0, 0, 128, 128);
  }
  return (fogSprite = c);
}
export function fogBanks(v, wx, t) {
  const out = [], CELL = 700, ox = wx.wind.x * t * 0.35, oy = wx.wind.y * t * 0.35;
  const x0 = Math.floor((v.x - ox) / CELL) - 1, x1 = Math.floor((v.x + v.w - ox) / CELL) + 1;
  const y0 = Math.floor((v.y - oy) / CELL) - 1, y1 = Math.floor((v.y + v.h - oy) / CELL) + 1;
  const dens = clamp01((wx.fog ?? 0) - 0.4);
  for (let i = x0; i <= x1; i++) for (let j = y0; j <= y1; j++) {
    if (h(i, j, 41) > dens * 1.3) continue;
    const r = CELL * (0.7 + h(j, i, 42) * 0.9), breathe = 1 + 0.08 * Math.sin(t * 0.2 + i * 1.7 + j);
    out.push({ x: (i + h(i, j, 43)) * CELL + ox, y: (j + h(i, j, 44)) * CELL + oy, r: r * breathe, a: 0.45 + 0.4 * h(i, j, 45) });
  }
  return out;
}
export function drawFogBanks(ctx, v, wx, t) {
  if ((wx.fog ?? 0) < 0.5) return 0;
  const spr = fogBlob(), banks = fogBanks(v, wx, t);
  if (!spr) return 0;
  ctx.save();
  for (const b of banks) { ctx.globalAlpha = b.a * clamp01(wx.fog - 0.4); ctx.drawImage(spr, b.x - b.r, b.y - b.r, 2 * b.r, 2 * b.r); }
  ctx.restore();
  return banks.length;
}

// --- Gewitter: Blitzstrahl (Zickzack mit Verästelungen, aus dem Himmel = oberhalb im Bild) und Himmelsblitz ------
export function boltPath(x, y, seed, height = 2400) {
  const main = [], branches = [];
  let px = x + (hash01(seed * 3 + 1) - 0.5) * 500, py = y - height;
  main.push(px, py);
  const n = 22;
  for (let k = 1; k <= n; k++) {
    const u = k / n, tx = px + (x - px) / (n - k + 1), ty = y - height * (1 - u);
    const j = (hash01(seed * 17 + k) - 0.5) * 90 * (1 - u * 0.6);
    const nx = tx + j, ny = ty;
    main.push(nx, ny);
    if (k > 3 && k < n - 2 && hash01(seed * 29 + k) < 0.22) { // Ast
      const b = [nx, ny]; let bx = nx, by = ny;
      const dir = hash01(seed * 37 + k) < 0.5 ? -1 : 1, m = 4 + Math.floor(hash01(seed * 41 + k) * 5);
      for (let q = 1; q <= m; q++) { bx += dir * (15 + hash01(seed * 43 + k * 7 + q) * 45); by += 30 + hash01(seed * 47 + k + q) * 50; b.push(bx, by); }
      branches.push(b);
    }
    px = nx;
  }
  main[main.length - 2] = x; main[main.length - 1] = y;
  return { main, branches };
}

export function drawLightning(ctx, v, strikes, cam) {
  let n = 0;
  for (const s of strikes) {
    if (!s.near) continue;
    const x = cam.x + s.dx * 0.14, y = cam.y + s.dy * 0.08; // nahe Einschläge landen im Bild oder knapp daneben
    if (x < v.x - 400 || x > v.x + v.w + 400 || y < v.y - 200 || y > v.y + v.h + 2600) continue;
    const bp = s._path ??= boltPath(x, y, s.seed);
    const a = clamp01(s.flash * 1.3);
    ctx.save();
    ctx.lineJoin = 'round'; ctx.lineCap = 'round';
    const line = (pts) => { ctx.beginPath(); ctx.moveTo(pts[0], pts[1]); for (let i = 2; i < pts.length; i += 2) ctx.lineTo(pts[i], pts[i + 1]); ctx.stroke(); };
    ctx.strokeStyle = `rgba(140,160,255,${0.3 * a})`; ctx.lineWidth = 44; line(bp.main);
    ctx.strokeStyle = `rgba(190,205,255,${0.6 * a})`; ctx.lineWidth = 12; line(bp.main);
    ctx.strokeStyle = `rgba(255,255,255,${a})`; ctx.lineWidth = 4; line(bp.main);
    ctx.lineWidth = 2; for (const b of bp.branches) { ctx.strokeStyle = `rgba(230,236,255,${0.8 * a})`; line(b); }
    // Einschlag: heller Fleck am Boden
    if (ctx.createRadialGradient) {
      const gr = ctx.createRadialGradient(x, y, 0, x, y, 260);
      gr.addColorStop(0, `rgba(235,240,255,${0.7 * a})`); gr.addColorStop(1, 'rgba(235,240,255,0)');
      ctx.fillStyle = gr; ctx.fillRect(x - 260, y - 260, 520, 520);
    }
    ctx.restore();
    n++;
  }
  return n;
}

// Himmelsblitz über dem ganzen Bild (nach der Lichtkarte): kaltes Weiß, nahe Blitze stärker
export function drawSkyFlash(ctx, v, flash) {
  if (flash < 0.01) return;
  ctx.fillStyle = `rgba(215,225,255,${Math.min(0.75, 0.6 * flash)})`;
  ctx.fillRect(v.x - 5, v.y - 5, v.w + 10, v.h + 10);
}

// --- Sturm: Laub und Papierfetzen, die übers Bild fegen -------------------------------------------------------------
export function stormDebris(v, wx, t, gust = 1) {
  const s = wx.storm ?? 0, out = [];
  if (s < 0.2) return out;
  const n = Math.round(60 * s), wl = Math.hypot(wx.wind.x, wx.wind.y) || 1, ux = wx.wind.x / wl, uy = wx.wind.y / wl;
  for (let i = 0; i < n; i++) {
    const life = 1.6 + h(i, 51) * 1.4, ph = (t / life + h(i, 52)) % 1, cyc = Math.floor(t / life + h(i, 52));
    const sp = (380 + h(i, 53) * 420) * gust, bx = v.x + h(i, cyc, 54) * v.w, by = v.y + h(i, cyc, 55) * v.h;
    const d = (ph - 0.5) * life * sp, wob = Math.sin(t * 5 + i) * 10;
    out.push({ x: bx + ux * d - uy * wob, y: by + uy * d + ux * wob, a: t * (6 + h(i, 56) * 8) + i, kind: h(i, 57) < 0.8 ? 'leaf' : 'paper', c: h(i, 58) });
  }
  return out;
}
export function drawStormDebris(ctx, v, wx, t, gust = 1) {
  const list = stormDebris(v, wx, t, gust);
  for (const d of list) {
    ctx.save(); ctx.translate(d.x, d.y); ctx.rotate(d.a);
    if (d.kind === 'leaf') { ctx.fillStyle = d.c < 0.4 ? '#8a6a2c' : d.c < 0.7 ? '#a0772a' : '#6b7a2c'; ctx.beginPath(); ctx.ellipse(0, 0, 3.2, 1.6, 0, 0, Math.PI * 2); ctx.fill(); }
    else { ctx.fillStyle = 'rgba(235,232,222,0.9)'; ctx.fillRect(-3, -2, 6, 4); }
    ctx.restore();
  }
  return list.length;
}

// Gischt hinter schnellen Autos auf nasser Straße, Schneestaub auf Schnee
export function drawSpray(ctx, car, speed, wet, snow) {
  const k = clamp01((speed - 120) / 400) * Math.max(wet, snow);
  if (k < 0.03) return;
  const c = Math.cos(car.angle), s = Math.sin(car.angle), bx = car.x - c * (car.hw + 4), by = car.y - s * (car.hw + 4);
  ctx.fillStyle = snow > wet ? `rgba(240,244,250,${0.4 * k})` : `rgba(200,208,218,${0.35 * k})`;
  for (let i = 0; i < 3; i++) {
    const d = 6 + i * 9, r = car.hh * (0.8 + i * 0.35);
    ctx.beginPath(); ctx.ellipse(bx - c * d, by - s * d, r * 0.9, r, car.angle, 0, Math.PI * 2); ctx.fill();
  }
}
