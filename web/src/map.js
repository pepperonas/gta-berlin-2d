// Stadtkarte: Raster aus Straßen und Häuserblöcken, deterministisch aus LAYOUT + Seed.
// Austauschbar: eigenes LAYOUT (eine Zeichenkette je Blockreihe) an buildCity übergeben.
//   B = Wohnblock   S = Späti (Auftraggeber + Parkplatz)   L = Lagerhalle (Abholort)
//   P = Park        W = Wasser (Spree, mit Kai)
import { TILE, GRID, ROAD_W, BLOCK_W, COLS, ROWS, MAP_W, MAP_H, WORLD_W, WORLD_H } from './config.js';
import { mulberry32 } from './rng.js';

export const T = { ROAD: 0, SIDEWALK: 1, BUILDING: 2, GRASS: 3, WATER: 4, PLAZA: 5 };

export const LAYOUT = [
  'BBPPBBB',
  'BSBPBBB',
  'BBBBBBB',
  'WWWWWWW',
  'BBBPBBB',
  'BBBBBLB',
];

export const STREETS_V = [
  'Am Mauerweg', 'Adalbertstraße', 'Mariannenstraße', 'Manteuffelstraße',
  'Wrangelstraße', 'Falckensteinstraße', 'Schlesische Straße', 'Treptower Ring',
];
export const STREETS_H = [
  'Karl-Marx-Allee', 'Boxhagener Straße', 'Köpenicker Straße', 'Holzmarktufer',
  'Paul-Lincke-Ufer', 'Sonnenallee', 'Hermannstraße',
];
const DISTRICTS = ['Friedrichshain', 'Friedrichshain', 'Mitte', 'Spree', 'Kreuzberg', 'Neukölln'];

const WALLS = ['#c9b79c', '#b8a488', '#d6c7a1', '#9aa3ab', '#a89080', '#8f9aa6', '#c4a484', '#b3aa9a', '#a3b1a0', '#c7a9a0'];

export function buildCity(opts = {}) {
  const layout = opts.layout ?? LAYOUT;
  if (layout.length !== ROWS || layout.some((row) => row.length !== COLS)) {
    throw new Error(`LAYOUT muss ${ROWS} Zeilen à ${COLS} Zeichen haben`);
  }
  const rng = mulberry32(opts.seed ?? 1989);
  const tiles = new Uint8Array(MAP_W * MAP_H).fill(T.ROAD);
  const fill = (tx, ty, tw, th, t) => {
    for (let y = ty; y < ty + th; y++) for (let x = tx; x < tx + tw; x++) tiles[y * MAP_W + x] = t;
  };

  const blocks = [], buildings = [], waters = [], trees = [], crates = [], parked = [];
  const places = {};

  const addBuilding = (tx, ty, tw, th, props = {}) => {
    fill(tx, ty, tw, th, T.BUILDING);
    const b = {
      x: tx * TILE, y: ty * TILE, w: tw * TILE, h: th * TILE,
      height: props.height ?? rng.int(48, 130),
      wall: props.wall ?? rng.pick(WALLS),
      kind: props.kind ?? 'house',
      seed: Math.floor(rng() * 1e9),
    };
    buildings.push(b);
    return b;
  };

  for (let r = 0; r < ROWS; r++) {
    for (let c = 0; c < COLS; c++) {
      const type = layout[r][c];
      const tx = c * GRID + ROAD_W, ty = r * GRID + ROAD_W;
      const block = {
        c, r, type, tx, ty,
        x: tx * TILE, y: ty * TILE, w: BLOCK_W * TILE, h: BLOCK_W * TILE,
      };
      block.ring = { x: block.x + TILE / 2, y: block.y + TILE / 2, w: block.w - TILE, h: block.h - TILE };
      blocks.push(block);
      fill(tx, ty, BLOCK_W, BLOCK_W, T.SIDEWALK);
      const ix = tx + 1, iy = ty + 1, n = BLOCK_W - 2; // Innenfläche 12×12

      if (type === 'W') {
        fill(ix, iy, n, n, T.WATER);
        waters.push({ x: ix * TILE, y: iy * TILE, w: n * TILE, h: n * TILE });
      } else if (type === 'P') {
        fill(ix, iy, n, n, T.GRASS);
        const mid = Math.floor(n / 2);
        fill(ix, iy + mid, n, 1, T.SIDEWALK);
        fill(ix + mid, iy, 1, n, T.SIDEWALK);
        for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) {
          if (Math.abs(x - mid) <= 1 || Math.abs(y - mid) <= 1) continue;
          if (rng() < 0.16) {
            trees.push({
              x: (ix + x) * TILE + TILE / 2 + rng.range(-5, 5),
              y: (iy + y) * TILE + TILE / 2 + rng.range(-5, 5),
              r: 9, size: rng.range(14, 20),
            });
          }
        }
      } else if (type === 'L') {
        addBuilding(ix, iy, n, 5, { kind: 'warehouse', height: 92, wall: '#7f8a93' });
        fill(ix, iy + 6, n, n - 6, T.PLAZA);
        const yard = { x: ix * TILE, y: (iy + 6) * TILE, w: n * TILE, h: (n - 6) * TILE };
        places.pickup = { x: yard.x + yard.w / 2, y: yard.y + yard.h / 2 - 8, name: 'Lagerhalle Neukölln' };
        for (const [cx, cy] of [[yard.x + 14, yard.y + 14], [yard.x + 38, yard.y + 14], [yard.x + yard.w - 14, yard.y + 14], [yard.x + yard.w - 14, yard.y + 38]]) {
          crates.push({ x: cx - 10, y: cy - 10, w: 20, h: 20 });
        }
      } else if (type === 'S') {
        // Fester Zuschnitt: links unten Späti, rechts unten Parkplatz.
        const sx = 5, sy = 5;
        addBuilding(ix, iy, n, sy, { height: 110 });
        addBuilding(ix, iy + sy + 1, sx, n - sy - 1, { kind: 'spaeti', height: 52, wall: '#d9c46a' });
        const lot = { tx: ix + sx + 1, ty: iy + sy + 1, tw: n - sx - 1, th: n - sy - 1 };
        fill(lot.tx, lot.ty, lot.tw, lot.th, T.PLAZA);
        const lotPx = { x: lot.tx * TILE, y: lot.ty * TILE, w: lot.tw * TILE, h: lot.th * TILE };
        const giverX = ix * TILE + (sx * TILE) / 2;
        const sidewalkY = (ty + BLOCK_W - 1) * TILE + TILE / 2;
        places.giver = { x: giverX, y: sidewalkY - 2, name: 'Späti „Zum Kiez“' };
        places.playerSpawn = { x: giverX + 34, y: sidewalkY };
        places.dropoff = { x: lotPx.x + lotPx.w / 2 - 22, y: lotPx.y + lotPx.h / 2, name: 'Parkplatz am Späti' };
        places.playerCar = { x: lotPx.x + lotPx.w - 22, y: lotPx.y + lotPx.h / 2 + 10, angle: Math.PI / 2 };
        parked.push({ x: lotPx.x + 22, y: lotPx.y + 30, angle: Math.PI / 2 });
      } else {
        // Wohnblock: bis zu vier Häuser mit Durchgängen (1 Kachel).
        const sx = rng.int(4, 7), sy = rng.int(4, 7);
        const lots = [
          [ix, iy, sx, sy], [ix + sx + 1, iy, n - sx - 1, sy],
          [ix, iy + sy + 1, sx, n - sy - 1], [ix + sx + 1, iy + sy + 1, n - sx - 1, n - sy - 1],
        ];
        const mode = rng.int(0, 3);
        if (mode === 0) lots.forEach((l) => addBuilding(...l));
        else if (mode === 1) { addBuilding(ix, iy, n, sy); addBuilding(...lots[2]); addBuilding(...lots[3]); }
        else if (mode === 2) { addBuilding(ix, iy, sx, n); addBuilding(...lots[1]); addBuilding(...lots[3]); }
        else { addBuilding(ix, iy, n, sy); addBuilding(ix, iy + sy + 1, n, n - sy - 1); }
      }
    }
  }

  for (const k of ['giver', 'pickup', 'dropoff', 'playerSpawn', 'playerCar']) {
    if (!places[k]) throw new Error(`LAYOUT braucht einen Block für "${k}" (S und L)`);
  }

  const B = 200; // unsichtbare Wände um die Welt
  const border = [
    { x: -B, y: -B, w: WORLD_W + 2 * B, h: B }, { x: -B, y: WORLD_H, w: WORLD_W + 2 * B, h: B },
    { x: -B, y: 0, w: B, h: WORLD_H }, { x: WORLD_W, y: 0, w: B, h: WORLD_H },
  ];

  const city = {
    tiles, blocks, buildings, waters, trees, crates, parked, places, border,
    vRoads: Array.from({ length: COLS + 1 }, (_, i) => i * GRID * TILE),
    hRoads: Array.from({ length: ROWS + 1 }, (_, j) => j * GRID * TILE),
    roadW: ROAD_W * TILE,
  };
  city.rects = [...buildings, ...waters, ...crates, ...border];
  return city;
}

export function tileAt(city, x, y) {
  const tx = Math.floor(x / TILE), ty = Math.floor(y / TILE);
  if (tx < 0 || ty < 0 || tx >= MAP_W || ty >= MAP_H) return T.BUILDING;
  return city.tiles[ty * MAP_W + tx];
}

// Index der Straße, auf der x bzw. y liegt – oder -1.
export function roadIndexAt(starts, roadW, v) {
  for (let i = 0; i < starts.length; i++) if (v >= starts[i] && v < starts[i] + roadW) return i;
  return -1;
}

export function locationName(city, x, y) {
  const i = roadIndexAt(city.vRoads, city.roadW, x);
  const j = roadIndexAt(city.hRoads, city.roadW, y);
  if (i >= 0 && j >= 0) return `${STREETS_V[i]} / ${STREETS_H[j]}`;
  if (i >= 0) return STREETS_V[i];
  if (j >= 0) return STREETS_H[j];
  const r = Math.floor(y / (GRID * TILE));
  return DISTRICTS[Math.max(0, Math.min(DISTRICTS.length - 1, r))];
}
