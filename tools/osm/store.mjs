// OSM-Daten für den Karten-Build, speicherschonend: Koordinaten aller Knoten in typisierten Feldern (sortiert nach ID,
// Suche per Halbierung), nur getaggte Knoten als Objekte, Wege und Relationen gefiltert auf das, was der Build nutzt.
//   storeFromPbf(buffer)          – Geofabrik-Auszug (tools/osm/pbf.mjs)
//   storeFromElements(elements)   – Overpass-Format (Test-Fixture)
// Schnittstelle: coord(id) → [lat, lon] | null, has(id), tagged (getaggte Knoten), ways (Map id → Weg),
// relations (Array), timestamp.
import { readPbf } from './pbf.mjs';

// Wege, die der Build braucht (Straßen, Gebäude, Flächen, Gleise, Sperren, POIs, Hausnummern, Plätze).
const WAY_KEYS = ['highway', 'building', 'landuse', 'leisure', 'natural', 'waterway', 'amenity', 'railway', 'man_made',
  'barrier', 'shop', 'tourism', 'addr:housenumber', 'place', 'public_transport', 'area:highway'];
// Relationen: Multipolygone (Gebäude, Flächen, Wasser), Abbiegeverbote, Ortsteilgrenzen.
export const keepRelation = (t) => !!t && (t.type === 'multipolygon' || t.type === 'restriction'
  || (t.type === 'boundary' && t.boundary === 'administrative' && ['9', '10'].includes(t.admin_level)));
const keepWay = (t) => !!t && WAY_KEYS.some((k) => t[k] !== undefined);
// Getaggte Knoten ohne Nutzen für das Spiel (nur Erfassungsdetails) nicht als Objekt halten.
const USELESS = new Set(['created_by', 'source', 'note', 'fixme', 'FIXME', 'check_date', 'survey:date', 'description']);
const usefulTags = (t) => { for (const k in t) if (!USELESS.has(k)) return true; return false; };

class Store {
  constructor(ids, lat, lon, n) { this.ids = ids; this.lat = lat; this.lon = lon; this.n = n; this.tagged = []; this.ways = new Map(); this.relations = []; this.timestamp = null; }
  index(id) {
    let lo = 0, hi = this.n - 1;
    const ids = this.ids;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1, v = ids[mid];
      if (v === id) return mid;
      if (v < id) lo = mid + 1; else hi = mid - 1;
    }
    return -1;
  }
  has(id) { return this.index(id) >= 0; }
  coord(id) { const i = this.index(id); return i < 0 ? null : [this.lat[i] / 1e7, this.lon[i] / 1e7]; }
  // Iteration im Overpass-Stil über alles Getaggte (Knoten, Wege, Relationen)
  *elements() { yield* this.tagged; for (const w of this.ways.values()) if (w.tags) yield w; for (const r of this.relations) yield r; }
}

function grow(a, n) { const b = new a.constructor(n); b.set(a); return b; }

export function storeFromPbf(buf) {
  // 1. Durchgang: Relationen – welche Wege werden als Mitglieder gebraucht?
  const members = new Set();
  let timestamp = null;
  for (const b of readPbf(buf, { nodes: false, ways: false, relations: true })) {
    if (b.header) { timestamp = b.header.timestamp ?? null; continue; }
    for (const r of b.relations) if (keepRelation(r.tags)) for (const m of r.members) if (m.type === 'way') members.add(m.ref);
  }
  // 2. Durchgang: alles
  let cap = 1 << 22, n = 0;
  let ids = new Float64Array(cap), lat = new Int32Array(cap), lon = new Int32Array(cap);
  const s = new Store(null, null, null, 0);
  let last = -Infinity, sorted = true;
  for (const b of readPbf(buf)) {
    if (b.header) continue;
    for (const nd of b.nodes) {
      if (n === cap) { cap *= 2; ids = grow(ids, cap); lat = grow(lat, cap); lon = grow(lon, cap); }
      ids[n] = nd.id; lat[n] = Math.round(nd.lat * 1e7); lon[n] = Math.round(nd.lon * 1e7); n++;
      if (nd.id <= last) sorted = false; last = nd.id;
      if (nd.tags && usefulTags(nd.tags)) s.tagged.push({ type: 'node', id: nd.id, lat: nd.lat, lon: nd.lon, tags: nd.tags });
    }
    for (const w of b.ways) if (keepWay(w.tags) || members.has(w.id)) s.ways.set(w.id, { type: 'way', id: w.id, nodes: w.nodes, tags: w.tags });
    for (const r of b.relations) if (keepRelation(r.tags)) s.relations.push({ type: 'relation', id: r.id, members: r.members, tags: r.tags });
  }
  if (!sorted) { // Geofabrik liefert sortiert; sonst nachsortieren
    const order = Array.from({ length: n }, (_, i) => i).sort((a, b) => ids[a] - ids[b]);
    const i2 = new Float64Array(n), la2 = new Int32Array(n), lo2 = new Int32Array(n);
    order.forEach((k, i) => { i2[i] = ids[k]; la2[i] = lat[k]; lo2[i] = lon[k]; });
    ids = i2; lat = la2; lon = lo2;
  }
  Object.assign(s, { ids, lat, lon, n, timestamp });
  return s;
}

export function storeFromElements(elements, { timestamp = null } = {}) {
  const nodes = elements.filter((e) => e.type === 'node').sort((a, b) => a.id - b.id);
  const s = new Store(new Float64Array(nodes.map((x) => x.id)), new Int32Array(nodes.map((x) => Math.round(x.lat * 1e7))), new Int32Array(nodes.map((x) => Math.round(x.lon * 1e7))), nodes.length);
  for (const nd of nodes) if (nd.tags) s.tagged.push(nd);
  for (const e of elements) {
    if (e.type === 'way') s.ways.set(e.id, e);
    else if (e.type === 'relation' && keepRelation(e.tags)) s.relations.push(e);
  }
  s.timestamp = timestamp;
  return s;
}
