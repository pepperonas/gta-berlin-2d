// Menschen zeichnen (nur Darstellung): Spieler und Passanten in Draufsicht, lokal +x = Blickrichtung, +y = rechts.
// Aussehen aus figure.js (Typ, Kleidung, Frisur, Kopfbedeckung, Zubehör), Bewegung aus gait.js (Schritt, Arme,
// Schulterdrehung, Wippen, Laufpose, Atmen im Stand). Nichts hier verändert die Simulation.
//
// Aufbau wie ein Mensch von oben: Schultern als gerundetes Rechteck (Superellipse, Anzug eckiger, Mantel länger),
// Arme aus Ober- und Unterarm mit Ellbogen (kurze Ärmel lassen den Unterarm frei), Hände, Beine mit Schuhen (Kappe,
// Sohle), Kopf länger als breit mit Ohren, Nasenspitze, Haaransatz und Strähnen; Kleidung mit Nähten, Kragen, Revers,
// Kapuze, Reflexstreifen; Kopfbedeckungen mit Schirm, Strickrippen, Krempe. Rumpf und Kopf werden je Aussehen einmal
// als Bild gerechnet (mit Volumen-Schattierung) und nur noch gedreht/gesetzt – bewegte Teile (Arme, Beine) kommen je Bild.
import { sprites, shade, smallShadow } from './assets.js';
import { figureLook, PLAYER_LOOK } from './figure.js';
import { createAnim, stepAnim, gaitPose, legFrame } from './gait.js';

const TAU = Math.PI * 2;
const OUTLINE = 'rgba(12,14,20,0.42)';
const EDGE = 'rgba(10,12,16,0.55)'; // Kontur der Gliedmaßen (breiter Strich unter der Farbe)
// Detailstufe (render.js setzt sie nach Renderer.quality): ohne Kontur der Gliedmaßen, wenn die Zeit knapp ist
let detail = true;
export function setPeopleDetail(on) { detail = !!on; }

// Licht in lokalen Koordinaten (Einheitsvektor zur Sonne); nachts von oben links
function localLight(sun, face) {
  const dx = sun && sun.strength > 0.05 ? sun.dx : 0.6, dy = sun && sun.strength > 0.05 ? sun.dy : 0.8;
  const c = Math.cos(face), s = Math.sin(face);
  const lx = -(dx * c + dy * s), ly = -(-dx * s + dy * c), n = Math.hypot(lx, ly) || 1;
  return [lx / n, ly / n];
}

// --- Grundformen --------------------------------------------------------------------------------------------------
function capsule(ctx, x0, y0, x1, y1, w, col) {
  ctx.strokeStyle = col; ctx.lineWidth = w; ctx.lineCap = 'round';
  ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x1, y1); ctx.stroke();
}
const disc = (ctx, x, y, r, col) => { ctx.fillStyle = col; ctx.beginPath(); ctx.arc(x, y, r, 0, TAU); ctx.fill(); };
const oval = (ctx, x, y, rx, ry, col, rot = 0) => { ctx.fillStyle = col; ctx.beginPath(); ctx.ellipse(x, y, rx, ry, rot, 0, TAU); ctx.fill(); };
function rrect(ctx, x, y, w, h, r) { ctx.beginPath(); if (ctx.roundRect) ctx.roundRect(x, y, w, h, r); else ctx.rect(x, y, w, h); }
const edge = (ctx, w = 0.5, col = OUTLINE) => { ctx.strokeStyle = col; ctx.lineWidth = w; ctx.stroke(); };
function line(ctx, pts, w, col, alpha = 1) {
  ctx.save(); ctx.globalAlpha = alpha; ctx.strokeStyle = col; ctx.lineWidth = w; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  ctx.beginPath(); ctx.moveTo(pts[0], pts[1]); for (let i = 2; i < pts.length; i += 2) ctx.lineTo(pts[i], pts[i + 1]); ctx.stroke(); ctx.restore();
}
function arc(ctx, x, y, r, a0, a1, w, col, alpha = 1) {
  ctx.save(); ctx.globalAlpha = alpha; ctx.strokeStyle = col; ctx.lineWidth = w; ctx.lineCap = 'round';
  ctx.beginPath(); ctx.arc(x, y, r, a0, a1); ctx.stroke(); ctx.restore();
}
// Volumen: heller zur Mitte (Licht von oben), dunkler zum Rand (Umgebungsverdeckung). Ohne Verlauf (Tests) flach.
function shaded(ctx, x, y, r, col, lift = 0.14, drop = -0.3) {
  const g = ctx.createRadialGradient?.(x, y, r * 0.08, x, y, r);
  if (!g || !g.addColorStop) return col;
  g.addColorStop(0, shade(col, lift)); g.addColorStop(0.6, col); g.addColorStop(1, shade(col, drop));
  return g;
}

// Rumpf von oben: Superellipse (n groß = eckige Schultern), vorn/hinten verschieden tief
function bodyPath(ctx, hw, front, back, n, x0 = 0) {
  ctx.beginPath();
  for (let i = 0; i < 36; i++) {
    const t = (i / 36) * TAU, c = Math.cos(t), s = Math.sin(t);
    const x = Math.sign(c) * Math.abs(c) ** (2 / n) * (c > 0 ? front : back), y = Math.sign(s) * Math.abs(s) ** (2 / n) * hw;
    if (i) ctx.lineTo(x0 + x, y); else ctx.moveTo(x0 + x, y);
  }
  ctx.closePath();
}
const torsoDims = (look) => {
  const coat = look.top === 'coat', suit = look.top === 'suit';
  return { hw: coat ? 5.6 : suit ? 5.5 : 5.25, front: coat ? 2.9 : 2.6, back: coat ? 3.9 : 2.95, n: suit ? 3.6 : coat ? 2.9 : 2.7 };
};

// --- Bild-Zwischenspeicher für Rumpf und Kopf ----------------------------------------------------------------------
const HEAD_K = 0.9; // Kopf zu Schulterbreite ≈ 1 : 2 (echt ≈ 1 : 2,3; etwas größer, damit Frisur und Hut lesbar bleiben)
// RES Bildpunkte je Einheit (Figur ≈ 12 Einheiten breit), SPR_MAX Bilder (Rumpf und Kopf getrennt, gleiche Köpfe/Rümpfe
// werden geteilt); bei vollem Speicher fliegt das am längsten nicht benutzte heraus (Map-Reihenfolge = Nutzung)
const RES = 6, SPR_MAX = 900, spr = new Map(), keys = new WeakMap();
const TORSO_BOX = [-7.2, -6.8, 11.8, 13.6], HEAD_BOX = [-6.2, -5.2, 11.8, 10.4];
function makeCanvas(w, h) {
  try {
    if (typeof OffscreenCanvas !== 'undefined') return new OffscreenCanvas(w, h);
    if (typeof document !== 'undefined') return Object.assign(document.createElement('canvas'), { width: w, height: h });
  } catch { /* kein Canvas */ }
  return null;
}
// Zeichnet paint(ctx) über ein gespeichertes Bild (einmal je Schlüssel); ohne Canvas (Node) direkt
function layer(ctx, key, box, paint) {
  let c = spr.get(key);
  if (c === undefined) {
    c = makeCanvas(Math.ceil(box[2] * RES), Math.ceil(box[3] * RES));
    const g = c?.getContext?.('2d');
    if (g && typeof ctx.drawImage === 'function') {
      g.scale(RES, RES); g.translate(-box[0], -box[1]); paint(g);
      if (spr.size >= SPR_MAX) spr.delete(spr.keys().next().value);
      spr.set(key, c);
    } else c = null; // ohne Canvas nichts merken: direkt zeichnen
  } else { spr.delete(key); spr.set(key, c); } // zuletzt benutzt ans Ende
  if (c) ctx.drawImage(c, box[0], box[1], box[2], box[3]); else paint(ctx);
}
export const personSpriteCount = () => spr.size;
function keysOf(look) {
  let k = keys.get(look);
  if (!k) {
    k = {
      torso: `t|${look.top}|${look.topColor}|${look.under}|${look.tie}|${look.studs}|${look.bag}|${look.bagColor}|${look.acc === 'camera'}|${look.kind}`,
      head: `h|${look.skin}|${look.hair}|${look.hairStyle}|${look.hat}|${look.hatColor}|${look.beard}|${look.headphones}`,
    };
    if (typeof look === 'object' && look) keys.set(look, k);
  }
  return k;
}

// --- Rumpf -------------------------------------------------------------------------------------------------------
function paintBackpack(ctx, col) {
  ctx.fillStyle = col; rrect(ctx, -6.5, -3.3, 3.9, 6.6, 1.4); ctx.fill(); edge(ctx, 0.45);
  ctx.fillStyle = shade(col, -0.25); rrect(ctx, -4.4, -3, 1.6, 6, 0.8); ctx.fill(); // Deckel oben
  line(ctx, [-6.1, -2.4, -6.1, 2.4], 0.25, shade(col, 0.3), 0.8);                                                       // Reißverschluss
  oval(ctx, -5.9, 0, 0.35, 0.8, shade(col, -0.35));                                                                     // Griff
}
function backpackStraps(ctx, col) {
  const s = shade(col, -0.3);
  for (const y of [-2.5, 2.5]) { line(ctx, [-2.7, y, -0.4, y * 1.05, 1.9, y * 0.95], 1.0, s); line(ctx, [-2.4, y - 0.3, 1.6, y * 0.95 - 0.3], 0.18, shade(col, 0.2), 0.6); }
}

function paintTorso(ctx, look) {
  const { hw, front, back, n } = torsoDims(look), top = look.top;
  const base = top === 'vest' ? (look.under ?? '#3a4a5a') : look.topColor;
  if (look.bag === 'backpack') paintBackpack(ctx, look.bagColor);
  // Körper mit Volumen
  bodyPath(ctx, hw, front, back, n);
  ctx.fillStyle = shaded(ctx, 0.4, 0, hw + 0.6, base); ctx.fill(); edge(ctx, 0.55);
  // Rückenfalten und Schulternähte (leise)
  line(ctx, [-1.4, -3.6, -2.2, -1.6], 0.35, shade(base, -0.3), 0.35); line(ctx, [-1.4, 3.6, -2.2, 1.6], 0.35, shade(base, -0.3), 0.35);
  line(ctx, [0.3, -3.1, -0.2, -hw + 0.4], 0.3, shade(base, -0.3), 0.4); line(ctx, [0.3, 3.1, -0.2, hw - 0.4], 0.3, shade(base, -0.3), 0.4);
  const dk = shade(look.topColor, -0.28), lt = shade(look.topColor, 0.25);
  switch (top) {
    case 'tee':
      arc(ctx, 0.35, 0, 3.05, 0, TAU, 0.55, shade(base, -0.25), 0.8);                                     // Halsausschnitt
      break;
    case 'jacket': {
      const under = look.under ?? '#2b2b2b';
      ctx.fillStyle = under; ctx.beginPath(); ctx.moveTo(1.9, -1.2); ctx.lineTo(front + 0.05, -0.25); ctx.lineTo(front + 0.05, 0.25); ctx.lineTo(1.9, 1.2); ctx.closePath(); ctx.fill(); // offen vorn
      line(ctx, [1.9, -1.2, front, -0.3], 0.35, dk); line(ctx, [1.9, 1.2, front, 0.3], 0.35, dk);
      arc(ctx, 0.3, 0, 3.15, 0, TAU, 1.1, dk, 0.9);                                                        // Kragen
      arc(ctx, 0.3, 0, 3.55, -2.3, 2.3, 0.25, lt, 0.5);
      line(ctx, [-2.7, -hw + 1.2, -2.7, hw - 1.2], 0.3, dk, 0.35);                                          // Rückennaht
      break;
    }
    case 'suit':
      ctx.fillStyle = '#f2f2f2'; ctx.beginPath(); ctx.moveTo(1.2, -1.6); ctx.lineTo(front + 0.05, -0.2); ctx.lineTo(front + 0.05, 0.2); ctx.lineTo(1.2, 1.6); ctx.closePath(); ctx.fill(); // Hemd
      capsule(ctx, 1.9, 0, front - 0.1, 0, 0.85, look.tie ?? '#8a2f2f'); disc(ctx, 1.9, 0, 0.62, look.tie ?? '#8a2f2f');                             // Krawatte
      line(ctx, [0.9, -1.9, front, -0.25], 0.55, dk); line(ctx, [0.9, 1.9, front, 0.25], 0.55, dk);                                                     // Revers
      arc(ctx, 0.3, 0, 3.1, 1.1, TAU - 1.1, 0.8, dk, 0.9);
      line(ctx, [0.2, -hw + 0.3, -0.8, -hw + 0.3], 0.35, lt, 0.4); line(ctx, [0.2, hw - 0.3, -0.8, hw - 0.3], 0.35, lt, 0.4);                        // Schulterpolster
      oval(ctx, 1.7, -3.3, 0.45, 0.8, '#e8e8e8');                                                                                                         // Einstecktuch
      break;
    case 'coat':
      arc(ctx, 0.3, 0, 3.25, 0, TAU, 1.5, dk, 0.95);                                                        // hochgeschlagener Kragen
      line(ctx, [front - 0.2, -1.4, 1.9, -0.9], 0.3, dk, 0.6);
      for (const y of [-1.3, 1.3]) for (const x of [1.9, 2.5]) disc(ctx, x, y, 0.3, shade(look.topColor, -0.45));   // Zweireiher-Knöpfe
      line(ctx, [-2.4, 0, -back + 0.2, 0], 0.35, dk, 0.5);                                                  // Rückenschlitz
      line(ctx, [-3.1, -hw + 0.9, -3.1, hw - 0.9], 0.55, dk, 0.55);                                         // Gürtel hinten
      break;
    case 'hoodie':
      oval(ctx, -3.1, 0, 1.9, 3.1, shade(look.topColor, -0.18)); edge(ctx, 0.35);                          // Kapuze auf dem Rücken
      oval(ctx, -2.7, 0, 0.95, 2.0, shade(look.topColor, -0.42));
      capsule(ctx, 2.1, -0.8, 3.1, -0.9, 0.35, '#eeeeee'); capsule(ctx, 2.1, 0.8, 3.1, 0.9, 0.35, '#eeeeee'); // Kordeln
      break;
    case 'vest': {
      bodyPath(ctx, hw - 0.35, front - 0.2, back - 0.1, n);
      ctx.fillStyle = shaded(ctx, 0.4, 0, hw, look.topColor, 0.1, -0.2); ctx.fill();
      for (const y of [-2.6, 2.6]) capsule(ctx, -back + 0.5, y, front - 0.3, y, 0.75, '#dfe6ea');           // Reflexstreifen über die Schultern
      capsule(ctx, -2.0, -hw + 0.7, -2.0, hw - 0.7, 0.75, '#dfe6ea');                                        // und quer über den Rücken
      ctx.fillStyle = base; ctx.beginPath(); ctx.ellipse(1.8, 0, 1.2, 1.5, 0, 0, TAU); ctx.fill();           // Hemd am Hals
      break;
    }
    case 'sport':
      for (const y of [-3.5, 3.5]) capsule(ctx, -back + 0.6, y, front - 0.4, y, 0.55, lt);                   // Seitenstreifen
      line(ctx, [2.0, 0, front, 0], 0.3, dk, 0.7);                                                           // Reißverschluss
      arc(ctx, 0.35, 0, 3.0, 0, TAU, 0.5, dk, 0.7);
      break;
  }
  if (look.studs) {
    for (const y of [-4.3, -3.1, 3.1, 4.3]) disc(ctx, 0.2, y, 0.42, '#c9ced4');
    ctx.fillStyle = '#b8342a'; ctx.fillRect(-2.9, -1.3, 1.3, 2.6); ctx.fillStyle = '#e8e8e8'; ctx.fillRect(-2.6, -0.9, 0.7, 1.8); // Aufnäher
  }
  if (look.bag === 'backpack') backpackStraps(ctx, look.bagColor);
  if (look.acc === 'camera') {
    line(ctx, [0.9, -3.6, 2.6, -0.4], 0.35, '#1d1d1d'); line(ctx, [0.9, 3.6, 2.6, 0.4], 0.35, '#1d1d1d');
    ctx.fillStyle = '#1d1d1d'; ctx.fillRect(2.2, -1.2, 1.9, 2.4); disc(ctx, 4.2, 0.1, 0.75, '#3e4c5a'); disc(ctx, 4.2, 0.1, 0.35, '#8fb3cf');
  }
}

// --- Kopf (von oben: Stirn vorn, Hinterkopf hinten) ----------------------------------------------------------------
function hairStrands(ctx, hair, cx) {
  // kurze, dem Kopf folgende Striche vom Wirbel nach vorn (dunkel/hell im Wechsel), dazu ein Glanz auf der Lichtseite
  const d = shade(hair, -0.32), l = shade(hair, 0.3);
  for (const [y0, y1, col, al] of [[-0.9, -1.7, d, 0.35], [-0.3, -0.6, l, 0.3], [0.3, 0.6, d, 0.35], [0.9, 1.7, l, 0.3], [-1.5, -2.2, d, 0.3], [1.5, 2.2, d, 0.3]]) {
    ctx.save(); ctx.globalAlpha = al; ctx.strokeStyle = col; ctx.lineWidth = 0.2; ctx.lineCap = 'round';
    ctx.beginPath(); ctx.moveTo(cx - 1.2, y0 * 0.6); ctx.quadraticCurveTo(cx + 0.6, y0 * 1.1, cx + 1.9, y1 * 0.75); ctx.stroke(); ctx.restore();
  }
  ctx.save(); ctx.globalAlpha = 0.22; oval(ctx, cx - 0.6, -0.9, 1.3, 0.7, '#ffffff', 0.25); ctx.restore();
}

function paintHead(ctx, look) {
  const hs = look.hairStyle, hat = look.hat, hair = look.hair, skin = look.skin;
  if (hat === 'scarf') return paintScarf(ctx, look);
  const hard = hat === 'helmet' || hat === 'hat' || hat === 'sunhat';
  // Hinter dem Kopf: lange Haare fallen auf die Schultern, Zopf, Dutt
  if (hs === 'long') {
    ctx.fillStyle = shade(hair, -0.08); ctx.beginPath(); ctx.moveTo(0.6, -2.7);
    ctx.bezierCurveTo(-1.8, -3.6, -4.6, -2.9, -4.9, 0); ctx.bezierCurveTo(-4.6, 2.9, -1.8, 3.6, 0.6, 2.7); ctx.closePath(); ctx.fill(); edge(ctx, 0.35);
    for (const y of [-1.8, -0.6, 0.6, 1.8]) line(ctx, [-1.5, y * 0.8, -4.3, y * 1.1], 0.22, shade(hair, -0.3), 0.5);
  }
  if (hs === 'ponytail') {
    ctx.fillStyle = hair; ctx.beginPath(); ctx.moveTo(-2.6, -0.75); ctx.quadraticCurveTo(-5.6, -0.6, -5.4, 0); ctx.quadraticCurveTo(-5.6, 0.6, -2.6, 0.75); ctx.closePath(); ctx.fill(); edge(ctx, 0.3);
    line(ctx, [-3.2, 0, -5.1, 0], 0.2, shade(hair, 0.3), 0.5); disc(ctx, -2.75, 0, 0.55, '#e84393');
  }
  if (hs === 'bun' && !hat) { disc(ctx, -2.75, 0, 1.4, hair); edge(ctx, 0.35); arc(ctx, -2.75, 0, 0.8, 0.5, 4.2, 0.3, shade(hair, -0.3), 0.7); }
  // Ohren (unter langen Haaren und harten Hüten nicht zu sehen)
  if (hs !== 'long' && !hard) for (const s of [-1, 1]) { oval(ctx, 0.15, s * 2.5, 0.75, 0.5, shade(skin, -0.08)); edge(ctx, 0.3); }
  // Schädel mit Volumen, Nasenspitze vorn
  ctx.fillStyle = shaded(ctx, 0.3, 0, 3.1, skin, 0.1, -0.22); ctx.beginPath(); ctx.ellipse(0, 0, 2.95, 2.55, 0, 0, TAU); ctx.fill(); edge(ctx, 0.5);
  oval(ctx, 2.95, 0, 0.62, 0.52, shade(skin, -0.05)); edge(ctx, 0.25, 'rgba(60,30,20,0.35)');
  arc(ctx, 0.9, 0, 1.75, -0.75, 0.75, 0.3, shade(skin, -0.25), 0.45);                 // Brauenbogen
  if (look.beard) {
    ctx.fillStyle = shade(hair, -0.12); ctx.beginPath(); ctx.arc(0.6, 0, 2.5, -1.05, 1.05); ctx.arc(1.1, 0, 1.6, 1.0, -1.0, true); ctx.closePath(); ctx.fill();
    oval(ctx, 2.95, 0, 0.62, 0.52, shade(skin, -0.05));                                // Nase über dem Bart
  }
  // Haare
  if (hs === 'bald') {
    arc(ctx, 0, 0, 2.3, Math.PI * 0.62, Math.PI * 1.38, 1.0, hair, 0.95);             // Haarkranz
    oval(ctx, 0.2, -0.6, 1.1, 0.6, 'rgba(255,255,255,0.35)', 0.3);                     // Glanz auf der Glatze
  } else if (hs === 'mohawk') {
    ctx.save(); ctx.globalAlpha = 0.45; oval(ctx, -0.4, 0, 2.7, 2.35, shade(skin, -0.2)); ctx.restore(); // rasierte Seiten
    ctx.fillStyle = hair; ctx.beginPath(); ctx.moveTo(2.0, -0.6);
    for (let i = 0; i <= 6; i++) ctx.lineTo(2.0 - i * 1.05, i % 2 ? -1.5 : -0.45);
    ctx.lineTo(-4.4, 0);
    for (let i = 6; i >= 0; i--) ctx.lineTo(2.0 - i * 1.05, i % 2 ? 1.5 : 0.45);
    ctx.closePath(); ctx.fill(); edge(ctx, 0.35);
    line(ctx, [1.6, 0, -3.8, 0], 0.3, shade(hair, 0.35), 0.6);
  } else if (hs === 'curly') {
    oval(ctx, -0.45, 0, 2.75, 2.65, hair);
    for (let i = 0; i < 11; i++) { const a = 0.75 + (i / 10) * (TAU - 1.5); disc(ctx, -0.45 + Math.cos(a) * 2.35, Math.sin(a) * 2.3, 0.95, hair); }
    for (const [x, y] of [[-1.2, -1], [-0.2, 0.9], [-1.8, 0.6], [0.6, -0.9], [-2.4, -0.4], [0.9, 0.5]]) arc(ctx, x, y, 0.55, 0.5, 4.5, 0.25, shade(hair, 0.3), 0.55);
  } else {
    ctx.fillStyle = hair; ctx.beginPath(); ctx.ellipse(-0.45, 0, 2.72, 2.62, 0, 0, TAU); ctx.fill(); edge(ctx, 0.35);
    if (hs === 'long') for (const s of [-1, 1]) oval(ctx, -0.3, s * 2.35, 2.3, 0.85, hair);      // Seitenhaar über den Ohren
    hairStrands(ctx, hair, -0.4);
  }
  // Kopfbedeckungen
  const hc = look.hatColor ?? '#2b2b2b';
  switch (hat) {
    case 'cap': case 'capback': {
      const s = hat === 'cap' ? 1 : -1;
      ctx.fillStyle = shade(hc, -0.22); ctx.beginPath(); ctx.ellipse(s * 3.0, 0, 2.1, 2.35, 0, 0, TAU); ctx.fill(); edge(ctx, 0.35); // Schirm
      ctx.fillStyle = shaded(ctx, -0.3, 0, 3.2, hc); ctx.beginPath(); ctx.ellipse(-0.3, 0, 2.95, 2.75, 0, 0, TAU); ctx.fill(); edge(ctx, 0.45);
      for (const a of [0.6, -0.6, Math.PI]) line(ctx, [-0.35, 0, -0.35 + Math.cos(a) * 2.8, Math.sin(a) * 2.6], 0.22, shade(hc, -0.35), 0.6); // Nähte
      disc(ctx, -0.35, 0, 0.38, shade(hc, -0.3));
      if (hat === 'capback') arc(ctx, -2.6, 0, 1.0, 2.2, 4.1, 0.35, shade(hc, -0.4), 0.8);     // Verschluss
      break;
    }
    case 'beanie':
      ctx.fillStyle = shaded(ctx, -0.3, 0, 3.3, hc); ctx.beginPath(); ctx.ellipse(-0.3, 0, 3.05, 2.8, 0, 0, TAU); ctx.fill(); edge(ctx, 0.45);
      for (let i = 0; i < 12; i++) { const a = (i / 12) * TAU; line(ctx, [-0.3 + Math.cos(a) * 1.1, Math.sin(a) * 1.0, -0.3 + Math.cos(a) * 2.8, Math.sin(a) * 2.55], 0.25, shade(hc, -0.25), 0.5); } // Strickrippen
      arc(ctx, -0.3, 0, 2.6, -1.25, 1.25, 1.0, shade(hc, -0.15), 1);                        // Umschlag vorn
      disc(ctx, -0.5, 0, 1.05, shade(hc, 0.22)); for (const [x, y] of [[-0.8, -0.3], [-0.2, 0.3], [-0.6, 0.5]]) disc(ctx, x, y, 0.22, shade(hc, 0.4)); // Bommel
      break;
    case 'hat':
      ctx.fillStyle = shade(hc, -0.14); ctx.beginPath(); ctx.ellipse(0, 0, 4.3, 4.0, 0, 0, TAU); ctx.fill(); edge(ctx, 0.45);
      arc(ctx, 0, 0, 3.9, 0, TAU, 0.25, shade(hc, 0.2), 0.5);                                  // Krempenrand
      ctx.fillStyle = shaded(ctx, -0.1, 0, 3.0, hc); ctx.beginPath(); ctx.ellipse(-0.1, 0, 2.8, 2.4, 0, 0, TAU); ctx.fill();
      arc(ctx, -0.1, 0, 2.55, 0, TAU, 0.55, '#1d1d1d', 0.9);                                   // Hutband
      line(ctx, [-1.8, 0, 1.6, 0], 0.45, shade(hc, -0.35), 0.8);                                // Kniff
      break;
    case 'sunhat':
      ctx.fillStyle = hc; ctx.beginPath(); ctx.arc(0, 0, 4.4, 0, TAU); ctx.fill(); edge(ctx, 0.4);
      for (const r of [3.0, 3.6, 4.1]) arc(ctx, 0, 0, r, 0, TAU, 0.2, shade(hc, -0.25), 0.45); // Strohgeflecht
      ctx.fillStyle = shaded(ctx, -0.1, 0, 2.6, hc, 0.1, -0.2); ctx.beginPath(); ctx.arc(-0.1, 0, 2.45, 0, TAU); ctx.fill();
      arc(ctx, -0.1, 0, 2.5, 0, TAU, 0.5, '#c0392b', 0.85);                                     // Band
      break;
    case 'helmet':
      ctx.fillStyle = shade(hc, -0.12); ctx.beginPath(); ctx.ellipse(0, 0, 3.6, 3.35, 0, 0, TAU); ctx.fill(); edge(ctx, 0.45);
      ctx.fillStyle = shaded(ctx, -0.4, -0.3, 3.6, hc, 0.25, -0.15); ctx.beginPath(); ctx.ellipse(-0.1, 0, 3.1, 2.85, 0, 0, TAU); ctx.fill();
      capsule(ctx, -2.9, 0, 2.9, 0, 0.9, shade(hc, 0.3)); for (const y of [-1.4, 1.4]) line(ctx, [-2.3, y, 2.2, y], 0.3, shade(hc, -0.2), 0.7); // Grat
      break;
  }
  if (look.headphones) {
    arc(ctx, 0.2, 0, 3.0, Math.PI * 0.52, Math.PI * 1.48, 0.8, '#1d1d1d', 1);
    for (const s of [-1, 1]) { oval(ctx, 0.2, s * 3.0, 1.15, 0.8, '#e84393'); edge(ctx, 0.35, 'rgba(0,0,0,0.6)'); }
  }
}

// Kopftuch: umschließt Kopf und Hals und fällt auf die Schultern, vorn nur das Gesicht frei
function paintScarf(ctx, look) {
  const c = look.hatColor ?? '#2b2b2b', skin = look.skin;
  ctx.fillStyle = shade(c, -0.1); ctx.beginPath(); ctx.moveTo(1.2, -3.2); ctx.bezierCurveTo(-2.4, -4.4, -5.2, -2.6, -5.0, 0); ctx.bezierCurveTo(-5.2, 2.6, -2.4, 4.4, 1.2, 3.2); ctx.closePath(); ctx.fill(); edge(ctx, 0.4);
  ctx.fillStyle = shaded(ctx, -0.2, 0, 3.8, c, 0.14, -0.22); ctx.beginPath(); ctx.ellipse(-0.3, 0, 3.55, 3.3, 0, 0, TAU); ctx.fill(); edge(ctx, 0.45);
  for (const [r, a0, a1] of [[2.6, 1.9, 3.4], [2.9, 3.1, 4.4], [1.9, 2.2, 4.1]]) arc(ctx, -0.6, 0, r, a0, a1, 0.3, shade(c, -0.32), 0.6); // Falten
  oval(ctx, 2.15, 0, 1.3, 1.7, skin); edge(ctx, 0.3, 'rgba(60,30,20,0.35)');
  oval(ctx, 3.1, 0, 0.5, 0.45, shade(skin, -0.06));
  arc(ctx, 2.0, 0, 1.75, -1.2, 1.2, 0.5, shade(c, -0.2), 0.8);                                  // Saum ums Gesicht
}

// Liegend auf dem Rücken: das Gesicht zeigt nach oben (+x = Scheitel)
function paintFaceUp(ctx, look) {
  const skin = look.skin, hair = look.hat === 'scarf' ? look.hatColor : look.hair;
  if (look.hat === 'scarf') oval(ctx, 0.6, 0, 3.3, 3.2, hair);
  else if (look.hairStyle === 'long') oval(ctx, 1.4, 0, 2.6, 3.2, hair);
  oval(ctx, 0, 0, 2.7, 2.35, skin); edge(ctx, 0.45);
  if (look.hat !== 'scarf' && look.hairStyle !== 'bald') { ctx.fillStyle = hair; ctx.beginPath(); ctx.ellipse(1.2, 0, 1.9, 2.45, 0, -Math.PI / 2, Math.PI / 2); ctx.fill(); }
  for (const s of [-1, 1]) { line(ctx, [0.1, s * 0.5, 0.1, s * 1.3], 0.3, '#2a1d18', 0.85); line(ctx, [0.6, s * 0.45, 0.6, s * 1.4], 0.25, shade(look.hair, -0.1), 0.7); } // geschlossene Augen, Brauen
  oval(ctx, -0.7, 0, 0.45, 0.35, shade(skin, -0.12));
  line(ctx, [-1.5, -0.55, -1.6, 0.55], 0.3, '#7a3b30', 0.8);
  if (look.beard) oval(ctx, -1.6, 0, 1.1, 1.6, shade(look.hair, -0.1));
}

// --- Gliedmaßen (je Bild) ------------------------------------------------------------------------------------------
// shade() zerlegt und baut Farbtexte – je Bild für jede Figur zu teuer, daher gemerkt
const SHADES = new Map();
function tone(col, f) { const k = col + f; let v = SHADES.get(k); if (v === undefined) { if (SHADES.size > 4000) SHADES.clear(); v = shade(col, f); SHADES.set(k, v); } return v; }
const SHORT = new Set(['tee', 'sport']);
const sleeveOf = (look) => (look.top === 'vest' ? (look.under ?? '#3a4a5a') : look.topColor);

// Arm von der Schulter zur Hand (lokal). Von oben hängt ein lockerer Arm fast senkrecht (kurz in der Projektion); der
// Ellbogen liegt knapp außerhalb der Linie Schulter–Hand, bei vorgestreckter Hand (Waffe, Handy) etwas weiter.
function drawArm(ctx, look, side, hx, hy) {
  const sx = 0.1, sy = side * 4.35, dx = hx - sx, dy = hy - sy;
  const bend = 0.3 + Math.max(0, Math.min(1, (dx - 1) / 5)) * 0.5;
  const ex = sx + dx * 0.45 - bend * 0.3, ey = sy + dy * 0.45 + side * bend;
  const sleeve = tone(sleeveOf(look), -0.1), skin = look.skin, short = SHORT.has(look.top);
  ctx.lineJoin = 'round';
  // Kontur in einem Zug, dann Ärmel und Unterarm
  if (detail) { ctx.strokeStyle = EDGE; ctx.lineWidth = 3.0; ctx.lineCap = 'round'; ctx.beginPath(); ctx.moveTo(sx, sy); ctx.lineTo(ex, ey); ctx.lineTo(hx, hy); ctx.stroke(); }
  if (short) {
    const mx = sx + (ex - sx) * 0.8, my = sy + (ey - sy) * 0.8;
    capsule(ctx, ex, ey, hx, hy, 1.85, skin); capsule(ctx, mx, my, ex, ey, 2.0, skin);
    capsule(ctx, sx, sy, mx, my, 2.55, sleeve);
  } else {
    capsule(ctx, ex, ey, hx, hy, 2.15, sleeve); capsule(ctx, sx, sy, ex, ey, 2.45, sleeve);
  }
  // Hand
  ctx.fillStyle = skin; ctx.beginPath(); ctx.ellipse(hx, hy, 1.2, 1.0, Math.atan2(hy - ey, hx - ex), 0, TAU); ctx.fill(); // Kontur gibt der Armstrich
}

// Beine mit Schuhen; der Fuß in der Luft ist etwas größer und heller (näher an der Kamera)
function drawLegs(ctx, look, pose) {
  const legCol = look.top === 'coat' ? tone(look.pants, -0.05) : look.pants;
  const light = look.shoes === '#f0f0f0' || look.shoes === '#f2f2f2';
  for (const [f, y] of [[pose.footL, -1.9], [pose.footR, 1.9]]) {
    const fx = f.x, lift = f.lift, hx = -0.2, hy = y * 0.82;
    if (detail && (Math.abs(fx) > 1.2 || lift > 0.05)) capsule(ctx, hx, hy, fx, y, 3.2, EDGE); // im Stand verdeckt der Rumpf das Bein
    if (look.shorts) { capsule(ctx, hx, hy, fx * 0.45, y, 2.8, legCol); capsule(ctx, fx * 0.45, y, fx, y, 2.1, look.skin); }
    else capsule(ctx, hx, hy, fx, y, 2.7, legCol);
    const k = 1 + lift * 0.28, tip = fx >= 0 ? 1.1 : -0.2, cx = fx + tip * k;
    const col = lift > 0.3 ? tone(look.shoes, 0.12) : look.shoes;
    if (detail) oval(ctx, cx, y, 2.1 * k, 1.4 * k, EDGE);                                            // Sohle/Kontur
    oval(ctx, cx, y, 1.8 * k, 1.12 * k, col);
    if (light) { oval(ctx, cx + 0.9 * k, y, 0.7 * k, 0.8 * k, '#d9dde2'); capsule(ctx, cx - 1.2 * k, y - 0.25, cx + 0.2 * k, y - 0.25, 0.3, '#3a6fd8'); } // Kappe, Streifen am Turnschuh
  }
}

// Zubehör in den Händen (ohne Waffe)
function drawCarried(ctx, look, pose, time, moving) {
  const R = pose.handR, Lh = pose.handL;
  switch (look.acc) {
    case 'briefcase': ctx.fillStyle = '#3a2618'; ctx.fillRect(R.x - 2.3, R.y + 0.6, 4.6, 2.3); edge(ctx, 0.3); ctx.fillStyle = '#b08a3a'; ctx.fillRect(R.x - 0.4, R.y + 0.6, 0.8, 0.6); capsule(ctx, R.x - 2.1, R.y + 1.7, R.x + 2.1, R.y + 1.7, 0.2, '#5a3a24'); break;
    case 'shopping': ctx.fillStyle = '#f2f2f2'; ctx.fillRect(Lh.x - 1.9, Lh.y - 2.9, 3.8, 2.5); ctx.fillStyle = '#3aa85a'; ctx.fillRect(Lh.x - 1.9, Lh.y - 1.3, 3.8, 0.6); oval(ctx, Lh.x - 0.7, Lh.y - 2.4, 0.7, 0.45, '#e2a13a'); break;
    case 'bottle': ctx.fillStyle = '#3f6b2a'; ctx.fillRect(R.x, R.y - 0.65, 3.3, 1.3); ctx.fillStyle = '#d8c86a'; ctx.fillRect(R.x + 2.9, R.y - 0.35, 0.7, 0.7); break;
    case 'phone': if (!moving || Math.sin(time * 0.4) > 0) { ctx.fillStyle = '#15171b'; ctx.fillRect(Lh.x + 0.4, Lh.y - 0.75, 1.5, 2.3); ctx.fillStyle = '#9fd3ff'; ctx.fillRect(Lh.x + 0.6, Lh.y - 0.5, 1.1, 1.8); } break;
  }
  if (look.bag === 'tote') { ctx.fillStyle = look.bagColor; ctx.fillRect(R.x - 1.5, R.y + 0.8, 3.0, 3.2); edge(ctx, 0.3); capsule(ctx, R.x - 0.8, R.y + 0.9, R.x + 0.8, R.y + 0.9, 0.25, tone(look.bagColor, -0.3)); }
}

// Stock: von der Hand schräg nach vorn auf den Boden
function drawCane(ctx, pose) { const h = pose.handR; capsule(ctx, h.x, h.y, h.x + 2.4, h.y + 1.2, 1.0, '#5a3a1e'); disc(ctx, h.x + 2.4, h.y + 1.2, 0.5, '#2b1c10'); }

// Kinderwagen vor der Person (Griff an den Händen)
function drawStroller(ctx, look) {
  const c = look.strollerColor;
  capsule(ctx, 6.2, -2.8, 6.2, 2.8, 1.1, '#2b2b2b');                                         // Griff
  capsule(ctx, 6.4, -2.6, 9, -3, 0.7, '#555555'); capsule(ctx, 6.4, 2.6, 9, 3, 0.7, '#555555');
  for (const [x, y] of [[9, -3.7], [9, 3.7], [15.4, -3.7], [15.4, 3.7]]) { ctx.fillStyle = '#1d1d1d'; ctx.fillRect(x - 1.1, y - 0.65, 2.2, 1.3); disc(ctx, x, y, 0.3, '#9a9a9a'); }
  ctx.fillStyle = shaded(ctx, 12.2, 0, 4.6, c); ctx.beginPath(); ctx.ellipse(12.2, 0, 4.5, 3.5, 0, 0, TAU); ctx.fill(); edge(ctx, 0.45);
  ctx.fillStyle = shade(c, -0.25); ctx.beginPath(); ctx.ellipse(13.6, 0, 2.7, 3.1, 0, -Math.PI / 2, Math.PI / 2); ctx.fill(); // Verdeck
  for (const a of [-0.9, 0, 0.9]) line(ctx, [13.6, 0, 13.6 + Math.cos(a) * 2.6, Math.sin(a) * 3.0], 0.25, shade(c, -0.45), 0.8);
  oval(ctx, 10.2, 0, 2.2, 2.6, '#e8e2d6');                                                   // Decke
  oval(ctx, 11.3, 0, 1.3, 1.2, '#f5d6c6'); edge(ctx, 0.25);                                  // Baby
}

// --- Oberkörper und Kopf (gespeicherte Bilder) + Licht je Bild -----------------------------------------------------
function drawUpper(ctx, look, pose, light, player) {
  const twist = pose?.twist ?? 0, bob = pose?.bob ?? 1, lean = pose?.lean ?? 0, sway = pose?.sway ?? 0, breath = pose?.breath ?? 0;
  const [lx, ly] = light, k = keysOf(look);
  ctx.save();
  ctx.translate(0, sway * 0.3);
  ctx.save();
  ctx.rotate(twist); ctx.scale(bob + breath, bob + breath);
  layer(ctx, k.torso, TORSO_BOX, (g) => paintTorso(g, look));
  // Sonne auf der Schulterseite, Schatten auf der anderen
  const a0 = ctx.globalAlpha ?? 1;
  ctx.globalAlpha = a0 * 0.2; oval(ctx, lx * 1.4, ly * 2.3, 2.3, 3.4, '#ffffff'); ctx.globalAlpha = a0;
  ctx.restore();
  const hx = 0.4 + lean * 0.7;
  ctx.save(); ctx.translate(hx, 0); ctx.scale(HEAD_K, HEAD_K);
  layer(ctx, k.head, HEAD_BOX, (g) => paintHead(g, look));
  ctx.restore();
  void player;
  ctx.restore();
}

// Liegend (umgehauen, tot, auf der Wiese)
function drawLying(ctx, p, look, dead) {
  for (const y of [-1.9, 1.9]) { capsule(ctx, -5.5, y, -12, y * 1.05, 3.2, EDGE); capsule(ctx, -5.5, y, -12, y * 1.05, 2.7, look.pants); }
  for (const y of [-2, 2]) { ctx.fillStyle = look.shoes; ctx.beginPath(); ctx.ellipse(-12.8, y, 1.1, 1.35, 0, 0, TAU); ctx.fill(); edge(ctx, 0.3, 'rgba(0,0,0,0.6)'); }
  // Schultern am Kopfende des Rumpfs: bewusstlos liegen die Arme neben dem Körper, tot verdreht und abgespreizt
  ctx.save(); ctx.translate(4.4, 0);
  if (dead) { drawArm(ctx, look, -1, -2.5, -9.6); drawArm(ctx, look, 1, 2.5, 9.2); }
  else { drawArm(ctx, look, -1, -8.2, -5.3); drawArm(ctx, look, 1, -7.6, 5.5); }
  ctx.restore();
  const base = look.top === 'vest' ? (look.under ?? '#3a4a5a') : look.topColor;
  bodyPath(ctx, 4.4, 6.2, 6.0, 3.2);
  ctx.fillStyle = shaded(ctx, 0.5, 0, 6.5, base); ctx.fill(); edge(ctx, 0.55);
  if (look.top === 'vest') for (const y of [-2.2, 2.2]) capsule(ctx, -5, y, 5.4, y, 0.8, '#dfe6ea');
  line(ctx, [5.8, 0, -5, 0], 0.35, shade(look.topColor, -0.3), 0.6);                          // Knopfleiste/Reißverschluss
  arc(ctx, 6.5, 0, 1.8, Math.PI * 0.6, Math.PI * 1.4, 0.6, shade(look.topColor, -0.3), 0.8); // Kragen
  ctx.save(); ctx.translate(9.2, 0); paintFaceUp(ctx, look); ctx.restore();
  void p;
}

// Sitzend (Bank, Café): Oberschenkel nach vorn, Füße davor, Hände im Schoß
function drawSitting(ctx, look, light) {
  for (const y of [-1.95, 1.95]) {
    capsule(ctx, 0.8, y, 6.2, y * 1.05, 3.3, EDGE); capsule(ctx, 0.8, y, 6.2, y * 1.05, 2.8, look.pants);
    ctx.fillStyle = look.shoes; ctx.beginPath(); ctx.ellipse(8.1, y * 1.08, 1.6, 1.15, 0, 0, TAU); ctx.fill(); edge(ctx, 0.3, 'rgba(0,0,0,0.6)');
  }
  ctx.save(); ctx.translate(-1.2, 0);
  drawArm(ctx, look, -1, 4.4, -2.5); drawArm(ctx, look, 1, 4.4, 2.5);
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
  // Spieler: Ring am Boden (statt Umriss auf dem Körper) – hebt ihn aus der Menge, ohne die Figur zu übermalen
  if (player && !down) {
    ctx.strokeStyle = 'rgba(0,0,0,0.35)'; ctx.lineWidth = 1.6; ctx.beginPath(); ctx.arc(0, 0, 8.2 * sc, 0, TAU); ctx.stroke();
    ctx.strokeStyle = 'rgba(255,255,255,0.8)'; ctx.lineWidth = 0.8; ctx.stroke();
  }
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
  const arm = (side, x, y) => drawArm(ctx, look, side, x, y);
  if (act && ACT_ARMS[act]) { // Tätigkeit: eigene Armhaltung statt Gehbewegung
    ACT_ARMS[act](ctx, look, time + (p.id ?? 0), arm);
    drawUpper(ctx, look, { ...pose, twist: 0 }, light, player);
    ctx.restore(); return;
  }
  // Tritt: ein Bein nach vorn
  if (attack?.kind === 'kick') { capsule(ctx, 1, 1.9, 11, 1.9, 3.3, EDGE); capsule(ctx, 1, 1.9, 11, 1.9, 2.8, look.pants); oval(ctx, 12, 1.9, 1.8, 1.25, look.shoes); edge(ctx, 0.35, 'rgba(0,0,0,0.6)'); }
  const gun = weapon === 'pistol' || weapon === 'smg' || weapon === 'shotgun';
  if (gun || (attack && attack.kind !== 'kick') || weapon === 'bat' || weapon === 'knife') {
    drawArmsWithWeapon(ctx, weapon, attack, arm, pose.handR.x * 0.8);
  } else {
    if (style.cane) drawCane(ctx, pose);
    arm(-1, pose.handL.x, pose.handL.y); arm(1, pose.handR.x, pose.handR.y);
    drawCarried(ctx, look, pose, time, moving);
  }
  drawUpper(ctx, look, pose, light, player);
  ctx.restore();
}

// Rumpf und Kopf in ruhiger Haltung, für Figuren mit eigener Armhaltung (Radfahrer in critters.js)
export function drawTorsoHead(ctx, shirt, skin, look, player) {
  const l = { ...PLAYER_LOOK, kind: 'rider', topColor: shirt, skin, hair: look?.hair ?? '#2b2118', hairStyle: 'short', bag: look?.bag ?? null, bagColor: look?.bagColor ?? '#333333', scale: 1 };
  drawUpper(ctx, l, null, [0.6, -0.8], player);
}

// Arme mit Waffe (lokal: +x = Blickrichtung, rechte Hand bei +y). attack.t läuft von der Dauer auf 0.
function drawArmsWithWeapon(ctx, weapon, attack, arm, swing) {
  if (weapon === 'pistol' || weapon === 'smg' || weapon === 'shotgun') { // beide Hände vorn an der Waffe, Rückstoß beim Schuss
    const kick = attack?.kind === 'shot' ? -1.5 : 0;
    const len = weapon === 'pistol' ? 6 : weapon === 'smg' ? 9 : 15;
    arm(-1, 5.6 + kick, 0.1); arm(1, 6.4 + kick, 1.9);
    ctx.fillStyle = weapon === 'shotgun' ? '#5a3b22' : '#1b1d21'; ctx.fillRect(4 + kick, -0.3, len, 2.4); edge(ctx, 0.3, 'rgba(0,0,0,0.7)');
    ctx.fillStyle = 'rgba(255,255,255,0.18)'; ctx.fillRect(4.4 + kick, -0.15, len - 0.8, 0.5);                  // Glanz auf dem Schlitten
    if (weapon === 'smg') { ctx.fillStyle = '#1b1d21'; ctx.fillRect(7 + kick, 1.8, 2, 3); }
    if (weapon === 'shotgun') { ctx.fillStyle = '#1b1d21'; ctx.fillRect(9 + kick, -0.1, 10, 1.6); }
    return;
  }
  const u = attack ? 1 - Math.max(0, attack.t) / 0.22 : 0; // 0 … 1 im Schlag
  if (weapon === 'bat') { // Schläger schwingt von hinten links nach vorn rechts
    const a = attack ? -1.6 + u * 2.6 : 2.3;
    arm(-1, -swing * 0.3, -5.4);
    arm(1, 3, 5);
    ctx.save(); ctx.translate(3, 5); ctx.rotate(a);
    ctx.fillStyle = '#9b6a3a'; ctx.fillRect(0, -1, 16, 2.2); ctx.fillStyle = '#7a4f28'; ctx.fillRect(10, -1.4, 6, 2.8);
    ctx.fillStyle = 'rgba(255,255,255,0.2)'; ctx.fillRect(1, -0.8, 14, 0.5);
    ctx.restore();
    return;
  }
  const reach = attack ? Math.sin(u * Math.PI) * 7 : 0; // Faust/Messer stößt vor und zurück
  arm(-1, -swing * 0.3 + 1.8, -5.3);
  arm(1, 4 + reach, 4.4);
  if (weapon === 'knife') { ctx.fillStyle = '#c9ced4'; ctx.fillRect(4.5 + reach, 3.8, 5, 1.2); ctx.fillStyle = '#222'; ctx.fillRect(3.5 + reach, 3.7, 1.4, 1.4); }
}

// Armhaltungen je Tätigkeit (lokal: +x Blickrichtung, rechte Hand +y); t läuft für Bewegung, arm(side, x, y) zeichnet
// einen Arm zur Hand
export const ACT_ARMS = {
  smoke: (ctx, look, t, arm) => { // Zigarette zum Mund und zurück
    const up = Math.max(0, Math.sin(t * 0.8)) > 0.6;
    arm(-1, 1.5, -5.4); arm(1, up ? 3.5 : 2, up ? 1.5 : 5.4);
    ctx.fillStyle = '#f2f2f2'; ctx.fillRect(up ? 3.8 : 2.4, up ? 1 : 4.9, 2.2, 0.7);
    ctx.fillStyle = '#ff7a2a'; ctx.fillRect(up ? 5.8 : 4.4, up ? 1 : 4.9, 0.8, 0.7);
  },
  drink: (ctx, look, t, arm) => { // Flasche (Berliner Späti)
    const up = Math.sin(t * 0.6) > 0.7;
    arm(-1, 1.5, -5.4); arm(1, up ? 3.5 : 3, up ? 1 : 4.8);
    ctx.fillStyle = (t | 0) % 2 ? '#3f6b2a' : '#6b4a1e'; ctx.fillRect(up ? 3.2 : 2.6, up ? -0.2 : 4, 4.2, 1.6);
  },
  chat: (ctx, look, t, arm) => { // Gestikulieren
    const a = Math.sin(t * 3.1) * 2, b = Math.sin(t * 2.3 + 1) * 2;
    arm(-1, 3.5 + a, -3.8); arm(1, 3.5 + b, 3.8);
  },
  wait: (ctx, look, t, arm) => { // aufs Handy schauen
    arm(-1, 3.5, -1.2); arm(1, 3.5, 1.2);
    ctx.fillStyle = '#1a1c20'; ctx.fillRect(3.6, -1.4, 1.6, 2.8);
    ctx.fillStyle = Math.sin(t * 0.3) > -0.5 ? '#9fd3ff' : '#35495e'; ctx.fillRect(3.8, -1.1, 1.1, 2.2);
  },
  music: (ctx, look, t, arm) => { // Gitarre
    ctx.fillStyle = '#8a5a2b'; ctx.beginPath(); ctx.ellipse(3.2, 2.2, 3.2, 2.4, 0.5, 0, TAU); ctx.fill(); edge(ctx, 0.35);
    ctx.fillStyle = '#2b1a0c'; ctx.beginPath(); ctx.arc(3.2, 2.2, 0.9, 0, TAU); ctx.fill();
    ctx.fillStyle = '#5a3a1b'; ctx.fillRect(3.5, -6, 1.1, 6.5);
    arm(-1, 4, -4 + Math.sin(t * 4) * 0.6); arm(1, 3.8, 2 + Math.sin(t * 9) * 0.8);
  },
  queue: (ctx, look, t, arm) => { arm(-1, 1.4, -5.3); arm(1, 1.4, 5.3); },
  browse: (ctx, look, t, arm) => { arm(-1, -2, -3); arm(1, -2, 3); }, // Hände hinter dem Rücken
};
