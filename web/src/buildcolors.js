// Farben eines Gebäudes (rein, deterministisch aus Seed und OSM-Aussehen): Fassade, Dachhaut, flache Dachmitte.
// Reihenfolge: OSM-Farbe (building:colour/roof:colour) > OSM-Material > Bezirk/Stil > Gebäudeart. Die Paletten bilden
// ab, was man in Berlin von oben sieht: Biberschwanz- und Pfannenziegel, Schiefer/Anthrazit, Bitumen und Kies auf
// Flachdächern, Kupfer auf Kirchen und Kuppeln, Gründächer; Stuckfassaden im Altbau, farbig sanierte Platte im Osten,
// helle Putzvillen und Backstein.
import { BUILDING_KIND as K, ROOF_MAT as RM, WALL_MAT as WM, BUILDING_SUB as SUB } from './citycodes.js';
import { lookOf, PITCHED } from './roofs.js';

export const PAL = {
  tile: ['#9c5a44', '#a8664c', '#8f5240', '#b0725a', '#8a4a3a', '#a4553f', '#7e4636', '#b8694c'],
  slate: ['#4b4f55', '#55595f', '#44474d', '#5c5f63', '#3d4045'],
  brownTile: ['#6e5548', '#5e4a40'],
  flat: ['#7d7b78', '#86837e', '#737271', '#8e8a84', '#6b6a6a', '#9a958d', '#807d77'],
  metal: ['#8d959c', '#7f8890', '#9aa1a6'],
  green: ['#6a8a4e', '#5f7f45', '#738f55'],
  copper: ['#5f8f7f', '#6c9a86', '#57857a'],
  glassRoof: ['#8fb0c4'],
  // Fassaden
  stucco: ['#d8c7a3', '#e3d5b5', '#cdb58e', '#c9b9a4', '#d9c2b0', '#bfb6a4', '#e6dcc6', '#cfc0a0', '#b9a78d', '#d7cbb0', '#c7b49a', '#dcc9a8', '#a9b0a8', '#c9bfae', '#d3b9a0', '#b8b3a6'],
  platte: ['#e7d9a8', '#d7b98c', '#c9d6dc', '#e3c3a4', '#d9d3c6', '#b9cfb4', '#e6e1d3', '#cfd9e4', '#e8cfa0', '#d4c9e0'],
  villa: ['#ece6da', '#e5dcc8', '#d9d2c4', '#efe8d8', '#e8dfcc', '#a8674c', '#dcd3c0'],
  modern: ['#d8d6d0', '#c9c7c2', '#e2e0da', '#b8bcc0', '#a9aeb3', '#cfcac0'],
  brick: ['#9b5b43', '#a8674c', '#8c4f3b', '#b07156', '#7f4a3a'],
  concrete: ['#b5b3ad', '#a9a8a3', '#c2bfb7'],
  glassWall: ['#8fa4b4', '#7e95a6', '#9db1bf'],
  wood: ['#9a7a5a', '#8b6c4e'],
  stone: ['#c8bca4', '#bfb39a'],
  metalWall: ['#9aa3aa'],
};
const OTHER_WALLS = {
  [K.public]: ['#a3b1a0', '#9aa3ab', '#b0aaa0', '#c2b8a6', '#c9c0ae', '#b7a38c'],
  [K.industrial]: ['#8f9aa6', '#9aa0a3', '#858d93', '#a3a8a0', '#9a6a55'],
  [K.church]: ['#a0674e', '#8f5a45', '#b8ab94'],
  [K.small]: ['#9d968c', '#8c877f', '#a79f92'],
  [K.spaeti]: ['#d9c46a'],
  [K.warehouse]: ['#7f8a93'],
};

export const hex = (rgb) => '#' + ((1 << 24) | rgb).toString(16).slice(1);
export function mix(hexA, hexB, t) {
  const a = parseInt(hexA.slice(1), 16), b = parseInt(hexB.slice(1), 16);
  const c = (sh) => Math.round(((a >> sh) & 255) * (1 - t) + ((b >> sh) & 255) * t);
  return `#${((1 << 24) | (c(16) << 16) | (c(8) << 8) | c(0)).toString(16).slice(1)}`;
}
const pick = (pal, n) => pal[Math.abs(n) % pal.length];

export function wallColor(b, facade) {
  const lk = lookOf(b), s = b.seed;
  if (b.wallRgb >= 0) return hex(b.wallRgb);
  switch (lk.wmat) {
    case WM.brick: return pick(PAL.brick, s);
    case WM.concrete: return pick(facade === 'platte' ? PAL.platte : PAL.concrete, s);
    case WM.glass: return pick(PAL.glassWall, s);
    case WM.wood: return pick(PAL.wood, s);
    case WM.stone: return pick(PAL.stone, s);
    case WM.metal: return pick(PAL.metalWall, s);
    default: break;
  }
  if (b.kind !== K.house) return pick(OTHER_WALLS[b.kind] ?? PAL.stucco, s);
  if (facade === 'platte') return pick(PAL.platte, s);
  if (facade === 'modern') return pick(PAL.modern, s);
  if (lk.sub === SUB.villa || lk.sub === SUB.terrace || (b.meters ?? 0) <= 9) return pick(PAL.villa, s);
  return pick(PAL.stucco, s);
}

// Dachhaut (geneigte Flächen bzw. ganze Fläche) und flache Mitte (Berliner Dach/Mansarde, Flachdach)
export function roofColors(b, style, wall) {
  const lk = lookOf(b), s = (b.seed >> 3) >>> 0, r = (s % 997) / 997;
  const flat = mix(pick(PAL.flat, s >> 4), wall, 0.12);
  const banded = style === 'berlin' || style === 'mansard'; // geneigter Rand, flache Mitte
  if (b.roofRgb >= 0) { const c = hex(b.roofRgb); return { skin: c, center: banded ? flat : c }; }
  let skin = null;
  switch (lk.rmat) {
    case RM.tiles: skin = pick(PAL.tile, s); break;
    case RM.concrete: skin = pick(PAL.brownTile, s); break;
    case RM.tar: skin = pick(PAL.flat, s); break;
    case RM.metal: skin = pick(PAL.metal, s); break;
    case RM.glass: skin = pick(PAL.glassRoof, s); break;
    case RM.slate: skin = pick(PAL.slate, s); break;
    case RM.green: skin = pick(PAL.green, s); break;
    default: break;
  }
  if (!skin) {
    if (b.kind === K.spaeti) skin = mix(wall, '#6f6a62', 0.4);
    else if (style === 'corrugated') skin = b.kind === K.warehouse ? '#7f8a93' : r < 0.2 ? '#8a5a4a' : pick(PAL.metal, s);
    else if (style === 'dome') skin = r < 0.7 ? pick(PAL.copper, s) : pick(PAL.slate, s);
    else if (b.kind === K.church) skin = r < 0.55 ? pick(PAL.slate, s) : r < 0.8 ? pick(PAL.copper, s) : pick(PAL.tile, s);
    else if (style === 'mansard') skin = r < 0.75 ? pick(PAL.slate, s) : pick(PAL.tile, s);
    else if (style === 'round') skin = pick(PAL.metal, s);
    else if (style === 'berlin') skin = r < 0.55 ? pick(PAL.tile, s) : r < 0.85 ? pick(PAL.slate, s) : pick(PAL.brownTile, s);
    else if (PITCHED.has(style)) skin = b.kind === K.small ? mix(wall, '#6f6a62', 0.5)
      : r < 0.56 ? pick(PAL.tile, s) : r < 0.86 ? pick(PAL.slate, s) : pick(PAL.brownTile, s);
    else { // Flachdach: Bitumen/Kies, auf großen Dächern manchmal begrünt
      const big = (b.bbox?.w ?? 0) * (b.bbox?.h ?? 0) > 60000;
      skin = big && r < 0.07 ? pick(PAL.green, s) : big && b.kind === K.public && r < 0.15 ? pick(PAL.metal, s) : flat;
    }
  }
  return { skin, center: banded ? flat : skin };
}
