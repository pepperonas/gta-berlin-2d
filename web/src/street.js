// Straßenquerschnitt zur Laufzeit (geteilt mit dem Karten-Build): Lage der Fahrstreifen, Mittellinie, Parkstreifen.
// Alle Maße in derselben Einheit wie der Querschnitt (m im Build, px im Spiel); positiv = rechts in Wegrichtung.

// unit: Einheiten je Meter (1 im Build, city.scale im Spiel). Ist eine Straße mit Gegenverkehr so eng, dass je
// Richtung weniger als 2,6 m bleiben (Berliner Nebenstraßen mit Parkstreifen), fahren beide Richtungen in der Mitte
// der Restfahrbahn – wie im echten Verkehr, man weicht einander aus.
export function laneOffsets(cs, unit = 1) {
  const xL = -cs.width / 2 + cs.left.parkW + cs.left.cycle;
  const xR = cs.width / 2 - cs.right.parkW - cs.right.cycle;
  const n = cs.fwd + cs.bwd, lw = (xR - xL) / Math.max(1, n);
  const center = xL + cs.bwd * lw;
  const fwd = [], bwd = [];
  if (cs.fwd && cs.bwd && lw < 2.6 * unit) {
    const mid = (xL + xR) / 2;
    for (let i = 0; i < cs.fwd; i++) fwd.push(mid);
    for (let j = 0; j < cs.bwd; j++) bwd.push(mid);
    return { fwd, bwd, center: mid, laneW: lw, xL, xR, narrow: true };
  }
  for (let i = 0; i < cs.fwd; i++) fwd.push(center + (i + 0.5) * lw); // von der Mitte nach außen
  for (let j = 0; j < cs.bwd; j++) bwd.push(center - (j + 0.5) * lw);
  return { fwd, bwd, center, laneW: lw, xL, xR };
}

// Mitte des Parkstreifens einer Seite (side: -1 links, +1 rechts) und seine Tiefe auf der Fahrbahn.
export function parkingStrip(cs, side) {
  const s = side < 0 ? cs.left : cs.right;
  return { offset: side * (cs.width / 2 - s.parkW / 2), depth: s.parkW, orient: s.orient, kind: s.park };
}
