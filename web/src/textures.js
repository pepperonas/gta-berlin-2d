// Bodentexturen als Kachelmuster in Weltkoordinaten (10 px = 1 m): einmal je Zeichenfläche erzeugt, deterministisch
// (geseedeter Zufall), dann nur noch als fillStyle/strokeStyle benutzt – keine Mehrkosten je Form.
// Kacheln werden doppelt aufgelöst gezeichnet und per Muster-Transformation halbiert (schärfer bei Zoom > 1).
import { mulberry32 } from './rng.js';
import { makeCanvas } from './lighting.js';
import { ART } from './visualstyle.js';

const RES = 2;

function tile(size, seed, paint) {
  const c = makeCanvas(size * RES, size * RES), g = c.getContext('2d');
  g.scale?.(RES, RES);
  paint(g, mulberry32(seed), size);
  return c;
}

function speckle(g, rnd, size, n, colors, rMin, rMax) {
  for (let i = 0; i < n; i++) {
    g.fillStyle = colors[Math.floor(rnd() * colors.length)];
    const r = rMin + rnd() * (rMax - rMin), x = rnd() * size, y = rnd() * size;
    for (const ox of [-size, 0, size]) for (const oy of [-size, 0, size]) { // auch links/oben nahtlos
      if (x + ox + r < 0 || x + ox - r > size || y + oy + r < 0 || y + oy - r > size) continue;
      g.beginPath(); g.arc(x + ox, y + oy, r, 0, Math.PI * 2); g.fill();
    }
  }
}

// Gehweg: heller Stein mit feiner Körnung und kaum sichtbaren, unregelmäßigen Platten (ein strenges Raster läge
// achsparallel über schräg verlaufenden Straßen und wirkte wie ein gekachelter Platz)
function paintSidewalk(g, rnd, size) {
  g.fillStyle = ART.sidewalk; g.fillRect(0, 0, size, size);
  for (let i = 0; i < 90; i++) { // weiche, große Farbunterschiede (Platten verschiedenen Alters)
    const v = Math.round((rnd() - 0.5) * 10);
    g.fillStyle = `rgba(${166 + v},${162 + v},${151 + v},0.2)`;
    const x = rnd() * size, y = rnd() * size, w = 10 + rnd() * 18, h = 10 + rnd() * 18;
    for (const [ox, oy] of [[0, 0], [-size, 0], [0, -size], [-size, -size]]) g.fillRect(x + ox, y + oy, w, h);
  }
  speckle(g, rnd, size, 380, ['rgba(255,255,255,0.09)', 'rgba(0,0,0,0.08)', 'rgba(60,50,40,0.06)'], 0.25, 0.75);
}

// Berliner Gehweg (Detailstufe zu Fuß): große Granitplatten in der Mitte, Mosaikpflaster zu beiden Seiten
function paintGranite(g, rnd, size) {
  g.fillStyle = '#b3b0a8'; g.fillRect(0, 0, size, size);
  const n = 2, s = size / n;
  for (let i = 0; i < n; i++) for (let j = 0; j < n; j++) { const v = Math.round((rnd() - 0.5) * 16); g.fillStyle = `rgb(${179 + v},${176 + v},${168 + v})`; g.fillRect(i * s + 0.6, j * s + 0.6, s - 1.2, s - 1.2); }
  speckle(g, rnd, size, 220, ['rgba(255,255,255,0.12)', 'rgba(0,0,0,0.1)'], 0.2, 0.6);
}
function paintMosaic(g, rnd, size) {
  g.fillStyle = '#6e6c67'; g.fillRect(0, 0, size, size);
  const s = size / 8;
  for (let i = 0; i < 8; i++) for (let j = 0; j < 8; j++) { const v = Math.round((rnd() - 0.5) * 30); g.fillStyle = `rgb(${128 + v},${125 + v},${118 + v})`; g.fillRect(i * s + 0.35 + rnd() * 0.3, j * s + 0.35 + rnd() * 0.3, s - 0.8, s - 0.8); }
}

function paintAsphalt(g, rnd, size) {
  g.fillStyle = ART.asphalt; g.fillRect(0, 0, size, size);
  speckle(g, rnd, size, 48, ['rgba(113,121,124,0.025)', 'rgba(9,21,28,0.025)'], 12, 46);
  speckle(g, rnd, size, 6400, ['rgba(235,225,203,0.065)', 'rgba(0,0,0,0.12)', 'rgba(141,160,168,0.08)'], 0.16, 0.55);
  // Vereinzelte feine Haarrisse, innerhalb der Kachel auslaufend.
  g.strokeStyle = 'rgba(12,18,21,0.17)'; g.lineWidth = 0.65;
  for (let k = 0; k < 3; k++) { let x = 30 + rnd() * (size - 90), y = 30 + rnd() * (size - 90);
    g.beginPath(); g.moveTo(x, y); for (let j = 0; j < 6; j++) { x += 3 + rnd() * 5; y += (rnd() - 0.5) * 12; g.lineTo(x, y); } g.stroke(); }

}

function paintGrass(base, blot, dark) {
  return (g, rnd, size) => {
    g.fillStyle = base; g.fillRect(0, 0, size, size);
    speckle(g, rnd, size, 18, [blot, dark], 4, 11);
    speckle(g, rnd, size, 800, ['rgba(226,225,157,0.13)', 'rgba(17,45,34,0.13)'], 0.25, 0.75);
    g.strokeStyle = 'rgba(215,220,159,0.12)'; g.lineWidth = 0.45; g.beginPath();
    for (let i = 0; i < 450; i++) { const x = 2 + rnd() * (size - 4), y = 3 + rnd() * (size - 6); g.moveTo(x, y); g.lineTo(x + 0.5, y - 1.7); } g.stroke();
  };
}

function paintWood(g, rnd, size) {
  g.fillStyle = ART.forest; g.fillRect(0, 0, size, size);
  speckle(g, rnd, size, 40, ['#3f5c3b', '#506946', '#344f36'], 4, 10); // Unterholz
  speckle(g, rnd, size, 120, ['rgba(80,60,30,0.18)', 'rgba(0,0,0,0.12)'], 0.5, 1.6);
}

function paintSand(g, rnd, size) {
  g.fillStyle = '#d6c48d'; g.fillRect(0, 0, size, size);
  speckle(g, rnd, size, 300, ['rgba(120,90,40,0.15)', 'rgba(255,255,255,0.18)'], 0.3, 0.9);
}

// Platz: Granitplatten im Läuferverband, jede Reihe mit eigenem Versatz und unregelmäßig langen Platten (kein
// Schachbrett), wenig Farbunterschied, feine Fugen, Körnung; die Plattenlängen jeder Reihe gehen nahtlos über die Kante.
function paintPlaza(g, rnd, size) {
  g.fillStyle = '#7f7c76'; g.fillRect(0, 0, size, size); // Fugen
  const rows = 6, h = size / rows;
  for (let r = 0; r < rows; r++) {
    let x = rnd() * size;
    const end = x + size;
    while (x < end - 0.5) {
      const w = Math.min(end - x, 7 + rnd() * 9);
      const v = Math.round((rnd() - 0.5) * 9), warm = Math.round(rnd() * 3);
      g.fillStyle = `rgb(${150 + v + warm},${147 + v},${140 + v - warm})`;
      for (const ox of [0, -size]) g.fillRect(x + ox + 0.35, r * h + 0.35, w - 0.7, h - 0.7);
      x += w;
    }
  }
  speckle(g, rnd, size, 300, ['rgba(255,255,255,0.08)', 'rgba(0,0,0,0.07)', 'rgba(70,60,50,0.05)'], 0.2, 0.6);
}

function paintRailBed(g, rnd, size) {
  g.fillStyle = '#7b756c'; g.fillRect(0, 0, size, size);
  speckle(g, rnd, size, 500, ['rgba(40,35,30,0.22)', 'rgba(255,250,240,0.12)'], 0.3, 0.9); // Schotter
}

function paintCobble(g, rnd, size) {
  g.fillStyle = '#44464a'; g.fillRect(0, 0, size, size);
  const w = 6, h = 5; // Großpflaster in versetzten Reihen
  for (let row = 0, y = 0; y < size; row++, y += h) for (let x = (row % 2) * (w / 2) - w; x < size; x += w) {
    const v = Math.round((rnd() - 0.5) * 16);
    g.fillStyle = `rgb(${86 + v},${88 + v},${92 + v})`;
    g.beginPath();
    if (g.roundRect) g.roundRect(x + 0.5, y + 0.5, w - 1, h - 1, 1.2); else g.rect(x + 0.5, y + 0.5, w - 1, h - 1);
    g.fill();
  }
}

// Dachkies/Bitumen: nur Körnung auf durchsichtigem Grund (über die Dachfarbe gelegt)
function paintGravel(g, rnd, size) {
  speckle(g, rnd, size, 420, ['rgba(0,0,0,0.13)', 'rgba(255,255,255,0.10)', 'rgba(60,40,20,0.08)'], 0.3, 0.9);
}


// Transparente Fassadenhaut: unter den Fenstern, ohne die OSM-Grundfarbe zu ersetzen.
function paintPlaster(g, rnd, size) {
  speckle(g, rnd, size, 24, ['rgba(100,82,57,0.025)', 'rgba(255,248,229,0.04)'], 3, 14);
  speckle(g, rnd, size, 650, ['rgba(45,36,26,0.08)', 'rgba(255,251,234,0.1)'], 0.16, 0.55);
}
function paintBrick(g, rnd, size) {
  paintPlaster(g, rnd, size);
  g.strokeStyle = 'rgba(230,216,190,0.22)'; g.lineWidth = 0.45; g.beginPath();
  for (let y = 0, row = 0; y <= size; y += 4, row++) {
    g.moveTo(0, y); g.lineTo(size, y);
    for (let x = row % 2 ? 4 : 0; x <= size; x += 8) { g.moveTo(x, y); g.lineTo(x, y + 4); }
  } g.stroke();
}

function paintWaterRipples(g, rnd, size) {
  g.lineWidth = 0.6;
  for (let row = 0; row < size / 8; row++) {
    const phase = rnd() * Math.PI * 2;
    g.strokeStyle = `rgba(211,235,227,${0.12 + rnd() * 0.3})`; g.beginPath();
    for (let x = 0; x <= size; x += 3) {
      const y = row * 8 + 4 + Math.sin(x / size * Math.PI * 6 + phase) * 1.1;
      if (x === 0) g.moveTo(x, y); else g.lineTo(x, y);
    } g.stroke();
  }
}

// kind → [Kachelgröße (px), Seed, Maler]
const DEFS = {
  sidewalk: [192, 11, paintSidewalk],
  granite: [60, 23, paintGranite],
  mosaic: [16, 24, paintMosaic],
  asphalt: [256, 12, paintAsphalt],
  cobble: [24, 13, paintCobble],
  grass: [96, 14, paintGrass(ART.grass, ART.grassLight, ART.grassDark)],
  allotments: [96, 15, paintGrass('#758b57', '#829764', '#657b4c')],
  cemetery: [96, 16, paintGrass('#657e51', '#718a5b', '#526b46')],
  pitch: [96, 17, paintGrass('#64804c', '#6c8954', '#587345')],
  wood: [96, 18, paintWood],
  sand: [64, 19, paintSand],
  plaza: [64, 20, paintPlaza],
  rail: [64, 21, paintRailBed],
  gravel: [48, 22, paintGravel],
  plaster: [64, 29, paintPlaster], brick: [64, 30, paintBrick],
  waterRipples: [256, 31, paintWaterRipples],
};
export const TEXTURE_KINDS = Object.keys(DEFS);

const cache = new WeakMap();
// Muster für eine Zeichenfläche; fällt ohne Canvas (Tests) auf null zurück – dann zeichnet render.js einfarbig.
export function texture(ctx, kind) {
  let m = cache.get(ctx);
  if (!m) { m = new Map(); cache.set(ctx, m); }
  if (m.has(kind)) return m.get(kind);
  const [size, seed, paint] = DEFS[kind];
  let pat = null;
  try {
    pat = ctx.createPattern(tile(size, seed, paint), 'repeat') ?? null;
    if (pat && typeof DOMMatrix !== 'undefined' && pat.setTransform) pat.setTransform(new DOMMatrix().scale(1 / RES));
  } catch { pat = null; }
  m.set(kind, pat);
  return pat;
}
