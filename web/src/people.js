// Menschen zeichnen (nur Darstellung): Spieler und Passanten in Draufsicht, lokal +x = Blickrichtung, +y = rechts.
// Aussehen aus figure.js (Typ, Kleidung, Frisur, Kopfbedeckung, Zubehör), Bewegung aus gait.js (Schritt, Arme,
// Schulterdrehung, Wippen, Laufpose, Atmen im Stand). Nichts hier verändert die Simulation.
import { sprites, shade, smallShadow } from './assets.js';
import { figureLook, PLAYER_LOOK } from './figure.js';
import { createAnim, stepAnim, gaitPose, legFrame } from './gait.js';

const TAU = Math.PI * 2;
const OUTLINE = 'rgba(12,14,20,0.42)';

// Licht in lokalen Koordinaten (Einheitsvektor zur Sonne); nachts von oben links
function localLight(sun, face) {
  const dx = sun && sun.strength > 0.05 ? sun.dx : 0.6, dy = sun && sun.strength > 0.05 ? sun.dy : 0.8;
  const c = Math.cos(face), s = Math.sin(face);
  const lx = -(dx * c + dy * s), ly = -(-dx * s + dy * c), n = Math.hypot(lx, ly) || 1;
  return [lx / n, ly / n];
}

// Kapsel (Bein, Ärmel): Linie mit runden Enden
function capsule(ctx, x0, y0, x1, y1, w, col) {
  ctx.strokeStyle = col; ctx.lineWidth = w; ctx.lineCap = 'round';
  ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x1, y1); ctx.stroke();
}
const disc = (ctx, x, y, r, col) => { ctx.fillStyle = col; ctx.beginPath(); ctx.arc(x, y, r, 0, TAU); ctx.fill(); };
const oval = (ctx, x, y, rx, ry, col, rot = 0) => { ctx.fillStyle = col; ctx.beginPath(); ctx.ellipse(x, y, rx, ry, rot, 0, TAU); ctx.fill(); };

// Beine mit Schuhen; der Fuß in der Luft ist etwas größer und heller (näher an der Kamera)
function drawLegs(ctx, look, pose) {
  const legCol = look.top === 'coat' ? shade(look.pants, -0.05) : look.pants;
  for (const [f, y] of [[pose.footL, -1.9], [pose.footR, 1.9]]) {
    const fx = f.x, lift = f.lift;
    if (look.shorts) { capsule(ctx, 0, y * 0.8, fx * 0.5, y, 2.7, legCol); capsule(ctx, fx * 0.5, y, fx, y, 2.1, look.skin); }
    else capsule(ctx, 0, y * 0.8, fx, y, 2.7, legCol);
    const k = 1 + lift * 0.28;
    // Schuh: Ferse am Fußpunkt, Spitze nach vorn; der hintere Fuß schaut hinten unter dem Rumpf hervor
    const tip = fx >= 0 ? 1.1 : -0.2;
    oval(ctx, fx + tip * k, y, 1.8 * k, 1.15 * k, lift > 0.3 ? shade(look.shoes, 0.12) : look.shoes);
  }
}

// Arme (ohne Waffe): Ärmel von der Schulter zur Hand, Hand in Hautfarbe; Zubehör in der Hand
function drawArms(ctx, look, pose, time, moving) {
  const sl = shade(look.topColor, -0.16);
  for (const [h, y] of [[pose.handL, -4.3], [pose.handR, 4.3]]) {
    capsule(ctx, 0, y, h.x * 0.8, h.y, 2.4, sl);
    disc(ctx, h.x, h.y, 1.1, look.skin);
  }
  const R = pose.handR, Lh = pose.handL;
  switch (look.acc) {
    case 'briefcase': ctx.fillStyle = '#3a2618'; ctx.fillRect(R.x - 2.2, R.y + 0.6, 4.4, 2.2); ctx.fillStyle = '#b08a3a'; ctx.fillRect(R.x - 0.4, R.y + 0.6, 0.8, 0.6); break;
    case 'shopping': ctx.fillStyle = '#f2f2f2'; ctx.fillRect(Lh.x - 1.8, Lh.y - 2.8, 3.6, 2.4); ctx.fillStyle = '#3aa85a'; ctx.fillRect(Lh.x - 1.8, Lh.y - 1.2, 3.6, 0.6); break;
    case 'bottle': ctx.fillStyle = '#3f6b2a'; ctx.fillRect(R.x, R.y - 0.6, 3.2, 1.3); break;
    case 'phone': if (!moving || Math.sin(time * 0.4) > 0) { ctx.fillStyle = '#15171b'; ctx.fillRect(Lh.x + 0.4, Lh.y - 0.7, 1.4, 2.2); ctx.fillStyle = '#9fd3ff'; ctx.fillRect(Lh.x + 0.6, Lh.y - 0.45, 1, 1.7); } break;
  }
  if (look.bag === 'tote') { ctx.fillStyle = look.bagColor; ctx.fillRect(R.x - 1.4, R.y + 0.8, 2.8, 3); }
}

// Stock: von der Hand schräg nach vorn auf den Boden
function drawCane(ctx, pose) { const h = pose.handR; capsule(ctx, h.x, h.y, h.x + 2.4, h.y + 1.2, 0.9, '#5a3a1e'); }

// Kinderwagen vor der Person (Griff an den Händen)
function drawStroller(ctx, look, pose) {
  const c = look.strollerColor;
  capsule(ctx, 6.2, -2.8, 6.2, 2.8, 1, '#2b2b2b');                    // Griff
  capsule(ctx, 6.4, -2.6, 8.5, -3, 0.7, '#555'); capsule(ctx, 6.4, 2.6, 8.5, 3, 0.7, '#555');
  ctx.fillStyle = '#1d1d1d'; for (const [x, y] of [[9, -3.6], [9, 3.6], [15.4, -3.6], [15.4, 3.6]]) ctx.fillRect(x - 1, y - 0.6, 2, 1.2);
  oval(ctx, 12.2, 0, 4.4, 3.4, c);
  oval(ctx, 13.4, 0, 2.6, 3, shade(c, -0.25));                        // Verdeck
  oval(ctx, 10.6, 0, 1.4, 1.3, '#f5d6c6');                            // Baby
}

// Oberkörper und Kopf. pose kann null sein (Tätigkeit, Sitzen): dann ruhige Haltung.
function drawUpper(ctx, look, pose, light, player) {
  const twist = pose?.twist ?? 0, bob = pose?.bob ?? 1, lean = pose?.lean ?? 0, sway = pose?.sway ?? 0, breath = pose?.breath ?? 0;
  const top = look.topColor, [lx, ly] = light;
  ctx.save();
  ctx.translate(0, sway * 0.3);
  // Rucksack hinten
  if (look.bag === 'backpack') { ctx.fillStyle = look.bagColor; ctx.beginPath(); ctx.roundRect?.(-6.2, -3.2, 3.4, 6.4, 1.2) ?? ctx.rect(-6.2, -3.2, 3.4, 6.4); ctx.fill(); capsule(ctx, -3.5, -2.6, -1.2, -3.4, 0.7, shade(look.bagColor, -0.3)); capsule(ctx, -3.5, 2.6, -1.2, 3.4, 0.7, shade(look.bagColor, -0.3)); }
  ctx.save();
  ctx.rotate(twist); ctx.scale(bob + breath, bob + breath);
  // Rumpf (Mantel länger nach hinten)
  const coat = look.top === 'coat', rx = coat ? 4.3 : 3.5, cx = coat ? -0.5 : 0;
  const base = look.top === 'vest' ? look.under : top;
  oval(ctx, cx, 0, rx, 5.7, base);
  ctx.strokeStyle = OUTLINE; ctx.lineWidth = 0.55; ctx.stroke();
  switch (look.top) {
    case 'jacket': oval(ctx, 2.2, 0, 1.2, 1.6, look.under ?? shade(top, -0.35)); capsule(ctx, 0.2, -3.8, 0.2, 3.8, 0.6, shade(top, -0.3)); break; // offen vorn, Kragen
    case 'suit': oval(ctx, 2.3, 0, 1.3, 1.9, '#f2f2f2'); capsule(ctx, 2.2, 0, 3.4, 0, 0.9, look.tie ?? '#8a2f2f'); break;
    case 'coat': capsule(ctx, 0.6, -2.4, 0.6, 2.4, 1.4, shade(top, -0.2)); break;                                   // Kragen
    case 'hoodie': oval(ctx, -2.4, 0, 1.8, 2.8, shade(top, -0.22)); capsule(ctx, 2.2, -0.8, 3.4, -0.8, 0.4, '#eee'); capsule(ctx, 2.2, 0.8, 3.4, 0.8, 0.4, '#eee'); break;
    case 'vest': { ctx.fillStyle = top; ctx.beginPath(); ctx.ellipse(0, 0, 3.4, 5.5, 0, 0, TAU); ctx.fill(); capsule(ctx, -1.2, -4.9, -1.2, 4.9, 0.5, '#dfe6ea'); capsule(ctx, 1.4, -4.7, 1.4, 4.7, 0.5, '#dfe6ea'); break; } // Warnweste mit Reflexstreifen
    case 'sport': capsule(ctx, -1, -4.8, -1, 4.8, 0.7, shade(top, 0.35)); break;
  }
  if (look.studs) for (const y of [-4.2, -3, 3, 4.2]) disc(ctx, 0.2, y, 0.45, '#c9ced4');
  if (look.acc === 'camera') { capsule(ctx, 0.8, -3.6, 2.6, 0, 0.35, '#1d1d1d'); ctx.fillStyle = '#1d1d1d'; ctx.fillRect(2.2, -1, 1.8, 2.4); disc(ctx, 4, 0.2, 0.6, '#5a6a7a'); }
  // Licht von der Sonnenseite auf die Schultern
  ctx.save(); ctx.globalAlpha = 0.22; oval(ctx, cx + lx * 1.4, ly * 2.2, rx * 0.6, 3.6, '#ffffff'); ctx.restore();
  ctx.restore();
  // Kopf
  const hx = 0.4 + lean * 0.7;
  drawHead(ctx, look, hx, light);
  if (player) { ctx.strokeStyle = 'rgba(255,255,255,0.9)'; ctx.lineWidth = 0.8; ctx.beginPath(); ctx.ellipse(0, 0, 3.9, 6.1, twist, 0, TAU); ctx.stroke(); }
  ctx.restore();
}

function drawHead(ctx, look, hx, [lx, ly]) {
  const hair = look.hair, hs = look.hairStyle, hat = look.hat;
  // Haare unter der Kopfbedeckung (lange Haare schauen hinten heraus)
  if (hs === 'long' && hat !== 'scarf') oval(ctx, hx - 1.4, 0, 3.4, 3.3, hair);
  if (hs === 'ponytail') { oval(ctx, hx - 3.2, 0, 1.8, 0.9, hair); }
  if (hs === 'bun' && !hat) disc(ctx, hx - 3, 0, 1.3, hair);
  if (hat === 'scarf') { // Kopftuch: umschließt den Kopf, Gesicht vorn frei
    oval(ctx, hx - 0.6, 0, 3.8, 3.5, look.hatColor); ctx.strokeStyle = OUTLINE; ctx.lineWidth = 0.5; ctx.stroke();
    oval(ctx, hx + 1.9, 0, 1.3, 1.7, look.skin);
    return;
  }
  // Kopf: Haare als Kappe, Gesicht vorn
  const bald = hs === 'bald' || hs === 'mohawk';
  disc(ctx, hx, 0, 3.1, bald ? shade(look.skin, -0.12) : hair);
  ctx.strokeStyle = OUTLINE; ctx.lineWidth = 0.5; ctx.stroke();
  if (hs === 'curly') for (const [x, y] of [[-1.6, -1.8], [-1.6, 1.8], [-2.4, 0], [0, -2.5], [0, 2.5]]) disc(ctx, hx + x, y, 1.1, hair);
  oval(ctx, hx + 1.4, 0, 1.9, 2.2, look.skin);
  if (look.beard) { ctx.fillStyle = shade(hair, -0.1); ctx.beginPath(); ctx.arc(hx + 1.6, 0, 2.1, -0.9, 0.9); ctx.lineTo(hx + 1.4, 0); ctx.fill(); }
  if (hs === 'mohawk') { ctx.fillStyle = hair; ctx.beginPath(); ctx.moveTo(hx + 1.8, -0.7); for (let i = 0; i <= 5; i++) { const x = hx + 1.8 - i * 1.15; ctx.lineTo(x, i % 2 ? -1.4 : -0.5); } ctx.lineTo(hx - 4, 0); for (let i = 5; i >= 0; i--) { const x = hx + 1.8 - i * 1.15; ctx.lineTo(x, i % 2 ? 1.4 : 0.5); } ctx.closePath(); ctx.fill(); }
  // Glanz auf der Lichtseite
  ctx.save(); ctx.globalAlpha = 0.25; disc(ctx, hx + lx * 1.3, ly * 1.3, 1.2, '#ffffff'); ctx.restore();
  switch (hat) {
    case 'cap': disc(ctx, hx - 0.3, 0, 3.2, look.hatColor); oval(ctx, hx + 2.8, 0, 1.8, 2.4, shade(look.hatColor, -0.2)); break;
    case 'capback': disc(ctx, hx, 0, 3.2, look.hatColor); oval(ctx, hx - 3.1, 0, 1.6, 2.2, shade(look.hatColor, -0.2)); break;
    case 'beanie': disc(ctx, hx - 0.4, 0, 3.2, look.hatColor); capsule(ctx, hx + 1.5, -2.4, hx + 1.5, 2.4, 1, shade(look.hatColor, -0.2)); disc(ctx, hx - 1.2, 0, 0.9, shade(look.hatColor, 0.2)); break;
    case 'hat': disc(ctx, hx, 0, 4.3, shade(look.hatColor, -0.1)); disc(ctx, hx - 0.2, 0, 2.6, look.hatColor); capsule(ctx, hx - 0.2, -2.6, hx - 0.2, 2.6, 0.5, '#1d1d1d'); break;
    case 'sunhat': disc(ctx, hx, 0, 4.1, look.hatColor); disc(ctx, hx - 0.2, 0, 2.5, shade(look.hatColor, -0.12)); ctx.strokeStyle = OUTLINE; ctx.lineWidth = 0.4; ctx.beginPath(); ctx.arc(hx, 0, 4.1, 0, TAU); ctx.stroke(); break;
    case 'helmet': disc(ctx, hx - 0.2, 0, 3.5, look.hatColor); capsule(ctx, hx - 3, 0, hx + 2.8, 0, 0.8, shade(look.hatColor, -0.2)); ctx.strokeStyle = OUTLINE; ctx.lineWidth = 0.5; ctx.beginPath(); ctx.arc(hx - 0.2, 0, 3.5, 0, TAU); ctx.stroke(); break;
  }
  if (look.headphones) { ctx.strokeStyle = '#1d1d1d'; ctx.lineWidth = 0.8; ctx.beginPath(); ctx.arc(hx, 0, 3.3, Math.PI * 0.55, Math.PI * 1.45); ctx.stroke(); oval(ctx, hx + 0.2, -3.1, 1, 0.8, '#e84393'); oval(ctx, hx + 0.2, 3.1, 1, 0.8, '#e84393'); }
}

// Liegend (umgehauen, tot, auf der Wiese)
function drawLying(ctx, p, look, dead) {
  capsule(ctx, -6, -1.8, -12, -1.8, 2.6, look.pants); capsule(ctx, -6, 1.8, -12, 1.8, 2.6, look.pants);
  oval(ctx, -12.6, -1.8, 1.2, 1.3, look.shoes); oval(ctx, -12.6, 1.8, 1.2, 1.3, look.shoes);
  if (dead) { capsule(ctx, 1, -3, -1, -8.5, 2.3, shade(look.topColor, -0.16)); capsule(ctx, 2, 3, 4, 8.5, 2.3, shade(look.topColor, -0.16)); disc(ctx, -1, -8.8, 1.1, look.skin); disc(ctx, 4, 8.8, 1.1, look.skin); }
  else { capsule(ctx, 2, -3.6, 5, -5, 2.3, shade(look.topColor, -0.16)); capsule(ctx, 2, 3.6, 5, 5, 2.3, shade(look.topColor, -0.16)); }
  oval(ctx, 0, 0, 7.6, 4.2, look.top === 'vest' ? look.under : look.topColor);
  ctx.strokeStyle = OUTLINE; ctx.lineWidth = 0.55; ctx.stroke();
  const hairOrHat = look.hat === 'scarf' ? look.hatColor : look.hairStyle === 'bald' ? shade(look.skin, -0.12) : look.hair;
  disc(ctx, 9.4, 0, 3.1, hairOrHat); oval(ctx, 9.2, 0, 2.3, 1.9, look.skin);
}

// Sitzend (Bank, Café): Beine nach vorn, Hände im Schoß
function drawSitting(ctx, look, light) {
  capsule(ctx, 1, -1.9, 7.4, -1.9, 2.7, look.pants); capsule(ctx, 1, 1.9, 7.4, 1.9, 2.7, look.pants);
  oval(ctx, 8.4, -1.9, 1.5, 1.1, look.shoes); oval(ctx, 8.4, 1.9, 1.5, 1.1, look.shoes);
  ctx.save(); ctx.translate(-1.2, 0);
  const sl = shade(look.topColor, -0.16);
  capsule(ctx, 0, -4.2, 3, -2.8, 2.3, sl); capsule(ctx, 0, 4.2, 3, 2.8, 2.3, sl);
  disc(ctx, 3.2, -2.6, 1.05, look.skin); disc(ctx, 3.2, 2.6, 1.05, look.skin);
  drawUpper(ctx, look, null, light, false);
  ctx.restore();
}

// Aussehen einer Person: der Spieler hat sein festes, Passanten ihren Typ (figure.js). Ältere Aufrufer übergeben nur
// Hemd/Haut/Haare – dann ein Alltagslook in diesen Farben.
export function lookOf(p, { shirt, skin, hair, player } = {}) {
  if (player) return PLAYER_LOOK;
  const look = figureLook(p);
  if (p.id === undefined && (shirt || skin || hair)) return { ...look, topColor: shirt ?? look.topColor, skin: skin ?? look.skin, hair: hair ?? look.hair };
  return look;
}

export function drawPerson(ctx, p, { shirt, skin, hair, player = false, down = false, dead = false, sun = null, weapon = null, attack = null, act = null, time = 0 } = {}) {
  const look = lookOf(p, { shirt, skin, hair, player });
  const a = (p._anim ??= createAnim(p));
  const facing = p.facing ?? p.angle ?? 0;
  stepAnim(a, p, time, facing, p.move ?? facing);
  const face = down ? (p.facing ?? p.angle ?? 0) : a.face;
  const sc = look.scale ?? 1;
  ctx.save();
  ctx.translate(p.x, p.y);
  const [sx, sy, sa] = smallShadow(sun, down ? 4 : 17 * sc);
  ctx.fillStyle = `rgba(0,0,0,${sa})`;
  ctx.beginPath(); ctx.ellipse(sx * 0.5, sy * 0.5, (down ? 9 : 6 + Math.abs(sx) * 0.35) * sc, (down ? 4 : 5 + Math.abs(sy) * 0.35) * sc, 0, 0, TAU); ctx.fill();
  ctx.rotate(face);
  const key = player ? 'player' : 'pedestrian';
  if (sprites[key]) { ctx.drawImage(sprites[key], -8, -8, 16, 16); ctx.restore(); return; }
  if (act === 'lie') down = true;
  const light = localLight(sun, face);
  if (down) {
    if (dead) ctx.rotate((p.fall ?? 0) - face + 0.6); // in Schlagrichtung gefallen, verdreht
    drawLying(ctx, p, look, dead);
    ctx.restore(); return;
  }
  ctx.scale(sc, sc);
  if (act === 'sit') { drawSitting(ctx, look, light); ctx.restore(); return; }
  const style = { stoop: look.stoop, cane: look.acc === 'cane', stroller: look.acc === 'stroller', carry: look.acc === 'briefcase' || look.acc === 'shopping' };
  const pose = gaitPose(a, time, style, (p.id ?? 0) * 0.37);
  const moving = a.amp > 0.15;
  if (style.stroller && !act) drawStroller(ctx, look, pose);
  const lf = legFrame(a);
  ctx.save(); ctx.rotate(lf.rot);
  drawLegs(ctx, look, lf.back ? { ...pose, footL: { ...pose.footL, x: -pose.footL.x }, footR: { ...pose.footR, x: -pose.footR.x } } : pose);
  ctx.restore();
  if (act && ACT_ARMS[act]) { // Tätigkeit: eigene Armhaltung statt Gehbewegung
    ACT_ARMS[act](ctx, look.topColor, look.skin, time + (p.id ?? 0));
    drawUpper(ctx, look, { ...pose, twist: 0 }, light, player);
    ctx.restore(); return;
  }
  // Tritt: ein Bein nach vorn
  if (attack?.kind === 'kick') { capsule(ctx, 1, 1.9, 11, 1.9, 2.8, look.pants); oval(ctx, 12, 1.9, 1.7, 1.2, look.shoes); }
  const gun = weapon === 'pistol' || weapon === 'smg' || weapon === 'shotgun';
  if (gun || (attack && attack.kind !== 'kick') || weapon === 'bat' || weapon === 'knife') {
    drawArmsWithWeapon(ctx, weapon, attack, look.topColor, look.skin, pose.handR.x * 0.8);
  } else {
    if (style.cane) drawCane(ctx, pose);
    drawArms(ctx, look, pose, time, moving);
  }
  drawUpper(ctx, look, pose, light, player);
  ctx.restore();
}

// Rumpf und Kopf in ruhiger Haltung, für Figuren mit eigener Armhaltung (Radfahrer in critters.js)
export function drawTorsoHead(ctx, shirt, skin, look, player) {
  const l = { ...PLAYER_LOOK, kind: 'rider', topColor: shirt, skin, hair: look?.hair ?? '#2b2118', hairStyle: 'short', bag: look?.bag ?? null, bagColor: look?.bagColor ?? '#333', scale: 1 };
  drawUpper(ctx, l, null, [0.6, -0.8], player);
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
