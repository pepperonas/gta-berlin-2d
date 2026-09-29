// Bodentexturen als Kachelmuster in Weltkoordinaten (10 px = 1 m): einmal je Zeichenfläche erzeugt, deterministisch
// (geseedeter Zufall), dann nur noch als fillStyle/strokeStyle benutzt – keine Mehrkosten je Form.
// Kacheln werden doppelt aufgelöst gezeichnet und per Muster-Transformation halbiert (schärfer bei Zoom > 1).
import { mulberry32 } from './rng.js';
import { makeCanvas } from './lighting.js';

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
    for (const [ox, oy] of [[0, 0], [-size, 0], [0, -size], [-size, -size]]) { // nahtlos über die Kachelkante
      g.beginPath(); g.arc(x + ox, y + oy, r, 0, Math.PI * 2); g.fill();
      if (x + r < size && y + r < size) break;
    }
  }
}

// Gehweg: heller Stein mit feiner Körnung und kaum sichtbaren, unregelmäßigen Platten (ein strenges Raster läge
// achsparallel über schräg verlaufenden Straßen und wirkte wie ein gekachelter Platz)
function paintSidewalk(g, rnd, size) {
  g.fillStyle = '#9d9990'; g.fillRect(0, 0, size, size);
  for (let i = 0; i < 26; i++) { // weiche, große Farbunterschiede (Platten verschiedenen Alters)
    const v = Math.round((rnd() - 0.5) * 10);
    g.fillStyle = `rgba(${157 + v},${153 + v},${144 + v},0.55)`;
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
  g.fillStyle = '#3b3e43'; g.fillRect(0, 0, size, size);
  speckle(g, rnd, size, 420, ['rgba(255,255,255,0.05)', 'rgba(0,0,0,0.12)', 'rgba(255,255,255,0.03)'], 0.25, 0.7);
}

function paintGrass(base, blot, dark) {
  return (g, rnd, size) => {
    g.fillStyle = base; g.fillRect(0, 0, size, size);
    speckle(g, rnd, size, 18, [blot, dark], 4, 11);
    speckle(g, rnd, size, 260, ['rgba(255,255,160,0.07)', 'rgba(0,30,0,0.10)'], 0.4, 1.1);
  };
}

function paintWood(g, rnd, size) {
  g.fillStyle = '#3e7631'; g.fillRect(0, 0, size, size);
  speckle(g, rnd, size, 40, ['#356a2a', '#467f38', '#2f5e26'], 4, 10); // Unterholz
  speckle(g, rnd, size, 120, ['rgba(80,60,30,0.18)', 'rgba(0,0,0,0.12)'], 0.5, 1.6);
}

function paintSand(g, rnd, size) {
  g.fillStyle = '#d6c48d'; g.fillRect(0, 0, size, size);
  speckle(g, rnd, size, 300, ['rgba(120,90,40,0.15)', 'rgba(255,255,255,0.18)'], 0.3, 0.9);
}

function paintPlaza(g, rnd, size) {
  g.fillStyle = '#8e8b85'; g.fillRect(0, 0, size, size);
  const p = 8; // kleinere Platten
  for (let y = 0; y < size; y += p) for (let x = 0; x < size; x += p) {
    const v = Math.round((rnd() - 0.5) * 12);
    g.fillStyle = `rgb(${142 + v},${139 + v},${133 + v})`; g.fillRect(x + 0.5, y + 0.5, p - 1, p - 1);
  }
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

// kind → [Kachelgröße (px), Seed, Maler]
const DEFS = {
  sidewalk: [64, 11, paintSidewalk],
  granite: [60, 23, paintGranite],
  mosaic: [16, 24, paintMosaic],
  asphalt: [64, 12, paintAsphalt],
  cobble: [24, 13, paintCobble],
  grass: [96, 14, paintGrass('#5d9340', '#67a049', '#4f8237')],
  allotments: [96, 15, paintGrass('#6c9851', '#7aa75c', '#5b8646')],
  cemetery: [96, 16, paintGrass('#5b8a47', '#669652', '#4c7a3b')],
  pitch: [96, 17, paintGrass('#4d8c3c', '#579846', '#447e35')],
  wood: [96, 18, paintWood],
  sand: [64, 19, paintSand],
  plaza: [64, 20, paintPlaza],
  rail: [64, 21, paintRailBed],
  gravel: [48, 22, paintGravel],
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
