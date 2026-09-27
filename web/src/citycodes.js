// Codes im Kartenformat (web/data/city.json), geteilt zwischen Build (tools/osm) und Spiel.
export const ROAD_CLASSES = [null, 'motorway', 'trunk', 'primary', 'secondary', 'tertiary', 'unclassified', 'residential',
  'living_street', 'service', 'pedestrian', 'track', 'busway'];
export const ROAD_CLASS = Object.fromEntries(ROAD_CLASSES.map((c, i) => [c, i]).filter(([c]) => c));
export const TRAFFIC_MAX_CLASS = 8; // Autobahn … Spielstraße: KI-Verkehr; Zufahrten/Fußgängerzonen nicht
export const AREA_KIND = { rail: 0, plaza: 1, allotments: 2, cemetery: 3, grass: 4, pitch: 5, sand: 6, wood: 7 };
// Wandzüge (Kachelformat): Art der Wand; 1 = Stadtgrenze (unsichtbare Wand)
export const WALL_KIND = { other: 0, border: 1, quay: 2, rail: 3, railing: 4, fence: 5 };
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
