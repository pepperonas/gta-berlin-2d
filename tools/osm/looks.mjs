// Gebäude-Aussehen aus OSM-Tags: Dachform, Dach- und Fassadenmaterial, Gebäudetyp, Farben (roof:colour,
// building:colour als Name oder Hex). Rein, ohne Kartenbezug; der Bezirk kommt im Build dazu.
import { ROOF_SHAPE as R, ROOF_MAT as RM, WALL_MAT as WM, BUILDING_SUB as SUB } from '../../web/src/citycodes.js';

const SHAPES = {
  flat: R.flat, gabled: R.gabled, saltbox: R.gabled, double_saltbox: R.gabled, quadruple_saltbox: R.gabled, gabled_height_moved: R.gabled,
  hipped: R.hipped, 'half-hipped': R.hipped, side_hipped: R.hipped, 'side_half-hipped': R.hipped, 'hipped-and-gabled': R.hipped, many: R.hipped,
  pyramidal: R.pyramidal, cone: R.pyramidal, mansard: R.mansard, gambrel: R.mansard, skillion: R.skillion, lean_to: R.skillion,
  dome: R.dome, onion: R.dome, round: R.round, crosspitched: R.hipped,
};
const RMATS = {
  roof_tiles: RM.tiles, tile: RM.tiles, tiles: RM.tiles, clay: RM.tiles, shingles: RM.tiles, concrete: RM.concrete, eternit: RM.concrete,
  tar_paper: RM.tar, tar: RM.tar, asphalt: RM.tar, gravel: RM.tar, metal: RM.metal, metal_sheet: RM.metal, copper: RM.metal, zinc: RM.metal,
  glass: RM.glass, slate: RM.slate, grass: RM.green, plants: RM.green, green: RM.green,
};
const WMATS = {
  plaster: WM.plaster, render: WM.plaster, brick: WM.brick, clinker: WM.brick, concrete: WM.concrete, panel: WM.concrete, glass: WM.glass,
  mirror: WM.glass, wood: WM.wood, timber_framing: WM.wood, stone: WM.stone, sandstone: WM.stone, limestone: WM.stone, masonry: WM.stone,
  metal: WM.metal, steel: WM.metal,
};
const SUBS = {
  detached: SUB.villa, house: SUB.villa, semidetached_house: SUB.villa, bungalow: SUB.villa, villa: SUB.villa, farm: SUB.villa,
  terrace: SUB.terrace, apartments: SUB.apartments, residential: SUB.apartments, dormitory: SUB.apartments,
  retail: SUB.commercial, commercial: SUB.commercial, office: SUB.commercial, supermarket: SUB.commercial, hotel: SUB.commercial, kiosk: SUB.commercial,
  school: SUB.civic, university: SUB.civic, college: SUB.civic, hospital: SUB.civic, public: SUB.civic, civic: SUB.civic, government: SUB.civic,
  kindergarten: SUB.civic, museum: SUB.civic, train_station: SUB.civic,
  garage: SUB.garage, garages: SUB.garage, carport: SUB.garage, shed: SUB.garage,
};

// Farbnamen, wie sie in Berlin tatsächlich vorkommen (CSS-Namen und deutsche Ausreißer)
const NAMED = {
  black: 0x2b2b2b, white: 0xf2f0ea, grey: 0x8a8a8a, gray: 0x8a8a8a, darkgrey: 0x5e5e5e, darkgray: 0x5e5e5e, lightgrey: 0xc4c4c4,
  lightgray: 0xc4c4c4, silver: 0xbfbfbf, dimgray: 0x696969, dimgrey: 0x696969, red: 0xa04a3a, darkred: 0x7c3328, maroon: 0x7a3a2c,
  brown: 0x7b5a45, saddlebrown: 0x7d4a26, sienna: 0x8f5238, salmon: 0xd08672, orange: 0xd98a45, yellow: 0xe3d27f, lightyellow: 0xf1e9c0,
  beige: 0xe0d3b4, tan: 0xcbb08a, wheat: 0xe6d3a8, ivory: 0xf4f0de, cream: 0xefe5c8, green: 0x5d8a4e, darkgreen: 0x3f6a3a,
  olive: 0x7c7b45, blue: 0x5a7aa8, lightblue: 0xa9c4da, navy: 0x2f3f66, teal: 0x4f8a86, pink: 0xd9a8a8, purple: 0x7a5a8a,
  gold: 0xc9a84a, khaki: 0xc6ba86, anthracite: 0x3b3e42, rot: 0xa04a3a, grau: 0x8a8a8a, weiss: 0xf2f0ea, schwarz: 0x2b2b2b,
};

// Farbe → RGB-Zahl, -1 wenn unbekannt
export function parseColour(s) {
  if (!s) return -1;
  const v = String(s).trim().toLowerCase().split(';')[0].replace(/\s+/g, '');
  if (NAMED[v] !== undefined) return NAMED[v];
  let m = v.match(/^#?([0-9a-f]{6})$/);
  if (m) return parseInt(m[1], 16);
  m = v.match(/^#?([0-9a-f])([0-9a-f])([0-9a-f])$/);
  if (m) return parseInt(m[1] + m[1] + m[2] + m[2] + m[3] + m[3], 16);
  return -1;
}

const first = (s) => (s ? String(s).toLowerCase().split(';')[0].trim() : '');

export function buildingLook(t) {
  return {
    shape: SHAPES[first(t['roof:shape'])] ?? 0,
    rmat: RMATS[first(t['roof:material'])] ?? 0,
    wmat: WMATS[first(t['building:material'] ?? t['building:facade:material'])] ?? 0,
    sub: SUBS[first(t.building && t.building !== 'no' ? t.building : t['building:part'])] ?? 0,
    rc: parseColour(t['roof:colour']) + 1,
    fc: parseColour(t['building:colour'] ?? t['building:facade:colour']) + 1,
  };
}
