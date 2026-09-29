// Tunnelansicht: unter Tage wird die Stadt abgedunkelt; U-/S-Bahn-Linien, die im Bild unter Tage liegen, erscheinen als
// Betonröhre mit Gleisen und Lichtern, Bahnhöfe als helle Bahnsteige mit Namen, Züge in der Röhre werden gezeichnet.
// fade = world.underground (0…1). Nur Darstellung.
import { positionAt, pointOn, trainCars, TRAIN } from './transit.js';
import { undergroundAtS } from './tunnel.js';
import { drawTrainCar } from './railart.js';

const TUBE = '#3a3d44', LIGHT = 'rgba(255,236,190,0.85)';

export function drawTunnels(ctx, world, v, t, fade) {
  const out = { tubes: 0, platforms: 0, trains: 0, platformAt: [] };
  if (fade <= 0.01) return out;
  const tr = world.city.transit, st = world.transit;
  if (!tr || !st) return out;
  ctx.save();
  ctx.globalAlpha = 0.72 * fade; ctx.fillStyle = '#07080c'; ctx.fillRect(v.x, v.y, v.w, v.h);
  ctx.globalAlpha = fade;
  const own = world.player.ride ? (world.player.ride.ref.playerTrain ? world.playerTrain?.pid : world.player.ride.ref.pid) : null;
  const inView = (x, y, pad = 300) => x > v.x - pad && x < v.x + v.w + pad && y > v.y - pad && y < v.y + v.h + pad;
  const drawn = new Set();
  // Linienvarianten derselben Richtung haben fast deckungsgleiche Wege: ein Bahnsteig je Richtung (Hin- und
  // Rückrichtung liegen ~3 m versetzt in eigenen Röhren), der Name je Bahnhof nur einmal
  const stations = [];
  const seenStation = (name, x, y) => stations.some((q) => q.name === name && Math.hypot(q.x - x, q.y - y) < 400);
  const seenPlatform = (name, x, y, a) => out.platformAt.some((q) => q.name === name && Math.hypot(q.sx - x, q.sy - y) < 400 && Math.cos(q.a - a) > 0.87);
  const labels = []; // Bahnhofsnamen zuletzt, sonst liegen spätere Röhren und Züge darüber
  const cleanName = (n) => (n ?? '').replace(/^[SU]\s+/, '').replace(/\s*\(.*\)$/, '');
  for (const [pid] of st.tracked) {
    const p = tr.patterns[pid];
    if (p.mode !== 'ubahn' && p.mode !== 'sbahn') continue;
    if (drawn.has(p.shape)) continue;
    // Abschnitte unter Tage im Bild, in 60-px-Schritten
    const W = TRAIN[p.mode].W + 20, pts = [];
    const flush = () => {
      if (pts.length < 4) { pts.length = 0; return; }
      ctx.lineCap = 'round'; ctx.lineJoin = 'round';
      ctx.strokeStyle = TUBE; ctx.lineWidth = W; ctx.globalAlpha = fade * (pid === own ? 1 : 0.7);
      ctx.beginPath(); ctx.moveTo(pts[0], pts[1]); for (let i = 2; i < pts.length; i += 2) ctx.lineTo(pts[i], pts[i + 1]); ctx.stroke();
      ctx.strokeStyle = '#1c1e22'; ctx.lineWidth = 6; ctx.stroke();
      out.tubes++; pts.length = 0;
    };
    for (let s = 0; s <= p.shape.len; s += 60) {
      const q = pointOn(p, s);
      if (inView(q.x, q.y) && undergroundAtS(world.city, p, s)) {
        pts.push(q.x, q.y);
        if (Math.round(s / 60) % 4 === 0) { ctx.fillStyle = LIGHT; ctx.fillRect(q.x - 1.5, q.y - 1.5, 3, 3); }
      } else flush();
    }
    flush();
    drawn.add(p.shape);
    // Bahnsteige unter Tage: vom Haltepunkt (dort steht die Zugspitze) eine Zuglänge nach hinten
    const len = TRAIN[p.mode].cars * TRAIN[p.mode].carL + (TRAIN[p.mode].cars - 1) * TRAIN[p.mode].gap;
    p.stops.forEach((sv, i) => {
      const q = pointOn(p, sv);
      if (!inView(q.x, q.y) || !undergroundAtS(world.city, p, sv)) return;
      const name = cleanName(p.stopNames[i]);
      const a0 = Math.max(0, sv - len); // am Linienanfang gibt es hinter dem Halt keine Strecke
      const c = pointOn(p, a0 + len / 2), cx = c.x, cy = c.y;
      if (seenPlatform(name, q.x, q.y, q.angle)) return;
      out.platformAt.push({ x: cx, y: cy, a: c.angle, name, sx: q.x, sy: q.y });
      // Bahnsteigkanten folgen dem Linienweg (die Strecke kann im Bahnhof gebogen sein)
      const edge = (off) => { ctx.beginPath(); for (let d = 0; d <= len; d += 40) { const r = pointOn(p, a0 + Math.min(d, len)), nx = -Math.sin(r.angle), ny = Math.cos(r.angle); if (d === 0) ctx.moveTo(r.x + nx * off, r.y + ny * off); else ctx.lineTo(r.x + nx * off, r.y + ny * off); } ctx.stroke(); };
      ctx.lineCap = 'butt'; ctx.lineJoin = 'round';
      ctx.strokeStyle = 'rgba(214,214,206,0.95)'; ctx.lineWidth = 40;
      for (const side of [-1, 1]) edge(side * (W / 2 + 22));
      ctx.strokeStyle = p.color ? `#${p.color.replace('#', '')}` : '#ffd33d'; ctx.lineWidth = 4;
      edge(-(W / 2 + 4)); edge(W / 2 + 4); // Sicherheitslinie an beiden Bahnsteigkanten
      if (!seenStation(name, q.x, q.y)) { stations.push({ name, x: q.x, y: q.y }); labels.push({ name, x: cx, y: cy - W / 2 - 54 }); }
      out.platforms++;
    });
  }
  // Züge in der Röhre (Fahrplan und Spielerzug)
  const drawTrain = (p, s, lit) => { for (const c of trainCars(p, s)) if (inView(c.x, c.y) && undergroundAtS(world.city, p, s)) { drawTrainCar(ctx, c, p.mode, null, lit, t); out.trains++; } };
  for (const [pid, s] of st.tracked) {
    const p = tr.patterns[pid];
    if (p.mode !== 'ubahn' && p.mode !== 'sbahn') continue;
    for (const veh of s.veh) if (!veh.gone) { const pos = positionAt(p, veh.tau); drawTrain(p, pos.s, !pos.dwelling); }
  }
  if (world.playerTrain) { const p = tr.patterns[world.playerTrain.pid]; if (p.mode !== 'tram') drawTrain(p, world.playerTrain.s, true); }
  ctx.fillStyle = '#fff'; ctx.font = 'bold 22px system-ui, sans-serif'; ctx.textAlign = 'center';
  for (const l of labels) ctx.fillText(l.name, l.x, l.y);
  ctx.restore();
  return out;
}
