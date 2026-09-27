// Codes im Kartenformat (web/data/city.json), geteilt zwischen Build (tools/osm) und Spiel.
export const ROAD_CLASSES = [null, 'motorway', 'trunk', 'primary', 'secondary', 'tertiary', 'unclassified', 'residential',
  'living_street', 'service', 'pedestrian', 'track', 'busway'];
export const ROAD_CLASS = Object.fromEntries(ROAD_CLASSES.map((c, i) => [c, i]).filter(([c]) => c));
export const TRAFFIC_MAX_CLASS = 8; // Autobahn … Spielstraße: KI-Verkehr; Zufahrten/Fußgängerzonen nicht
export const AREA_KIND = { rail: 0, plaza: 1, allotments: 2, cemetery: 3, grass: 4, pitch: 5, sand: 6, wood: 7 };
export const BUILDING_KIND = { house: 0, public: 1, industrial: 2, church: 3, small: 4, spaeti: 5, warehouse: 6 };

// Bäume: Der Stamm (Kreis mit TREE_TRUNK_M) darf nie auf einer Fahrbahn bis einschließlich dieser Klasse stehen
// (Zufahrten eingeschlossen). Die Krone darf überragen. Der Karten-Build setzt das durch, Tests prüfen es.
export const TREE_TRUNK_M = 0.5;
export const TREE_FREE_MAX_CLASS = 9;

// POI-Kategorien (Geschäfte, Gastronomie, Haltestellen …). Reihenfolge = Code im Kartenformat.
export const POI_CATS = ['ubahn', 'sbahn', 'bahn', 'bus', 'mall', 'supermarket', 'shop', 'food', 'drink', 'cafe', 'service', 'culture', 'hotel'];
export const POI_CAT = Object.fromEntries(POI_CATS.map((c, i) => [c, i]));
