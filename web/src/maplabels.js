// Beschriftungen des Stadtplans je nach Maßstab (wie bei kiez-finder): ganz herausgezoomt die Bezirke, dann Ortsteile
// (Tegel, Prenzlauer Berg …), dann die Kiez-Namen aus OSM (Flughafenkiez, Reuterkiez …) in Akzentfarbe und Bahnhöfe,
// ganz nah die Straßennamen entlang der Straße. Wichtigeres wird zuerst gesetzt; was sich überlappen würde, entfällt.
// Rein rechnerisch (kein Canvas): die Textbreite misst der Aufrufer (measure), gezeichnet wird im HUD.

// Sichtbar, solange die Karte zwischen lo und hi Metern je HUD-Pixel zeigt.
export const LABEL_TIERS = {
  bezirk: [18, Infinity],
  ortsteil: [3, 40],
  kiez: [0, 11],
  station: [0, 8],
  mainStreet: [0, 6],
  street: [0, 2.6],
};

export const LABEL_STYLE = {
  bezirk: { size: 17, weight: 800, color: '#ffffff' },
  ortsteil: { size: 15, weight: 700, color: '#e6e6e6' },
  kiez: { size: 13, weight: 600, color: '#ffe08a', italic: true },
  station: { size: 12, weight: 600, color: '#d8d8d8' },
  mainStreet: { size: 12, weight: 700, color: '#f4f4f4' },
  street: { size: 11, weight: 600, color: '#e8e8e8' },
};

const inTier = (kind, mpp) => mpp > LABEL_TIERS[kind][0] && mpp <= LABEL_TIERS[kind][1];

// Straßen einmal für die Beschriftung aufbereiten: Weltkoordinaten, Hüllrechteck, Name.
export function prepareStreets(roads, names, undelta) {
  const out = [];
  for (const [cls, n, d] of roads) {
    if (n < 0) continue;
    const pts = undelta(d);
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (let i = 0; i < pts.length; i += 2) { x0 = Math.min(x0, pts[i]); x1 = Math.max(x1, pts[i]); y0 = Math.min(y0, pts[i + 1]); y1 = Math.max(y1, pts[i + 1]); }
    out.push({ cls, name: names[n], pts, x0, y0, x1, y1 });
  }
  return out;
}

// view: { f, ox, oy (Welt → HUD), x, y, w, h (Kartenfläche im HUD), mpp (Meter je HUD-Pixel) }
// data: { labels (Bezirke), ortsteile, kieze, stations, streets (prepareStreets) }; blocked: freizuhaltende Rechtecke
// (Hinweisleiste …) als { x0, y0, x1, y1 }.
// measure(text, style) → Breite in HUD-Pixeln. Liefert [{ kind, text, x, y, angle, ...style }].
export function mapLabels(data, view, measure, blocked = []) {
  const { f, ox, oy, x, y, w, h, mpp } = view;
  const placed = [], boxes = [...blocked];
  const sx = (wx) => ox + wx * f, sy = (wy) => oy + wy * f;
  const free = (b) => b.x0 >= x && b.x1 <= x + w && b.y0 >= y && b.y1 <= y + h && !boxes.some((o) => b.x0 < o.x1 && b.x1 > o.x0 && b.y0 < o.y1 && b.y1 > o.y0);
  const tryPut = (kind, text, px, py, angle = 0, pad = 3) => {
    const st = LABEL_STYLE[kind], tw = measure(text, st), th = st.size;
    // Hüllrechteck des (gedrehten) Textes
    const c = Math.abs(Math.cos(angle)), s = Math.abs(Math.sin(angle));
    const hw = (tw * c + th * s) / 2 + pad, hh = (tw * s + th * c) / 2 + pad;
    const b = { x0: px - hw, y0: py - hh, x1: px + hw, y1: py + hh };
    if (!free(b)) return false;
    boxes.push(b);
    placed.push({ kind, text, x: px, y: py, angle, ...st });
    return true;
  };
  const points = (kind, list, order = null) => {
    if (!inTier(kind, mpp)) return;
    const items = order ? [...list].sort(order) : list;
    for (const [wx, wy, text] of items) tryPut(kind, text, sx(wx), sy(wy));
  };
  // größere Flächen zuerst (Bezirke/Ortsteile tragen ihre Fläche als 4. Wert)
  const bigFirst = (a, b) => (b[3] ?? 0) - (a[3] ?? 0);
  points('bezirk', data.labels ?? [], bigFirst);
  points('ortsteil', data.ortsteile ?? [], bigFirst);
  if (inTier('station', mpp)) for (const [wx, wy, , text] of data.stations ?? []) tryPut('station', text, sx(wx) + 9 + measure(text, LABEL_STYLE.station) / 2, sy(wy));
  points('kiez', data.kieze ?? []);
  for (const kind of ['mainStreet', 'street']) if (inTier(kind, mpp)) streetLabels(kind, data.streets ?? [], view, tryPut, measure);
  return placed;
}

// Straßennamen: je Straße auf möglichst geraden, ausreichend langen Abschnitten, mittig und entlang der Straße
// (Text nie auf dem Kopf); ein Name wiederholt sich frühestens nach 320 HUD-Pixeln.
function streetLabels(kind, streets, view, tryPut, measure) {
  const { f, ox, oy, x, y, w, h } = view;
  const main = kind === 'mainStreet';
  const wx0 = (x - ox) / f, wy0 = (y - oy) / f, wx1 = (x + w - ox) / f, wy1 = (y + h - oy) / f;
  const cands = [];
  for (const s of streets) {
    if (main ? s.cls > 5 : s.cls <= 5) continue;
    if (s.x1 < wx0 || s.x0 > wx1 || s.y1 < wy0 || s.y0 > wy1) continue;
    const tw = measure(s.name, LABEL_STYLE[kind]) + 16;
    // fast gerade Läufe (Richtungsänderung < 0,3 rad) zusammenfassen
    const p = s.pts;
    let i0 = 0;
    for (let i = 2; i <= p.length - 2; i += 2) {
      const end = i === p.length - 2;
      const bend = !end && Math.abs(angleDiff(Math.atan2(p[i + 3] - p[i + 1], p[i + 2] - p[i]), Math.atan2(p[i + 1] - p[i0 + 1], p[i] - p[i0]))) > 0.3;
      if (!end && !bend) continue;
      const len = Math.hypot(p[i] - p[i0], p[i + 1] - p[i0 + 1]) * f;
      if (len >= tw) cands.push({ name: s.name, len, ax: ox + p[i0] * f, ay: oy + p[i0 + 1] * f, bx: ox + p[i] * f, by: oy + p[i + 1] * f });
      i0 = i;
    }
  }
  cands.sort((a, b) => b.len - a.len || (a.name < b.name ? -1 : 1));
  const done = new Map();
  for (const c of cands) {
    const mx = (c.ax + c.bx) / 2, my = (c.ay + c.by) / 2;
    const prev = done.get(c.name) ?? [];
    if (prev.some(([px, py]) => Math.hypot(px - mx, py - my) < 320)) continue;
    let a = Math.atan2(c.by - c.ay, c.bx - c.ax);
    if (a > Math.PI / 2) a -= Math.PI; else if (a < -Math.PI / 2) a += Math.PI; // lesbar, nicht kopfüber
    if (tryPut(kind, c.name, mx, my, a, 2)) { prev.push([mx, my]); done.set(c.name, prev); }
  }
}

function angleDiff(a, b) { let d = a - b; while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI; return d; }
