// Statistik dauerhaft speichern: IndexedDB (Datenbank „gta-berlin“, Tabelle „stats“; Schlüssel „total“ = über alle
// Spiele, „game“ = aktuelles Spiel). Ohne IndexedDB (Node-Tests, gesperrter Speicher) ein Speicher im Arbeitsspeicher –
// gleiche Schnittstelle, alles asynchron.
import { normalizeStats } from './stats.js';

const DB = 'gta-berlin', STORE = 'stats', VERSION = 1;

export function memoryStatsStore() {
  const m = new Map();
  return {
    kind: 'memory',
    async get(key) { return m.has(key) ? normalizeStats(JSON.parse(m.get(key))) : null; },
    async put(key, stats) { m.set(key, JSON.stringify(stats)); return true; },
    async clear(key) { m.delete(key); },
  };
}

export async function openStatsStore(idb = globalThis.indexedDB) {
  if (!idb) return memoryStatsStore();
  try {
    const db = await new Promise((res, rej) => {
      const r = idb.open(DB, VERSION);
      r.onupgradeneeded = () => { if (!r.result.objectStoreNames.contains(STORE)) r.result.createObjectStore(STORE); };
      r.onsuccess = () => res(r.result);
      r.onerror = () => rej(r.error);
      r.onblocked = () => rej(new Error('IndexedDB blockiert'));
    });
    const tx = (mode, fn) => new Promise((res, rej) => {
      const t = db.transaction(STORE, mode), q = fn(t.objectStore(STORE));
      t.oncomplete = () => res(q?.result); t.onerror = () => rej(t.error); t.onabort = () => rej(t.error);
    });
    return {
      kind: 'indexeddb',
      async get(key) { const v = await tx('readonly', (s) => s.get(key)); return v ? normalizeStats(v) : null; },
      async put(key, stats) { await tx('readwrite', (s) => s.put(JSON.parse(JSON.stringify(stats)), key)); return true; },
      async clear(key) { await tx('readwrite', (s) => s.delete(key)); },
    };
  } catch (err) {
    console.warn('Statistik: IndexedDB nicht verfügbar, nur für diese Sitzung', err);
    return memoryStatsStore();
  }
}
