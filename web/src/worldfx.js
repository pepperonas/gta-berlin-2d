// Begrenzte, rein visuelle Bewegungseffekte. Kein Zugriff auf den Simulations-Zufallsgenerator.
import { makeCanvas } from './lighting.js';
import { FX_BUDGET } from './visualstyle.js';
import { surfaceAt, T } from './map.js';
const TAU = Math.PI * 2;
export function tireEffect(surface, wet, snow, hard) {
  if (snow > 0.2) return 'snow';
  if (wet > 0.2 || surface === T.WATER) return 'spray';
  if (surface === T.GRASS || surface === T.SIDEWALK) return 'dust';
  return hard ? 'smoke' : null;
}
const COLORS = { dust: '173,154,121', smoke: '177,184,187', spray: '190,222,229', snow: '239,245,248' };
const sprites = new Map();
function puff(kind) {
  if (sprites.has(kind)) return sprites.get(kind);
  const c = makeCanvas(64, 64), g = c.getContext('2d'), rgb = COLORS[kind] ?? COLORS.smoke;
  const gr = g.createRadialGradient(28, 25, 2, 32, 32, 31);
  gr.addColorStop(0, `rgba(${rgb},0.65)`); gr.addColorStop(0.4, `rgba(${rgb},0.3)`); gr.addColorStop(1, `rgba(${rgb},0)`);
  g.fillStyle = gr; g.fillRect(0, 0, 64, 64); sprites.set(kind, c); return c;
}
export class WorldEffects {
  constructor() { this.particles = []; this.rings = []; this.emitters = new Map(); this.swimClock = 0; this.serial = 0; this.quality = 'high'; }
  random() { const v = Math.sin(++this.serial * 127.1) * 43758.5453; return v - Math.floor(v); }
  add(kind, x, y, vx = 0, vy = 0, size = 4, life = 0.8) {
    if (this.particles.length >= FX_BUDGET[this.quality].particles) return;
    this.particles.push({ kind, x, y, vx, vy, size, life, max: life });
  }
  ring(x, y, size = 12, life = 1.2) {
    if (this.rings.length >= FX_BUDGET[this.quality].rings) return;
    this.rings.push({ x, y, size, life, max: life });
  }
  handleEvents(events) {
    for (const e of events) {
      if (!['footSplash', 'footLand', 'footJump'].includes(e.type)) continue;
      if (e.type === 'footSplash') this.ring(e.x, e.y, 26, 1.5);
      if (e.surface === T.WATER && e.type !== 'footSplash') continue;
      const kind = e.type === 'footSplash' ? 'spray' : 'dust';
      for (let i = 0, n = e.type === 'footSplash' ? 16 : 5; i < n * FX_BUDGET[this.quality].emission; i++) {
        const a = this.random() * TAU, v = 9 + this.random() * 25;
        this.add(kind, e.x, e.y, Math.cos(a) * v, Math.sin(a) * v, kind === 'spray' ? 3 : 4, 0.5 + this.random() * 0.5);
      }
    }
  }
  update(world, dt, quality) {
    this.quality = quality;
    const budget = FX_BUDGET[quality], p = world.player;
    // Gleiches Emissionstempo bei 30/60/120 Hz; keine Stöße nach einem langen Frame.
    dt = Math.min(0.1, Math.max(0, dt));
    if (p.swimming && !p.inCar && !p.dead && !(p.jumpZ > 0)) {
      this.swimClock += dt;
      const period = p.moveSpeed > 0 ? 0.24 : 1.1;
      if (this.swimClock >= period) { this.swimClock %= period; this.ring(p.x - Math.cos(p.move ?? p.angle) * 5, p.y - Math.sin(p.move ?? p.angle) * 5, p.moveSpeed > 0 ? 18 : 10); }
    } else this.swimClock = 0;
    const live = new Set();
    for (const car of world.cars) {
      if (Math.abs(car.x - world.camera.x) > 1600 || Math.abs(car.y - world.camera.y) > 1100 || car.lvl > 0) continue;
      const speed = Math.hypot(car.vx, car.vy), hard = car.skid > 0.2 || car.controls?.handbrake;
      if (speed < 30) continue;
      const wet = world.wet ?? 0, snow = world.snow ?? 0;
      // Auf trockener Straße ohne Drift entsteht kein Effekt; die Kartenabfrage bleibt dafür aus.
      if (!hard && wet <= 0.2 && snow <= 0.2) continue;
      const kind = tireEffect(surfaceAt(world.city, car.x, car.y, car.lvl), wet, snow, hard);
      if (!kind) continue;
      live.add(car.id); const next = (this.emitters.get(car.id) ?? 0) + dt * (hard ? 14 : 8) * budget.emission;
      this.emitters.set(car.id, next % 1);
      const co = Math.cos(car.angle), si = Math.sin(car.angle);
      for (let i = 0; i < Math.floor(next); i++) for (const side of [-1, 1]) {
        this.add(kind, car.x - co * car.hw * 0.65 - si * side * car.hh, car.y - si * car.hw * 0.65 + co * side * car.hh,
          car.vx * 0.12 - si * side * 9, car.vy * 0.12 + co * side * 9, kind === 'smoke' ? 7 : 4, kind === 'smoke' ? 1.2 : 0.65);
      }
    }
    for (const id of this.emitters.keys()) if (!live.has(id)) this.emitters.delete(id);
    for (const q of this.particles) { q.life -= dt; q.x += q.vx * dt; q.y += q.vy * dt; q.size += dt * (q.kind === 'smoke' ? 13 : 7); }
    this.particles = this.particles.filter(q => q.life > 0).slice(-budget.particles);
    for (const q of this.rings) q.life -= dt;
    this.rings = this.rings.filter(q => q.life > 0).slice(-budget.rings);
  }
  drawWater(ctx) {
    ctx.save();
    for (const q of this.rings) {
      const age = 1 - q.life / q.max, r = 3 + age * q.size;
      ctx.strokeStyle = `rgba(203,233,231,${(1 - age) * 0.48})`; ctx.lineWidth = 0.7;
      ctx.beginPath(); ctx.ellipse(q.x, q.y, r, r * 0.72, 0, 0, TAU); ctx.stroke();
    }
    ctx.restore();
  }
  draw(ctx, view) {
    ctx.save();
    for (const q of this.particles) {
      if (q.x < view.x - 50 || q.x > view.x + view.w + 50 || q.y < view.y - 50 || q.y > view.y + view.h + 50) continue;
      const age = 1 - q.life / q.max; ctx.globalAlpha = Math.sin(Math.PI * age) * (q.kind === 'smoke' ? 0.55 : 0.75);
      ctx.drawImage(puff(q.kind), q.x - q.size, q.y - q.size, q.size * 2, q.size * 2);
    }
    ctx.restore();
  }
}
