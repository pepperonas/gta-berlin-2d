// Liest OpenStreetMap-Auszüge im PBF-Format (https://wiki.openstreetmap.org/wiki/PBF_Format) ohne Abhängigkeiten:
// Protocol Buffers von Hand dekodiert, Blöcke mit node:zlib entpackt.
//   for (const block of readPbf(buffer)) { block.nodes / block.ways / block.relations }
// Koordinaten in Grad, IDs als Number (OSM-IDs liegen weit unter 2^53).
import { inflateSync, deflateSync } from 'node:zlib';

// --- Protocol-Buffers-Grundlagen ---------------------------------------------------------------
class Reader {
  constructor(buf, pos = 0, end = buf.length) { this.b = buf; this.p = pos; this.end = end; }
  more() { return this.p < this.end; }
  // Varint als Number (bis 2^53 exakt; größere Werte kommen in OSM-Daten nicht vor)
  varint() {
    const b = this.b;
    let x = b[this.p++];
    if (x < 0x80) return x;
    x &= 0x7f;
    let mul = 128;
    for (;;) {
      const c = b[this.p++];
      x += (c & 0x7f) * mul;
      if (c < 0x80) return x;
      mul *= 128;
    }
  }
  svarint() { const n = this.varint(); return n % 2 ? -(n + 1) / 2 : n / 2; }
  // Feldkopf: [Feldnummer, Wire-Typ]
  key() { const k = this.varint(); return [Math.floor(k / 8), k & 7]; }
  bytes() { const n = this.varint(); const s = this.p; this.p += n; return [s, this.p]; }
  skip(wire) {
    if (wire === 0) this.varint();
    else if (wire === 1) this.p += 8;
    else if (wire === 2) { const n = this.varint(); this.p += n; } // erst lesen, dann addieren (sonst gilt die alte Position)
    else if (wire === 5) this.p += 4;
    else throw new Error(`PBF: unbekannter Wire-Typ ${wire}`);
  }
}

// Gepackte Varint-Folge [s, e) als Zahlenfeld; zigzag = sint, delta = aufsummiert.
function packed(buf, s, e, zigzag, delta) {
  const r = new Reader(buf, s, e), out = [];
  let acc = 0;
  while (r.more()) {
    let v = zigzag ? r.svarint() : r.varint();
    if (delta) { acc += v; v = acc; }
    out.push(v);
  }
  return out;
}

const utf8 = new TextDecoder();

// --- Datei: Folge von [Länge][BlobHeader][Blob] ------------------------------------------------
function* blobs(buf) {
  let p = 0;
  while (p < buf.length) {
    const hlen = buf.readUInt32BE(p); p += 4;
    const h = new Reader(buf, p, p + hlen); p += hlen;
    let type = '', size = 0;
    while (h.more()) {
      const [f, w] = h.key();
      if (f === 1) { const [s, e] = h.bytes(); type = utf8.decode(buf.subarray(s, e)); }
      else if (f === 3) size = h.varint();
      else h.skip(w);
    }
    const b = new Reader(buf, p, p + size); p += size;
    let data = null;
    while (b.more()) {
      const [f, w] = b.key();
      if (f === 1) { const [s, e] = b.bytes(); data = buf.subarray(s, e); }
      else if (f === 3) { const [s, e] = b.bytes(); data = inflateSync(buf.subarray(s, e)); }
      else if (f >= 4 && f <= 7 && w === 2) throw new Error('PBF: nur unkomprimierte oder zlib-Blöcke werden unterstützt');
      else b.skip(w);
    }
    yield { type, data };
  }
}

// OSMHeader: Hüllrechteck und Stand der Daten (Replikationszeitstempel).
function header(buf) {
  const r = new Reader(buf), out = { features: [] };
  while (r.more()) {
    const [f, w] = r.key();
    if (f === 4 || f === 5) { const [s, e] = r.bytes(); out.features.push(utf8.decode(buf.subarray(s, e))); }
    else if (f === 32) out.timestamp = new Date(r.varint() * 1000).toISOString().replace('.000', '');
    else if (f === 17) { const [s, e] = r.bytes(); out.source = utf8.decode(buf.subarray(s, e)); }
    else r.skip(w);
  }
  return out;
}

// PrimitiveBlock → { nodes: [{id, lat, lon, tags?}], ways: [{id, nodes, tags?}], relations: [{id, members, tags?}] }.
// want = { nodes, ways, relations } (false = Gruppe überspringen, spart Zeit beim ersten Durchgang).
function primitive(buf, want) {
  const r = new Reader(buf);
  let strings = [], gran = 100, latOff = 0, lonOff = 0;
  const groups = [];
  while (r.more()) {
    const [f, w] = r.key();
    if (f === 1) {
      const [s, e] = r.bytes(), st = new Reader(buf, s, e);
      strings = [];
      while (st.more()) { const [g, gw] = st.key(); if (g === 1) { const [a, b] = st.bytes(); strings.push(utf8.decode(buf.subarray(a, b))); } else st.skip(gw); }
    } else if (f === 2) groups.push(r.bytes());
    else if (f === 17) gran = r.varint();
    else if (f === 19) latOff = r.svarint();
    else if (f === 20) lonOff = r.svarint();
    else r.skip(w);
  }
  const deg = (off, v) => (off + gran * v) / 1e9;
  const tagsOf = (keys, vals) => { if (!keys.length) return undefined; const t = {}; for (let i = 0; i < keys.length; i++) t[strings[keys[i]]] = strings[vals[i]]; return t; };
  const out = { nodes: [], ways: [], relations: [] };
  for (const [gs, ge] of groups) {
    const g = new Reader(buf, gs, ge);
    while (g.more()) {
      const [f, w] = g.key();
      if (w !== 2) { g.skip(w); continue; }
      const [s, e] = g.bytes();
      if (f === 2 && want.nodes) dense(s, e);
      else if (f === 1 && want.nodes) node(s, e);
      else if (f === 3 && want.ways) way(s, e);
      else if (f === 4 && want.relations) relation(s, e);
    }
  }
  return out;

  function dense(s, e) {
    const d = new Reader(buf, s, e);
    let ids = [], lats = [], lons = [], kv = null;
    while (d.more()) {
      const [f, w] = d.key();
      if (f === 1) { const [a, b] = d.bytes(); ids = packed(buf, a, b, true, true); }
      else if (f === 8) { const [a, b] = d.bytes(); lats = packed(buf, a, b, true, true); }
      else if (f === 9) { const [a, b] = d.bytes(); lons = packed(buf, a, b, true, true); }
      else if (f === 10) { const [a, b] = d.bytes(); kv = packed(buf, a, b, false, false); }
      else d.skip(w);
    }
    let k = 0;
    for (let i = 0; i < ids.length; i++) {
      let tags;
      if (kv) {
        while (k < kv.length && kv[k] !== 0) { (tags ??= {})[strings[kv[k]]] = strings[kv[k + 1]]; k += 2; }
        k++; // 0 = Ende der Tags dieses Knotens
      }
      out.nodes.push({ id: ids[i], lat: deg(latOff, lats[i]), lon: deg(lonOff, lons[i]), tags });
    }
  }
  function node(s, e) {
    const d = new Reader(buf, s, e);
    let id = 0, lat = 0, lon = 0, keys = [], vals = [];
    while (d.more()) {
      const [f, w] = d.key();
      if (f === 1) id = d.svarint();
      else if (f === 8) lat = d.svarint();
      else if (f === 9) lon = d.svarint();
      else if (f === 2) { const [a, b] = d.bytes(); keys = packed(buf, a, b, false, false); }
      else if (f === 3) { const [a, b] = d.bytes(); vals = packed(buf, a, b, false, false); }
      else d.skip(w);
    }
    out.nodes.push({ id, lat: deg(latOff, lat), lon: deg(lonOff, lon), tags: tagsOf(keys, vals) });
  }
  function way(s, e) {
    const d = new Reader(buf, s, e);
    let id = 0, keys = [], vals = [], refs = [];
    while (d.more()) {
      const [f, w] = d.key();
      if (f === 1) id = d.varint();
      else if (f === 2) { const [a, b] = d.bytes(); keys = packed(buf, a, b, false, false); }
      else if (f === 3) { const [a, b] = d.bytes(); vals = packed(buf, a, b, false, false); }
      else if (f === 8) { const [a, b] = d.bytes(); refs = packed(buf, a, b, true, true); }
      else d.skip(w);
    }
    out.ways.push({ id, nodes: refs, tags: tagsOf(keys, vals) });
  }
  function relation(s, e) {
    const d = new Reader(buf, s, e);
    let id = 0, keys = [], vals = [], roles = [], mids = [], types = [];
    while (d.more()) {
      const [f, w] = d.key();
      if (f === 1) id = d.varint();
      else if (f === 2) { const [a, b] = d.bytes(); keys = packed(buf, a, b, false, false); }
      else if (f === 3) { const [a, b] = d.bytes(); vals = packed(buf, a, b, false, false); }
      else if (f === 8) { const [a, b] = d.bytes(); roles = packed(buf, a, b, false, false); }
      else if (f === 9) { const [a, b] = d.bytes(); mids = packed(buf, a, b, true, true); }
      else if (f === 10) { const [a, b] = d.bytes(); types = packed(buf, a, b, false, false); }
      else d.skip(w);
    }
    const T = ['node', 'way', 'relation'];
    out.relations.push({ id, tags: tagsOf(keys, vals), members: mids.map((ref, i) => ({ type: T[types[i]], ref, role: strings[roles[i]] })) });
  }
}

// Alle Datenblöcke der Datei; want wie bei primitive(). Der Kopf steht in .header.
export function* readPbf(buf, want = { nodes: true, ways: true, relations: true }) {
  for (const { type, data } of blobs(buf)) {
    if (type === 'OSMHeader') {
      const h = header(data);
      if (h.features.includes('HistoricalInformation')) throw new Error('PBF mit Versionsgeschichte wird nicht unterstützt');
      yield { header: h };
    } else if (type === 'OSMData') yield primitive(data, want);
  }
}

// --- Schreiben (nur für Tests: kleine PBF-Dateien erzeugen) --------------------------------------
function wVarint(out, v) { while (v >= 128) { out.push((v % 128) | 128); v = Math.floor(v / 128); } out.push(v); }
function wZig(out, v) { wVarint(out, v < 0 ? -2 * v - 1 : 2 * v); }
function wField(out, f, wire) { wVarint(out, f * 8 + wire); }
function wBytes(out, f, bytes) { wField(out, f, 2); wVarint(out, bytes.length); for (const b of bytes) out.push(b); }
function wPacked(f, vals, zig, delta) {
  const o = []; let prev = 0;
  for (const v of vals) { const x = delta ? v - prev : v; prev = v; if (zig) wZig(o, x); else wVarint(o, x); }
  const out = []; wBytes(out, f, o); return out;
}

// elements im Overpass-Format → PBF-Buffer (ein Block, dichte Knoten, zlib oder roh).
export function writePbf(elements, { timestamp = null, zlib = true } = {}) {
  const strings = [''], idx = new Map([['', 0]]);
  const sid = (s) => { let k = idx.get(s); if (k === undefined) { k = strings.length; strings.push(s); idx.set(s, k); } return k; };
  const nodes = elements.filter((e) => e.type === 'node'), ways = elements.filter((e) => e.type === 'way'), rels = elements.filter((e) => e.type === 'relation');
  const kv = []; for (const n of nodes) { for (const [k, v] of Object.entries(n.tags ?? {})) kv.push(sid(k), sid(v)); kv.push(0); }
  const dense = [
    ...wPacked(1, nodes.map((n) => n.id), true, true),
    ...wPacked(8, nodes.map((n) => Math.round(n.lat * 1e7)), true, true),
    ...wPacked(9, nodes.map((n) => Math.round(n.lon * 1e7)), true, true),
    ...wPacked(10, kv, false, false),
  ];
  const group1 = []; wBytes(group1, 2, dense);
  const group2 = [];
  for (const w of ways) {
    const o = []; wField(o, 1, 0); wVarint(o, w.id);
    const t = Object.entries(w.tags ?? {});
    o.push(...wPacked(2, t.map(([k]) => sid(k)), false, false), ...wPacked(3, t.map(([, v]) => sid(v)), false, false), ...wPacked(8, w.nodes, true, true));
    wBytes(group2, 3, o);
  }
  const group3 = [];
  for (const r of rels) {
    const o = []; wField(o, 1, 0); wVarint(o, r.id);
    const t = Object.entries(r.tags ?? {});
    o.push(...wPacked(2, t.map(([k]) => sid(k)), false, false), ...wPacked(3, t.map(([, v]) => sid(v)), false, false),
      ...wPacked(8, r.members.map((m) => sid(m.role)), false, false), ...wPacked(9, r.members.map((m) => m.ref), true, true),
      ...wPacked(10, r.members.map((m) => ['node', 'way', 'relation'].indexOf(m.type)), false, false));
    wBytes(group3, 4, o);
  }
  const st = []; for (const s of strings) wBytes(st, 1, [...Buffer.from(s)]);
  const block = []; wBytes(block, 1, st); wBytes(block, 2, group1); if (group2.length) wBytes(block, 2, group2); if (group3.length) wBytes(block, 2, group3);
  wField(block, 17, 0); wVarint(block, 100);
  const hdr = []; wBytes(hdr, 4, [...Buffer.from('OsmSchema-V0.6')]); wBytes(hdr, 4, [...Buffer.from('DenseNodes')]);
  if (timestamp) { wField(hdr, 32, 0); wVarint(hdr, Math.floor(Date.parse(timestamp) / 1000)); }
  const parts = [];
  for (const [type, raw] of [['OSMHeader', hdr], ['OSMData', block]]) {
    const blob = [];
    if (zlib) { const z = deflateSync(Buffer.from(raw)); wField(blob, 2, 0); wVarint(blob, raw.length); wBytes(blob, 3, [...z]); }
    else wBytes(blob, 1, raw);
    const bh = []; wBytes(bh, 1, [...Buffer.from(type)]); wField(bh, 3, 0); wVarint(bh, blob.length);
    const len = Buffer.alloc(4); len.writeUInt32BE(bh.length);
    parts.push(len, Buffer.from(bh), Buffer.from(blob));
  }
  return Buffer.concat(parts);
}
