// Begrenzter LRU-Cache für statische Rasterflächen; große Einzelobjekte werden direkt gezeichnet.
import { makeCanvas } from './lighting.js';
export class RasterCache {
  constructor({ maxPixels = 16 * 1024 * 1024, maxEntryPixels = 1024 * 1024, maxEntries = 128, create = makeCanvas } = {}) {
    Object.assign(this, { maxPixels, maxEntryPixels, maxEntries, create }); this.items = new Map(); this.pixels = 0;
  }
  get(key, stamp, width, height, paint) {
    const pixels = width * height;
    if (!(width > 0 && height > 0) || pixels > Math.min(this.maxEntryPixels, this.maxPixels)) return null;
    let item = this.items.get(key);
    if (item?.stamp === stamp && item.width === width && item.height === height) { this.items.delete(key); this.items.set(key, item); return item.canvas; }
    if (item) { this.pixels -= item.width * item.height; this.items.delete(key); }
    while (this.items.size && (this.pixels + pixels > this.maxPixels || this.items.size >= this.maxEntries)) {
      const oldest = this.items.keys().next().value, old = this.items.get(oldest); this.pixels -= old.width * old.height; this.items.delete(oldest);
    }
    const canvas = item?.width === width && item.height === height ? item.canvas : this.create(width, height);
    paint(canvas);
    this.items.set(key, { stamp, width, height, canvas }); this.pixels += pixels; return canvas;
  }
}
