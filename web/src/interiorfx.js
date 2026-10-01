import { makeCanvas } from './lighting.js';
let lamp;
export function drawInteriorGlow(ctx, x, y, rx = 60, ry = 38, alpha = 0.35) {
  if (!lamp) {
    lamp = makeCanvas(96, 96); const g = lamp.getContext('2d'), gr = g.createRadialGradient(48, 48, 0, 48, 48, 48);
    gr.addColorStop(0, 'rgba(255,231,180,0.8)'); gr.addColorStop(0.4, 'rgba(245,220,168,0.3)'); gr.addColorStop(1, 'rgba(245,220,168,0)');
    g.fillStyle = gr; g.fillRect(0, 0, 96, 96);
  }
  ctx.save(); ctx.globalCompositeOperation = 'screen'; ctx.globalAlpha *= alpha;
  ctx.drawImage(lamp, x - rx, y - ry, rx * 2, ry * 2); ctx.restore();
}
