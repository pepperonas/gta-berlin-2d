// Nachbarschaftsraster für bewegte Objekte (Autos, Passanten) – nur zur Beschleunigung, ohne Einfluss aufs Ergebnis:
// near() liefert die Indizes in der Reihenfolge der Liste (aufsteigend), genau wie eine Schleife über alle. So bleibt die
// Simulation bitgleich (deterministisch, Tests unverändert), prüft aber nur Objekte in den Zellen um den Ort.
// Das Raster gilt, solange sich die Objekte nicht bewegen – der Aufrufer baut es neu, wenn sie sich bewegt haben.
const KEY = (cx, cy) => cx * 65536 + cy;

export function buildGrid(list, cell, g = null) {
  g ??= { map: new Map(), pool: [] };
  for (const a of g.map.values()) { a.length = 0; g.pool.push(a); }
  g.map.clear(); g.cell = cell; g.list = list;
  for (let i = 0; i < list.length; i++) {
    const o = list[i], k = KEY(Math.floor(o.x / cell), Math.floor(o.y / cell));
    let a = g.map.get(k);
    if (!a) { a = g.pool.pop() ?? []; g.map.set(k, a); }
    a.push(i);
  }
  return g;
}

// Indizes aller Objekte, deren Zelle den Kreis (x, y, r) berühren kann – aufsteigend sortiert, in out
export function near(g, x, y, r, out = []) {
  out.length = 0;
  const c = g.cell, x0 = Math.floor((x - r) / c), x1 = Math.floor((x + r) / c), y0 = Math.floor((y - r) / c), y1 = Math.floor((y + r) / c);
  for (let cx = x0; cx <= x1; cx++) for (let cy = y0; cy <= y1; cy++) {
    const a = g.map.get(KEY(cx, cy));
    if (a) for (const i of a) out.push(i);
  }
  if (out.length > 1) out.sort((a, b) => a - b);
  return out;
}
