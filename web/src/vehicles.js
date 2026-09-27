// Autos in der Draufsicht: fünf Modelle innerhalb derselben Kollisionsbox (42 × 20 px = 4,2 × 2,0 m).
// Karosserie, Scheiben und Dach werden je Modell × Farbe einmal doppelt aufgelöst gezeichnet (Sprite-Cache),
// was sich bewegt – Räder (Vorderräder lenken), Licht, Blinker – kommt jedes Bild dazu.
// Das Modell ist reine Darstellung: aus der Autonummer abgeleitet, die Simulation merkt davon nichts.
import { shade } from './assets.js';

export const CAR_MODELS = ['kleinwagen', 'limousine', 'kombi', 'transporter', 'taxi'];
export const TAXI_COLOR = '#f1e9c8'; // Berliner Taxi: hellelfenbein

const h01 = (n) => { const x = Math.sin(n * 91.345 + 12.9898) * 43758.5453; return x - Math.floor(x); };

export function carModel(car) {
  if (car.model) return car.model;
  if (car.role === 'player') return 'limousine';
  const r = h01(car.id);
  return r < 0.08 ? 'taxi' : r < 0.2 ? 'transporter' : r < 0.45 ? 'kleinwagen' : r < 0.7 ? 'kombi' : 'limousine';
}
export const carColor = (car) => (car.wrecked ? '#3b332d' : carModel(car) === 'taxi' ? TAXI_COLOR : car.color);

function rr(g, x, y, w, h, r) {
  g.beginPath();
  g.moveTo(x + r, y); g.arcTo(x + w, y, x + w, y + h, r); g.arcTo(x + w, y + h, x, y + h, r);
  g.arcTo(x, y + h, x, y, r); g.arcTo(x, y, x + w, y, r); g.closePath();
}

// Karosserie in lokalen Koordinaten: +x = vorn, Mitte (0,0)
function paintBody(g, model, body, L, W, wrecked) {
  const len = model === 'kleinwagen' ? L - 6 : L, x0 = -len / 2, y0 = -W / 2;
  // Querverlauf: Kanten dunkler, Mitte heller → Wölbung
  const gr = g.createLinearGradient(0, y0, 0, y0 + W);
  gr.addColorStop(0, shade(body, -0.28)); gr.addColorStop(0.5, shade(body, 0.1)); gr.addColorStop(1, shade(body, -0.34));
  g.fillStyle = gr; rr(g, x0, y0, len, W, model === 'transporter' ? 3 : 5.5); g.fill();
  g.strokeStyle = shade(body, -0.5); g.lineWidth = 0.9; g.stroke();
  const glass = wrecked ? '#1c1916' : '#1f2a36';
  const glassG = g.createLinearGradient(0, y0, 0, y0 + W);
  glassG.addColorStop(0, glass); glassG.addColorStop(0.45, wrecked ? '#2a2521' : '#4b6379'); glassG.addColorStop(1, glass);
  const roof = shade(body, 0.14);
  const cabin = (xs, xe, frontCut, rearCut) => { // Scheibenkranz mit abgeschrägter Front-/Heckscheibe
    g.fillStyle = glassG; g.beginPath();
    g.moveTo(xs + rearCut, y0 + 2.2); g.lineTo(xe - frontCut, y0 + 2.2); g.lineTo(xe, y0 + 4); g.lineTo(xe, y0 + W - 4);
    g.lineTo(xe - frontCut, y0 + W - 2.2); g.lineTo(xs + rearCut, y0 + W - 2.2); g.lineTo(xs, y0 + W - 4); g.lineTo(xs, y0 + 4); g.closePath(); g.fill();
  };
  if (model === 'transporter') {
    cabin(len / 2 - 11, len / 2 - 5, 1.5, 0);
    g.fillStyle = roof; rr(g, x0 + 1.5, y0 + 1.8, len - 13, W - 3.6, 2); g.fill();          // Kastenaufbau
    g.strokeStyle = shade(body, -0.12); g.lineWidth = 0.7;
    for (let x = x0 + 6; x < len / 2 - 13; x += 5) { g.beginPath(); g.moveTo(x, y0 + 2.5); g.lineTo(x, y0 + W - 2.5); g.stroke(); }
  } else {
    const back = model === 'kombi' ? x0 + 5 : model === 'kleinwagen' ? x0 + 6 : x0 + 9;
    const front = model === 'kleinwagen' ? len / 2 - 9 : len / 2 - 12;
    cabin(back, front, 3.5, model === 'kombi' ? 0.8 : 2.5);
    g.fillStyle = roof; rr(g, back + (model === 'kombi' ? 1.5 : 3.5), y0 + 3.4, front - back - 7.5, W - 6.8, 2.2); g.fill();
    g.fillStyle = shade(body, -0.1); g.fillRect(front + 1.5, y0 + 2.5, 0.8, W - 5);            // Haubenkante
    if (model === 'taxi' && !wrecked) { // Dachschild
      g.fillStyle = '#ffd400'; rr(g, -3.5, -2.2, 7, 4.4, 1); g.fill();
      g.fillStyle = '#222'; g.fillRect(-2, -0.5, 4, 1);
    }
  }
  // Außenspiegel
  g.fillStyle = shade(body, -0.2);
  const mx = model === 'transporter' ? len / 2 - 11 : model === 'kleinwagen' ? len / 2 - 10 : len / 2 - 13;
  g.fillRect(mx, y0 - 1.6, 2.2, 1.8); g.fillRect(mx, y0 + W - 0.2, 2.2, 1.8);
  if (wrecked) { // Ruß und Beulen
    g.fillStyle = 'rgba(0,0,0,0.45)';
    for (const [x, y, r] of [[-6, -3, 5], [8, 4, 4], [-14, 5, 3.5], [13, -4, 3]]) { g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.fill(); }
    g.strokeStyle = 'rgba(255,255,255,0.12)'; g.lineWidth = 0.8;
    g.beginPath(); g.moveTo(len / 2 - 3, y0 + 3); g.lineTo(len / 2 - 8, y0 + 8); g.lineTo(len / 2 - 4, y0 + 12); g.stroke();
  }
}

const cache = new Map();
export const SPRITE_CACHE_MAX = 160;
export const spriteCacheSize = () => cache.size;

function makeCanvas(w, h) {
  if (typeof OffscreenCanvas !== 'undefined') return new OffscreenCanvas(w, h);
  if (typeof document !== 'undefined') return Object.assign(document.createElement('canvas'), { width: w, height: h });
  return null;
}

// Sprite je Modell × Farbe (× Wrack); null, wenn kein Canvas verfügbar ist (dann zeichnet assets.js flach)
export function carSprite(model, body, L, W, wrecked) {
  const key = `${model}|${body}|${wrecked ? 1 : 0}|${L}x${W}`;
  if (cache.has(key)) return cache.get(key);
  if (cache.size >= SPRITE_CACHE_MAX) cache.clear();
  let c = null;
  try {
    c = makeCanvas((L + 8) * 2, (W + 8) * 2);
    if (c) { const g = c.getContext('2d'); g.scale(2, 2); g.translate(L / 2 + 4, W / 2 + 4); paintBody(g, model, body, L, W, wrecked); }
  } catch { c = null; }
  cache.set(key, c);
  return c;
}

// Zeichnet ein Auto (Kontext bereits auf Automitte gedreht). Liefert false, wenn kein Sprite verfügbar war.
export function drawCarBody(ctx, car, t) {
  const L = car.hw * 2, W = car.hh * 2, model = carModel(car), body = carColor(car);
  const spr = carSprite(model, body, L, W, car.wrecked);
  if (!spr) return false;
  // Räder (unter der Karosserie, an den Ecken sichtbar); Vorderräder lenken mit
  const steer = (car.controls?.steer ?? 0) * 0.45, wx = L / 2 - 8, wy = W / 2 - 1.2;
  ctx.fillStyle = '#16171a';
  for (const [x, y, front] of [[wx, -wy, 1], [wx, wy, 1], [-wx + 1, -wy, 0], [-wx + 1, wy, 0]]) {
    ctx.save(); ctx.translate(x, y); if (front) ctx.rotate(steer); ctx.fillRect(-3.8, -1.7, 7.6, 3.4); ctx.restore();
  }
  ctx.drawImage(spr, -L / 2 - 4, -W / 2 - 4, L + 8, W + 8);
  if (car.wrecked) return true;
  const len = model === 'kleinwagen' ? L - 6 : L;
  // Scheinwerfer, Rück-, Brems-, Rückfahrlicht, Blinker
  ctx.fillStyle = '#fff6c8';
  ctx.fillRect(len / 2 - 2.2, -W / 2 + 2, 2.2, 3.6); ctx.fillRect(len / 2 - 2.2, W / 2 - 5.6, 2.2, 3.6);
  const braking = car.controls?.brake > 0.1;
  const vf = car.vx * Math.cos(car.angle) + car.vy * Math.sin(car.angle);
  ctx.fillStyle = braking ? '#ff3b30' : '#8a1c1c';
  ctx.fillRect(-len / 2, -W / 2 + 2, 1.8, 3.6); ctx.fillRect(-len / 2, W / 2 - 5.6, 1.8, 3.6);
  if (braking) { ctx.fillStyle = 'rgba(255,40,30,0.25)'; ctx.fillRect(-len / 2 - 6, -W / 2, 6, W); }
  if (vf < -5) { ctx.fillStyle = '#f4f7ff'; ctx.fillRect(-len / 2, -1.6, 1.8, 3.2); }
  const blink = car.ai?.blink ?? 0;
  if (blink && Math.floor(t * 3) % 2 === 0) {
    const y = blink > 0 ? W / 2 - 1.8 : -W / 2 + 0.2;
    ctx.fillStyle = '#ffa31a';
    ctx.fillRect(len / 2 - 3, y, 3, 1.6); ctx.fillRect(-len / 2, y, 3, 1.6);
  }
  return true;
}
