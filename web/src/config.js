// Zentrale Spielkonstanten. Welt-Einheit = Pixel bei Zoom 1; 10 px ≈ 1 m.
export const TILE = 24;
export const ROAD_W = 4;              // Straßenbreite in Kacheln (2 Fahrspuren à 2 Kacheln)
export const GRID = 18;               // Abstand zweier Straßen in Kacheln
export const BLOCK_W = GRID - ROAD_W; // Häuserblock inkl. Gehweg-Ring
export const COLS = 7;                // Blöcke horizontal
export const ROWS = 6;                // Blöcke vertikal
export const MAP_W = COLS * GRID + ROAD_W; // Kacheln
export const MAP_H = ROWS * GRID + ROAD_W;
export const WORLD_W = MAP_W * TILE;
export const WORLD_H = MAP_H * TILE;

export const DT = 1 / 60;             // fester Simulationsschritt

export const CAR = {
  length: 42, width: 20,
  maxSpeed: 330, maxReverse: 110,
  accel: 240, brake: 560, handbrake: 260,
  drag: 0.32, grip: 9, handbrakeGrip: 1.4,
  steerRate: 2.8,
  health: 100,
  damageThreshold: 70, damageFactor: 0.14,
  restitution: 0.3,
};

export const PLAYER = { radius: 7, walk: 80, run: 155, enterDist: 40 };
export const PED = { radius: 6, walk: 36, run: 120 };

export const MISSION = {
  timeLimit: 120,     // Sekunden ab Annahme
  loadTime: 2.0,      // A gedrückt halten zum Einladen
  reward: 500,
  timeBonus: 5,       // € je Restsekunde
  zoneRadius: 46,
  stopSpeed: 35,      // px/s – so langsam muss man in der Zone sein
  giverRadius: 30,
};

export const TRAFFIC = { cars: 16, pedestrians: 48 };

export const SPEED_TO_KMH = 0.36;
