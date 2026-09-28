// Codes im Kartenformat (web/data/city.json), geteilt zwischen Build (tools/osm) und Spiel.
export const ROAD_CLASSES = [null, 'motorway', 'trunk', 'primary', 'secondary', 'tertiary', 'unclassified', 'residential',
  'living_street', 'service', 'pedestrian', 'track', 'busway'];
export const ROAD_CLASS = Object.fromEntries(ROAD_CLASSES.map((c, i) => [c, i]).filter(([c]) => c));
export const TRAFFIC_MAX_CLASS = 8; // Autobahn … Spielstraße: KI-Verkehr; Zufahrten/Fußgängerzonen nicht
export const AREA_KIND = { rail: 0, plaza: 1, allotments: 2, cemetery: 3, grass: 4, pitch: 5, sand: 6, wood: 7, bridge: 8 };
// Wandzüge (Kachelformat): Art der Wand; 1 = Stadtgrenze (unsichtbare Wand)
export const WALL_KIND = { other: 0, border: 1, quay: 2, rail: 3, railing: 4, fence: 5 };
// Stadtmöbel (Kachelformat): Bänke (auch Picknicktische), Fahrradständer, Mülleimer
export const FURN_KIND = { bench: 0, bicycle: 1, bin: 2 };
// Einwohnerdichte: Rasterweite (m) des Dichtegitters je Kachel
export const DENS_CELL_M = 64;
// Geschätzte Kfz je Werktag nach Straßenklasse, wo keine Zählung vorliegt (Index = Klasse)
export const DTV_ESTIMATE = [0, 45000, 35000, 22000, 14000, 7000, 2500, 900, 200, 150, 0, 0, 300];
export const BUILDING_KIND = { house: 0, public: 1, industrial: 2, church: 3, small: 4, spaeti: 5, warehouse: 6 };

// Bäume: Der Stamm (Kreis mit TREE_TRUNK_M) darf nie auf einer Fahrbahn bis einschließlich dieser Klasse stehen
// (Zufahrten eingeschlossen). Die Krone darf überragen. Der Karten-Build setzt das durch, Tests prüfen es.
export const TREE_TRUNK_M = 0.5;
export const TREE_FREE_MAX_CLASS = 9;

// POI-Kategorien (Geschäfte, Gastronomie, Haltestellen …). Reihenfolge = Code im Kartenformat.
export const POI_CATS = ['ubahn', 'sbahn', 'bahn', 'bus', 'mall', 'supermarket', 'shop', 'food', 'drink', 'cafe', 'service', 'culture', 'hotel'];
export const POI_CAT = Object.fromEntries(POI_CATS.map((c, i) => [c, i]));

// Straßenquerschnitt (city.json v2): Parkstreifen-Art je Seite und Fahrbahnbelag.
export const PARK = { none: 0, lane: 1, half: 2, kerb: 3 };
export const PARK_ORIENT = ['parallel', 'diagonal', 'perpendicular'];
export const SURFACE = { asphalt: 0, cobble: 1, plates: 2, unpaved: 3 };

// Baumgattungen (Berliner Baumbestand, botanischer Gattungsname); Index = Code im Kartenformat, 0 = sonstige.
export const TREE_GENERA = ['sonstige', 'Tilia', 'Acer', 'Platanus', 'Aesculus', 'Quercus', 'Robinia', 'Betula', 'Populus',
  'Carpinus', 'Fraxinus', 'Prunus', 'Salix', 'Sorbus', 'Crataegus', 'Ulmus', 'Nadel'];

// Aussehen eines Gebäudes aus OSM (Kachelformat: 7. Feld „look“ als Bitfeld, 8./9. Feld Dach-/Fassadenfarbe als
// RGB + 1, 0 = unbekannt). Unbekanntes schätzt das Spiel aus Art, Höhe, Grundriss und Bezirk (roofs.js).
export const ROOF_SHAPE = { none: 0, flat: 1, gabled: 2, hipped: 3, pyramidal: 4, mansard: 5, skillion: 6, dome: 7, round: 8 };
export const ROOF_MAT = { none: 0, tiles: 1, concrete: 2, tar: 3, metal: 4, glass: 5, slate: 6, green: 7 };
export const WALL_MAT = { none: 0, plaster: 1, brick: 2, concrete: 3, glass: 4, wood: 5, stone: 6, metal: 7 };
export const BUILDING_SUB = { none: 0, villa: 1, terrace: 2, apartments: 3, commercial: 4, civic: 5, garage: 6 };
// Bezirke in der Reihenfolge von index.json (alphabetisch); im Bitfeld als Index + 1
export const BEZIRKE = ['Charlottenburg-Wilmersdorf', 'Friedrichshain-Kreuzberg', 'Lichtenberg', 'Marzahn-Hellersdorf', 'Mitte',
  'Neukölln', 'Pankow', 'Reinickendorf', 'Spandau', 'Steglitz-Zehlendorf', 'Tempelhof-Schöneberg', 'Treptow-Köpenick'];

export function packLook({ shape = 0, rmat = 0, wmat = 0, sub = 0, bez = 0 }) {
  return shape | (rmat << 4) | (wmat << 7) | (sub << 10) | (bez << 13);
}
export function unpackLook(v = 0) {
  return { shape: v & 15, rmat: (v >> 4) & 7, wmat: (v >> 7) & 7, sub: (v >> 10) & 7, bez: (v >> 13) & 15 };
}

// Ebenen (Höhenordnung aus OSM, tools/osm/levels.mjs): 0 = Boden, ≥ 1 Brücken, < 0 offene Unterführungen.
// In Kachel-Bitfeldern als 3 Bit mit Vorzeichen.
export const LVL_MIN = -2, LVL_MAX = 3;
export const packLvl = (l) => (l | 0) & 7;
export const unpackLvl = (v) => (((v | 0) & 7) << 29) >> 29;
