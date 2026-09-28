// Zentrale Spielkonstanten. Welt-Einheit = Pixel bei Zoom 1; 10 px = 1 m (muss zum Karten-Build passen,
// tools/osm/build.mjs --scale). Die Weltgröße kommt aus der Karte (city.width/height).
export const PX_PER_M = 10;

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

// Poller und Schranken: ab dieser Aufprallgeschwindigkeit (px/s, ≈ 16 km/h) fährt ein Auto sie um
export const KNOCK = { speed: 45, slow: 0.9, damage: 3 };

export const PLAYER = { radius: 7, walk: 80, run: 155, enterDist: 40 };
export const PED = { radius: 6, walk: 36, run: 120 };

export const MISSION = {
  timeLimit: 120,     // Rückfall; das echte Limit berechnet der Karten-Build aus der Route
  loadTime: 2.0,      // A gedrückt halten zum Einladen
  reward: 500,
  timeBonus: 5,       // € je Restsekunde
  zoneRadius: 46,
  stopSpeed: 35,      // px/s – so langsam muss man in der Zone sein
  giverRadius: 30,
};

// Bevölkerung lebt nur um die Kamera: Erzeugen im Ring spawnMin…spawnMax, Abbau jenseits despawn (px).
// Am Straßenrand geparkte Autos (auf OSM-Parkstreifen): Anteil belegter Stellplätze, Umkreis um die Kamera (px).
export const PARKED = { share: 0.6, radius: 1300, despawn: 1800 };

export const TRAFFIC = { cars: 22, pedestrians: 55, spawnMin: 750, spawnMax: 1800, despawn: 2400 };

// Darstellung: reale Gebäudehöhen (1 m = 10 px) werden für die Schrägansicht gestaucht.
// quality: 'high' | 'low' – fällt automatisch auf 'low', wenn das Zeichnen im Median über budgetMs braucht
// (zurück erst unter recoverMs); 'low' verzichtet auf Baumschatten und darauf, dass Häuser Licht in der Lichtkarte verdecken.
export const RENDER = { heightScale: 0.5, budgetMs: 14, recoverMs: 8, sampleFrames: 120 };

// Spieluhr: 1 Echtsekunde = 1 Spielminute (ein Tag dauert 24 min); neues Spiel beginnt am Nachmittag.
export const CLOCK = { minutesPerSecond: 1, start: 16 * 60 };

export const SPEED_TO_KMH = 0.36;
