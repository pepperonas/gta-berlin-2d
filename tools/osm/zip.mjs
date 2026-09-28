// ZIP ohne Abhängigkeiten (nur Node): Einträge auflisten und zeilenweise lesen, auch sehr große (Datenstrom mit
// node:zlib, die 400-MB-stop_times.txt des VBB wird nie ganz in den Speicher geholt). Unterstützt „stored“ (0) und
// „deflate“ (8), kein ZIP64 (VBB-Einträge liegen unter 4 GB). Zum Testen: makeZip baut ein ZIP mit „stored“-Einträgen.
import { openSync, readSync, fstatSync, closeSync, createReadStream } from 'node:fs';
import { createInflateRaw } from 'node:zlib';
import { createInterface } from 'node:readline';

export function listZip(file) {
  const fd = openSync(file, 'r');
  try {
    const size = fstatSync(fd).size, tail = Math.min(size, 65557), buf = Buffer.alloc(tail);
    readSync(fd, buf, 0, tail, size - tail);
    let eocd = -1;
    for (let i = tail - 22; i >= 0; i--) if (buf.readUInt32LE(i) === 0x06054b50) { eocd = i; break; }
    if (eocd < 0) throw new Error(`${file}: kein ZIP (Endverzeichnis fehlt)`);
    const count = buf.readUInt16LE(eocd + 10), cdSize = buf.readUInt32LE(eocd + 12), cdOff = buf.readUInt32LE(eocd + 16);
    const cd = Buffer.alloc(cdSize);
    readSync(fd, cd, 0, cdSize, cdOff);
    const entries = new Map();
    for (let p = 0, i = 0; i < count; i++) {
      if (cd.readUInt32LE(p) !== 0x02014b50) throw new Error(`${file}: Zentralverzeichnis kaputt`);
      const method = cd.readUInt16LE(p + 10), csize = cd.readUInt32LE(p + 20), usize = cd.readUInt32LE(p + 24);
      const nlen = cd.readUInt16LE(p + 28), xlen = cd.readUInt16LE(p + 30), clen = cd.readUInt16LE(p + 32), local = cd.readUInt32LE(p + 42);
      const name = cd.toString('utf8', p + 46, p + 46 + nlen);
      // Beginn der Daten: hinter dem lokalen Kopf (dessen Namens-/Extra-Längen können abweichen)
      const lh = Buffer.alloc(30); readSync(fd, lh, 0, 30, local);
      const start = local + 30 + lh.readUInt16LE(26) + lh.readUInt16LE(28);
      entries.set(name, { name, method, csize, usize, start });
      p += 46 + nlen + xlen + clen;
    }
    return entries;
  } finally { closeSync(fd); }
}

// Zeilen eines Eintrags als asynchroner Iterator
export async function* zipLines(file, name, entries = listZip(file)) {
  const e = entries.get(name);
  if (!e) throw new Error(`${file}: Eintrag ${name} fehlt`);
  let stream = createReadStream(file, { start: e.start, end: e.start + e.csize - 1 });
  if (e.method === 8) stream = stream.pipe(createInflateRaw());
  else if (e.method !== 0) throw new Error(`${name}: Kompression ${e.method} nicht unterstützt`);
  const rl = createInterface({ input: stream, crlfDelay: Infinity });
  for await (const line of rl) yield line;
}

// Einfaches CSV (GTFS): Kommas, Felder optional in "…" mit "" als Anführungszeichen
export function csvRow(line) {
  if (!line.includes('"')) return line.split(',');
  const out = [];
  let cur = '', q = false;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (q) { if (c === '"') { if (line[i + 1] === '"') { cur += '"'; i++; } else q = false; } else cur += c; }
    else if (c === '"') q = true;
    else if (c === ',') { out.push(cur); cur = ''; }
    else cur += c;
  }
  out.push(cur);
  return out;
}

// Zeilen als Objekte (Spaltennamen aus der Kopfzeile, BOM entfernt)
export async function* zipCsv(file, name, entries) {
  let head = null;
  for await (const line of zipLines(file, name, entries)) {
    if (!line) continue;
    const row = csvRow(line);
    if (!head) { head = row.map((h) => h.replace(/^﻿/, '').trim()); continue; }
    const o = {};
    for (let i = 0; i < head.length; i++) o[head[i]] = row[i] ?? '';
    yield o;
  }
}

// ZIP aus { name: text } bauen (nur „stored“, für Tests)
export function makeZip(files) {
  const crcTab = new Int32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c; });
  const crc = (b) => { let c = -1; for (const x of b) c = crcTab[(c ^ x) & 255] ^ (c >>> 8); return (c ^ -1) >>> 0; };
  const locals = [], centrals = [];
  let off = 0;
  for (const [name, text] of Object.entries(files)) {
    const data = Buffer.from(text), nb = Buffer.from(name), c = crc(data);
    const lh = Buffer.alloc(30);
    lh.writeUInt32LE(0x04034b50, 0); lh.writeUInt16LE(20, 4); lh.writeUInt32LE(c, 14); lh.writeUInt32LE(data.length, 18); lh.writeUInt32LE(data.length, 22); lh.writeUInt16LE(nb.length, 26);
    const ch = Buffer.alloc(46);
    ch.writeUInt32LE(0x02014b50, 0); ch.writeUInt16LE(20, 4); ch.writeUInt16LE(20, 6); ch.writeUInt32LE(c, 16); ch.writeUInt32LE(data.length, 20); ch.writeUInt32LE(data.length, 24); ch.writeUInt16LE(nb.length, 28); ch.writeUInt32LE(off, 42);
    locals.push(lh, nb, data); centrals.push(ch, nb);
    off += 30 + nb.length + data.length;
  }
  const cd = Buffer.concat(centrals), end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0); end.writeUInt16LE(Object.keys(files).length, 8); end.writeUInt16LE(Object.keys(files).length, 10); end.writeUInt32LE(cd.length, 12); end.writeUInt32LE(off, 16);
  return Buffer.concat([...locals, cd, end]);
}
