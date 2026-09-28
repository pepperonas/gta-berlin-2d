// Wegweiser an großen Kreuzungen und Kreiseln (Karten-Build, rein). Je Zufahrt ein Schild rechts vor der Kreuzung,
// je Ausfahrt eine Zeile: Richtung (Pfeil), Ziele, Straßennummer.
//  – Kreuzung: Knoten, an denen sich mindestens zwei verschieden benannte Haupt-/Nebenstraßen (Klasse ≤ 4) treffen,
//    dazu alle Kreisel (junction=roundabout/circular). Nahe Knoten (Richtungsfahrbahnen) und alle Knoten eines
//    Kreisels bilden eine Kreuzung.
//  – Ziele: zuerst die echte Beschilderung aus OSM (Relation destination_sign von–über–nach, sonst destination-Tags
//    der ausfahrenden Straße in Fahrtrichtung). Sonst folgt der Build der Straße bis 4 km weit und nennt die ersten
//    Ortsteile, durch die sie führt, dazu „Zentrum“, wenn sie deutlich auf die Mitte zuführt.
//  – Standort: 35 m vor der Kreuzung am rechten Fahrbahnrand, nie auf einer Fahrbahn, nie in einem Haus.
import { segDist2 } from '../../web/src/geom.js';

export const SIGN = { cluster: 45, maxSpan: 180, before: 35, trace: 4000, sample: 100, minNew: 250, zentrum: 1000, zentrumNear: 2500, maxDest: 2 };
const MAIN = 4, EXIT = 5; // Klassen: Kreuzung ab zwei Straßen bis Klasse 4, Ausfahrten bis Klasse 5

const norm = (a) => { while (a > Math.PI) a -= 2 * Math.PI; while (a < -Math.PI) a += 2 * Math.PI; return a; };

// opts: { edges, vertices, names, S, districtAt(x, y), bezirkAt?, center: [x, y], stations: [{ x, y, name }],
//         wayTags(id) → OSM-Tags, destRels: [{ from, via, to, text, ref }] (via = Knotenindex), vIndex (OSM-Knoten → Index),
//         clearance: { worst, place }, inBuilding(x, y), inside(x, y) }
export function buildSigns(opts) {
  const { edges, vertices, names, S, districtAt, center, stations = [], wayTags = () => ({}), destRels = [], clearance, inBuilding = () => false, inside = () => true } = opts;
  const vx = (k) => vertices[2 * k], vy = (k) => vertices[2 * k + 1];
  const pts = (ed) => [vx(ed.a), vy(ed.a), ...ed.p, vx(ed.b), vy(ed.b)];
  const at = new Map();
  edges.forEach((ed, k) => {
    if (ed.c > EXIT || ed.pass || !ed.in) return;
    for (const v of ed.a === ed.b ? [ed.a] : [ed.a, ed.b]) (at.get(v) ?? at.set(v, []).get(v)).push(k);
  });
  const may = (ed, from) => ed.o === 0 || (ed.o === 1 && ed.a === from) || (ed.o === -1 && ed.b === from); // befahrbar ab Knoten from
  // 1) Kreuzungsknoten
  const cand = [];
  for (const [v, ks] of at) {
    const main = ks.filter((k) => edges[k].c <= MAIN && !edges[k].rb);
    const named = new Set(main.map((k) => edges[k].n).filter((n) => n >= 0));
    const rb = ks.some((k) => edges[k].rb) && ks.some((k) => !edges[k].rb);
    if (named.size >= 2 || rb) cand.push(v);
  }
  // 2) Zusammenfassen: nahe Knoten (Richtungsfahrbahnen) und Kreiselringe
  const parent = new Map(cand.map((v) => [v, v]));
  const box = new Map(cand.map((v) => [v, [vx(v), vy(v), vx(v), vy(v)]]));
  const find = (v) => { while (parent.get(v) !== v) { parent.set(v, parent.get(parent.get(v))); v = parent.get(v); } return v; };
  const union = (a, b, force = false) => {
    a = find(a); b = find(b); if (a === b) return;
    const A = box.get(a), B = box.get(b), m = [Math.min(A[0], B[0]), Math.min(A[1], B[1]), Math.max(A[2], B[2]), Math.max(A[3], B[3])];
    if (!force && Math.hypot(m[2] - m[0], m[3] - m[1]) > SIGN.maxSpan * S) return;
    parent.set(b, a); box.set(a, m);
  };
  const G = SIGN.cluster * S, grid = new Map();
  for (const v of cand) { const key = `${Math.floor(vx(v) / G)},${Math.floor(vy(v) / G)}`; (grid.get(key) ?? grid.set(key, []).get(key)).push(v); }
  for (const v of cand) {
    const gx = Math.floor(vx(v) / G), gy = Math.floor(vy(v) / G);
    for (let i = -1; i <= 1; i++) for (let j = -1; j <= 1; j++) for (const u of grid.get(`${gx + i},${gy + j}`) ?? []) {
      if (u > v && Math.hypot(vx(u) - vx(v), vy(u) - vy(v)) < G) union(u, v);
    }
  }
  // Kreiselring: alle Knoten entlang der Kreiselkanten gehören zusammen (auch Knoten, die selbst keine Kandidaten sind)
  const rbEdges = edges.map((ed, k) => k).filter((k) => edges[k].rb && edges[k].in);
  for (const k of rbEdges) for (const v of [edges[k].a, edges[k].b]) if (!parent.has(v)) { parent.set(v, v); box.set(v, [vx(v), vy(v), vx(v), vy(v)]); }
  for (const k of rbEdges) union(edges[k].a, edges[k].b, true);
  const clusters = new Map();
  for (const v of parent.keys()) { const r = find(v); (clusters.get(r) ?? clusters.set(r, []).get(r)).push(v); }
  // Kreisel-Knoten ohne Zu-/Abfahrt allein sind keine Kreuzung
  const signs = [];
  const stats = { junctions: 0, signs: 0, rows: 0, osm: 0, traced: 0, hidden: 0 };
  for (const vs of [...clusters.values()].sort((a, b) => Math.min(...a) - Math.min(...b))) {
    const inC = new Set(vs);
    const cx = vs.reduce((s, v) => s + vx(v), 0) / vs.length, cy = vs.reduce((s, v) => s + vy(v), 0) / vs.length;
    // Rand-Kanten: genau ein Ende in der Kreuzung
    const bound = [];
    for (const v of vs) for (const k of at.get(v) ?? []) {
      const ed = edges[k], other = ed.a === v ? ed.b : ed.a;
      if (inC.has(other)) continue;
      bound.push({ k, v, other });
    }
    const approaches = bound.filter((b) => edges[b.k].c <= EXIT && may(edges[b.k], b.other));
    const exits = bound.filter((b) => edges[b.k].c <= EXIT && may(edges[b.k], b.v));
    if (approaches.length < 2 || exits.length < 2) continue;
    // Richtung eines Rand-Arms, gemessen über bis zu 60 m (vom Kreuzungsknoten nach außen)
    const armDir = (b) => {
      const p = pts(edges[b.k]), fwd = edges[b.k].a === b.v;
      const seq = []; for (let i = 0; i < p.length; i += 2) seq.push([p[i], p[i + 1]]);
      if (!fwd) seq.reverse();
      let len = 0, x = seq[0][0], y = seq[0][1];
      for (let i = 1; i < seq.length && len < 60 * S; i++) { len += Math.hypot(seq[i][0] - seq[i - 1][0], seq[i][1] - seq[i - 1][1]); x = seq[i][0]; y = seq[i][1]; }
      return Math.atan2(y - seq[0][1], x - seq[0][0]);
    };
    // Ausfahrten: gleiche Straße und Richtung zusammenfassen
    const exitRows = [];
    for (const b of exits) {
      const dir = armDir(b), name = edges[b.k].n;
      if (exitRows.some((r) => r.name === name && Math.abs(norm(r.dir - dir)) < 0.45)) continue;
      exitRows.push({ b, dir, name, ...destinationOf(b) });
    }
    if (exitRows.filter((r) => r.dests.length).length < 2) continue;
    stats.junctions++;
    const jName = junctionName(cx, cy, vs);
    for (const a of approaches) {
      const inDir = norm(armDir(a) + Math.PI); // Fahrtrichtung beim Hineinfahren
      const rows = [];
      for (const r of exitRows) {
        const turn = norm(r.dir - inDir);
        if (Math.abs(turn) > 2.6) continue; // Wenden: zurück in dieselbe Straße
        if (r.name === edges[a.k].n && Math.abs(turn) > 2.2) continue;
        const rel = destRels.find((d) => d.from === edges[a.k].id && d.to === edges[r.b.k].id && inC.has(d.via));
        const dests = rel ? rel.text.split(';').map((x) => x.trim()).filter(Boolean).slice(0, 3) : r.dests;
        if (!dests.length) continue;
        rows.push({ dir: r.dir, turn, dests, ref: rel?.ref ?? r.ref, osm: rel ? 1 : r.osm, street: r.name >= 0 ? names[r.name] : '' });
      }
      dedupeRows(rows);
      if (rows.length < 2) continue;
      rows.sort((p, q) => p.turn - q.turn); // links nach rechts
      for (const r of rows) delete r.street;
      const pos = placeSign(a, inDir);
      signs.push({ x: pos.x, y: pos.y, angle: inDir, name: jName, rows, vis: pos.ok ? 1 : 0 });
      stats.signs++; stats.rows += rows.length; if (!pos.ok) stats.hidden++;
      for (const r of rows) if (r.osm) stats.osm++; else stats.traced++;
    }

    // Ziele einer Ausfahrt: OSM-Beschilderung oder Verfolgung der Straße
    function destinationOf(b) {
      const ed = edges[b.k], t = wayTags(ed.id) ?? {}, fwd = ed.a === b.v; // Fahrt entlang der Weg-Richtung?
      const refTag = (t.ref ?? '').split(';').map((x) => x.trim()).find((x) => /^[AB] ?\d/.test(x)) ?? '';
      const dirTag = (k) => t[`${k}:${fwd ? 'forward' : 'backward'}`] ?? (ed.o !== 0 || fwd ? t[k] : undefined);
      const osm = dirTag('destination');
      if (osm) return { dests: osm.split(';').map((x) => x.trim()).filter(Boolean).slice(0, 3), ref: dirTag('destination:ref')?.split(';')[0]?.trim() || refTag, osm: 1 };
      return { dests: traceDests(b), ref: refTag, osm: 0 };
    }
    function traceDests(b) {
      const here = districtAt(cx, cy);
      const seen = [], out = [];
      let ed = edges[b.k], from = b.v, len = 0, next = SIGN.sample * S / 2, endX = cx, endY = cy;
      const visited = new Set();
      while (ed && len < SIGN.trace * S && !visited.has(ed)) {
        visited.add(ed);
        const p = pts(ed), seq = [];
        for (let i = 0; i < p.length; i += 2) seq.push([p[i], p[i + 1]]);
        if (ed.a !== from) seq.reverse();
        for (let i = 1; i < seq.length; i++) {
          const L = Math.hypot(seq[i][0] - seq[i - 1][0], seq[i][1] - seq[i - 1][1]);
          while (next <= len + L) {
            const f = (next - len) / L, x = seq[i - 1][0] + (seq[i][0] - seq[i - 1][0]) * f, y = seq[i - 1][1] + (seq[i][1] - seq[i - 1][1]) * f;
            const d = districtAt(x, y);
            if (d && d !== here && !seen.includes(d) && next >= SIGN.minNew * S) seen.push(d);
            next += SIGN.sample * S;
          }
          len += L;
          endX = seq[i][0]; endY = seq[i][1];
        }
        const v = ed.a === from ? ed.b : ed.a;
        const inDir = Math.atan2(seq[seq.length - 1][1] - seq[seq.length - 2][1], seq[seq.length - 1][0] - seq[seq.length - 2][0]);
        // weiter: gleiche Straße, sonst die geradeste Hauptstraße
        let best = null;
        for (const k of at.get(v) ?? []) {
          const e2 = edges[k];
          if (e2 === ed || !may(e2, v) || e2.c > EXIT) continue;
          const q = pts(e2), s0 = e2.a === v ? [q[0], q[1], q[2], q[3]] : [q[q.length - 2], q[q.length - 1], q[q.length - 4], q[q.length - 3]];
          const turn = Math.abs(norm(Math.atan2(s0[3] - s0[1], s0[2] - s0[0]) - inDir));
          const score = (e2.n === ed.n && e2.n >= 0 ? 0 : 2) + turn + (e2.c - ed.c) * 0.3;
          if (turn < 1.4 && (!best || score < best.score)) best = { e2, score };
        }
        from = v; ed = best?.e2;
        if (seen.length >= SIGN.maxDest) break;
      }
      for (const d of seen.slice(0, SIGN.maxDest)) out.push(d);
      // Richtung Zentrum?
      if (center && Math.hypot(cx - center[0], cy - center[1]) > SIGN.zentrumNear * S
        && Math.hypot(cx - center[0], cy - center[1]) - Math.hypot(endX - center[0], endY - center[1]) > SIGN.zentrum * S) out.unshift('Zentrum');
      if (!out.length && edges[b.k].n >= 0) out.push(names[edges[b.k].n]); // bleibt im Ortsteil: Straßenname
      return out.slice(0, 3);
    }
    function junctionName(x, y, vs2) {
      let best = null;
      for (const st of stations) { const d = Math.hypot(st.x - x, st.y - y); if (d < 120 * S && (!best || d < best.d)) best = { d, name: st.name }; }
      if (best) return best.name;
      const ns = [];
      for (const v of vs2) for (const k of at.get(v) ?? []) { const e = edges[k]; if (e.c <= MAIN && e.n >= 0 && !ns.includes(names[e.n])) ns.push(names[e.n]); }
      return ns.slice(0, 2).join(' / ');
    }
    // Standort: auf der Zufahrt SIGN.before vor dem Kreuzungsknoten, rechts neben der Fahrbahn
    function placeSign(a, inDir) {
      const p = pts(edges[a.k]), fwd = edges[a.k].a === a.v, seq = [];
      for (let i = 0; i < p.length; i += 2) seq.push([p[i], p[i + 1]]);
      if (!fwd) seq.reverse(); // ab dem Kreuzungsknoten nach außen
      const total = seq.reduce((s, q, i) => (i ? s + Math.hypot(q[0] - seq[i - 1][0], q[1] - seq[i - 1][1]) : 0), 0);
      const want = Math.min(SIGN.before * S, total * 0.7);
      let len = 0, x = seq[0][0], y = seq[0][1], ux = Math.cos(inDir), uy = Math.sin(inDir);
      for (let i = 1; i < seq.length; i++) {
        const L = Math.hypot(seq[i][0] - seq[i - 1][0], seq[i][1] - seq[i - 1][1]);
        if (len + L >= want) { const f = (want - len) / L; x = seq[i - 1][0] + (seq[i][0] - seq[i - 1][0]) * f; y = seq[i - 1][1] + (seq[i][1] - seq[i - 1][1]) * f; ux = -(seq[i][0] - seq[i - 1][0]) / L; uy = -(seq[i][1] - seq[i - 1][1]) / L; break; }
        len += L;
      }
      const half = edges[a.k].w / 10 * S / 2, rx = -uy, ry = ux; // rechts der Fahrtrichtung (y nach unten)
      for (const extra of [1.3, 2.3, 3.5, 5]) {
        let qx = x + rx * (half + extra * S), qy = y + ry * (half + extra * S);
        if (clearance) { if (clearance.worst(qx, qy)) { const pl = clearance.place(qx, qy); if (!pl) continue; [qx, qy] = pl; } }
        if (!inside(qx, qy) || inBuilding(qx, qy)) continue;
        return { x: Math.round(qx), y: Math.round(qy), ok: true };
      }
      return { x: Math.round(x + rx * (half + 1.3 * S)), y: Math.round(y + ry * (half + 1.3 * S)), ok: false };
    }
  }
  return { signs, stats };
}

// Jedes Ziel höchstens einmal je Schild: die wichtigste Zeile bekommt es (echte Beschilderung, dann geradeaus, dann die
// kleineren Abbiegungen); leer gewordene Zeilen nennen ihren Straßennamen, ist auch der vergeben, entfallen sie.
export function dedupeRows(rows) {
  const order = [...rows].sort((p, q) => (q.osm ?? 0) - (p.osm ?? 0) || Math.abs(p.turn) - Math.abs(q.turn));
  const used = new Set(), keep = new Set();
  for (const r of order) {
    r.dests = r.dests.filter((d) => !used.has(d));
    if (!r.dests.length && r.street && !used.has(r.street)) r.dests = [r.street];
    if (!r.dests.length) continue;
    for (const d of r.dests) used.add(d);
    keep.add(r);
  }
  const out = rows.filter((r) => keep.has(r));
  rows.length = 0; rows.push(...out);
  return rows;
}

// Hilfen für den Build: Relationen destination_sign → { from, via, to, text, ref }
export function destinationRelations(osm, vIndex) {
  const out = [];
  for (const r of osm.relations) {
    if (r.tags?.type !== 'destination_sign' || !r.tags.destination) continue;
    const from = r.members.find((m) => m.role === 'from' && m.type === 'way'), to = r.members.find((m) => m.role === 'to' && m.type === 'way');
    const via = r.members.find((m) => (m.role === 'intersection' || m.role === 'sign') && m.type === 'node');
    if (!from || !to || !via || !vIndex.has(via.ref)) continue;
    out.push({ from: from.ref, to: to.ref, via: vIndex.get(via.ref), text: r.tags.destination, ref: (r.tags['destination:ref'] ?? '').split(';')[0].trim() });
  }
  return out;
}

export { segDist2 };
