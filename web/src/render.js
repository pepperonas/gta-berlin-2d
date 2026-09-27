// Welt-Rendering in schräger Draufsicht: Boden flach, Gebäude als Quader mit sichtbarer Südfassade
// und leichter Parallaxe (Dach wandert von der Bildmitte weg → Seitenwände werden sichtbar).
import { TILE, MAP_W, MAP_H, MISSION } from './config.js';
import { T } from './map.js';
import { drawCar, drawPerson, drawTree, shade } from './assets.js';
import { playerCar, speedOf } from './world.js';

const GROUND = {
  [T.SIDEWALK]: ['#a19d95', '#9a968e'],
  [T.GRASS]: ['#5d9340', '#588c3c'],
  [T.WATER]: ['#2c6c98', '#2c6c98'],
  [T.PLAZA]: ['#65686d', '#62656a'],
  [T.BUILDING]: ['#55575b', '#55575b'],
};

export class Renderer {
  constructor(ctx) {
    this.ctx = ctx;
    this.skids = [];
    this.particles = [];
  }

  // Ereignisse der Simulation in Effekte übersetzen.
  handleEvents(events) {
    for (const e of events) {
      if (e.type === 'crash') for (let i = 0; i < 6 + e.strength * 14; i++) {
        const a = Math.random() * Math.PI * 2, v = 60 + Math.random() * 160 * e.strength;
        this.particles.push({ kind: 'spark', x: e.x, y: e.y, vx: Math.cos(a) * v, vy: Math.sin(a) * v, life: 0.35, max: 0.35 });
      }
      if (e.type === 'wreck') for (let i = 0; i < 20; i++) {
        this.particles.push({ kind: 'smoke', x: e.x + (Math.random() - 0.5) * 20, y: e.y, vx: (Math.random() - 0.5) * 30, vy: -20 - Math.random() * 30, life: 1.6, max: 1.6, r: 6 });
      }
    }
  }

  update(world, dt) {
    for (const c of world.cars) {
      const hard = c.skid > 0.2 || (c.controls.handbrake && speedOf(c) > 60) || (c.controls.brake > 0.8 && speedOf(c) > 150 && c.driver);
      if (hard) {
        const s = Math.sin(c.angle), co = Math.cos(c.angle);
        for (const side of [-1, 1]) {
          const x = c.x - co * (c.hw - 6) - s * side * (c.hh - 3), y = c.y - s * (c.hw - 6) + co * side * (c.hh - 3);
          const key = `${c.id}${side}`;
          const last = this._lastSkid?.[key];
          if (last && Math.hypot(last.x - x, last.y - y) < 30) this.skids.push({ x0: last.x, y0: last.y, x1: x, y1: y, life: 8 });
          (this._lastSkid ??= {})[key] = { x, y };
        }
      } else if (this._lastSkid) { delete this._lastSkid[`${c.id}-1`]; delete this._lastSkid[`${c.id}1`]; }
      if ((c.health < 35 || c.wrecked) && Math.random() < (c.wrecked ? 0.35 : 0.15)) {
        this.particles.push({ kind: 'smoke', x: c.x + Math.cos(c.angle) * c.hw * 0.7, y: c.y + Math.sin(c.angle) * c.hw * 0.7, vx: (Math.random() - 0.5) * 10, vy: -18, life: 1.4, max: 1.4, r: 4 });
      }
    }
    if (this.skids.length > 600) this.skids.splice(0, this.skids.length - 600);
    for (const s of this.skids) s.life -= dt;
    this.skids = this.skids.filter((s) => s.life > 0);
    for (const p of this.particles) {
      p.life -= dt; p.x += p.vx * dt; p.y += p.vy * dt;
      if (p.kind === 'spark') { p.vx *= 0.9; p.vy *= 0.9; } else p.r += dt * 8;
    }
    this.particles = this.particles.filter((p) => p.life > 0);
  }

  draw(world, W, H, scale, overlayMarkers = true) {
    const ctx = this.ctx, cam = world.camera, city = world.city;
    const s = scale * cam.zoom;
    ctx.setTransform(s, 0, 0, s, W / 2 - cam.x * s, H / 2 - cam.y * s);
    const vw = W / s, vh = H / s;
    const v = { x: cam.x - vw / 2, y: cam.y - vh / 2, w: vw, h: vh };
    const t = world.time;

    // 1) Asphalt
    ctx.fillStyle = '#3b3e43';
    ctx.fillRect(v.x - 5, v.y - 5, v.w + 10, v.h + 10);

    // 2) Bodenkacheln (Gehweg, Gras, Wasser, Plätze)
    const tx0 = Math.max(0, Math.floor(v.x / TILE)), ty0 = Math.max(0, Math.floor(v.y / TILE));
    const tx1 = Math.min(MAP_W - 1, Math.ceil((v.x + v.w) / TILE)), ty1 = Math.min(MAP_H - 1, Math.ceil((v.y + v.h + 160) / TILE));
    for (let ty = ty0; ty <= ty1; ty++) for (let tx = tx0; tx <= tx1; tx++) {
      const tt = city.tiles[ty * MAP_W + tx];
      if (tt === T.ROAD) continue;
      ctx.fillStyle = GROUND[tt][(tx + ty) & 1];
      ctx.fillRect(tx * TILE, ty * TILE, TILE + 0.5, TILE + 0.5);
    }
    // Wasser: Wellen + Kaikante
    for (const wa of city.waters) {
      if (!overlap(wa, v)) continue;
      ctx.strokeStyle = 'rgba(255,255,255,0.12)'; ctx.lineWidth = 1.5;
      for (let y = wa.y + 10; y < wa.y + wa.h; y += 22) {
        ctx.beginPath();
        for (let x = wa.x; x <= wa.x + wa.w; x += 12) ctx.lineTo(x, y + Math.sin(x * 0.05 + t * 1.5 + y) * 2);
        ctx.stroke();
      }
      ctx.strokeStyle = '#6f6a60'; ctx.lineWidth = 3; ctx.strokeRect(wa.x, wa.y, wa.w, wa.h);
    }
    // Bordsteinkanten
    ctx.strokeStyle = '#c9c5bb'; ctx.lineWidth = 1.5;
    for (const b of city.blocks) if (overlap(b, v)) ctx.strokeRect(b.x, b.y, b.w, b.h);

    this.drawMarkings(world, v);
    this.drawLots(world, v);

    // 3) Bremsspuren
    ctx.lineWidth = 3; ctx.lineCap = 'round';
    for (const k of this.skids) {
      ctx.strokeStyle = `rgba(20,20,20,${Math.min(0.5, k.life / 8 * 0.5)})`;
      ctx.beginPath(); ctx.moveTo(k.x0, k.y0); ctx.lineTo(k.x1, k.y1); ctx.stroke();
    }
    ctx.lineCap = 'butt';

    // 4) Missionsmarker am Boden
    if (overlayMarkers) this.drawZones(world);

    // 5) Tiefensortierte Objekte
    const list = [];
    const margin = 220;
    const near = (x, y) => x > v.x - margin && x < v.x + v.w + margin && y > v.y - margin && y < v.y + v.h + margin * 1.5;
    for (const b of city.buildings) if (b.x < v.x + v.w + margin && b.x + b.w > v.x - margin && b.y < v.y + v.h + margin && b.y + b.h > v.y - 40) list.push({ y: b.y + b.h, d: () => this.drawBuilding(b, cam, t) });
    for (const tr of city.trees) if (near(tr.x, tr.y)) list.push({ y: tr.y, d: () => drawTree(ctx, tr, t) });
    for (const cr of city.crates) if (near(cr.x, cr.y)) list.push({ y: cr.y + cr.h, d: () => drawCrate(ctx, cr) });
    for (const c of world.cars) if (near(c.x, c.y)) list.push({ y: c.y + 6, d: () => drawCar(ctx, c, t) });
    for (const p of world.peds) if (near(p.x, p.y)) list.push({ y: p.y, d: () => drawPerson(ctx, p, { shirt: p.shirt, skin: p.skin, down: p.state === 'down' }) });
    const pl = world.player;
    if (!pl.inCar) list.push({ y: pl.y, d: () => drawPerson(ctx, pl, { shirt: '#ff7a1a', player: true, down: pl.stun > 0 }) });
    list.sort((a, b) => a.y - b.y);
    for (const it of list) it.d();

    // 6) Partikel
    for (const p of this.particles) {
      const a = p.life / p.max;
      if (p.kind === 'spark') { ctx.fillStyle = `rgba(255,${180 + (a * 75) | 0},60,${a})`; ctx.fillRect(p.x - 1, p.y - 1, 2.5, 2.5); }
      else { ctx.fillStyle = `rgba(70,70,70,${a * 0.45})`; ctx.beginPath(); ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2); ctx.fill(); }
    }
    // Spieler-Markierung über dem Dach, falls er hinter einem Haus verschwindet
    if (!pl.inCar) {
      ctx.fillStyle = 'rgba(255,122,26,0.9)';
      ctx.beginPath(); ctx.moveTo(pl.x, pl.y - 12); ctx.lineTo(pl.x - 4, pl.y - 19); ctx.lineTo(pl.x + 4, pl.y - 19); ctx.fill();
    }
    ctx.setTransform(1, 0, 0, 1, 0, 0);
  }

  drawMarkings(world, v) {
    const ctx = this.ctx, city = world.city, rw = city.roadW;
    ctx.fillStyle = 'rgba(245,245,235,0.75)';
    // Mittellinien gestrichelt, nur zwischen Kreuzungen
    for (let i = 0; i < city.vRoads.length; i++) {
      const x = city.vRoads[i] + rw / 2;
      if (x < v.x - 10 || x > v.x + v.w + 10) continue;
      for (let j = 0; j < city.hRoads.length - 1; j++) {
        const y0 = city.hRoads[j] + rw, y1 = city.hRoads[j + 1];
        for (let y = y0 + 10; y < y1 - 14; y += 34) ctx.fillRect(x - 1, y, 2, 16);
        zebra(ctx, city.vRoads[i], y0 + 2, rw, false);
        zebra(ctx, city.vRoads[i], y1 - 12, rw, false);
      }
    }
    for (let j = 0; j < city.hRoads.length; j++) {
      const y = city.hRoads[j] + rw / 2;
      if (y < v.y - 10 || y > v.y + v.h + 10) continue;
      for (let i = 0; i < city.vRoads.length - 1; i++) {
        const x0 = city.vRoads[i] + rw, x1 = city.vRoads[i + 1];
        for (let x = x0 + 10; x < x1 - 14; x += 34) ctx.fillRect(x, y - 1, 16, 2);
        zebra(ctx, x0 + 2, city.hRoads[j], rw, true);
        zebra(ctx, x1 - 12, city.hRoads[j], rw, true);
      }
    }
  }

  drawLots(world, v) {
    const ctx = this.ctx, p = world.city.places;
    // Parkplatz am Späti: Stellplatzlinien
    const d = p.dropoff;
    ctx.strokeStyle = 'rgba(255,255,255,0.5)'; ctx.lineWidth = 1.5;
    for (let k = -2; k <= 2; k++) { ctx.beginPath(); ctx.moveTo(d.x + k * 30 + 15, d.y - 60); ctx.lineTo(d.x + k * 30 + 15, d.y - 25); ctx.stroke(); }
    const pk = p.pickup;
    ctx.fillStyle = 'rgba(255,210,0,0.25)';
    for (let k = -3; k <= 3; k++) ctx.fillRect(pk.x + k * 14 - 3, pk.y + 60, 6, 14);
  }

  drawZones(world) {
    const ctx = this.ctx, m = world.mission, p = world.city.places, t = world.time;
    const ring = (pt, color, r) => {
      const pulse = 1 + Math.sin(t * 4) * 0.06;
      ctx.fillStyle = color.replace('A', '0.18'); ctx.beginPath(); ctx.arc(pt.x, pt.y, r * pulse, 0, Math.PI * 2); ctx.fill();
      ctx.strokeStyle = color.replace('A', '0.9'); ctx.lineWidth = 2.5; ctx.setLineDash([10, 7]); ctx.lineDashOffset = -t * 20;
      ctx.beginPath(); ctx.arc(pt.x, pt.y, r * pulse, 0, Math.PI * 2); ctx.stroke(); ctx.setLineDash([]);
    };
    if (m.state === 'available') ring(p.giver, 'rgba(255,210,0,A)', MISSION.giverRadius);
    if (m.state === 'toPickup') ring(p.pickup, 'rgba(255,210,0,A)', MISSION.zoneRadius);
    if (m.state === 'toDropoff') ring(p.dropoff, 'rgba(80,220,120,A)', MISSION.zoneRadius);
  }

  drawBuilding(b, cam, t) {
    const ctx = this.ctx;
    const H = b.height;
    const cx = b.x + b.w / 2, cy = b.y + b.h / 2;
    const dx = (cx - cam.x) * H * 0.0005;
    const dy = -H * 0.5 + (cy - cam.y) * H * 0.00025;
    const x = b.x, y = b.y, w = b.w, h = b.h;
    const wall = b.wall;
    // Seitenwand
    if (Math.abs(dx) > 0.5) {
      const sx = dx > 0 ? x : x + w;
      ctx.fillStyle = shade(wall, -0.38);
      ctx.beginPath(); ctx.moveTo(sx, y); ctx.lineTo(sx, y + h); ctx.lineTo(sx + dx, y + h + dy); ctx.lineTo(sx + dx, y + dy); ctx.closePath(); ctx.fill();
    }
    // Südfassade mit Fenstern (affin auf das Parallelogramm abgebildet)
    ctx.save();
    ctx.transform(1, 0, dx / H, dy / H, x, y + h);
    ctx.fillStyle = shade(wall, -0.18);
    ctx.fillRect(0, 0, w, H);
    const floors = Math.max(1, Math.floor(H / 17));
    for (let f = 0; f < floors; f++) {
      const vy = 6 + f * 17;
      if (b.kind === 'spaeti' && f === 0) continue;
      for (let u = 6; u < w - 8; u += 14) {
        const lit = ((b.seed >> ((u + f * 7) % 24)) & 3) === 0;
        ctx.fillStyle = lit ? '#f7e3a1' : '#2d3440';
        ctx.fillRect(u, vy, 7, 9);
      }
    }
    if (b.kind === 'spaeti') {
      ctx.fillStyle = '#9fd3ff'; ctx.fillRect(6, 2, w - 30, 13);
      ctx.fillStyle = '#6b3f1d'; ctx.fillRect(w - 20, 0, 11, 16);
      ctx.fillStyle = '#e03b3b'; ctx.fillRect(0, 15, w, 12);
      ctx.save(); ctx.scale(1, -1); ctx.fillStyle = '#fff'; ctx.font = 'bold 10px Segoe UI, system-ui, sans-serif'; ctx.textAlign = 'center';
      ctx.fillText('SPÄTI 24/7', w / 2, -17); ctx.restore();
    } else if (b.kind === 'warehouse') {
      ctx.fillStyle = '#5a636b';
      for (let u = 20; u < w - 50; u += 70) ctx.fillRect(u, 0, 44, 34);
      ctx.fillStyle = '#434a50';
      for (let u = 20; u < w - 50; u += 70) for (let k = 4; k < 34; k += 6) ctx.fillRect(u, k, 44, 1.5);
    } else {
      ctx.fillStyle = '#4a3a2c'; ctx.fillRect(Math.min(w / 2, 20), 0, 10, 14);
    }
    ctx.restore();
    // Dach
    const rx = x + dx, ry = y + dy;
    ctx.fillStyle = shade(wall, b.kind === 'warehouse' ? -0.05 : 0.08);
    ctx.fillRect(rx, ry, w, h);
    ctx.strokeStyle = shade(wall, -0.3); ctx.lineWidth = 2; ctx.strokeRect(rx + 1, ry + 1, w - 2, h - 2);
    // Dachaufbauten (deterministisch aus seed)
    let sd = b.seed;
    const rnd = () => ((sd = (sd * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
    const n = Math.floor(w * h / 9000) + 1;
    for (let k = 0; k < n; k++) {
      const ux = rx + 8 + rnd() * (w - 30), uy = ry + 8 + rnd() * (h - 30), sz = 8 + rnd() * 12;
      ctx.fillStyle = shade(wall, -0.25); ctx.fillRect(ux + 2, uy + 2, sz, sz * 0.8);
      ctx.fillStyle = shade(wall, -0.05); ctx.fillRect(ux, uy, sz, sz * 0.8);
    }
    if (b.kind === 'warehouse') {
      ctx.fillStyle = '#e8e8e8'; ctx.font = 'bold 18px Segoe UI, system-ui, sans-serif'; ctx.textAlign = 'center';
      ctx.fillText('LAGER 7', rx + w / 2, ry + h / 2 + 6);
    }
  }
}

function zebra(ctx, x, y, size, vertical) {
  ctx.fillStyle = 'rgba(240,240,230,0.55)';
  for (let k = 6; k < size - 6; k += 12) {
    if (vertical) ctx.fillRect(x, y + k, 10, 7); else ctx.fillRect(x + k, y, 7, 10);
  }
}

function drawCrate(ctx, c) {
  ctx.fillStyle = 'rgba(0,0,0,0.3)'; ctx.fillRect(c.x + 3, c.y + 3, c.w, c.h);
  ctx.fillStyle = '#8a5f2b'; ctx.fillRect(c.x, c.y + c.h - 8, c.w, 8);
  ctx.fillStyle = '#b07a3a'; ctx.fillRect(c.x, c.y - 8, c.w, c.h);
  ctx.strokeStyle = '#6e4a1c'; ctx.lineWidth = 1.5; ctx.strokeRect(c.x + 1, c.y - 7, c.w - 2, c.h - 2);
  ctx.beginPath(); ctx.moveTo(c.x + 1, c.y - 7); ctx.lineTo(c.x + c.w - 1, c.y + c.h - 9); ctx.stroke();
}

function overlap(a, b) { return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y; }

export { playerCar };
