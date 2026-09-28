// Verdeckung der Spielfigur bzw. ihres Autos (rein, ohne Canvas): Wird sie im Bild von etwas übermalt, das in der
// Zeichenfolge nach ihr kommt – eine Baumkrone, ein Haus (Dach oder Fassade, auch in der Tordurchfahrt) oder ein
// Viadukt/eine Bahnbrücke –, zeichnet der Renderer ihre Umrisse obendrauf.
import { pointInRings, segDist2 } from './geom.js';

// Dachversatz eines Hauses in der Schrägansicht (wie render.js drawBuilding)
export function roofOffset(b, cam, heightScale) {
  const H = Math.max(18, b.height * heightScale);
  return { H, dx: (b.cx - cam.x) * H * 0.0005, dy: -H * 0.5 + (b.cy - cam.y) * H * 0.00025 };
}

// Krone eines Baums (wie assets.js drawTree): Mittelpunkt über dem Stamm, Radius = Größe
export const crownOf = (tr) => ({ x: tr.x, y: tr.y - Math.min(tr.size * 0.5, 30), r: tr.size });

// target: { x, y, key (Tiefenschlüssel wie in der Zeichenliste) } → 'tree' | 'building' | 'bridge' | null
export function coverOf(target, { trees = [], buildings = [], bridges = [], deck = 46, cam, heightScale }) {
  const { x, y, key } = target;
  for (const f of bridges) { // über allem gezeichnet
    const p = f.pts;
    for (let i = 0; i < p.length - 2; i += 2) if (segDist2(x, y, p[i], p[i + 1], p[i + 2], p[i + 3]) < (deck / 2) ** 2) return 'bridge';
  }
  for (const b of buildings) {
    if (b.bbox.y + b.bbox.h <= key) continue; // vorher gezeichnet
    const { dx, dy } = roofOffset(b, cam, heightScale);
    const bb = b.bbox;
    if (x < bb.x + Math.min(0, dx) - 2 || x > bb.x + bb.w + Math.max(0, dx) + 2 || y < bb.y + Math.min(0, dy) - 2 || y > bb.y + bb.h + Math.max(0, dy) + 2) continue;
    for (const k of [1, 0.8, 0.6, 0.4, 0.2, 0.08]) if (pointInRings(x - dx * k, y - dy * k, b.rings)) return 'building';
  }
  for (const tr of trees) {
    if (tr.y <= key) continue;
    const c = crownOf(tr);
    if (Math.hypot(x - c.x, y - c.y) < c.r * 0.9) return 'tree';
  }
  return null;
}
