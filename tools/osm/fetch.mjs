// Holt die Rohdaten für die Karte: LOR-Grenzen (Geoportal Berlin, WFS) und OpenStreetMap (Overpass-API).
// Ergebnis in data/raw/ (gitignored). Danach: node tools/osm/build.mjs
import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

export const AREAS = ['0210', '0220', '0230', '0810']; // Kreuzberg Nord/Süd/Ost, Neukölln (= Nord-Neukölln)
export const MARGIN_M = 250;
const WFS = 'https://gdi.berlin.de/services/wfs/lor_2021?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature'
  + '&TYPENAMES=lor_2021:c_lor_pgr_2021&OUTPUTFORMAT=application/json&SRSNAME=EPSG:4326';
const OVERPASS = process.env.OVERPASS_URL ? [process.env.OVERPASS_URL]
  : ['https://overpass-api.de/api/interpreter', 'https://overpass.private.coffee/api/interpreter', 'https://overpass.kumi.systems/api/interpreter'];

const raw = fileURLToPath(new URL('../../data/raw/', import.meta.url));

async function get(url, init) {
  const r = await fetch(url, init);
  if (!r.ok) throw new Error(`${url}: HTTP ${r.status} ${(await r.text()).slice(0, 300)}`);
  return r.text();
}

export function bboxOf(features, marginM = MARGIN_M) {
  let s = 90, w = 180, n = -90, e = -180;
  for (const f of features) for (const poly of f.geometry.coordinates) for (const ring of poly) for (const [x, y] of ring) {
    s = Math.min(s, y); n = Math.max(n, y); w = Math.min(w, x); e = Math.max(e, x);
  }
  const dLat = marginM / 110574, dLon = marginM / (111320 * Math.cos(((s + n) / 2) * Math.PI / 180));
  return [s - dLat, w - dLon, n + dLat, e + dLon];
}

export function overpassQuery([s, w, n, e]) {
  const bb = `${s.toFixed(6)},${w.toFixed(6)},${n.toFixed(6)},${e.toFixed(6)}`;
  return `[out:json][timeout:300][maxsize:1073741824][bbox:${bb}];
(
  way[highway];
  way[building];
  way[landuse]; way[leisure]; way[natural]; way[waterway]; way[amenity=parking];
  way[railway~"^(rail|light_rail|subway|platform)$"]; way[man_made=bridge];
  relation[type=multipolygon][building]; relation[type=multipolygon][landuse];
  relation[type=multipolygon][leisure]; relation[type=multipolygon][natural];
  relation[type=multipolygon][waterway]; relation[type=multipolygon][place=square];
  node[natural=tree];
  node[place~"^(neighbourhood|quarter|suburb|square)$"];
  node[shop]; way[shop]; relation[shop];
  node[amenity]; way[amenity]; relation[amenity];
  node[tourism]; way[tourism];
  node[railway~"^(station|halt)$"]; node[public_transport=station]; node[highway=bus_stop];
  node["addr:housenumber"]; way["addr:housenumber"]; relation["addr:housenumber"];
);
out body;
>;
out skel qt;`;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  await mkdir(raw, { recursive: true });
  console.log('LOR-Prognoseräume (Geoportal Berlin) …');
  const lor = JSON.parse(await get(WFS));
  lor.features = lor.features.filter((f) => AREAS.includes(f.properties.pgr_id));
  if (lor.features.length !== AREAS.length) throw new Error('LOR-Gebiete unvollständig');
  await writeFile(raw + 'lor.json', JSON.stringify(lor));
  const bbox = bboxOf(lor.features);
  console.log('Overpass', bbox.map((v) => v.toFixed(5)).join(','), '… (dauert 1–3 min)');
  const t0 = Date.now();
  // Öffentliche Overpass-Server sind oft ausgelastet (504/429): mehrere Server, je zwei Versuche.
  let osm = null, used = null;
  for (let attempt = 0; attempt < 2 && !osm; attempt++) for (const url of OVERPASS) {
    try {
      osm = await get(url, { method: 'POST', body: new URLSearchParams({ data: overpassQuery(bbox) }), headers: { 'User-Agent': 'gta-berlin-2d map build (private)' } });
      JSON.parse(osm); used = url; break;
    } catch (err) { console.warn(`  ${url}: ${String(err.message).slice(0, 80)}`); osm = null; }
  }
  if (!osm) throw new Error('kein Overpass-Server hat geantwortet – später erneut versuchen');
  await writeFile(raw + 'osm.json', osm);
  await writeFile(raw + 'fetched.json', JSON.stringify({ at: new Date().toISOString(), bbox, overpass: used }, null, 1));
  console.log(`fertig: ${(osm.length / 1e6).toFixed(1)} MB in ${((Date.now() - t0) / 1000).toFixed(0)} s → data/raw/`);
}
