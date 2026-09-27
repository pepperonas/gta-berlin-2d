// Holt die Rohdaten für die Karte von ganz Berlin nach data/raw/ (gitignored). Danach: npm run map:build
//  - OpenStreetMap: Berlin-Auszug von Geofabrik (PBF, ~100 MB, täglich aktualisiert)
//  - LOR-Prognoseräume (Geoportal Berlin, WFS): Stadtgrenze und Bezirke
//  - Berliner Baumbestand (Geoportal Berlin, WFS): Straßen- und Anlagenbäume
import { mkdir, writeFile, rename } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { fileURLToPath } from 'node:url';

const PBF = process.env.OSM_PBF_URL ?? 'https://download.geofabrik.de/europe/germany/berlin-latest.osm.pbf';
const LOR = 'https://gdi.berlin.de/services/wfs/lor_2021?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature'
  + '&TYPENAMES=lor_2021:c_lor_pgr_2021&OUTPUTFORMAT=application/json&SRSNAME=EPSG:4326';
const TREES_WFS = 'https://gdi.berlin.de/services/wfs/baumbestand?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature'
  + '&SRSNAME=EPSG:4326&OUTPUTFORMAT=application/json&PROPERTYNAME=gisid,gattung,art_dtsch,kronedurch,stammumfg,baumhoehe,geom&SORTBY=gisid';
const PAGE = 20000;
// Einwohnerdichte 2022 (Umweltatlas, Blöcke) und Verkehrsmengen DTVw 2019 (Kfz je Werktag, übergeordnetes Netz)
const DENSITY_WFS = 'https://gdi.berlin.de/services/wfs/ua_einwohnerdichte_2022?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature'
  + '&TYPENAMES=ua_einwohnerdichte_2022:einwohnerdichte2022&SRSNAME=EPSG:4326&OUTPUTFORMAT=application/json&PROPERTYNAME=ew_ha_2022,geom&SORTBY=schluessel';
const TRAFFIC_WFS = 'https://gdi.berlin.de/services/wfs/verkehrsmengen_2019?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature'
  + '&TYPENAMES=verkehrsmengen_2019:dtvw2019kfz&SRSNAME=EPSG:4326&OUTPUTFORMAT=application/json&PROPERTYNAME=elem_nr,dtvw_kfz,geom&SORTBY=elem_nr';

const round6 = (v) => Math.round(v * 1e6) / 1e6;
async function pages(url, onPage) {
  const out = [];
  for (let start = 0; ; start += PAGE) {
    const page = JSON.parse(await get(`${url}&COUNT=${PAGE}&STARTINDEX=${start}`));
    out.push(...page.features);
    onPage?.(start + page.features.length, page.numberMatched);
    if (page.features.length < PAGE) break;
  }
  return out;
}

// Einwohner je Hektar je Block: [ew_ha, [[lon, lat, lon, lat, …] je Ring]]
export async function fetchDensity(opts) {
  const fs = await pages(DENSITY_WFS, opts?.onPage);
  return fs.filter((f) => f.geometry).map((f) => [f.properties.ew_ha_2022 ?? 0,
    f.geometry.coordinates.flatMap((poly) => poly.map((ring) => ring.flatMap(([lon, lat]) => [round6(lon), round6(lat)])))]);
}

// Kfz je Werktag je Straßenabschnitt: [dtv, [lon, lat, …] je Linie]
export async function fetchTraffic(opts) {
  const fs = await pages(TRAFFIC_WFS, opts?.onPage);
  return fs.filter((f) => f.geometry && f.properties.dtvw_kfz > 0).map((f) => [f.properties.dtvw_kfz,
    (f.geometry.type === 'MultiLineString' ? f.geometry.coordinates : [f.geometry.coordinates]).map((l) => l.flatMap(([lon, lat]) => [round6(lon), round6(lat)]))]);
}

const raw = fileURLToPath(new URL('../../data/raw/', import.meta.url));

async function get(url, init) {
  for (let attempt = 0; ; attempt++) {
    try {
      const r = await fetch(url, init);
      if (!r.ok) throw new Error(`HTTP ${r.status} ${(await r.text()).slice(0, 200)}`);
      return await r.text();
    } catch (err) {
      if (attempt >= 3) throw new Error(`${url}: ${err.message}`);
      console.warn(`  ${String(err.message).slice(0, 80)} – neuer Versuch`);
      await new Promise((res) => setTimeout(res, 5000 * (attempt + 1)));
    }
  }
}

// Berliner Baumbestand (Straßen- und Anlagenbäume) seitenweise, nach gisid sortiert (stabile Seiten);
// kompakt [lon, lat, gattung, art, krone m, umfang cm, höhe m, Straßenbaum 1/0].
export async function fetchTrees({ onPage = () => {} } = {}) {
  const out = [], seen = new Set();
  for (const layer of ['strassenbaeume', 'anlagenbaeume']) {
    for (let start = 0; ; start += PAGE) {
      const page = JSON.parse(await get(`${TREES_WFS}&TYPENAMES=baumbestand:${layer}&COUNT=${PAGE}&STARTINDEX=${start}`));
      for (const f of page.features) {
        const p = f.properties;
        if (!f.geometry || seen.has(p.gisid)) continue;
        seen.add(p.gisid);
        const [lon, lat] = f.geometry.coordinates;
        out.push([lon, lat, p.gattung ?? '', p.art_dtsch ?? '', p.kronedurch ?? 0, p.stammumfg ?? 0, p.baumhoehe ?? 0, layer === 'strassenbaeume' ? 1 : 0]);
      }
      onPage(layer, start + page.features.length, page.numberMatched);
      if (page.features.length < PAGE) break;
    }
  }
  return out;
}

async function download(url, file) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${url}: HTTP ${r.status}`);
  await pipeline(Readable.fromWeb(r.body), createWriteStream(file + '.part'));
  await rename(file + '.part', file);
  return r.url; // nach Weiterleitung: Datei mit Datum
}

async function fetchLife() {
  console.log('Einwohnerdichte 2022 (Geoportal Berlin) …');
  const dichte = await fetchDensity({ onPage: (n, all) => process.stdout.write(`\r  ${n} / ${all}   `) });
  process.stdout.write('\n');
  if (dichte.length < 20000) throw new Error(`Einwohnerdichte unvollständig: ${dichte.length} Blöcke`);
  await writeFile(raw + 'dichte.json', JSON.stringify(dichte));
  console.log('Verkehrsmengen DTVw 2019 (Geoportal Berlin) …');
  const verkehr = await fetchTraffic({ onPage: (n, all) => process.stdout.write(`\r  ${n} / ${all}   `) });
  process.stdout.write('\n');
  if (verkehr.length < 5000) throw new Error(`Verkehrsmengen unvollständig: ${verkehr.length} Abschnitte`);
  await writeFile(raw + 'verkehr.json', JSON.stringify(verkehr));
  return { dichte: dichte.length, verkehr: verkehr.length };
}

if (import.meta.url === `file://${process.argv[1]}` && process.argv.includes('--life')) {
  // nur die Daten für Belebung (Dichte, Verkehrsmengen) nachladen: node tools/osm/fetch.mjs --life
  await mkdir(raw, { recursive: true });
  console.log(await fetchLife());
} else if (import.meta.url === `file://${process.argv[1]}`) {
  await mkdir(raw, { recursive: true });
  const t0 = Date.now();
  console.log('LOR-Prognoseräume (Geoportal Berlin) …');
  const lor = JSON.parse(await get(LOR));
  if (lor.features.length < 58) throw new Error(`LOR unvollständig: ${lor.features.length} Prognoseräume`);
  await writeFile(raw + 'lor.json', JSON.stringify(lor));
  console.log(`OpenStreetMap-Auszug (${PBF}) …`);
  const pbfUrl = await download(PBF, raw + 'berlin.osm.pbf');
  console.log('Baumbestand (Geoportal Berlin) …');
  const trees = await fetchTrees({ onPage: (l, n, all) => process.stdout.write(`\r  ${l}: ${n} / ${all}   `) });
  process.stdout.write('\n');
  await writeFile(raw + 'baeume.json', JSON.stringify(trees));
  await fetchLife();
  await writeFile(raw + 'fetched.json', JSON.stringify({ at: new Date().toISOString(), pbf: pbfUrl, lor: lor.features.length, trees: trees.length }, null, 1));
  console.log(`fertig: ${trees.length} Bäume, ${lor.features.length} Prognoseräume in ${((Date.now() - t0) / 1000).toFixed(0)} s → data/raw/`);
}
