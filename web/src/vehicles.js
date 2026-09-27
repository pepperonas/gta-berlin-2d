// Autos in der Draufsicht: fünf Modelle innerhalb derselben Kollisionsbox (42 × 20 px = 4,2 × 2,0 m).
// Karosserie, Scheiben und Dach werden je Modell × Farbe einmal doppelt aufgelöst gezeichnet (Sprite-Cache),
// was sich bewegt – Räder (Vorderräder lenken), Licht, Blinker – kommt jedes Bild dazu.
// Das Modell ist reine Darstellung: aus der Autonummer abgeleitet, die Simulation merkt davon nichts.
import { shade } from './assets.js';

export const CAR_MODELS = ['kleinwagen', 'limousine', 'kombi', 'transporter', 'taxi'];
export const TAXI_COLOR = '#f1e9c8'; // Berliner Taxi: hellelfenbein

const h01 = (n) => { const x = Math.sin(n * 91.345 + 12.9898) * 43758.5453; return x - Math.floor(x); };

export const SPECIAL_MODELS = ['truck', 'delivery', 'garbage', 'police', 'ambulance'];
export function carModel(car) {
  if (car.model) return car.model;
  if (car.kind && car.kind !== 'car') return car.kind; // LKW, Paketwagen, Müllauto, Einsatzfahrzeuge (fleet.js)
  if (car.role === 'player') return 'limousine';
  const r = h01(car.id);
  return r < 0.08 ? 'taxi' : r < 0.2 ? 'transporter' : r < 0.45 ? 'kleinwagen' : r < 0.7 ? 'kombi' : 'limousine';
}
export const carColor = (car) => (car.wrecked ? '#3b332d' : carModel(car) === 'taxi' ? TAXI_COLOR : car.color);
export const hasLightBar = (car) => car.kind === 'police' || car.kind === 'ambulance';

function rr(g, x, y, w, h, r) {
  g.beginPath();
  g.moveTo(x + r, y); g.arcTo(x + w, y, x + w, y + h, r); g.arcTo(x + w, y + h, x, y + h, r);
  g.arcTo(x, y + h, x, y, r); g.arcTo(x, y, x + w, y, r); g.closePath();
}

// Karosserie in lokalen Koordinaten: +x = vorn, Mitte (0,0)
function paintBody(g, model, body, L, W, wrecked) {
  if (SPECIAL_MODELS.includes(model)) return paintSpecial(g, model, body, L, W, wrecked);
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

// Nutzfahrzeuge und Einsatzwagen: Fahrerhaus vorn, Aufbau dahinter (Draufsicht)
function paintSpecial(g, model, body, L, W, wrecked) {
  const x0 = -L / 2, y0 = -W / 2, glass = wrecked ? '#1c1916' : '#233140';
  const box = (x, w, fill, r = 2) => { g.fillStyle = fill; rr(g, x, y0 + 0.6, w, W - 1.2, r); g.fill(); g.strokeStyle = shade(fill, -0.45); g.lineWidth = 0.9; g.stroke(); };
  const ribs = (xa, xb, step, col) => { g.strokeStyle = col; g.lineWidth = 0.7; for (let x = xa; x < xb; x += step) { g.beginPath(); g.moveTo(x, y0 + 2); g.lineTo(x, y0 + W - 2); g.stroke(); } };
  const cabin = (xs, len, col) => { // Fahrerhaus mit Frontscheibe
    box(xs, len, col, 3.5);
    g.fillStyle = glass; g.fillRect(xs + len - 5, y0 + 2.4, 3.2, W - 4.8);
    g.fillStyle = shade(col, 0.12); g.fillRect(xs + 2, y0 + 3, len - 8, W - 6);
  };
  if (model === 'truck') {
    const cab = 15; cabin(L / 2 - cab, cab, body);
    box(x0, L - cab - 2, wrecked ? '#4a4540' : shade(body === '#e9e6df' ? '#dcdad4' : body, 0.25), 1.5);
    ribs(x0 + 6, L / 2 - cab - 4, 7, 'rgba(0,0,0,0.12)');
  } else if (model === 'delivery') {
    const cab = 12; box(x0, L, body, 3);
    g.fillStyle = glass; g.fillRect(L / 2 - 5, y0 + 2.4, 3, W - 4.8);
    g.fillStyle = shade(body, 0.1); rr(g, x0 + 1.5, y0 + 1.8, L - cab - 3, W - 3.6, 1.5); g.fill();
    g.fillStyle = body === '#ffcc00' ? '#d40511' : shade(body, -0.25); g.fillRect(x0 + 3, y0 + W / 2 - 1, L - cab - 7, 2); // Firmenstreifen (ohne Marke)
  } else if (model === 'garbage') {
    const cab = 16; cabin(L / 2 - cab, cab, body);
    box(x0 + 9, L - cab - 11, body, 2);
    ribs(x0 + 14, L / 2 - cab - 3, 9, shade(body, -0.25));
    box(x0, 10, wrecked ? '#333' : '#4b4f54', 2);                   // Schüttung / Presse hinten
    g.fillStyle = '#f5f5f0'; for (const y of [y0 + 2.5, y0 + W - 4.5]) g.fillRect(x0 + 12, y, L - cab - 16, 2); // Reflexstreifen
  } else if (model === 'police') {
    const len = L, xs = x0; box(xs, len, body, 5.5);
    g.fillStyle = glass; g.fillRect(len / 2 - 12, y0 + 2.2, 3.5, W - 4.4); g.fillRect(xs + 5, y0 + 2.2, 3, W - 4.4);
    g.fillStyle = shade(body, 0.08); rr(g, xs + 9, y0 + 3.2, len - 22, W - 6.4, 2); g.fill();
    g.fillStyle = '#1f4e9c'; g.fillRect(xs + 2, y0 + 0.7, len - 4, 2.2); g.fillRect(xs + 2, y0 + W - 2.9, len - 4, 2.2); // blaue Flanken
    g.fillStyle = '#1f4e9c'; g.fillRect(-4, -3, 6, 6);            // Dachkennung
  } else if (model === 'ambulance') {
    const cab = 13; cabin(L / 2 - cab, cab, body);
    box(x0, L - cab - 1, body, 2);
    g.fillStyle = '#d0102a'; g.fillRect(x0 + 1, y0 + 0.8, L - 2, 2.4); g.fillRect(x0 + 1, y0 + W - 3.2, L - 2, 2.4); // Leuchtstreifen
    g.fillStyle = '#d0102a'; g.fillRect(-10, -1.4, 10, 2.8); g.fillRect(-6.4, -5, 2.8, 10); // Stern des Lebens stilisiert: Kreuz auf dem Dach
    g.fillStyle = '#f07d00'; for (let y = y0 + 3; y < y0 + W - 3; y += 4) g.fillRect(x0, y, 1.6, 2);           // Heckwarnmarkierung
  }
  if (wrecked) { g.fillStyle = 'rgba(0,0,0,0.45)'; for (const [x, y, r] of [[-L / 4, -3, 6], [L / 5, 4, 5]]) { g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.fill(); } }
}

// Blaulicht: Balken auf dem Dach, links/rechts im Wechsel (4 Hz); Warnblinker; Müllwerker am Heck
function drawDuty(ctx, car, t, L, W) {
  if (hasLightBar(car) && (car.siren || car.blue)) {
    const ph = Math.floor(t * 8) % 2, bx = car.kind === 'ambulance' ? L / 2 - 16 : -2;
    for (const s of [-1, 1]) {
      const on = (s < 0) === (ph === 0);
      ctx.fillStyle = on ? '#4aa3ff' : '#123a7a';
      ctx.fillRect(bx - 2, s * 1 + (s < 0 ? -4.5 : 0.5), 5, 4);
      if (on) { ctx.fillStyle = 'rgba(80,160,255,0.35)'; ctx.beginPath(); ctx.arc(bx, s * 3, 9, 0, Math.PI * 2); ctx.fill(); }
    }
    if (car.kind === 'ambulance') { ctx.fillStyle = ph ? '#4aa3ff' : '#123a7a'; ctx.fillRect(-L / 2 + 1, -2, 2.5, 4); }
  }
  if (car.kind === 'garbage' && car.work) {
    const ph = t * 4 % (Math.PI * 2);
    ctx.fillStyle = `rgba(255,150,20,${0.55 + 0.4 * Math.sin(ph)})`; ctx.beginPath(); ctx.arc(L / 2 - 8, 0, 3, 0, Math.PI * 2); ctx.fill(); // Rundumleuchte
    for (const [dx, dy] of [[-L / 2 - 6, -W / 2 + 2], [-L / 2 - 4, W / 2 - 1]]) { // zwei Müllwerker mit Tonne
      const bob = Math.sin(t * 6 + dy) * 1.2;
      ctx.fillStyle = '#1b5e20'; ctx.fillRect(dx - 9 + bob, dy - 3, 5, 6);               // Tonne (grün)
      ctx.fillStyle = '#f07d00'; ctx.beginPath(); ctx.arc(dx + bob * 0.5, dy, 3.4, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = '#d7ccc8'; ctx.beginPath(); ctx.arc(dx + bob * 0.5, dy, 1.8, 0, Math.PI * 2); ctx.fill();
    }
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
  drawDuty(ctx, car, t, L, W);
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
  if ((blink || car.hazard) && Math.floor(t * 3) % 2 === 0) { // Blinker bzw. Warnblinker (beide Seiten)
    ctx.fillStyle = '#ffa31a';
    for (const y of car.hazard ? [W / 2 - 1.8, -W / 2 + 0.2] : [blink > 0 ? W / 2 - 1.8 : -W / 2 + 0.2]) { ctx.fillRect(len / 2 - 3, y, 3, 1.6); ctx.fillRect(-len / 2, y, 3, 1.6); }
  }
  return true;
}
