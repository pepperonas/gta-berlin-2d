// Straßenquerschnitt aus OSM-Tags: Fahrbahnbreite (Bordstein zu Bordstein), Fahrstreifen je Richtung,
// Parkstreifen und Radfahrstreifen je Seite, Tempolimit, Belag. Seiten „left/right“ beziehen sich wie in OSM auf die
// Richtung des Weges. Pur und ohne Abhängigkeiten, damit es einzeln testbar ist.
import { PARK, SURFACE } from '../../web/src/citycodes.js';

const num = (v) => { const m = /^\s*(-?\d+(?:[.,]\d+)?)/.exec(v ?? ''); return m ? parseFloat(m[1].replace(',', '.')) : NaN; };

// Standard-Fahrstreifenbreite je Straßenklasse (m).
const LANE_W = { motorway: 3.5, trunk: 3.4, primary: 3.25, secondary: 3.25, tertiary: 3.1, unclassified: 3.0, residential: 2.9,
  living_street: 3.0, busway: 3.25, service: 3.0, pedestrian: 3.0, track: 3.0 };
// Standard-Fahrbahnbreite, wenn weder width noch lanes bekannt sind (ohne Park-/Radstreifen).
const TRAVEL_W = { motorway: 7.5, trunk: 7, primary: 7, secondary: 6.5, tertiary: 6.2, unclassified: 5.5, residential: 5.5,
  living_street: 4.5, busway: 6.5, service: 3.5, pedestrian: 5, track: 3 };
const PARK_DEPTH = { parallel: 2.0, diagonal: 4.5, perpendicular: 5.0 };

function sideTag(t, key, side) { return t[`${key}:${side}`] ?? t[`${key}:both`]; }

// Parkstreifen einer Seite: { kind: PARK.*, orient, width (m auf der Fahrbahn) }
export function parkingSide(t, side) {
  let v = sideTag(t, 'parking', side);
  let orient = t[`parking:${side}:orientation`] ?? t['parking:both:orientation'];
  if (v === undefined) { // altes Schema parking:lane:*
    const old = t[`parking:lane:${side}`] ?? t['parking:lane:both'];
    if (['parallel', 'diagonal', 'perpendicular'].includes(old)) {
      orient = old;
      const pos = t[`parking:lane:${side}:${old}`] ?? t[`parking:lane:both:${old}`];
      v = pos === 'half_on_kerb' ? 'half_on_kerb' : pos === 'on_kerb' ? 'on_kerb' : pos === 'street_side' ? 'street_side' : 'lane';
    } else v = 'no';
  }
  orient = PARK_DEPTH[orient] ? orient : 'parallel';
  const depth = PARK_DEPTH[orient];
  switch (v) {
    case 'lane': return { kind: PARK.lane, orient, width: depth };
    case 'half_on_kerb': return { kind: PARK.half, orient, width: depth / 2 };
    case 'on_kerb': return { kind: PARK.kerb, orient, width: 0 };
    default: return { kind: PARK.none, orient, width: 0 }; // no, separate, street_side, shoulder, …
  }
}

// Radfahrstreifen auf der Fahrbahn (m, 0 = keiner).
export function cycleLane(t, side) {
  const v = sideTag(t, 'cycleway', side) ?? t.cycleway;
  if (v !== 'lane') return 0;
  const w = num(t[`cycleway:${side}:width`] ?? t['cycleway:both:width'] ?? t['cycleway:width']);
  return w >= 0.8 && w <= 3.5 ? w : 1.6;
}

// Radweg neben der Fahrbahn (baulich getrennt, cycleway=track; m, 0 = keiner) – liegt außerhalb des Bordsteins.
export function cycleTrack(t, side) {
  const v = sideTag(t, 'cycleway', side) ?? t.cycleway;
  if (v !== 'track') return 0;
  const w = num(t[`cycleway:${side}:width`] ?? t['cycleway:both:width'] ?? t['cycleway:width']);
  return w >= 0.8 && w <= 4 ? w : 2;
}

export function maxspeedOf(t, base) {
  const v = t.maxspeed;
  if (v === 'walk') return 7;
  const n = num(v);
  if (n > 0 && n <= 130) return /mph/.test(v) ? Math.round(n * 1.609) : n;
  if (base === 'living_street') return 7;
  if (base === 'motorway') return 80;
  return ['residential', 'service', 'unclassified'].includes(base) ? 30 : 50;
}

export function surfaceOf(t) {
  const v = t.surface;
  if (['sett', 'cobblestone', 'unhewn_cobblestone', 'cobblestone:flattened'].includes(v)) return SURFACE.cobble;
  if (['paving_stones', 'concrete', 'concrete:plates', 'concrete:lanes'].includes(v)) return SURFACE.plates;
  if (['unpaved', 'gravel', 'fine_gravel', 'compacted', 'dirt', 'ground', 'sand', 'grass'].includes(v)) return SURFACE.unpaved;
  return SURFACE.asphalt;
}

// Vollständiger Querschnitt. oneway: 1 (in Wegrichtung), -1 (gegen), 0.
export function crossSection(t, base, oneway) {
  const L = parkingSide(t, 'left'), R = parkingSide(t, 'right');
  const cL = cycleLane(t, 'left'), cR = cycleLane(t, 'right');
  const laneW = LANE_W[base] ?? 3;
  let fwd = num(t['lanes:forward']), bwd = num(t['lanes:backward']);
  const total = num(t.lanes);
  if (!(fwd >= 1)) fwd = NaN; if (!(bwd >= 1)) bwd = NaN;
  if (oneway === 1) { fwd = fwd || total || 1; bwd = 0; }
  else if (oneway === -1) { bwd = bwd || total || 1; fwd = 0; }
  else if (!fwd || !bwd) {
    if (total >= 2) { fwd = fwd || Math.ceil(total / 2); bwd = bwd || Math.max(1, total - fwd); }
    else { fwd = fwd || 1; bwd = bwd || 1; }
  }
  const side = L.width + R.width + cL + cR;
  // Berlin: width:carriageway = Bordstein zu Bordstein (bevorzugt), sonst width (oft ALKIS)
  let width = num(t['width:carriageway']);
  if (!(width >= 3 && width <= 45)) width = num(t.width);
  const lanesKnown = total >= 1 || num(t['lanes:forward']) >= 1 || num(t['lanes:unmarked']) >= 1;
  if (!(width >= 3 && width <= 45)) {
    const travel = lanesKnown ? (fwd + bwd) * laneW : TRAVEL_W[base] ?? 5.5;
    width = travel + side;
  }
  // Fahrstreifen aus der Breite ableiten, wenn nicht getaggt (breite Hauptstraßen haben 2 je Richtung).
  let travel = width - side;
  if (travel < (fwd + bwd) * 2.3) { // Parken/Radstreifen passen nicht vollständig: anteilig kürzen
    const k = Math.max(0, (width - (fwd + bwd) * 2.3) / (side || 1));
    L.width *= Math.min(1, k); R.width *= Math.min(1, k);
    travel = width - L.width - R.width - cL * Math.min(1, k) - cR * Math.min(1, k);
  }
  if (!lanesKnown && ['primary', 'secondary', 'trunk'].includes(base)) {
    const per = oneway ? travel : travel / 2;
    const n = Math.max(1, Math.min(3, Math.floor(per / 3.0)));
    if (oneway === 1) fwd = n; else if (oneway === -1) bwd = n; else { fwd = n; bwd = n; }
  }
  return {
    width, fwd, bwd,
    left: { park: L.kind, parkW: L.width, orient: L.orient, cycle: cL, track: cycleTrack(t, 'left') },
    right: { park: R.kind, parkW: R.width, orient: R.orient, cycle: cR, track: cycleTrack(t, 'right') },
    maxspeed: maxspeedOf(t, base), surface: surfaceOf(t), lit: t.lit === 'yes' ? 1 : 0, gaslight: t.lit_by_gaslight === 'yes' ? 1 : 0,
  };
}

export { laneOffsets } from '../../web/src/street.js';
