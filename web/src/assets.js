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

export function drawCar(ctx, car, t) {
  ctx.save();
  ctx.translate(car.x, car.y);
  ctx.rotate(car.angle);
  const L = car.hw * 2, W = car.hh * 2;
  // Schatten
  ctx.fillStyle = 'rgba(0,0,0,0.35)';
  roundRect(ctx, -L / 2 + 3, -W / 2 + 4, L, W, 5); ctx.fill();
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

export function drawPerson(ctx, p, { shirt, skin = '#f2d0b1', hair = '#2b2118', player = false, down = false }) {
  ctx.save();
  ctx.translate(p.x, p.y);
  ctx.fillStyle = 'rgba(0,0,0,0.3)';
  ctx.beginPath(); ctx.ellipse(1.5, 2, down ? 9 : 6, down ? 4 : 5, 0, 0, Math.PI * 2); ctx.fill();
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

export function drawTree(ctx, tr, t) {
  ctx.fillStyle = 'rgba(0,0,0,0.28)';
  ctx.beginPath(); ctx.ellipse(tr.x + 6, tr.y + 4, tr.size, tr.size * 0.6, 0, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = '#5b3d22'; ctx.fillRect(tr.x - 2, tr.y - 12, 4, 12);
  const sway = Math.sin(t * 1.3 + tr.x) * 0.8;
  const cy = tr.y - 16;
  if (sprites.tree) { ctx.drawImage(sprites.tree, tr.x - tr.size + sway, cy - tr.size, tr.size * 2, tr.size * 2); return; }
  ctx.fillStyle = '#2f6b2a'; ctx.beginPath(); ctx.arc(tr.x + sway, cy, tr.size, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = '#3f8a35'; ctx.beginPath(); ctx.arc(tr.x - 3 + sway, cy - 3, tr.size * 0.7, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = '#58a748'; ctx.beginPath(); ctx.arc(tr.x - 5 + sway, cy - 6, tr.size * 0.35, 0, Math.PI * 2); ctx.fill();
}
