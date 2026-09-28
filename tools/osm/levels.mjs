// Ebenen aus OSM (rein): OSM kennt keine absoluten Höhen für Wege, aber die Höhenordnung (layer, bridge, tunnel).
//  – Brücke (bridge gesetzt und nicht „no“): max(1, layer)
//  – Tunnel (tunnel=yes/culvert/flooded): null → wird verworfen (echte Tunnel spielen nicht mit)
//  – layer < 0 ohne Tunnel: offene Unterführung/Einschnitt (z. B. Tamara-Danz-Straße unter der Warschauer Straße)
//  – layer > 0 ohne Brückentag: Boden (OSM nutzt layer dort zu oft ohne Höhenbezug)
// Ebenenwechsel gibt es nur, wo Wege verschiedener Ebenen einen Knoten teilen (Portale, siehe build.mjs).
import { LVL_MIN, LVL_MAX } from '../../web/src/citycodes.js';

const num = (v) => { const m = /^\s*(-?\d+(?:[.,]\d+)?)/.exec(v ?? ''); return m ? parseFloat(m[1].replace(',', '.')) : NaN; };
const clamp = (l) => Math.max(LVL_MIN, Math.min(LVL_MAX, l));

export const isBridge = (t) => !!t.bridge && t.bridge !== 'no';
export const isTunnel = (t) => t.tunnel === 'yes' || t.tunnel === 'culvert' || t.tunnel === 'flooded';

export function levelOf(t) {
  if (isTunnel(t)) return null;
  const L = num(t.layer), l = Number.isFinite(L) ? Math.round(L) : 0;
  if (isBridge(t)) return clamp(Math.max(1, l));
  return l < 0 ? clamp(l) : 0;
}
