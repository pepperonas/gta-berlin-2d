// Fahrzeugoberflächen in Draufsicht (+x vorn). Nur Darstellung; alle Maße liegen innerhalb der bestehenden Hülle.
import { shade } from './assets.js';

const SPORT = new Set(['sportwagen', 'supersport', 'leichtbau', 'heckcoupe', 'gtcoupe', 'roadster', 'leichtcoupe', 'coupe', 'elektrosport']);
const CLASSIC = new Set(['oldtimer', 'zweitakter', 'kleinbus', 'niva', 'gklasse', 'defender', 'musclecar']);
const TWO_DOOR = new Set([...SPORT, 'musclecar', 'zweitakter', 'niva']);
function box(g, x, y, w, h, r, fill, stroke = null) {
  if (!(w > 0 && h > 0)) return;
  r = Math.min(r, w / 2, h / 2);
  g.beginPath(); g.moveTo(x + r, y); g.arcTo(x + w, y, x + w, y + h, r);
  g.arcTo(x + w, y + h, x, y + h, r); g.arcTo(x, y + h, x, y, r); g.arcTo(x, y, x + w, y, r); g.closePath();
  g.fillStyle = fill; g.fill();
  if (stroke) { g.strokeStyle = stroke; g.lineWidth = 0.35; g.stroke(); }
}
function poly(g, pts, fill, stroke = null) {
  g.beginPath(); g.moveTo(...pts[0]); for (const p of pts.slice(1)) g.lineTo(...p); g.closePath();
  g.fillStyle = fill; g.fill();
  if (stroke) { g.strokeStyle = stroke; g.lineWidth = 0.35; g.stroke(); }
}
function line(g, pts, color, width = 0.35) {
  g.beginPath(); g.moveTo(...pts[0]); for (const p of pts.slice(1)) g.lineTo(...p);
  g.strokeStyle = color; g.lineWidth = width; g.stroke();
}
function finish(g, color, y, width, wrecked = false) {
  const gr = g.createLinearGradient(0, y, 0, y + width);
  for (const [p, s] of [[0, -0.48], [0.12, -0.12], [0.26, 0.28], [0.44, 0.08], [0.72, -0.08], [0.94, -0.36], [1, -0.55]]) gr.addColorStop(p, shade(color, wrecked ? Math.min(0, s) : s));
  return gr;
}
function outline(g, L, W, boxy, sporty) {
  const end = boxy ? 0.43 : sporty ? 0.29 : 0.34, waist = sporty ? 0.425 : 0.47;
  g.beginPath(); g.moveTo(-L / 2, -W * end);
  g.bezierCurveTo(-L * 0.48, -W * 0.48, -L * 0.34, -W * 0.51, -L * 0.25, -W * 0.5);
  g.bezierCurveTo(-L * 0.1, -W * waist, L * 0.1, -W * waist, L * 0.27, -W * 0.5);
  g.bezierCurveTo(L * 0.4, -W * 0.5, L * 0.5, -W * 0.43, L / 2, -W * end);
  g.lineTo(L / 2, W * end);
  g.bezierCurveTo(L * 0.5, W * 0.43, L * 0.4, W * 0.5, L * 0.27, W * 0.5);
  g.bezierCurveTo(L * 0.1, W * waist, -L * 0.1, W * waist, -L * 0.25, W * 0.5);
  g.bezierCurveTo(-L * 0.34, W * 0.51, -L * 0.48, W * 0.48, -L / 2, W * end); g.closePath();
}

// Front, Dach und Heckscheibe bilden eine zusammenhängende Kabine. Normierte Maße verhindern negative Dachbreiten.
export function paintPassenger(g, model, body, L, W, sh, wrecked) {
  const sport = SPORT.has(model), classic = CLASSIC.has(model), boxy = sh.r <= 3.5 || sh.van;
  const rear = -L / 2, front = L / 2;
  g.save();
  outline(g, L, W, boxy, sport);
  g.fillStyle = finish(g, body, -W / 2, W, wrecked); g.fill();
  g.strokeStyle = shade(body, -0.65); g.lineWidth = 0.65; g.stroke();
  g.save(); g.clip();
  // Schultern, Kotflügel und Schweller: gerichtete Reflexe statt eines flachen Rechtecks.
  for (const side of [-1, 1]) {
    line(g, [[rear + 3, side * W * 0.4], [-L * 0.22, side * W * 0.445], [L * 0.23, side * W * 0.445], [front - 3, side * W * 0.37]], side < 0 && !wrecked ? 'rgba(255,255,255,0.42)' : 'rgba(0,0,0,0.32)', 0.55);
    for (const ax of [-L * 0.29, L * 0.29]) {
      line(g, [[ax - 3, side * W * 0.47], [ax - 1, side * W * 0.405], [ax + 3, side * W * 0.42]], side < 0 ? 'rgba(255,255,255,0.2)' : 'rgba(0,0,0,0.2)', 0.65);
    }
    box(g, -L * 0.18, side < 0 ? -W * 0.495 : W * 0.46, L * 0.35, 0.5, 0.2, '#24282b');
  }
  if (sh.stripes) for (const y of [-2.15, 0.75]) box(g, rear + 2, y, L - 4, 1.4, 0, wrecked ? '#514a43' : '#d5d4cb');
  let cr = Math.max(-0.39, -0.5 + (sh.back ?? 5) / L), cf = 0.5 - (sh.front ?? 8) / L;
  if (sh.vents === 'mid') { cr = -0.16; cf = 0.26; }
  if (model === 'gtcoupe' || model === 'musclecar') { cr = -0.32; cf = 0.12; }
  if (sh.open) { cr = -0.25; cf = 0.14; }
  if (sh.bed) { cr = -0.05; cf = 0.29; }
  if (sh.van) { cr = -0.43; cf = 0.36; }
  const ca = cr * L, cb = cf * L, span = cb - ca;
  const ra = ca + span * (sh.van ? 0.05 : sh.rails ? 0.12 : 0.23), rb = cb - span * (sh.van ? 0.18 : 0.27);
  const outer = W * 0.385, roofHalf = W * (sport ? 0.265 : 0.29);
  const glass = g.createLinearGradient(ca, -outer, cb, outer);
  glass.addColorStop(0, '#111d25'); glass.addColorStop(0.42, wrecked ? '#262320' : '#466171'); glass.addColorStop(0.53, '#263944'); glass.addColorStop(1, '#101b23');
  const window = (pts) => {
    poly(g, pts, glass, '#101619');
    if (!wrecked) {
      g.save(); g.clip();
      poly(g, [[ca - 4, -W], [ca + 1, -W], [cb + 5, W], [cb + 2, W]], 'rgba(208,232,239,0.17)');
      line(g, [[ca - 1, -W], [cb + 7, W]], 'rgba(230,247,250,0.28)', 0.3); g.restore();
    }
  };
  // Ladefläche liegt wirklich hinter der Fahrerkabine, mit Radkästen, Längssicken und Heckklappe.
  if (sh.bed) {
    box(g, rear + 2, -W * 0.36, ca - rear - 2.8, W * 0.72, 1, '#292d30', '#898b88');
    for (let y = -W * 0.25; y < W * 0.3; y += 1.6) line(g, [[rear + 3, y], [ca - 2, y]], '#51575a', 0.45);
    for (const side of [-1, 1]) box(g, rear + 6, side < 0 ? -W * 0.37 : W * 0.26, 5.5, W * 0.12, 1, '#414548');
  }
  window([[ca, -outer * 0.79], [ra, -roofHalf], [ra, roofHalf], [ca, outer * 0.79]]);
  window([[rb, -roofHalf], [cb, -outer * 0.84], [cb, outer * 0.84], [rb, roofHalf]]);
  for (const side of [-1, 1]) {
    window([[ca + 0.7, side * outer], [cb - 1, side * outer], [rb, side * (roofHalf + 0.55)], [ra, side * (roofHalf + 0.55)]]);
    if (!TWO_DOOR.has(model) && !sh.bed && !sh.van) line(g, [[ra + (rb - ra) * 0.53, side * roofHalf], [ra + (rb - ra) * 0.53, side * outer]], '#171c20', 0.9);
    // Türfugen reichen nur über die Flanke, nicht quer durch das Dach.
    const doors = TWO_DOOR.has(model) || sh.bed ? [ca + 1, cb] : [ca + 1, (ra + rb) / 2, cb];
    for (const x of doors) line(g, [[x, side * outer], [x - 0.4, side * W * 0.46]], shade(body, -0.48), 0.35);
    for (const x of doors.slice(0, -1)) box(g, x + 1.1, side < 0 ? -W * 0.433 : W * 0.411, 1.9, 0.45, 0.2, classic || sh.chrome ? '#c5c8c6' : shade(body, 0.3));
  }
  if (sh.open) {
    box(g, ra - 0.9, -roofHalf, rb - ra + 1.4, roofHalf * 2, 1.2, '#141b1e');
    for (const side of [-1, 1]) {
      box(g, ra + 0.4, side * W * 0.15 - 1.7, 4.2, 3.4, 0.85, '#865641', '#b98260');
      box(g, ra + 0.3, side * W * 0.15 - 1.35, 1, 2.7, 0.35, '#382b27');
      line(g, [[ra - 0.7, side * W * 0.1], [ra - 0.7, side * W * 0.23]], '#b1b7b9', 0.65);
    }
    g.strokeStyle = '#afb2ad'; g.lineWidth = 0.45; g.beginPath(); g.arc(rb - 0.5, -W * 0.15, 1.3, 0, Math.PI * 2); g.stroke();
  } else {
    box(g, ra, -roofHalf, rb - ra, roofHalf * 2, sport ? 1.3 : 0.8, sh.glassRoof ? glass : finish(g, sh.twoTone ?? sh.roof ?? body, -roofHalf, roofHalf * 2, wrecked), 'rgba(12,19,22,0.65)');
    line(g, [[ra + 0.9, -roofHalf + 0.65], [rb - 0.9, -roofHalf + 0.65]], 'rgba(255,255,255,0.35)', 0.35);
    if (sh.stripes) for (const y of [-2.15, 0.75]) box(g, ra + 0.2, y, rb - ra - 0.4, 1.4, 0, wrecked ? '#514a43' : '#d5d4cb');
    if (sh.glassRoof) line(g, [[(ra + rb) / 2, -roofHalf], [(ra + rb) / 2, roofHalf]], '#11181c', 0.7);
    if (sh.rails) for (const side of [-1, 1]) { line(g, [[ra, side * (roofHalf - 0.2)], [rb, side * (roofHalf - 0.2)]], '#20282c', 1); line(g, [[ra + 1, side * (roofHalf - 0.35)], [rb - 1, side * (roofHalf - 0.35)]], '#afb7b6', 0.35); }
    if (sh.van) for (let y = -roofHalf + 2; y < roofHalf - 1; y += 2.2) line(g, [[ra + 2, y], [rb - 2, y]], 'rgba(0,0,0,0.17)');
  }
  // Scheibenwischer, Heizdrähte und Haubenspalte.
  for (const side of [-1, 1]) line(g, [[rb + 0.65, side * W * 0.06], [rb + 1.1, side * W * 0.23]], '#10181c', 0.45);
  if (!sh.van && !sh.open) for (let x = ca + 0.7; x < ra - 0.4; x += 0.85) line(g, [[x, -roofHalf * 0.85], [x, roofHalf * 0.85]], 'rgba(179,157,116,0.3)', 0.18);
  for (const side of [-1, 1]) line(g, [[cb + 0.8, side * outer * 0.85], [front - 3, side * W * 0.28]], 'rgba(0,0,0,0.3)', 0.4);
  line(g, [[cb + 0.7, -outer * 0.83], [cb + 0.7, outer * 0.83]], shade(body, -0.3));
  if (sh.vents) {
    const a = sh.vents === 'hood' ? cb + 2 : rear + 3, b = sh.vents === 'hood' ? front - 5 : ca - 1;
    if (b > a) { box(g, a, -W * 0.2, b - a, W * 0.4, 0.6, '#242b2d'); for (let x = a + 0.8; x < b; x += 1.3) line(g, [[x, -W * 0.17], [x, W * 0.17]], '#667072', 0.35); }
  }
  // Stoßfänger, Grill, Kennzeichen und Abgasanlage (E-Autos ohne Endrohre).
  box(g, front - 1.7, -W * 0.24, 1.15, W * 0.48, 0.3, classic ? '#bbc1c0' : '#1b242a');
  if (!sh.glassRoof) for (let y = -W * 0.19; y < W * 0.2; y += 1) line(g, [[front - 1.5, y], [front - 0.7, y]], '#737f82', 0.3);
  box(g, rear + 0.3, -W * 0.26, 1, W * 0.52, 0.3, sh.chrome ? '#c8cdcb' : '#272c2d');
  for (const x of [rear + 0.4, front - 0.85]) { box(g, x, -1.55, 0.65, 3.1, 0.1, '#d5d9d4'); box(g, x, -1.55, 0.65, 0.5, 0, '#2f5780'); }
  if (model !== 'elektro' && model !== 'elektrosport') for (const side of sport || model === 'musclecar' ? [-1, 1] : [1]) box(g, rear, side * W * 0.25 - 0.65, 1.5, 1.3, 0.5, '#a7b0b1', '#172024');
  g.restore();
  // Spiegel sitzen an den A-Säulen; dunkle Unterseite, lackiertes Gehäuse und Glas.
  for (const side of [-1, 1]) {
    line(g, [[cb - 1.5, side * W * 0.4], [cb - 2, side * W * 0.54]], '#20272b', 0.7);
    box(g, cb - 3, side < 0 ? -W * 0.57 : W * 0.48, 2.4, W * 0.09, 0.55, shade(body, -0.1), '#1c252a');
    line(g, [[cb - 2.8, side * W * 0.53], [cb - 1.2, side * W * 0.53]], '#b1c4ca', 0.4);
  }
  if (sh.wing || model === 'supersport') { box(g, rear + 2.2, -W * 0.28, 1.2, W * 0.56, 0.1, '#1b2227'); box(g, rear + 1.5, -W * 0.43, 1.5, W * 0.86, 0.45, finish(g, body, -W / 2, W), '#10171b'); }
  if (sh.spare) { box(g, rear - 1.7, -2.7, 2.8, 5.4, 1, '#1b2023', '#616668'); box(g, rear - 1.2, -1.6, 1.4, 3.2, 0.6, '#666d6b'); }
  if (model === 'taxi' && !wrecked) { box(g, (ra + rb) / 2 - 1.2, -3.2, 2.4, 6.4, 0.55, '#eccc69', '#554a2c'); line(g, [[(ra + rb) / 2, -1.8], [(ra + rb) / 2, 1.8]], '#443b24', 0.5); }
  paintLamps(g, model, L, W, false, wrecked);
  if (wrecked) {
    for (const [x, y, r] of [[-L * 0.2, -2, 4], [L * 0.24, 2, 3]]) { g.fillStyle = 'rgba(12,10,8,0.65)'; g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.fill(); }
    line(g, [[cb - 1, -4], [rb + 0.8, -1], [cb - 1, 1], [rb + 1, 4]], '#88918b', 0.3);
  }
  g.restore();
}

// Dieselben Gehäuse für Sprite und Bremslicht: aktive Leuchten übermalen keine Modellgeometrie.
export function paintLamps(g, model, L, W, braking, wrecked = false) {
  const classic = CLASSIC.has(model), sport = SPORT.has(model), nose = L / 2, rear = -L / 2;
  for (const side of [-1, 1]) {
    const y = side * W * 0.3;
    box(g, nose - 3.4, y - 1.65, 2.7, 3.3, classic ? 1.3 : 0.6, '#151e24');
    if (classic) {
      for (const dy of model === 'musclecar' ? [-0.8, 0.8] : [0]) { g.beginPath(); g.arc(nose - 2.05, y + dy, model === 'musclecar' ? 0.65 : 1.1, 0, Math.PI * 2); g.fillStyle = wrecked ? '#494438' : '#e5ddba'; g.fill(); }
    } else {
      poly(g, [[nose - 3, y - 1.15], [nose - 1.3, y - 0.95], [nose - 1.1, y + 1.25], [nose - (sport ? 2.2 : 3), y + 0.9]], wrecked ? '#474947' : '#a9c0c8');
      if (!wrecked) line(g, [[nose - 1.45, y - 1.05], [nose - 1.3, y + 0.9], [nose - 2.2, y + 1.05]], '#f2f8ed', 0.45);
    }
    box(g, rear + 0.5, y - 1.6, 1.6, 3.2, 0.45, '#271d21');
    if (model === 'musclecar') for (let i = -1; i <= 1; i++) box(g, rear + 0.85, y + i * 0.9 - 0.26, 0.9, 0.52, 0.1, wrecked ? '#463130' : braking ? '#ff6652' : '#b53131');
    else box(g, rear + 0.85, y - 1.3, 0.8, 2.6, 0.25, wrecked ? '#463130' : braking ? '#ff6652' : '#b53131');
  }
  if (!wrecked && (model === 'elektro' || model === 'elektrosport')) line(g, [[rear + 1.2, -W * 0.24], [rear + 1.2, W * 0.24]], braking ? '#ff6652' : '#8f2529', 0.5);
}
