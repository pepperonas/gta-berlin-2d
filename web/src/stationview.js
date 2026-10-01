// Darstellung eines U-Bahnhofs von innen (station.js) und der Eingänge oben an der Straße. Nur Zeichnung.
// Innen: Fliesenwände in der Bahnhofsfarbe mit Namensschildern, zwei Gleise mit Schwellen, Mittelbahnsteig mit weißer
// Kante, Säulen, Treppen an beiden Enden (Ausgang zur Straße), Fahrgastinfo mit den nächsten Zügen, Wartende, Züge.
import { STATION, toWorld, pillars, trainsAt, departures, waiting } from './station.js';
import { drawTrainCar } from './railart.js';
import { texture } from './textures.js';
import { drawInteriorGlow } from './interiorfx.js';
import { drawPerson } from './people.js';

const U_BLUE = '#1d4f91', S_GREEN = '#008d4f';

// Eingang an der Straße: Treppenschacht mit Geländer, daneben der Mast mit dem blauen U (bzw. grünen S)
export function drawEntrance(ctx, ex, sbahn = false) {
  ctx.save(); ctx.translate(ex.x, ex.y); ctx.rotate(ex.face + Math.PI); // Treppe führt von der Straße weg hinunter
  ctx.fillStyle = '#1b1c20'; ctx.fillRect(-16, -9, 32, 18);           // Schacht
  ctx.strokeStyle = '#5b5d63'; ctx.lineWidth = 1;
  for (let i = -12; i <= 12; i += 4) { ctx.beginPath(); ctx.moveTo(i, -8); ctx.lineTo(i, 8); ctx.stroke(); } // Stufen
  ctx.strokeStyle = '#c9c9c4'; ctx.lineWidth = 2;
  ctx.beginPath(); ctx.moveTo(-16, -10); ctx.lineTo(16, -10); ctx.moveTo(-16, 10); ctx.lineTo(16, 10); ctx.moveTo(16, -10); ctx.lineTo(16, 10); ctx.stroke(); // Geländer
  ctx.restore();
  // Mast mit Schild (nicht gedreht, damit das U lesbar bleibt)
  const px = ex.x + Math.cos(ex.face) * -4 + Math.cos(ex.face + Math.PI / 2) * 14, py = ex.y + Math.sin(ex.face) * -4 + Math.sin(ex.face + Math.PI / 2) * 14;
  ctx.strokeStyle = '#444'; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(px, py); ctx.lineTo(px, py - 16); ctx.stroke();
  ctx.fillStyle = sbahn ? S_GREEN : U_BLUE; ctx.fillRect(px - 7, py - 30, 14, 14);
  ctx.fillStyle = '#fff'; ctx.font = 'bold 11px system-ui, sans-serif'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  ctx.fillText(sbahn ? 'S' : 'U', px, py - 22.5);
}

// Bahnhof von innen. v = sichtbares Rechteck (Weltkoordinaten), t = Spielzeit, player: Spielfigur (zum Zeichnen)
export function drawStation(ctx, world, st, v, t, opts = {}) {
  const out = { trains: 0, people: 0, pillars: 0, boards: 0 };
  ctx.save();
  ctx.fillStyle = '#0e0f12'; ctx.fillRect(v.x - 10, v.y - 10, v.w + 20, v.h + 20); // Erdreich/Dunkel
  ctx.translate(st.x, st.y); ctx.rotate(st.axis);
  const HL = st.HL, H = STATION.half, T = STATION.track, Wl = STATION.wall, end = HL + 30;
  // Wände (Fliesen in der Bahnhofsfarbe), dahinter Beton
  ctx.fillStyle = '#26282d'; ctx.fillRect(-end - 40, -Wl - 30, 2 * end + 80, 2 * Wl + 60);
  ctx.fillStyle = st.color; ctx.fillRect(-end, -Wl, 2 * end, 2 * Wl);
  ctx.strokeStyle = 'rgba(0,0,0,0.12)'; ctx.lineWidth = 0.6;
  ctx.beginPath();
  for (let u = -end; u <= end; u += 6) { ctx.moveTo(u, -Wl); ctx.lineTo(u, -T - 22); ctx.moveTo(u, T + 22); ctx.lineTo(u, Wl); }
  for (const s of [-1, 1]) for (let w = T + 22; w <= Wl; w += 6) { ctx.moveTo(-end, s * w); ctx.lineTo(end, s * w); }
  ctx.stroke();
  // Gleisbetten mit Schwellen und Schienen
  for (const s of [-1, 1]) {
    ctx.fillStyle = '#2a2826'; ctx.fillRect(-end, s * T - 22, 2 * end, 44);
    ctx.fillStyle = '#4a4238';
    for (let u = -end; u < end; u += 11) ctx.fillRect(u, s * T - 13, 4, 26);
    ctx.strokeStyle = '#9a9ea6'; ctx.lineWidth = 2;
    ctx.beginPath(); ctx.moveTo(-end, s * T - 7); ctx.lineTo(end, s * T - 7); ctx.moveTo(-end, s * T + 7); ctx.lineTo(end, s * T + 7); ctx.stroke();
    ctx.strokeStyle = '#6b6f76'; ctx.lineWidth = 3; // Stromschiene
    ctx.beginPath(); ctx.moveTo(-end, s * (T + 16)); ctx.lineTo(end, s * (T + 16)); ctx.stroke();
  }
  // Namensschilder an den Wänden hinter den Gleisen (lesbar von der Bahnsteigmitte)
  ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.font = 'bold 13px system-ui, sans-serif';
  for (let u = -HL + 120; u <= HL - 120; u += 240) for (const s of [-1, 1]) {
    const y = s * (Wl - 14), w = Math.max(90, st.name.length * 8 + 16);
    ctx.fillStyle = '#0f3b73'; ctx.fillRect(u - w / 2, y - 9, w, 18);
    ctx.fillStyle = '#fff'; ctx.fillText(st.name, u, y + 0.5);
  }
  // Züge (vor dem Bahnsteig, damit die Kante darüber liegt)
  const trains = trainsAt(world, st);
  for (const tn of trains) for (let i = 0; i < tn.cars.length; i++) {
    const c = tn.cars[i];
    if (Math.abs(c.u) > end + c.L) continue;
    ctx.save(); ctx.rotate(-st.axis); ctx.translate(-st.x, -st.y); // zurück in Weltkoordinaten
    const wpos = toWorld(st, c.u, c.v);
    drawTrainCar(ctx, { x: wpos.x, y: wpos.y, angle: st.axis + (tn.dir < 0 ? Math.PI : 0), L: c.L, W: c.W, first: i === 0, last: i === tn.cars.length - 1 }, tn.mode, null, !tn.dwelling, t);
    ctx.restore();
    out.trains++;
  }
  // Tunnelmünder an den Enden
  ctx.fillStyle = '#050506';
  for (const s of [-1, 1]) for (const k of [-1, 1]) { ctx.beginPath(); ctx.ellipse(s * (end + 6), k * T, 10, 26, 0, 0, Math.PI * 2); ctx.fill(); }
  // Bahnsteig: Terrazzo, weiße Kante, Treppen an den Enden
  ctx.fillStyle = texture(ctx, 'plaza') ?? '#b8b3a8'; ctx.fillRect(-HL, -H, 2 * HL, 2 * H);
  ctx.fillStyle = 'rgba(0,0,0,0.05)';
  for (let u = -HL; u < HL; u += 20) for (let w = -H; w < H; w += 20) if (((u + w) / 20) % 2 === 0) ctx.fillRect(u, w, 20, 20);
  ctx.fillStyle = '#f2efe6'; ctx.fillRect(-HL, -H, 2 * HL, 4); ctx.fillRect(-HL, H - 4, 2 * HL, 4);
  ctx.strokeStyle = 'rgba(0,0,0,0.25)'; ctx.lineWidth = 1; ctx.setLineDash([6, 4]);
  ctx.beginPath(); ctx.moveTo(-HL, -H + 12); ctx.lineTo(HL, -H + 12); ctx.moveTo(-HL, H - 12); ctx.lineTo(HL, H - 12); ctx.stroke(); ctx.setLineDash([]);
  for (const e of [-1, 1]) {
    const u0 = e > 0 ? HL - STATION.stairL : -HL, sw = STATION.stairW;
    ctx.fillStyle = '#6d6a64'; ctx.fillRect(u0, -sw / 2, STATION.stairL, sw);
    ctx.strokeStyle = '#8d8a83'; ctx.lineWidth = 1.2;
    ctx.beginPath(); for (let u = u0 + 4; u < u0 + STATION.stairL; u += 6) { ctx.moveTo(u, -sw / 2 + 2); ctx.lineTo(u, sw / 2 - 2); } ctx.stroke();
    ctx.strokeStyle = '#d8d5cc'; ctx.lineWidth = 2;
    ctx.beginPath(); ctx.moveTo(u0, -sw / 2); ctx.lineTo(u0 + STATION.stairL, -sw / 2); ctx.moveTo(u0, sw / 2); ctx.lineTo(u0 + STATION.stairL, sw / 2); ctx.stroke();
    // Ausgangsschild über der Treppe
    const ex = st.exits[e < 0 ? 0 : 1], label = `Ausgang ${ex?.street || ''}`.trim();
    const su = e > 0 ? u0 - 8 : u0 + STATION.stairL + 8, w = Math.max(80, label.length * 6.4 + 22);
    ctx.fillStyle = '#123f7a'; ctx.fillRect(su - w / 2, -sw / 2 - 20, w, 15);
    ctx.fillStyle = '#fff'; ctx.font = 'bold 10px system-ui, sans-serif';
    ctx.fillText(`${e > 0 ? '→' : '←'} ${label}`, su, -sw / 2 - 12);
  }
  for (const u of pillars(st)) { drawInteriorGlow(ctx, u, 0, 85, H, 0.38); ctx.fillStyle = '#faf3db'; ctx.fillRect(u - 16, -H + 8, 32, 1.4); ctx.fillRect(u - 16, H - 10, 32, 1.4); }
  // Säulen
  ctx.fillStyle = shade(st.color, -0.35);
  for (const u of pillars(st)) { ctx.fillStyle = 'rgba(9,20,27,0.25)'; ctx.beginPath(); ctx.ellipse(u + 4, 2, STATION.pillar * 1.6, STATION.pillar, 0, 0, Math.PI * 2); ctx.fill(); ctx.fillStyle = shade(st.color, -0.35); ctx.beginPath(); ctx.arc(u, 0, STATION.pillar, 0, Math.PI * 2); ctx.fill(); ctx.fillStyle = 'rgba(255,234,198,0.4)'; ctx.fillRect(u - 4, -4, 1.3, 7); out.pillars++; }
  // Fahrgastinfo (zwei Tafeln): Linie, Ziel, Minuten
  const deps = departures(world, st);
  for (const bu of [-HL / 3, HL / 3]) {
    ctx.fillStyle = '#111317'; ctx.fillRect(bu - 58, -15, 116, 30);
    ctx.font = 'bold 9px ui-monospace, monospace'; ctx.textAlign = 'left';
    deps.slice(0, 2).forEach((d, k) => {
      ctx.fillStyle = '#ffb000';
      const min = d.sec < 45 ? 'sofort' : `${Math.round(d.sec / 60)} min`;
      ctx.fillText(`${d.line} ${d.dest.slice(0, 13)}`, bu - 54, -7 + k * 13);
      ctx.textAlign = 'right'; ctx.fillText(min, bu + 54, -7 + k * 13); ctx.textAlign = 'left';
    });
    ctx.textAlign = 'center';
    out.boards++;
  }
  ctx.restore();
  // Wartende (steigen in einen haltenden Zug auf ihrer Seite ein – dann sind sie weg) und Spielfigur in Weltkoordinaten
  const hour = Math.floor(((world.clock % 1440) + 1440) % 1440 / 60), dwellSide = new Set(trains.filter((x) => x.dwelling).map((x) => x.dir));
  if (st._wait?.hour !== hour) st._wait = { hour, list: waiting(st, hour).map((q) => { const w = toWorld(st, q.u, q.v); return { ...q, x: w.x, y: w.y, angle: st.axis + q.angle, speed: 0, state: 'hang' }; }) };
  for (const q of st._wait.list) {
    if (dwellSide.has(q.side)) continue;
    drawPerson(ctx, q, { time: t, act: 'wait', sun: null });
    out.people++;
  }
  if (opts.player) opts.player();
  // Licht: Leuchtbänder über den Bahnsteigkanten, zu den Tunneln hin dunkler
  if (ctx.createLinearGradient) {
    ctx.save(); ctx.translate(st.x, st.y); ctx.rotate(st.axis);
    const g = ctx.createLinearGradient(-end - 60, 0, end + 60, 0);
    g.addColorStop(0, 'rgba(0,0,0,0.85)'); g.addColorStop(0.12, 'rgba(0,0,0,0)'); g.addColorStop(0.88, 'rgba(0,0,0,0)'); g.addColorStop(1, 'rgba(0,0,0,0.85)');
    ctx.fillStyle = g; ctx.fillRect(-end - 60, -Wl - 30, 2 * end + 120, 2 * Wl + 60);
    ctx.restore();
  }
  return out;
}

function shade(hex, k) {
  const n = parseInt(hex.slice(1), 16), f = (c) => Math.max(0, Math.min(255, Math.round(c * (1 + k))));
  return `rgb(${f(n >> 16)},${f((n >> 8) & 255)},${f(n & 255)})`;
}
