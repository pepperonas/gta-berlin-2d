// Darstellung von Straßenbahnen, S- und U-Bahnen (von oben): Wagen als Folge entlang des Linienwegs, Gleise der
// Straßenbahn in der Fahrbahn. Farben allgemein gehalten (keine Firmenzeichen).
import { offsetPolyline } from './geom.js';
import { smallShadow, shade } from './assets.js';

const STYLE = {
  tram: { roof: '#e8e4d8', side: '#f2c230', line: '#3a3a3a' },
  sbahn: { roof: '#5b5f66', side: '#9b2b25', line: '#d9a441' },
  ubahn: { roof: '#6a6457', side: '#f0c419', line: '#3a3a3a' },
};

export function drawTrainCar(ctx, c, mode, sun, lit, t) {
  const st = STYLE[mode] ?? STYLE.tram, L = c.L, W = c.W;
  const [sx, sy, sa] = smallShadow(sun, mode === 'tram' ? 30 : 36);
  ctx.save(); ctx.translate(c.x, c.y);
  ctx.fillStyle = `rgba(0,0,0,${sa})`; ctx.save(); ctx.translate(sx * 0.6, sy * 0.6); ctx.rotate(c.angle); ctx.fillRect(-L / 2, -W / 2, L, W); ctx.restore();
  ctx.rotate(c.angle);
  ctx.fillStyle = st.side; ctx.fillRect(-L / 2, -W / 2, L, W);                         // Wagenkasten (Seiten sichtbar)
  ctx.fillStyle = st.roof; ctx.fillRect(-L / 2 + 2, -W / 2 + 3, L - 4, W - 6);          // Dach
  ctx.fillStyle = shade(st.roof, -0.15);
  for (let x = -L / 2 + 12; x < L / 2 - 10; x += 22) ctx.fillRect(x, -W / 2 + 5, 8, W - 10); // Dachgeräte
  if (mode === 'tram' && c.mid !== false) { ctx.strokeStyle = '#333'; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(-6, -5); ctx.lineTo(6, 5); ctx.moveTo(-6, 5); ctx.lineTo(6, -5); ctx.stroke(); } // Stromabnehmer
  ctx.fillStyle = st.line; ctx.fillRect(-L / 2, -W / 2, L, 1.2); ctx.fillRect(-L / 2, W / 2 - 1.2, L, 1.2);
  if (c.first) { // Führerstand vorn: abgerundet, Scheinwerfer
    ctx.fillStyle = '#26303c'; ctx.fillRect(L / 2 - 5, -W / 2 + 3, 3, W - 6);
    ctx.fillStyle = '#fff6c8'; ctx.fillRect(L / 2 - 1.5, -W / 2 + 2, 1.5, 3); ctx.fillRect(L / 2 - 1.5, W / 2 - 5, 1.5, 3);
  }
  if (c.last) { ctx.fillStyle = '#8a1c1c'; ctx.fillRect(-L / 2, -W / 2 + 2, 1.5, 3); ctx.fillRect(-L / 2, W / 2 - 5, 1.5, 3); }
  ctx.restore();
}

// Straßenbahngleise: zwei Schienen (Spur 1435 mm = 14,35 px) entlang des Linienwegs, je Weg einmal als Path2D
export function tramRails(shape) {
  if (shape._rails) return shape._rails;
  if (typeof Path2D === 'undefined') return (shape._rails = null);
  const p = new Path2D();
  for (const off of [-7.2, 7.2]) {
    const q = offsetPolyline(shape.pts, off);
    p.moveTo(q[0], q[1]);
    for (let i = 2; i < q.length; i += 2) p.lineTo(q[i], q[i + 1]);
  }
  return (shape._rails = p);
}
