// Autos in der Draufsicht: elf Pkw-Modelle (carmodels.js) innerhalb derselben Kollisionsbox (42 × 20 px = 4,2 × 2,0 m).
// Karosserie, Scheiben und Dach werden je Modell × Farbe einmal doppelt aufgelöst gezeichnet (Sprite-Cache),
// was sich bewegt – Räder (Vorderräder lenken), Licht, Blinker – kommt jedes Bild dazu.
// Das Modell (und seine Technik) kommt aus carmodels.js; gefahrene Autos nicken und wanken mit ihrem Schwerpunkt.
import { shade } from './assets.js';
import { CAR_MODELS, SPECIAL_MODELS, carModel, specOf } from './carmodels.js';

export { CAR_MODELS, SPECIAL_MODELS, carModel };
export const TAXI_COLOR = '#f1e9c8'; // Berliner Taxi: hellelfenbein

// Umriss je Pkw-Modell: len = Verkürzung, inset = schmaler, r = Eckenradius, back/front = Scheibenkranz (vom Heck bzw.
// von der Front gemessen), cut = Schräge der Front-/Heckscheibe, roof = Dachfarbe (null = Wagenfarbe heller)
const SHAPES = {
  kleinwagen: { len: 6, inset: 0, r: 5.5, back: 6, front: 9, cut: [3.5, 2.5] },
  kompakt: { len: 3, inset: 0, r: 5.5, back: 6, front: 10, cut: [3.5, 1.2] },
  limousine: { len: 0, inset: 0, r: 5.5, back: 9, front: 12, cut: [3.5, 2.5] },
  taxi: { len: 0, inset: 0, r: 5.5, back: 9, front: 12, cut: [3.5, 2.5] },
  kombi: { len: 0, inset: 0, r: 5.5, back: 5, front: 12, cut: [3.5, 0.8] },
  elektro: { len: 0, inset: 0, r: 6, back: 6, front: 11, cut: [4, 1.5], glassRoof: true },
  gelaende: { len: 0, inset: 0, r: 3, back: 4, front: 11, cut: [2, 0.5], rails: true, spare: true },
  sportwagen: { len: 0, inset: 0, r: 7, back: 17, front: 14, cut: [5, 3], vents: 'mid' },
  heckcoupe: { len: 2, inset: 0.5, r: 8, back: 11, front: 13, cut: [4, 5], vents: 'rear' },
  zweitakter: { len: 9, inset: 1, r: 7, back: 6, front: 9, cut: [3, 2], roof: '#ecebe4' },
  hothatch: { len: 3, inset: 0, r: 5, back: 6, front: 10, cut: [3.5, 1.2], stripes: true, vents: 'hood' },
  roadster: { len: 5, inset: 0.5, r: 7, back: 12, front: 15, cut: [4, 0], open: true },
  musclecar: { len: 0, inset: 0, r: 4, back: 11, front: 17, cut: [3, 3], stripes: true, vents: 'hood' },
  oldtimer: { len: 0, inset: 0.5, r: 8, back: 11, front: 13, cut: [2.5, 2.5], chrome: true },
  pickup: { len: 0, inset: 0, r: 3.5, back: 18, front: 11, cut: [2, 0.5], bed: true },
  kleinbus: { len: 3, inset: 0, r: 6, back: 3, front: 5, cut: [1.2, 0.5], twoTone: '#efeee8' },
  rallye: { len: 2, inset: 0, r: 5, back: 6, front: 10, cut: [3.5, 1.2], wing: true, vents: 'hood' },
};
export const bodyLength = (model, L) => L - (SHAPES[model]?.len ?? 0);
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
  const sh = SHAPES[model] ?? SHAPES.limousine;
  const len = L - sh.len, x0 = -len / 2;
  W -= 2 * sh.inset;
  const y0 = -W / 2;
  // Querverlauf: Kanten dunkler, Mitte heller → Wölbung
  const gr = g.createLinearGradient(0, y0, 0, y0 + W);
  gr.addColorStop(0, shade(body, -0.28)); gr.addColorStop(0.5, shade(body, 0.1)); gr.addColorStop(1, shade(body, -0.34));
  g.fillStyle = gr; rr(g, x0, y0, len, W, model === 'transporter' ? 3 : sh.r); g.fill();
  g.strokeStyle = shade(body, -0.5); g.lineWidth = 0.9; g.stroke();
  const glass = wrecked ? '#1c1916' : '#1f2a36';
  const glassG = g.createLinearGradient(0, y0, 0, y0 + W);
  glassG.addColorStop(0, glass); glassG.addColorStop(0.45, wrecked ? '#2a2521' : '#4b6379'); glassG.addColorStop(1, glass);
  const roof = sh.roof ?? shade(body, 0.14);
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
    const back = x0 + sh.back, front = len / 2 - sh.front;
    if (sh.vents === 'mid') { // Mittelmotor: Motorabdeckung mit Lüftungsschlitzen hinter der Kabine, Lufteinlässe an den Flanken
      g.fillStyle = shade(body, -0.18); rr(g, x0 + 4, y0 + 4, back - x0 - 5, W - 8, 2); g.fill();
      g.strokeStyle = 'rgba(0,0,0,0.45)'; g.lineWidth = 0.6;
      for (let x = x0 + 6; x < back - 2; x += 1.8) { g.beginPath(); g.moveTo(x, y0 + 5.5); g.lineTo(x, y0 + W - 5.5); g.stroke(); }
      g.fillStyle = '#15181c'; g.fillRect(back - 2, y0 + 0.4, 4, 1.4); g.fillRect(back - 2, y0 + W - 1.8, 4, 1.4);
      g.fillStyle = shade(body, -0.35); g.fillRect(x0 + 0.6, y0 + 2, 1.6, W - 4);                         // Heckflügel
    }
    if (sh.vents === 'rear') { // Heckmotor: Lüftungsgitter im Motordeckel ganz hinten
      g.strokeStyle = 'rgba(0,0,0,0.4)'; g.lineWidth = 0.6;
      for (let x = x0 + 2.5; x < x0 + 8; x += 1.4) { g.beginPath(); g.moveTo(x, y0 + 5); g.lineTo(x, y0 + W - 5); g.stroke(); }
    }
    if (sh.bed) { // Pick-up: offene Ladefläche hinter der Kabine
      g.fillStyle = shade(body, -0.35); rr(g, x0 + 1.5, y0 + 2, back - x0 - 3, W - 4, 1.5); g.fill();
      g.strokeStyle = 'rgba(0,0,0,0.3)'; g.lineWidth = 0.6;
      for (let x = x0 + 4; x < back - 2; x += 3) { g.beginPath(); g.moveTo(x, y0 + 2.5); g.lineTo(x, y0 + W - 2.5); g.stroke(); }
    }
    if (sh.vents === 'hood') { // Lufthutze auf der Haube
      g.fillStyle = shade(body, -0.3); rr(g, front + 3, -2.2, Math.max(3, len / 2 - front - 7), 4.4, 1); g.fill();
    }
    if (sh.open) { // Roadster: offen – Sitze, Überrollbügel und Frontscheibe statt Dach
      g.fillStyle = glassG; g.fillRect(front - 1.5, y0 + 2.4, 2, W - 4.8);
      g.fillStyle = '#2b2522'; rr(g, back + 1, y0 + 3, front - back - 3, W - 6, 2); g.fill();
      g.fillStyle = '#5a4034'; for (const s of [-1, 1]) { rr(g, back + 3, s * W / 4 - 2.4, 6, 4.8, 1.5); g.fill(); }
      g.fillStyle = shade(body, -0.4); g.fillRect(back + 0.5, y0 + 3, 1.2, W - 6);
    } else {
    cabin(back, front, sh.cut[0], sh.cut[1]);
    const rx = back + Math.max(1.5, sh.cut[1] + 1), rw = front - rx - sh.cut[0] - 1;
    if (sh.glassRoof) { g.fillStyle = '#1a2330'; rr(g, rx, y0 + 3.4, rw, W - 6.8, 2.2); g.fill(); g.fillStyle = 'rgba(160,190,220,0.25)'; g.fillRect(rx + 1, y0 + 4.2, rw - 2, 1.6); }
    else { g.fillStyle = roof; rr(g, rx, y0 + 3.4, rw, W - 6.8, 2.2); g.fill(); }
    if (sh.rails) { g.fillStyle = '#2b2e33'; g.fillRect(rx, y0 + 3.6, rw, 0.9); g.fillRect(rx, y0 + W - 4.5, rw, 0.9); }
    if (sh.twoTone) { g.fillStyle = sh.twoTone; rr(g, rx + 1, y0 + 4, rw - 2, W - 8, 2); g.fill(); }
    }
    if (sh.stripes) { g.fillStyle = 'rgba(255,255,255,0.75)'; g.fillRect(x0 + 1, -2.4, len - 2, 1.4); g.fillRect(x0 + 1, 1, len - 2, 1.4); } // Rennstreifen
    if (sh.wing) { g.fillStyle = shade(body, -0.35); g.fillRect(x0 + 0.5, y0 + 1.5, 2, W - 3); }                                          // Heckflügel
    if (sh.chrome) { g.fillStyle = '#d9dde2'; g.fillRect(len / 2 - 1.2, y0 + 1.5, 1.2, W - 3); g.fillRect(x0, y0 + 1.5, 1.2, W - 3); }     // Chromstoßstangen
    if (sh.spare) { g.fillStyle = '#1c1d20'; g.beginPath(); g.arc(x0 - 0.6, 0, 3.4, 0, Math.PI * 2); g.fill(); g.fillStyle = '#55585e'; g.beginPath(); g.arc(x0 - 0.6, 0, 1.4, 0, Math.PI * 2); g.fill(); }
    g.fillStyle = shade(body, -0.1); g.fillRect(front + 1.5, y0 + 2.5, 0.8, W - 5);            // Haubenkante
    if (model === 'taxi' && !wrecked) { // Dachschild
      g.fillStyle = '#ffd400'; rr(g, -3.5, -2.2, 7, 4.4, 1); g.fill();
      g.fillStyle = '#222'; g.fillRect(-2, -0.5, 4, 1);
    }
  }
  // Außenspiegel
  g.fillStyle = shade(body, -0.2);
  const mx = model === 'transporter' ? len / 2 - 11 : len / 2 - (SHAPES[model]?.front ?? 12) - 1;
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
  else if (model === 'bus') { // Linienbus: gelb, Fensterband an den Seiten, Klimageräte und Lüfter auf dem Dach
    box(x0, L, body, 4);
    g.fillStyle = glass; g.fillRect(L / 2 - 4, y0 + 2, 3, W - 4);                            // Frontscheibe
    g.fillStyle = '#2b2f36'; g.fillRect(x0 + 4, y0 + 0.8, L - 12, 2); g.fillRect(x0 + 4, y0 + W - 2.8, L - 12, 2); // Fensterband
    g.fillStyle = shade(body, 0.12); rr(g, x0 + 3, y0 + 3.5, L - 10, W - 7, 2); g.fill();
    g.fillStyle = '#d8d8d2'; rr(g, -18, -6, 26, 12, 2); g.fill();                             // Klimaanlage
    g.fillStyle = '#b9b9b2'; g.fillRect(x0 + 6, -4, 14, 8);                                   // Motorklappe hinten
    g.fillStyle = shade(body, -0.2); for (const x of [-40, 22, 34]) g.fillRect(x, -3, 5, 6);   // Dachluken
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
  if (car.line) { // Liniennummer auf dem Dach (von oben lesbar)
    ctx.save(); ctx.rotate(-car.angle); ctx.fillStyle = '#1b1b1b'; ctx.font = 'bold 9px system-ui, sans-serif';
    ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText(car.line, 0, 0); ctx.restore();
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
  const steer = car.dyn && car.driver === 'player' ? car.dyn.delta : (car.controls?.steer ?? 0) * 0.45, wx = L / 2 - 8, wy = W / 2 - 1.2;
  ctx.fillStyle = '#16171a';
  for (const [x, y, front] of [[wx, -wy, 1], [wx, wy, 1], [-wx + 1, -wy, 0], [-wx + 1, wy, 0]]) {
    ctx.save(); ctx.translate(x, y); if (front) ctx.rotate(steer); ctx.fillRect(-3.8, -1.7, 7.6, 3.4); ctx.restore();
  }
  // Nicken und Wanken (gefahrenes Auto, dynamics.js): die Karosserie weicht der Beschleunigung aus, je höher der
  // Schwerpunkt, desto weiter – Bremsen taucht vorn ein, Anfahren hinten, Kurven drücken nach außen
  const [bx, by] = bodyShift(car);
  if (bx || by) ctx.translate(bx, by);
  ctx.drawImage(spr, -L / 2 - 4, -W / 2 - 4, L + 8, W + 8);
  if (car.wrecked) { if (bx || by) ctx.translate(-bx, -by); return true; }
  drawDuty(ctx, car, t, L, W);
  const len = bodyLength(model, L);
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
  if (bx || by) ctx.translate(-bx, -by);
  return true;
}

// Versatz der Karosserie (px, Fahrzeugkoordinaten) aus der gefederten Beschleunigung: 0,35 px je m/s² und Meter
// Schwerpunkthöhe, höchstens 6 px (bewusst etwas übertrieben: man soll den Schwerpunkt sehen). Ohne Fahrdynamik (Verkehr, geparkt) keiner.
export function bodyShift(car) {
  const d = car.dyn;
  if (!d || car.wrecked || car.driver !== 'player') return [0, 0];
  const k = specOf(car).h * 0.35, cl = (x) => Math.max(-6, Math.min(6, x));
  return [cl(-d.ax * k), cl(-d.ay * k)];
}

// Motorrad und Motorroller in der Draufsicht (Kontext NICHT gedreht; zeichnet selbst am Ort des Fahrzeugs).
// Reifen, Lenker (lenkt mit), Tank/Sitzbank bzw. Rollerverkleidung mit Trittbrett, Licht, Fahrer mit Helm, der sich in
// die Kurve legt (Schräglage aus der Fahrdynamik) und beim Wheelie das Vorderrad anhebt; umgefallen liegt es ohne Fahrer.
const MOTO_JACKETS = ['#2b2d33', '#5b3a29', '#1f3b5c', '#6b1f1f', '#3c4a2b', '#8a8f96'];
export function drawMoto(ctx, car, t, sun, { player = false } = {}) {
  const scooter = car.kind === 'scooter', L = car.hw * 2, W = car.hh * 2, body = car.wrecked ? '#3b332d' : car.color;
  const d = car.driver === 'player' ? car.dyn : null, lean = d?.lean ?? 0, steer = d ? d.delta : (car.controls?.steer ?? 0) * 0.3;
  const rider = !!car.driver && !car.fallen, lift = d?.wheelie ?? 0;
  const sl = Math.hypot(sun?.dx ?? 1, sun?.dy ?? 1) || 1;
  ctx.save();
  ctx.translate(car.x, car.y);
  // Schatten (umgefallen breit, sonst schmal und in Sonnenrichtung)
  ctx.save(); ctx.rotate(car.angle);
  ctx.fillStyle = 'rgba(0,0,0,0.28)';
  ctx.beginPath(); ctx.ellipse((sun?.dx ?? 1) / sl * 3, (sun?.dy ?? 1) / sl * 3, L / 2 + 1, car.fallen ? W : W / 2 + (rider ? 2 : 0), 0, 0, Math.PI * 2); ctx.fill();
  ctx.restore();
  ctx.rotate(car.angle + (car.fallen ? 0.2 : 0));
  const side = car.fallen ? 1.9 : 1; // liegend: breiter (von der Seite gesehen)
  // Hinterrad, Vorderrad (lenkt; beim Wheelie leicht angehoben = heller und kürzer)
  ctx.fillStyle = '#141518';
  ctx.fillRect(-L / 2, -1.6 * side, scooter ? 5 : 7, 3.2 * side);
  ctx.save(); ctx.translate(L / 2 - (scooter ? 3 : 4), 0); ctx.rotate(car.fallen ? 0 : steer * 0.8);
  ctx.fillStyle = lift > 0.3 ? '#2a2c31' : '#141518'; ctx.fillRect(-3.2 + lift, -1.4 * side, 6.4 - lift, 2.8 * side);
  ctx.restore();
  if (scooter) { // Verkleidung, Trittbrett, Beinschild
    ctx.fillStyle = shade(body, -0.25); ctx.fillRect(-L / 2 + 3, -W / 2 * side * 0.8, L - 7, W * side * 0.8);
    ctx.fillStyle = body; ctx.beginPath(); ctx.ellipse(-L / 2 + 6, 0, 5, W / 2 * side, 0, 0, Math.PI * 2); ctx.fill(); // Heckverkleidung
    ctx.beginPath(); ctx.ellipse(L / 2 - 6, 0, 2.6, W / 2 * side * 0.95, 0, 0, Math.PI * 2); ctx.fill();               // Beinschild
    ctx.fillStyle = '#2a2a2a'; ctx.fillRect(-L / 2 + 3.5, -1.8 * side, 6.5, 3.6 * side);                                   // Sitzbank
  } else { // Motorblock, Tank, Sitzbank, Heck
    ctx.fillStyle = '#4a4d53'; ctx.fillRect(-2.5, -W / 2 * side * 0.9, 6, W * side * 0.9);
    ctx.fillStyle = body; ctx.beginPath(); ctx.ellipse(3.5, 0, 4.4, 2.7 * side, 0, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = shade(body, 0.25); ctx.fillRect(2.5, -0.7 * side, 3, 1.4 * side);                                      // Glanz
    ctx.fillStyle = '#1e1f23'; ctx.fillRect(-7, -1.7 * side, 6, 3.4 * side);
    ctx.fillStyle = body; ctx.fillRect(-L / 2 + 2, -1.3 * side, 5, 2.6 * side);
  }
  // Lenker und Lichter
  ctx.save(); ctx.translate(L / 2 - (scooter ? 6 : 6.5), 0); ctx.rotate(car.fallen ? 0 : steer * 0.8);
  ctx.fillStyle = '#26272b'; ctx.fillRect(-0.6, -W / 2 - 0.8, 1.2, W + 1.6);
  ctx.restore();
  if (!car.wrecked && !car.fallen) {
    ctx.fillStyle = '#fff6c8'; ctx.fillRect(L / 2 - 1.4, -1, 1.4, 2);
    ctx.fillStyle = car.controls?.brake > 0.1 ? '#ff3b30' : '#8a1c1c'; ctx.fillRect(-L / 2 - 0.6, -0.8, 1.2, 1.6);
  }
  if (rider) { // Fahrer: Oberkörper legt sich in die Kurve (nach innen), Helm mit Visier
    const jacket = player ? '#ff7a1a' : MOTO_JACKETS[(car.id * 7) % MOTO_JACKETS.length];
    const lx = 0, ly = Math.max(-3, Math.min(3, Math.sin(lean) * 7)); // rechts positiv
    ctx.fillStyle = shade(jacket, -0.3); ctx.fillRect(scooter ? -3 : -4, -3.2 + ly * 0.3, 3, 6.4);                              // Oberschenkel
    ctx.fillStyle = jacket; ctx.beginPath(); ctx.ellipse(lx - (scooter ? 1.5 : 0.5), ly * 0.6, 3.2, 4.2, 0, 0, Math.PI * 2); ctx.fill();
    ctx.strokeStyle = jacket; ctx.lineWidth = 1.6; ctx.lineCap = 'round';
    for (const s of [-1, 1]) { ctx.beginPath(); ctx.moveTo(lx + 0.5, ly * 0.6 + s * 3); ctx.lineTo(L / 2 - 6.5, s * (W / 2 + 0.2)); ctx.stroke(); } // Arme zum Lenker
    ctx.fillStyle = player ? '#e9e9ea' : '#16171a'; ctx.beginPath(); ctx.arc(lx + 1, ly, 2.6, 0, Math.PI * 2); ctx.fill();   // Helm
    ctx.fillStyle = 'rgba(120,170,220,0.7)'; ctx.fillRect(lx + 2.3, ly - 1.4, 1.2, 2.8);                                        // Visier
  }
  ctx.restore();
}
