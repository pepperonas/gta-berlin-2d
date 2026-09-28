// Austauschbare Grafiken: Jede Figur hat eine prozedurale Platzhalter-Zeichnung.
// Liegt in assets/manifest.json unter "sprites" ein Bild für den Schlüssel, wird stattdessen das Bild gezeichnet.
//   car        – Draufsicht, Front zeigt nach RECHTS, wird auf 42×20 skaliert (Farbe wird dann ignoriert)
//   pedestrian – Draufsicht, Blick nach rechts, 16×16
//   player     – wie pedestrian
//   tree       – Draufsicht Baumkrone, 40×40
import { drawCarBody } from './vehicles.js';

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
  if (drawCarBody(ctx, car, t)) { // Modell-Sprite (vehicles.js); ohne Canvas unten der flache Platzhalter
    if (car.cargo) { ctx.fillStyle = '#b9853f'; ctx.fillRect(-12, -4, 6, 8); ctx.strokeStyle = '#6e4a1c'; ctx.strokeRect(-12, -4, 6, 8); }
    if (car.driver === 'player' && car.horn) { ctx.fillStyle = 'rgba(255,255,255,0.8)'; ctx.fillRect(L / 2 + 2, -1, 3, 2); }
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

export function drawPerson(ctx, p, { shirt, skin = '#f2d0b1', hair = '#2b2118', player = false, down = false, dead = false, sun = null, weapon = null, attack = null, act = null, time = 0 }) {
  ctx.save();
  ctx.translate(p.x, p.y);
  const [sx, sy, sa] = smallShadow(sun, down ? 4 : 17);
  ctx.fillStyle = `rgba(0,0,0,${sa})`;
  ctx.beginPath(); ctx.ellipse(sx * 0.5, sy * 0.5, down ? 9 : 6 + Math.abs(sx) * 0.35, down ? 4 : 5 + Math.abs(sy) * 0.35, 0, 0, Math.PI * 2); ctx.fill();
  ctx.rotate(p.facing ?? p.angle ?? 0);
  const key = player ? 'player' : 'pedestrian';
  if (sprites[key]) { ctx.drawImage(sprites[key], -8, -8, 16, 16); ctx.restore(); return; }
  if (act === 'lie') down = true;
  if (down) {
    if (dead) ctx.rotate((p.fall ?? 0) - (p.facing ?? p.angle ?? 0) + 0.6); // in Schlagrichtung gefallen, verdreht
    const look = personLook(p, hair);
    ctx.fillStyle = look.pants; ctx.fillRect(-12, -3, 6, 2.4); ctx.fillRect(-12, 0.8, 6, 2.4);
    ctx.fillStyle = shirt; ctx.beginPath(); ctx.ellipse(0, 0, 8, 4, 0, 0, Math.PI * 2); ctx.fill();
    if (dead) { ctx.fillStyle = shade(shirt, -0.2); ctx.fillRect(-2, -8, 3, 5); ctx.fillRect(2, 3, 3, 5); } // Arme ausgebreitet
    ctx.fillStyle = look.hair; ctx.beginPath(); ctx.arc(9.5, 0, 3.2, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = skin; ctx.beginPath(); ctx.arc(9, 0, 2.6, 0, Math.PI * 2); ctx.fill();
    ctx.restore(); return;
  }
  // Aussehen je Person fest (aus ihrer Nummer): Hose, Haare, manchmal Tasche oder Rucksack
  const look = personLook(p, hair);
  if (act === 'sit') { drawSitting(ctx, p, shirt, skin, look); ctx.restore(); return; }
  const ph = (p.step ?? 0) * 0.35, swing = Math.sin(ph) * 3, arm = Math.sin(ph) * 2.4;
  // Beine (Schrittlänge folgt der zurückgelegten Strecke)
  ctx.fillStyle = look.pants;
  ctx.fillRect(swing - 1.6, -3.4, 3.4, 2.4); ctx.fillRect(-swing - 1.6, 1.0, 3.4, 2.4);
  ctx.fillStyle = '#1e2126';
  ctx.fillRect(swing + 1.4, -3.4, 1.2, 2.4); ctx.fillRect(-swing + 1.4, 1.0, 1.2, 2.4); // Schuhe
  if (act && ACT_ARMS[act]) { // Tätigkeit: eigene Armhaltung statt Gehbewegung
    ACT_ARMS[act](ctx, shirt, skin, time + (p.id ?? 0));
    drawTorsoHead(ctx, shirt, skin, look, player);
    ctx.restore();
    return;
  }
  // Tritt: ein Bein nach vorn
  if (attack?.kind === 'kick') { ctx.fillStyle = look.pants; ctx.fillRect(3, 0.8, 9, 3); ctx.fillStyle = '#1e2126'; ctx.fillRect(11, 0.8, 2.4, 3); }
  const gun = weapon === 'pistol' || weapon === 'smg' || weapon === 'shotgun';
  if (gun || (attack && attack.kind !== 'kick') || weapon === 'bat' || weapon === 'knife') {
    drawArmsWithWeapon(ctx, weapon, attack, shirt, skin, arm);
  } else {
  // Arme gegengleich zu den Beinen
  ctx.fillStyle = shade(shirt, -0.18);
  ctx.beginPath(); ctx.ellipse(-arm, -5.2, 2.2, 1.3, 0, 0, Math.PI * 2); ctx.fill();
  ctx.beginPath(); ctx.ellipse(arm, 5.2, 2.2, 1.3, 0, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = skin;
  ctx.beginPath(); ctx.arc(-arm + 1.8, -5.3, 1, 0, Math.PI * 2); ctx.fill();
  ctx.beginPath(); ctx.arc(arm + 1.8, 5.3, 1, 0, Math.PI * 2); ctx.fill();
  }
  // Rumpf, Rucksack/Tasche, Kopf
  ctx.fillStyle = shirt;
  ctx.beginPath(); ctx.ellipse(0, 0, 3.6, 5.6, 0, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = shade(shirt, 0.15); ctx.beginPath(); ctx.ellipse(0.8, -0.6, 1.8, 3.6, 0, 0, Math.PI * 2); ctx.fill();
  if (look.bag === 'backpack') { ctx.fillStyle = look.bagColor; ctx.fillRect(-4.8, -2.8, 2.6, 5.6); }
  else if (look.bag === 'tote') { ctx.fillStyle = look.bagColor; ctx.fillRect(arm - 1.5, 6.2, 3, 2.4); }
  ctx.fillStyle = look.hair; ctx.beginPath(); ctx.arc(0.3, 0, 3.3, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = skin; ctx.beginPath(); ctx.arc(1.5, 0, 2.1, 0, Math.PI * 2); ctx.fill();
  if (player) { ctx.strokeStyle = '#fff'; ctx.lineWidth = 1; ctx.beginPath(); ctx.ellipse(0, 0, 3.6, 5.6, 0, 0, Math.PI * 2); ctx.stroke(); }
  ctx.restore();
}

// Arme mit Waffe (lokal: +x = Blickrichtung, rechte Hand bei +y). attack.t läuft von der Dauer auf 0.
function drawArmsWithWeapon(ctx, weapon, attack, shirt, skin, arm) {
  const sl = shade(shirt, -0.18);
  const hand = (x, y) => { ctx.fillStyle = skin; ctx.beginPath(); ctx.arc(x, y, 1.1, 0, Math.PI * 2); ctx.fill(); };
  const sleeve = (x, y) => { ctx.fillStyle = sl; ctx.beginPath(); ctx.ellipse(x, y, 2.4, 1.3, 0, 0, Math.PI * 2); ctx.fill(); };
  if (weapon === 'pistol' || weapon === 'smg' || weapon === 'shotgun') { // beide Hände vorn an der Waffe, Rückstoß beim Schuss
    const kick = attack?.kind === 'shot' ? -1.5 : 0;
    const len = weapon === 'pistol' ? 6 : weapon === 'smg' ? 9 : 15;
    ctx.fillStyle = weapon === 'shotgun' ? '#5a3b22' : '#1b1d21'; ctx.fillRect(4 + kick, -0.3, len, 2.4);
    if (weapon === 'smg') ctx.fillRect(7 + kick, 1.8, 2, 3);
    if (weapon === 'shotgun') { ctx.fillStyle = '#1b1d21'; ctx.fillRect(9 + kick, -0.1, 10, 1.6); }
    sleeve(3 + kick, -2.6); sleeve(3 + kick, 3.2); hand(5.5 + kick, 0); hand(6.5 + kick, 2);
    return;
  }
  const u = attack ? 1 - Math.max(0, attack.t) / 0.22 : 0; // 0 … 1 im Schlag
  if (weapon === 'bat') { // Schläger schwingt von hinten links nach vorn rechts
    const a = attack ? -1.6 + u * 2.6 : 2.3;
    sleeve(1.5, 4.2); hand(3, 5);
    ctx.save(); ctx.translate(3, 5); ctx.rotate(a);
    ctx.fillStyle = '#9b6a3a'; ctx.fillRect(0, -1, 16, 2.2); ctx.fillStyle = '#7a4f28'; ctx.fillRect(10, -1.4, 6, 2.8);
    ctx.restore();
    sleeve(-arm * 0.3, -5.2);
    return;
  }
  const reach = attack ? Math.sin(u * Math.PI) * 7 : 0; // Faust/Messer stößt vor und zurück
  sleeve(-arm * 0.3, -5.2); hand(-arm * 0.3 + 1.8, -5.3);
  sleeve(2 + reach, 4.2); hand(4 + reach, 4.4);
  if (weapon === 'knife') { ctx.fillStyle = '#c9ced4'; ctx.fillRect(4.5 + reach, 3.8, 5, 1.2); ctx.fillStyle = '#222'; ctx.fillRect(3.5 + reach, 3.7, 1.4, 1.4); }
}

// Rumpf und Kopf (wie beim Gehen), für Tätigkeiten mit eigener Armhaltung
export function drawTorsoHead(ctx, shirt, skin, look, player) {
  ctx.fillStyle = shirt;
  ctx.beginPath(); ctx.ellipse(0, 0, 3.6, 5.6, 0, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = shade(shirt, 0.15); ctx.beginPath(); ctx.ellipse(0.8, -0.6, 1.8, 3.6, 0, 0, Math.PI * 2); ctx.fill();
  if (look.bag === 'backpack') { ctx.fillStyle = look.bagColor; ctx.fillRect(-4.8, -2.8, 2.6, 5.6); }
  ctx.fillStyle = look.hair; ctx.beginPath(); ctx.arc(0.3, 0, 3.3, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = skin; ctx.beginPath(); ctx.arc(1.5, 0, 2.1, 0, Math.PI * 2); ctx.fill();
  if (player) { ctx.strokeStyle = '#fff'; ctx.lineWidth = 1; ctx.beginPath(); ctx.ellipse(0, 0, 3.6, 5.6, 0, 0, Math.PI * 2); ctx.stroke(); }
}

// Sitzend (Bank, Café): Beine nach vorn, Hände im Schoß
function drawSitting(ctx, p, shirt, skin, look) {
  ctx.fillStyle = look.pants; ctx.fillRect(1, -3.2, 7, 2.6); ctx.fillRect(1, 0.6, 7, 2.6);
  ctx.fillStyle = '#1e2126'; ctx.fillRect(7.5, -3.2, 1.6, 2.6); ctx.fillRect(7.5, 0.6, 1.6, 2.6);
  ctx.save(); ctx.translate(-1.2, 0);
  ctx.fillStyle = shade(shirt, -0.18);
  ctx.beginPath(); ctx.ellipse(2.4, -3.6, 2.2, 1.3, 0.4, 0, Math.PI * 2); ctx.fill();
  ctx.beginPath(); ctx.ellipse(2.4, 3.6, 2.2, 1.3, -0.4, 0, Math.PI * 2); ctx.fill();
  drawTorsoHead(ctx, shirt, skin, look, false);
  ctx.restore();
}

const sleeveAt = (ctx, shirt, x, y) => { ctx.fillStyle = shade(shirt, -0.18); ctx.beginPath(); ctx.ellipse(x, y, 2.2, 1.3, 0, 0, Math.PI * 2); ctx.fill(); };
const handAt = (ctx, skin, x, y) => { ctx.fillStyle = skin; ctx.beginPath(); ctx.arc(x, y, 1, 0, Math.PI * 2); ctx.fill(); };
// Armhaltungen je Tätigkeit (lokal: +x Blickrichtung, rechte Hand +y); t läuft für Bewegung
export const ACT_ARMS = {
  smoke: (ctx, shirt, skin, t) => { // Zigarette zum Mund und zurück
    const up = Math.max(0, Math.sin(t * 0.8)) > 0.6;
    sleeveAt(ctx, shirt, 0, -5.2); handAt(ctx, skin, 1.5, -5.4);
    sleeveAt(ctx, shirt, up ? 2 : 0.5, up ? 2.5 : 5); handAt(ctx, skin, up ? 3.5 : 2, up ? 1.5 : 5.4);
    ctx.fillStyle = '#f2f2f2'; ctx.fillRect(up ? 3.8 : 2.4, up ? 1 : 4.9, 2.2, 0.7);
    ctx.fillStyle = '#ff7a2a'; ctx.fillRect(up ? 5.8 : 4.4, up ? 1 : 4.9, 0.8, 0.7);
  },
  drink: (ctx, shirt, skin, t) => { // Flasche (Berliner Späti)
    const up = Math.sin(t * 0.6) > 0.7;
    sleeveAt(ctx, shirt, 0, -5.2); handAt(ctx, skin, 1.5, -5.4);
    sleeveAt(ctx, shirt, up ? 2 : 1, up ? 2 : 4.8); handAt(ctx, skin, up ? 3.5 : 3, up ? 1 : 4.8);
    ctx.fillStyle = (t | 0) % 2 ? '#3f6b2a' : '#6b4a1e'; ctx.fillRect(up ? 3.2 : 2.6, up ? -0.2 : 4, 4.2, 1.6);
  },
  chat: (ctx, shirt, skin, t) => { // Gestikulieren
    const a = Math.sin(t * 3.1) * 2, b = Math.sin(t * 2.3 + 1) * 2;
    sleeveAt(ctx, shirt, 1.5 + a * 0.3, -4.6); handAt(ctx, skin, 3.5 + a, -3.8);
    sleeveAt(ctx, shirt, 1.5 + b * 0.3, 4.6); handAt(ctx, skin, 3.5 + b, 3.8);
  },
  wait: (ctx, shirt, skin, t) => { // aufs Handy schauen
    sleeveAt(ctx, shirt, 1.6, -3); sleeveAt(ctx, shirt, 1.6, 3); handAt(ctx, skin, 3.5, -1.2); handAt(ctx, skin, 3.5, 1.2);
    ctx.fillStyle = '#1a1c20'; ctx.fillRect(3.6, -1.4, 1.6, 2.8);
    ctx.fillStyle = Math.sin(t * 0.3) > -0.5 ? '#9fd3ff' : '#35495e'; ctx.fillRect(3.8, -1.1, 1.1, 2.2);
  },
  music: (ctx, shirt, skin, t) => { // Gitarre
    ctx.fillStyle = '#8a5a2b'; ctx.beginPath(); ctx.ellipse(3.2, 2.2, 3.2, 2.4, 0.5, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = '#2b1a0c'; ctx.beginPath(); ctx.arc(3.2, 2.2, 0.9, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = '#5a3a1b'; ctx.fillRect(3.5, -6, 1.1, 6.5);
    sleeveAt(ctx, shirt, 1.5, -4.5); handAt(ctx, skin, 4, -4 + Math.sin(t * 4) * 0.6);
    sleeveAt(ctx, shirt, 1.5, 4); handAt(ctx, skin, 3.8, 2 + Math.sin(t * 9) * 0.8);
  },
  queue: (ctx, shirt, skin) => { sleeveAt(ctx, shirt, 0, -5.2); sleeveAt(ctx, shirt, 0, 5.2); handAt(ctx, skin, 1.4, -5.3); handAt(ctx, skin, 1.4, 5.3); },
  browse: (ctx, shirt, skin) => { sleeveAt(ctx, shirt, -0.8, -4.8); sleeveAt(ctx, shirt, -0.8, 4.8); handAt(ctx, skin, -2, -3); handAt(ctx, skin, -2, 3); }, // Hände hinter dem Rücken
};

// Hund an der Leine, läuft hinter/neben dem Halter (nur Darstellung; Lage wird am Passanten nachgeführt)
export function drawDog(ctx, p, t) {
  const f = p.facing ?? 0, tx = p.x - Math.cos(f) * 15 + Math.sin(f) * 6, ty = p.y - Math.sin(f) * 15 - Math.cos(f) * 6;
  const d = (p._dog ??= { x: tx, y: ty, a: f });
  d.x += (tx - d.x) * 0.15; d.y += (ty - d.y) * 0.15;
  const dx = p.x - d.x, dy = p.y - d.y;
  if (Math.hypot(dx, dy) > 3) d.a = Math.atan2(dy, dx);
  const col = ['#6b4a2b', '#1d1d1d', '#d9b27a', '#8c8c8c', '#f2ead8'][(p.id ?? 0) % 5];
  ctx.strokeStyle = 'rgba(40,30,20,0.8)'; ctx.lineWidth = 0.7;
  ctx.beginPath(); ctx.moveTo(p.x + Math.sin(f) * 4, p.y - Math.cos(f) * 4); ctx.lineTo(d.x + Math.cos(d.a) * 5, d.y + Math.sin(d.a) * 5); ctx.stroke();
  ctx.save(); ctx.translate(d.x, d.y); ctx.rotate(d.a);
  ctx.fillStyle = 'rgba(0,0,0,0.25)'; ctx.beginPath(); ctx.ellipse(1, 1.5, 6, 3, 0, 0, Math.PI * 2); ctx.fill();
  const leg = Math.sin((p.step ?? 0) * 0.6) * 1.5;
  ctx.fillStyle = shade(col, -0.25); ctx.fillRect(2 + leg, -3, 1.2, 1.4); ctx.fillRect(2 - leg, 1.6, 1.2, 1.4); ctx.fillRect(-3 - leg, -3, 1.2, 1.4); ctx.fillRect(-3 + leg, 1.6, 1.2, 1.4);
  ctx.fillStyle = col; ctx.beginPath(); ctx.ellipse(0, 0, 5, 2.4, 0, 0, Math.PI * 2); ctx.fill();
  ctx.beginPath(); ctx.arc(5, 0, 2, 0, Math.PI * 2); ctx.fill();
  ctx.strokeStyle = col; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(-5, 0); ctx.lineTo(-7.5, Math.sin(t * 12) * 1.8); ctx.stroke();
  ctx.fillStyle = '#1a1a1a'; ctx.fillRect(6.6, -0.5, 1, 1);
  ctx.restore();
}

const PANTS = ['#2c3e50', '#34495e', '#1f2a36', '#5d4e3c', '#3d5a80', '#6b6b6b', '#2b2b2b', '#7a5c3a'];
const HAIR = ['#2b2118', '#4a3524', '#1a1a1a', '#8a6a3d', '#c9a45a', '#a33b20', '#d8d4cf', '#5a4636'];
const BAGS = ['#6d4c2f', '#2f3b52', '#8a2f2f', '#3d6b4f', '#1f1f1f'];
// Aussehen je Person, deterministisch aus der Nummer (reine Darstellung, kein Zufall der Simulation)
export function personLook(p, hair) {
  if (p._look) return p._look;
  const h = (k) => { const x = Math.sin((p.id ?? 1) * 127.1 + k * 311.7) * 43758.5453; return x - Math.floor(x); };
  const bagR = h(4);
  const look = {
    pants: PANTS[Math.floor(h(1) * PANTS.length)],
    hair: p.id === undefined ? hair : HAIR[Math.floor(h(2) * HAIR.length)],
    bag: bagR < 0.18 ? 'backpack' : bagR < 0.3 ? 'tote' : null,
    bagColor: BAGS[Math.floor(h(3) * BAGS.length)],
  };
  if (p.id !== undefined) p._look = look;
  return look;
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

// fx (optional): { snow 0…1 (Schnee auf der Krone), wind {x,y}, storm 0…1 (Böenstärke: Krone neigt sich und schwankt) }
export function drawTree(ctx, tr, t, sun, fx = null) {
  const r = tr.size, lift = Math.min(r * 0.5, 30);
  // Bei Sonne wirft die Schattenebene (lighting.js) den Kronenschatten; sonst nur ein weicher Fleck unter dem Baum.
  if (!sun || sun.strength < 0.5) {
    ctx.fillStyle = `rgba(0,0,0,${0.22 * (1 - (sun?.strength ?? 0))})`;
    ctx.beginPath(); ctx.ellipse(tr.x, tr.y, r * 0.8, r * 0.5, 0, 0, Math.PI * 2); ctx.fill();
  }
  ctx.fillStyle = '#5b3d22'; ctx.fillRect(tr.x - Math.max(2, tr.r), tr.y - lift, Math.max(4, tr.r * 2), lift);
  const storm = fx?.storm ?? 0, wl = fx?.wind ? Math.hypot(fx.wind.x, fx.wind.y) || 1 : 1;
  const bend = storm * r * 0.18, shake = 0.8 + storm * r * 0.08 * (0.6 + 0.4 * Math.sin(t * 5.3 + tr.y));
  const sway = Math.sin(t * (1.3 + storm * 2.4) + tr.x) * shake + (fx?.wind ? fx.wind.x / wl * bend : 0);
  const swayY = fx?.wind ? fx.wind.y / wl * bend * 0.6 + Math.cos(t * 2.1 + tr.y) * storm * 1.5 : 0;
  const cy = tr.y - lift + swayY;
  const snow = fx?.snow ?? 0;
  if (sprites.tree) { ctx.drawImage(sprites.tree, tr.x - r + sway, cy - r, r * 2, r * 2); if (snow > 0.03) drawCrownSnow(ctx, tr, tr.x + sway, cy, r, snow); return; }
  const spr = treeSprite(tr.genus, tr.seed % TREE_VARIANTS);
  if (spr) { ctx.drawImage(spr, tr.x - r * 1.15 + sway, cy - r * 1.15, r * 2.3, r * 2.3); if (snow > 0.03) drawCrownSnow(ctx, tr, tr.x + sway, cy, r, snow); return; }
  const [c0, c1, c2] = TREE_STYLE[tr.genus] ?? TREE_STYLE.sonstige;
  if (tr.genus === 'Nadel') { // Nadelbaum: gezackte Krone
    ctx.fillStyle = c0; ctx.beginPath();
    for (let k = 0; k < 16; k++) { const a = k / 16 * Math.PI * 2, rr = k % 2 ? r * 0.72 : r; ctx.lineTo(tr.x + sway + Math.cos(a) * rr, cy + Math.sin(a) * rr); }
    ctx.fill();
    ctx.fillStyle = c1; ctx.beginPath(); ctx.arc(tr.x - r * 0.12 + sway, cy - r * 0.12, r * 0.55, 0, Math.PI * 2); ctx.fill();
    if (snow > 0.03) drawCrownSnow(ctx, tr, tr.x + sway, cy, r, snow);
    return;
  }
  ctx.fillStyle = c0; ctx.beginPath(); ctx.arc(tr.x + sway, cy, r, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = c1; ctx.beginPath(); ctx.arc(tr.x - r * 0.18 + sway, cy - r * 0.18, r * 0.7, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = c2; ctx.beginPath(); ctx.arc(tr.x - r * 0.3 + sway, cy - r * 0.3, r * 0.35, 0, Math.PI * 2); ctx.fill();
  if (snow > 0.03) drawCrownSnow(ctx, tr, tr.x + sway, cy, r, snow);
}

// Schnee auf der Krone: Polster auf den oberen, lichtzugewandten Blattballen (links oben), je Baum fest verteilt;
// Nadelbäume halten mehr Schnee
function drawCrownSnow(ctx, tr, x, y, r, depth) {
  const k = Math.min(1, depth * (tr.genus === 'Nadel' ? 1.4 : 1.1));
  const n = 7 + (tr.seed % 5);
  let a = (tr.seed * 2654435761) >>> 0; const rnd = () => ((a = (a * 1664525 + 1013904223) >>> 0) / 4294967296);
  ctx.fillStyle = `rgba(243,246,251,${0.9 * k})`;
  ctx.beginPath();
  for (let i = 0; i < n; i++) {
    const ang = Math.PI * (1.05 + rnd() * 0.75), d = r * (0.2 + rnd() * 0.62), rr = r * (0.12 + rnd() * 0.16) * (0.5 + 0.5 * k);
    const px = x + Math.cos(ang) * d, py = y + Math.sin(ang) * d;
    ctx.moveTo(px + rr, py); ctx.ellipse(px, py, rr, rr * 0.75, 0, 0, Math.PI * 2);
  }
  ctx.fill();
  ctx.fillStyle = `rgba(200,212,232,${0.35 * k})`; // Schattensaum unten rechts an den Polstern
  ctx.beginPath(); ctx.arc(x + r * 0.15, y + r * 0.1, r * 0.72, 0.1 * Math.PI, 0.6 * Math.PI); ctx.lineTo(x + r * 0.15, y + r * 0.1); ctx.fill();
}

// Baumkronen-Sprites: je Gattung drei Varianten, einmal in 128 px gezeichnet und beim Zeichnen skaliert.
// Lappige Krone aus überlappenden Blattballen, Licht von links oben, dunkler Rand – wirkt weicher als drei Kreise.
export const TREE_VARIANTS = 3;
const treeSprites = new Map();
export function treeSprite(genus, variant) {
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
