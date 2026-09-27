// Austauschbare Grafiken: Jede Figur hat eine prozedurale Platzhalter-Zeichnung.
// Liegt in assets/manifest.json unter "sprites" ein Bild für den Schlüssel, wird stattdessen das Bild gezeichnet.
//   car        – Draufsicht, Front zeigt nach RECHTS, wird auf 42×20 skaliert (Farbe wird dann ignoriert)
//   pedestrian – Draufsicht, Blick nach rechts, 16×16
//   player     – wie pedestrian
//   tree       – Draufsicht Baumkrone, 40×40
export const sprites = {};

export async function loadSprites(manifest, base = 'assets/') {
  const entries = Object.entries(manifest?.sprites ?? {});
  await Promise.all(entries.map(([key, file]) => new Promise((resolve) => {
    const img = new Image();
    img.onload = () => { sprites[key] = img; resolve(); };
    img.onerror = () => { console.warn(`Sprite "${key}" (${file}) nicht ladbar – Platzhalter bleibt`); resolve(); };
    img.src = base + file;
  })));
}

export function shade(hex, f) {
  const n = parseInt(hex.slice(1), 16);
  let r = (n >> 16) & 255, g = (n >> 8) & 255, b = n & 255;
  if (f < 0) { r *= 1 + f; g *= 1 + f; b *= 1 + f; }
  else { r += (255 - r) * f; g += (255 - g) * f; b += (255 - b) * f; }
  return `rgb(${r | 0},${g | 0},${b | 0})`;
}

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y); ctx.arcTo(x + w, y, x + w, y + h, r); ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r); ctx.arcTo(x, y, x + w, y, r); ctx.closePath();
}

// Schattenversatz kleiner Objekte (Autos, Figuren): wandert mit der Sonne, nachts nur ein Kontaktschatten darunter.
export function smallShadow(sun, h) {
  if (!sun) return [3, 4, 0.35];
  const k = Math.min(h * 1.6, h * 0.35 * sun.len) * sun.strength;
  return [sun.dx * k, sun.dy * k, 0.22 + 0.13 * sun.strength];
}

export function drawCar(ctx, car, t, sun) {
  const L = car.hw * 2, W = car.hh * 2;
  // Schatten (in Sonnenrichtung versetzt)
  const [sx, sy, sa] = smallShadow(sun, 16);
  ctx.save();
  ctx.translate(car.x + sx, car.y + sy);
  ctx.rotate(car.angle);
  ctx.fillStyle = `rgba(0,0,0,${sa})`;
  roundRect(ctx, -L / 2 - 1, -W / 2 - 1, L + 2, W + 2, 6); ctx.fill();
  ctx.restore();
  ctx.save();
  ctx.translate(car.x, car.y);
  ctx.rotate(car.angle);
  if (sprites.car) {
    ctx.drawImage(sprites.car, -L / 2, -W / 2, L, W);
    if (car.wrecked) { ctx.fillStyle = 'rgba(20,15,10,0.6)'; ctx.fillRect(-L / 2, -W / 2, L, W); }
    ctx.restore();
    return;
  }
  const body = car.wrecked ? '#3b332d' : car.color;
  ctx.fillStyle = body;
  roundRect(ctx, -L / 2, -W / 2, L, W, 5); ctx.fill();
  ctx.strokeStyle = shade(car.wrecked ? '#3b332d' : car.color, -0.45); ctx.lineWidth = 1.2; ctx.stroke();
  // Motorhaube/Kofferraum-Kanten
  ctx.fillStyle = shade(body, -0.12);
  ctx.fillRect(L / 2 - 11, -W / 2 + 2, 1, W - 4);
  ctx.fillRect(-L / 2 + 8, -W / 2 + 2, 1, W - 4);
  // Kabine
  ctx.fillStyle = car.wrecked ? '#1c1916' : '#1d2733';
  roundRect(ctx, -9, -W / 2 + 2.5, 20, W - 5, 3); ctx.fill();
  ctx.fillStyle = car.wrecked ? '#2a2521' : shade(body, 0.12);
  roundRect(ctx, -5, -W / 2 + 3.5, 11, W - 7, 2); ctx.fill();
  // Scheiben-Glanz
  if (!car.wrecked) { ctx.fillStyle = 'rgba(160,200,255,0.35)'; ctx.fillRect(8, -W / 2 + 4, 2.5, W - 8); }
  // Lichter
  const braking = car.controls.brake > 0.1 && !car.wrecked;
  ctx.fillStyle = car.wrecked ? '#444' : '#fff6c8';
  ctx.fillRect(L / 2 - 2.5, -W / 2 + 2, 2.5, 4); ctx.fillRect(L / 2 - 2.5, W / 2 - 6, 2.5, 4);
  ctx.fillStyle = braking ? '#ff3b30' : '#8a1c1c';
  ctx.fillRect(-L / 2, -W / 2 + 2, 2, 4); ctx.fillRect(-L / 2, W / 2 - 6, 2, 4);
  if (braking) { ctx.fillStyle = 'rgba(255,40,30,0.25)'; ctx.fillRect(-L / 2 - 6, -W / 2, 6, W); }
  // Ladung: Kisten auf dem Rücksitz sichtbar
  if (car.cargo) { ctx.fillStyle = '#b9853f'; ctx.fillRect(-12, -4, 6, 8); ctx.strokeStyle = '#6e4a1c'; ctx.strokeRect(-12, -4, 6, 8); }
  if (car.driver === 'player' && car.horn) { ctx.fillStyle = 'rgba(255,255,255,0.8)'; ctx.fillRect(L / 2 + 2, -1, 3, 2); }
  ctx.restore();
}

export function drawPerson(ctx, p, { shirt, skin = '#f2d0b1', hair = '#2b2118', player = false, down = false, sun = null }) {
  ctx.save();
  ctx.translate(p.x, p.y);
  const [sx, sy, sa] = smallShadow(sun, down ? 4 : 17);
  ctx.fillStyle = `rgba(0,0,0,${sa})`;
  ctx.beginPath(); ctx.ellipse(sx * 0.5, sy * 0.5, down ? 9 : 6 + Math.abs(sx) * 0.35, down ? 4 : 5 + Math.abs(sy) * 0.35, 0, 0, Math.PI * 2); ctx.fill();
  ctx.rotate(p.facing ?? p.angle ?? 0);
  const key = player ? 'player' : 'pedestrian';
  if (sprites[key]) { ctx.drawImage(sprites[key], -8, -8, 16, 16); ctx.restore(); return; }
  if (down) {
    ctx.fillStyle = shirt; ctx.beginPath(); ctx.ellipse(0, 0, 8, 4, 0, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = skin; ctx.beginPath(); ctx.arc(9, 0, 3, 0, Math.PI * 2); ctx.fill();
    ctx.restore(); return;
  }
  const swing = Math.sin((p.step ?? 0) * 0.35) * 3;
  ctx.fillStyle = '#23272e';
  ctx.fillRect(swing - 1.5, -4, 3, 2.6); ctx.fillRect(-swing - 1.5, 1.4, 3, 2.6);
  ctx.fillStyle = shirt;
  ctx.beginPath(); ctx.ellipse(0, 0, 3.6, 6, 0, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = hair; ctx.beginPath(); ctx.arc(0.5, 0, 3.4, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = skin; ctx.beginPath(); ctx.arc(1.6, 0, 2.2, 0, Math.PI * 2); ctx.fill();
  if (player) { ctx.strokeStyle = '#fff'; ctx.lineWidth = 1; ctx.beginPath(); ctx.ellipse(0, 0, 3.6, 6, 0, 0, Math.PI * 2); ctx.stroke(); }
  ctx.restore();
}

// Kronenfarben je Gattung (Berliner Baumbestand): dunkel, mittel, Lichtkante.
export const TREE_STYLE = {
  Tilia: ['#4e8a36', '#62a045', '#7cbb5a'], Acer: ['#3d7a31', '#4f9140', '#69ab55'], Platanus: ['#5f8f3c', '#72a54b', '#8dbe64'],
  Aesculus: ['#2c6326', '#3a7a31', '#4f9142'], Quercus: ['#33662a', '#447d36', '#5b9648'], Robinia: ['#6b9d45', '#80b357', '#9bcb70'],
  Betula: ['#7bab4e', '#8fc05f', '#a8d67b'], Populus: ['#4d8c3b', '#5fa24b', '#79bb62'], Carpinus: ['#3f7a33', '#4f9142', '#66aa57'],
  Fraxinus: ['#4a8537', '#5b9a46', '#76b35e'], Prunus: ['#5a8f40', '#6fa551', '#e6b7c6'], Salix: ['#6c9b4a', '#7fb05b', '#9cc77a'],
  Sorbus: ['#4a8236', '#5c9845', '#d97a3a'], Crataegus: ['#467e33', '#579442', '#71ad5a'], Ulmus: ['#3b7430', '#4c8b3e', '#65a454'],
  Nadel: ['#1f4a26', '#2a5e30', '#3b7a40'], sonstige: ['#2f6b2a', '#3f8a35', '#58a748'],
};

export function drawTree(ctx, tr, t, sun) {
  const r = tr.size, lift = Math.min(r * 0.5, 30);
  // Bei Sonne wirft die Schattenebene (lighting.js) den Kronenschatten; sonst nur ein weicher Fleck unter dem Baum.
  if (!sun || sun.strength < 0.5) {
    ctx.fillStyle = `rgba(0,0,0,${0.22 * (1 - (sun?.strength ?? 0))})`;
    ctx.beginPath(); ctx.ellipse(tr.x, tr.y, r * 0.8, r * 0.5, 0, 0, Math.PI * 2); ctx.fill();
  }
  ctx.fillStyle = '#5b3d22'; ctx.fillRect(tr.x - Math.max(2, tr.r), tr.y - lift, Math.max(4, tr.r * 2), lift);
  const sway = Math.sin(t * 1.3 + tr.x) * 0.8;
  const cy = tr.y - lift;
  if (sprites.tree) { ctx.drawImage(sprites.tree, tr.x - r + sway, cy - r, r * 2, r * 2); return; }
  const spr = treeSprite(tr.genus, tr.seed % TREE_VARIANTS);
  if (spr) { ctx.drawImage(spr, tr.x - r * 1.15 + sway, cy - r * 1.15, r * 2.3, r * 2.3); return; }
  const [c0, c1, c2] = TREE_STYLE[tr.genus] ?? TREE_STYLE.sonstige;
  if (tr.genus === 'Nadel') { // Nadelbaum: gezackte Krone
    ctx.fillStyle = c0; ctx.beginPath();
    for (let k = 0; k < 16; k++) { const a = k / 16 * Math.PI * 2, rr = k % 2 ? r * 0.72 : r; ctx.lineTo(tr.x + sway + Math.cos(a) * rr, cy + Math.sin(a) * rr); }
    ctx.fill();
    ctx.fillStyle = c1; ctx.beginPath(); ctx.arc(tr.x - r * 0.12 + sway, cy - r * 0.12, r * 0.55, 0, Math.PI * 2); ctx.fill();
    return;
  }
  ctx.fillStyle = c0; ctx.beginPath(); ctx.arc(tr.x + sway, cy, r, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = c1; ctx.beginPath(); ctx.arc(tr.x - r * 0.18 + sway, cy - r * 0.18, r * 0.7, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = c2; ctx.beginPath(); ctx.arc(tr.x - r * 0.3 + sway, cy - r * 0.3, r * 0.35, 0, Math.PI * 2); ctx.fill();
}

// Baumkronen-Sprites: je Gattung drei Varianten, einmal in 128 px gezeichnet und beim Zeichnen skaliert.
// Lappige Krone aus überlappenden Blattballen, Licht von links oben, dunkler Rand – wirkt weicher als drei Kreise.
export const TREE_VARIANTS = 3;
const treeSprites = new Map();
function treeSprite(genus, variant) {
  const key = genus + '|' + variant;
  if (treeSprites.has(key)) return treeSprites.get(key);
  let c = null;
  try {
    if (typeof OffscreenCanvas === 'undefined' && typeof document === 'undefined') throw 0;
    const N = 128, R = N / 2 / 1.15; // Kronenradius im Sprite (Rand für Ausläufer)
    c = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(N, N) : Object.assign(document.createElement('canvas'), { width: N, height: N });
    const g = c.getContext('2d'), m = N / 2;
    const [c0, c1, c2] = TREE_STYLE[genus] ?? TREE_STYLE.sonstige;
    let a = (variant * 2654435761) >>> 0; const rnd = () => ((a = (a * 1664525 + 1013904223) >>> 0) / 4294967296);
    const blob = (x, y, rr, col) => { g.fillStyle = col; g.beginPath(); g.arc(x, y, rr, 0, Math.PI * 2); g.fill(); };
    if (genus === 'Nadel') {
      g.fillStyle = shade(c0, -0.25); g.beginPath();
      for (let k = 0; k < 20; k++) { const an = k / 20 * Math.PI * 2, rr = (k % 2 ? 0.7 : 1.02) * R; g.lineTo(m + Math.cos(an) * rr, m + Math.sin(an) * rr); }
      g.fill();
      g.fillStyle = c0; g.beginPath();
      for (let k = 0; k < 20; k++) { const an = k / 20 * Math.PI * 2 + 0.1, rr = (k % 2 ? 0.6 : 0.92) * R; g.lineTo(m + Math.cos(an) * rr, m + Math.sin(an) * rr); }
      g.fill();
      blob(m - R * 0.15, m - R * 0.15, R * 0.45, c1);
      blob(m - R * 0.25, m - R * 0.25, R * 0.18, c2);
    } else {
      const lobes = 7 + (variant % 2);
      // Rand (etwas dunkler, leicht größer) → Kontur ohne Strich
      for (let k = 0; k < lobes; k++) { const an = k / lobes * Math.PI * 2 + rnd() * 0.5; blob(m + Math.cos(an) * R * 0.5, m + Math.sin(an) * R * 0.5, R * (0.5 + rnd() * 0.08), shade(c0, -0.22)); }
      blob(m, m, R * 0.62, shade(c0, -0.22));
      for (let k = 0; k < lobes; k++) { const an = k / lobes * Math.PI * 2 + rnd() * 0.5; blob(m + Math.cos(an) * R * 0.46, m + Math.sin(an) * R * 0.46, R * (0.45 + rnd() * 0.08), c0); }
      blob(m, m, R * 0.58, c0);
      for (let k = 0; k < 5; k++) { const an = rnd() * Math.PI * 2, d = R * rnd() * 0.35; blob(m - R * 0.12 + Math.cos(an) * d, m - R * 0.12 + Math.sin(an) * d, R * (0.24 + rnd() * 0.1), c1); }
      for (let k = 0; k < 4; k++) blob(m - R * (0.25 + rnd() * 0.2), m - R * (0.25 + rnd() * 0.2), R * (0.09 + rnd() * 0.08), c2);
    }
  } catch { c = null; }
  treeSprites.set(key, c);
  return c;
}
