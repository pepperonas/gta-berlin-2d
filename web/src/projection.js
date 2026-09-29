// Kartenprojektion (rein rechnerisch, gemeinsam mit dem Build in tools/osm/geo.mjs).
// Transversale Mercator-Projektion (Krüger-Reihen, GRS80) um den Mittelmeridian lon0, Maßstab 1 an lon0.
// Liefert Meter: x nach Osten, y nach Norden (relativ zu lat0/lon0).
export function makeProjection(lat0, lon0) {
  const a = 6378137, f = 1 / 298.257222101;
  const n = f / (2 - f), n2 = n * n, n3 = n2 * n;
  const A = a / (1 + n) * (1 + n2 / 4 + n2 * n2 / 64);
  const al = [n / 2 - 2 * n2 / 3 + 5 * n3 / 16, 13 * n2 / 48 - 3 * n3 / 5, 61 * n3 / 240];
  const e = Math.sqrt(f * (2 - f));
  const rad = Math.PI / 180;
  const raw = (lat, lon) => {
    const phi = lat * rad, lam = (lon - lon0) * rad;
    const t = Math.sinh(Math.atanh(Math.sin(phi)) - e * Math.atanh(e * Math.sin(phi)));
    const xi = Math.atan2(t, Math.cos(lam)), eta = Math.atanh(Math.sin(lam) / Math.sqrt(1 + t * t));
    let x = eta, y = xi;
    for (let j = 1; j <= 3; j++) {
      x += al[j - 1] * Math.cos(2 * j * xi) * Math.sinh(2 * j * eta);
      y += al[j - 1] * Math.sin(2 * j * xi) * Math.cosh(2 * j * eta);
    }
    return [A * x, A * y];
  };
  const [, y0] = raw(lat0, lon0);
  return (lat, lon) => { const [x, y] = raw(lat, lon); return [x, y - y0]; };
}

// Geokoordinaten → Karten-px mit denselben Parametern wie der Build (index.json meta: origin.lat0/lon0/bbox, scale)
export function geoToPx(meta) {
  const [s, w, n, e] = meta.origin.bbox, S = meta.scale;
  const proj = makeProjection(meta.origin.lat0, meta.origin.lon0);
  const c = [proj(s, w), proj(s, e), proj(n, w), proj(n, e)];
  const minX = Math.min(...c.map((q) => q[0])), maxY = Math.max(...c.map((q) => q[1]));
  return (lat, lon) => { const [x, y] = proj(lat, lon); return [Math.round((x - minX) * S), Math.round((maxY - y) * S)]; };
}
