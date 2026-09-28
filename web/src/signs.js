// Wegweiser im Spiel (rein): Dekodieren aus der Kachel, Erkennen von Straßennamen (weiße Tafel).
// Gebaut werden die Schilder im Karten-Build (tools/osm/signs.mjs).

// Kachelzeile [x, y, Fahrtrichtung ×1000, Name, sichtbar, [[Richtung ×1000, Abbiegen ×1000, Ziele, Nummer], …]]
export function decodeSign(row, nm) {
  const [x, y, a, n, vis, rows] = row;
  return {
    x, y, angle: a / 1000, name: nm(n) ?? '', vis: !!vis, layer: 'sign',
    rows: rows.map(([dir, turn, d, ref]) => ({ dir: dir / 1000, turn: turn / 1000, dests: (nm(d) ?? '').split(';').filter(Boolean), ref: ref >= 0 ? nm(ref) ?? '' : '' })),
  };
}

// Ziel ist ein Straßenname (weißes Schild für den Nahbereich) statt eines Orts?
export const isStreet = (d) => /(straße|str\.|damm|allee|ufer|weg|platz|chaussee|brücke|gasse|ring|steig|promenade|zeile)$/i.test(d);
