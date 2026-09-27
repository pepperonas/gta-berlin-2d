// Darstellung von Rädern (mit Fahrer), E-Rollern, Tauben und Enten – reine Zeichnung, Aussehen aus der Nummer.
import { smallShadow, shade, personLook, drawTorsoHead } from './assets.js';

const FRAMES = ['#1e272e', '#c0392b', '#2980b9', '#27ae60', '#ecf0f1', '#8e44ad', '#16a085', '#f39c12'];
const SCOOTER = ['#2ecc71', '#1abc9c', '#e84393', '#f1c40f']; // Verleihroller in Leihfirmenfarben (ohne Marke)
const hN = (n) => { const x = Math.sin(n * 12.9898 + 4.1414) * 43758.5453; return x - Math.floor(x); };

export function drawBike(ctx, b, shirt, sun, t) {
  ctx.save(); ctx.translate(b.x, b.y);
  const [sx, sy, sa] = smallShadow(sun, 14);
  ctx.fillStyle = `rgba(0,0,0,${sa})`;
  ctx.beginPath(); ctx.ellipse(sx * 0.4, sy * 0.4, 11, 4, b.angle, 0, Math.PI * 2); ctx.fill();
  ctx.rotate(b.angle);
  const scooter = b.kind === 'scooter';
  if (b.state === 'lying') ctx.rotate(1.3); // umgefallen
  if (scooter) {
    ctx.fillStyle = '#2d3436'; ctx.fillRect(-7, -1.6, 12, 3.2);              // Trittbrett
    ctx.fillStyle = SCOOTER[b.seed % SCOOTER.length]; ctx.fillRect(4, -1.2, 3, 2.4); // Lenksäule
    ctx.fillStyle = '#111'; ctx.fillRect(5.5, -4.5, 1.4, 9);                  // Lenker
    ctx.beginPath(); ctx.arc(-7, 0, 1.6, 0, 7); ctx.arc(7, 0, 1.6, 0, 7); ctx.fill();
  } else {
    ctx.fillStyle = '#15171a';
    ctx.fillRect(-10, -0.9, 6, 1.8); ctx.fillRect(4, -0.9, 6, 1.8);          // Reifen von oben
    ctx.strokeStyle = FRAMES[b.seed % FRAMES.length]; ctx.lineWidth = 1.6;
    ctx.beginPath(); ctx.moveTo(-7, 0); ctx.lineTo(7, 0); ctx.stroke();      // Rahmen
    ctx.fillStyle = '#222'; ctx.fillRect(5.5, -4.2, 1.5, 8.4);               // Lenker
    if (b.seed % 3 === 0) { ctx.fillStyle = '#6d4c41'; ctx.fillRect(-11, -2.2, 4, 4.4); } // Korb/Tasche
  }
  if (b.state === 'lying') { ctx.restore(); return; }
  // Fahrer: Rad = sitzend mit tretenden Knien, Roller = stehend
  const look = personLook({ id: b.seed }), skin = ['#f2d0b1', '#e0ac69', '#c68642', '#8d5524', '#f5d6c6'][b.seed % 5];
  if (!scooter) {
    const k = Math.sin(b.pedal) * 2.2;
    ctx.fillStyle = look.pants; ctx.fillRect(-1 + k, -3.2, 5, 2.2); ctx.fillRect(-1 - k, 1, 5, 2.2);
  } else { ctx.fillStyle = look.pants; ctx.fillRect(-3, -2.8, 3.2, 2); ctx.fillRect(-1, 0.8, 3.2, 2); }
  ctx.fillStyle = skin; ctx.fillRect(3, -4.6, 3.4, 1.6); ctx.fillRect(3, 3, 3.4, 1.6);   // Hände am Lenker
  ctx.translate(scooter ? -0.5 : -1.5, 0);
  drawTorsoHead(ctx, shirt, skin, look, false);
  if (!scooter && b.seed % 4 === 0) { ctx.fillStyle = '#e0e0e0'; ctx.beginPath(); ctx.arc(1.5, 0, 3.4, 0, 7); ctx.fill(); } // Helm
  ctx.restore();
}

export function drawParkedScooter(ctx, s) {
  ctx.save(); ctx.translate(s.x, s.y); ctx.rotate(s.angle);
  ctx.fillStyle = 'rgba(0,0,0,0.22)'; ctx.fillRect(-7, s.lying ? -3 : -1, 15, s.lying ? 6 : 4);
  ctx.fillStyle = '#2d3436'; ctx.fillRect(-7, -1.5, 12, 3);
  ctx.fillStyle = SCOOTER[s.seed % SCOOTER.length]; ctx.fillRect(4, -1.2, 3, 2.4);
  ctx.fillStyle = '#111'; if (s.lying) ctx.fillRect(6, 1, 1.4, 9); else ctx.fillRect(5.5, -4.5, 1.4, 9);
  ctx.restore();
}

// Taube: am Boden grau mit schillerndem Hals; im Flug mit schlagenden Flügeln und Schatten am Boden
export function drawBird(ctx, a, sun) {
  const fly = a.state === 'fly' || a.state === 'land', z = a.z ?? 0;
  if (fly) { // Schatten am Boden, versetzt nach Höhe
    ctx.fillStyle = 'rgba(0,0,0,0.18)';
    ctx.beginPath(); ctx.ellipse(a.x + z * 0.35, a.y + z * 0.5, 4, 2.5, a.facing, 0, 7); ctx.fill();
  }
  ctx.save(); ctx.translate(a.x, a.y - z * 0.6); ctx.rotate(a.facing);
  if (a.kind === 'duck') { drawDuckBody(ctx, a); ctx.restore(); return; }
  const g = hN(a.seed) < 0.15 ? '#f2f2f2' : hN(a.seed) < 0.3 ? '#6d5a4c' : '#7d8491'; // wenige weiße und braune
  if (fly) {
    const wing = Math.sin(a.flap * 26) * 5 + 2;
    ctx.fillStyle = shade(g, -0.1);
    ctx.beginPath(); ctx.moveTo(1, 0); ctx.lineTo(-2, -wing - 3); ctx.lineTo(-4, 0); ctx.lineTo(-2, wing + 3); ctx.closePath(); ctx.fill();
  }
  if (!fly) { ctx.fillStyle = 'rgba(0,0,0,0.2)'; ctx.beginPath(); ctx.ellipse(1, 1.5, 3.5, 2, 0, 0, 7); ctx.fill(); }
  ctx.fillStyle = g; ctx.beginPath(); ctx.ellipse(0, 0, 3.6, 2.2, 0, 0, 7); ctx.fill();
  ctx.fillStyle = '#5b6b62'; ctx.beginPath(); ctx.arc(2.6, 0, 1.4, 0, 7); ctx.fill();   // Hals schimmert grünlich
  const peck = !fly && Math.sin(a.flap * 7 + a.seed) > 0.6 ? 1 : 0;
  ctx.fillStyle = '#4a4f57'; ctx.beginPath(); ctx.arc(3.6 + peck, 0, 1.2, 0, 7); ctx.fill();
  ctx.restore();
}

function drawDuckBody(ctx, a) {
  const male = hN(a.seed) < 0.5, bob = Math.sin(a.flap * 1.7 + a.seed) * 0.4;
  ctx.strokeStyle = 'rgba(255,255,255,0.25)'; ctx.lineWidth = 0.8; // Bugwelle
  ctx.beginPath(); ctx.arc(-1, 0, 6 + (Math.abs(a.vx) + Math.abs(a.vy) > 10 ? 2 : 0), 2.2, 4.1); ctx.stroke();
  ctx.fillStyle = male ? '#8d8a82' : '#8b6b4a'; ctx.beginPath(); ctx.ellipse(0, bob, 5, 3, 0, 0, 7); ctx.fill();
  ctx.fillStyle = male ? '#1e6b3a' : '#7a5a3a'; ctx.beginPath(); ctx.arc(4, bob, 1.9, 0, 7); ctx.fill(); // Kopf (Erpel grün)
  ctx.fillStyle = '#e5b12e'; ctx.fillRect(5.4, bob - 0.6, 1.8, 1.2);                               // Schnabel
}
