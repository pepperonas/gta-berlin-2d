// HUD, Menüs und Overlays. Gezeichnet in einem virtuellen 720 px hohen Raster mit 5 % Title-Safe-Rand (TV).
import { WORLD_W, WORLD_H, MAP_W, MAP_H, TILE, SPEED_TO_KMH, MISSION, CAR, PLAYER } from './config.js';
import { T, locationName } from './map.js';
import { missionObjective, BRIEFING } from './mission.js';
import { playerCar, speedOf } from './world.js';

const FONT = 'Segoe UI, system-ui, -apple-system, sans-serif';
const YELLOW = '#ffd33d';
const GLYPH = { A: '#3fb54a', B: '#e2383f', X: '#2f7fe0', Y: '#f2b705' };
const KEYS = { A: 'E', B: 'Esc', X: 'H', Y: 'F', RB: 'Leer', LT: 'S', RT: 'W', MENU: 'Esc', VIEW: 'M' };

export class Hud {
  constructor(ctx) { this.ctx = ctx; this.minimap = null; this.device = 'gamepad'; }

  begin(W, H) {
    this.s = H / 720; this.vw = W / this.s; this.vh = 720;
    this.m = { x: this.vw * 0.05, y: this.vh * 0.05 };
    this.ctx.setTransform(this.s, 0, 0, this.s, 0, 0);
  }

  text(str, x, y, { size = 20, color = '#fff', align = 'left', weight = 600, shadow = true, base = 'alphabetic' } = {}) {
    const c = this.ctx;
    c.font = `${weight} ${size}px ${FONT}`; c.textAlign = align; c.textBaseline = base;
    if (shadow) { c.fillStyle = 'rgba(0,0,0,0.75)'; c.fillText(str, x + 1.5, y + 2); }
    c.fillStyle = color; c.fillText(str, x, y);
    return c.measureText(str).width;
  }

  panel(x, y, w, h, alpha = 0.62) {
    const c = this.ctx;
    c.fillStyle = `rgba(12,14,20,${alpha})`;
    rr(c, x, y, w, h, 10); c.fill();
    c.strokeStyle = 'rgba(255,255,255,0.12)'; c.lineWidth = 1; c.stroke();
  }

  // Tastensymbol: Controller-Knopf oder Tastaturtaste, je nach zuletzt benutztem Gerät.
  glyph(key, x, y, r = 13) {
    const c = this.ctx;
    if (this.device === 'keyboard') {
      const label = KEYS[key] ?? key;
      c.font = `700 ${r}px ${FONT}`;
      const w = Math.max(r * 2, c.measureText(label).width + 12);
      c.fillStyle = '#e8e8e8'; rr(c, x - w / 2, y - r, w, r * 2, 5); c.fill();
      c.fillStyle = '#222'; c.textAlign = 'center'; c.textBaseline = 'middle'; c.fillText(label, x, y + 1);
      return w;
    }
    c.fillStyle = GLYPH[key] ?? '#555';
    c.beginPath(); c.arc(x, y, r, 0, Math.PI * 2); c.fill();
    c.strokeStyle = 'rgba(0,0,0,0.4)'; c.lineWidth = 1.5; c.stroke();
    c.fillStyle = '#fff'; c.font = `800 ${r * 1.1}px ${FONT}`; c.textAlign = 'center'; c.textBaseline = 'middle';
    c.fillText(key.length > 1 ? key : key, x, y + 1);
    return r * 2;
  }

  // Text mit führendem „A: …“ → Knopfsymbol + Text, zentriert.
  prompt(str, cx, y) {
    const mt = /^([ABXY]) ?(gedrückt halten)?: ?(.*)$/.exec(str);
    const c = this.ctx;
    c.font = `600 22px ${FONT}`;
    const body = mt ? (mt[2] ? `halten: ${mt[3]}` : mt[3]) : str;
    const tw = c.measureText(body).width;
    const gw = mt ? (this.device === 'keyboard' ? 44 : 30) : 0;
    const w = tw + gw + 36;
    this.panel(cx - w / 2, y - 22, w, 44, 0.72);
    let x = cx - w / 2 + 18;
    if (mt) { this.glyph(mt[1], x + gw / 2 - 2, y); x += gw + 6; }
    this.text(body, x, y + 8, { size: 22 });
  }

  buildMinimap(city) {
    const cv = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(MAP_W, MAP_H) : Object.assign(document.createElement('canvas'), { width: MAP_W, height: MAP_H });
    const c = cv.getContext('2d');
    const col = { [T.ROAD]: '#6c7079', [T.SIDEWALK]: '#3a3d44', [T.BUILDING]: '#23252b', [T.GRASS]: '#2f5a2a', [T.WATER]: '#1f4f78', [T.PLAZA]: '#50545c' };
    for (let y = 0; y < MAP_H; y++) for (let x = 0; x < MAP_W; x++) { c.fillStyle = col[city.tiles[y * MAP_W + x]]; c.fillRect(x, y, 1, 1); }
    this.minimap = cv;
  }

  drawGameplay(world, g) {
    const c = this.ctx, m = this.m, vw = this.vw, vh = this.vh;
    if (!this.minimap) this.buildMinimap(world.city);
    const car = playerCar(world);
    const ctx = { places: world.city.places, player: world.player, cars: world.cars };
    const obj = missionObjective(world.mission, ctx);

    // Oben links: Ort + Geld
    this.text(locationName(world.city, world.player.x, world.player.y), m.x, m.y + 22, { size: 22, weight: 700 });
    this.text(`${world.money.toLocaleString('de-DE')} €`, m.x, m.y + 52, { size: 26, color: '#8fe388', weight: 800 });

    // Oben rechts: Missionsziel + Zeit
    const ms = world.mission.state;
    if (obj.text) {
      const w = 420;
      this.panel(vw - m.x - w, m.y, w, ms === 'toPickup' || ms === 'toDropoff' ? 92 : 58);
      this.text('AUFTRAG', vw - m.x - w + 16, m.y + 22, { size: 13, color: YELLOW, weight: 800 });
      this.text(obj.text, vw - m.x - w + 16, m.y + 46, { size: 19 });
      if (ms === 'toPickup' || ms === 'toDropoff') {
        const tm = world.mission.timer;
        const urgent = tm < 15;
        const blink = urgent && Math.floor(world.time * 4) % 2 === 0;
        this.text(fmtTime(tm), vw - m.x - 16, m.y + 80, { size: 26, align: 'right', weight: 800, color: blink ? '#ff4d4d' : urgent ? '#ff8080' : '#fff' });
        this.text(ms === 'toDropoff' ? 'Kisten geladen ✓' : 'Zeit bis Ladenschluss', vw - m.x - w + 16, m.y + 78, { size: 15, color: '#c8c8c8', weight: 500 });
      }
    }

    // Unten links: Minikarte (beim Briefing ausgeblendet)
    if (ms !== 'briefing') this.drawMinimap(world, obj.target, m.x, vh - m.y - 200, 200);

    // Unten rechts: Fahrzeugzustand
    if (car) {
      const w = 250, h = 106, x = vw - m.x - w, y = vh - m.y - h;
      this.panel(x, y, w, h);
      const kmh = Math.round(speedOf(car) * SPEED_TO_KMH);
      this.text(`${kmh}`, x + 20, y + 52, { size: 44, weight: 800 });
      this.text('km/h', x + 26 + c.measureText(`${kmh}`).width, y + 52, { size: 16, color: '#bbb', weight: 500 });
      const hp = car.health / CAR.health;
      this.text(car.wrecked ? 'ZUSTAND: SCHROTT' : 'ZUSTAND', x + 20, y + 76, { size: 12, color: car.wrecked ? '#ff6b6b' : '#bbb', weight: 800 });
      c.fillStyle = 'rgba(255,255,255,0.15)'; rr(c, x + 20, y + 84, w - 40, 10, 5); c.fill();
      c.fillStyle = hp > 0.6 ? '#4cd964' : hp > 0.3 ? '#ffcc00' : '#ff3b30';
      if (hp > 0) { rr(c, x + 20, y + 84, (w - 40) * hp, 10, 5); c.fill(); }
      if (car.cargo) this.text('▣ Kisten', x + w - 20, y + 30, { size: 16, align: 'right', color: '#e0b060', weight: 700 });
    }

    // Richtungspfeil zum Ziel (am Bildschirmrand, wenn außer Sicht)
    if (obj.target) this.drawTargetArrow(world, obj.target, g);

    // Hinweise unten mittig
    const mission = world.mission;
    let hint = mission.prompt;
    if (!hint && world.notice) hint = world.notice.text;
    if (!hint && !world.player.inCar) {
      const near = world.cars.some((cc) => !cc.wrecked && Math.hypot(cc.x - world.player.x, cc.y - world.player.y) < PLAYER.enterDist);
      if (near) hint = 'Y: Einsteigen';
    }
    if (!hint && car && speedOf(car) < 20 && !car.wrecked && g.hintT < 12) hint = 'Y: Aussteigen';
    if (!hint && car && car.wrecked) hint = 'Y: Aussteigen – das Auto ist Schrott';
    if (hint) this.prompt(hint, vw / 2, vh - m.y - 40);
    if (mission.load > 0 && mission.state === 'toPickup') {
      const w = 300, x = vw / 2 - w / 2, y = vh - m.y - 92;
      c.fillStyle = 'rgba(0,0,0,0.6)'; rr(c, x, y, w, 14, 7); c.fill();
      c.fillStyle = YELLOW; rr(c, x, y, w * Math.min(1, mission.load / MISSION.loadTime), 14, 7); c.fill();
    }
    if (mission.state === 'briefing') this.drawBriefing();
  }

  drawMinimap(world, target, x, y, size) {
    const c = this.ctx, p = playerCar(world) ?? world.player;
    const zoom = size / 1700; // Minikarte zeigt ~1700 Welt-px
    c.save();
    rr(c, x, y, size, size, 12); c.fillStyle = '#15171c'; c.fill(); c.clip();
    c.imageSmoothingEnabled = false;
    const k = TILE * zoom;
    c.drawImage(this.minimap, x + size / 2 - (p.x / TILE) * k, y + size / 2 - (p.y / TILE) * k, MAP_W * k, MAP_H * k);
    c.imageSmoothingEnabled = true;
    const toMini = (wx, wy) => [x + size / 2 + (wx - p.x) * zoom, y + size / 2 + (wy - p.y) * zoom];
    for (const car of world.cars) {
      if (car === playerCar(world)) continue;
      const [mx, my] = toMini(car.x, car.y);
      c.fillStyle = car.cargo ? '#e0b060' : car.id === world.playerCarId ? '#ff7a7a' : 'rgba(210,210,210,0.55)';
      c.fillRect(mx - 1.5, my - 1.5, 3, 3);
    }
    if (target) {
      let [tx, ty] = toMini(target.x, target.y);
      const cx = x + size / 2, cy = y + size / 2, lim = size / 2 - 9;
      const dx = tx - cx, dy = ty - cy, f = Math.max(Math.abs(dx), Math.abs(dy)) / lim;
      if (f > 1) { tx = cx + dx / f; ty = cy + dy / f; }
      c.fillStyle = YELLOW; c.strokeStyle = '#000'; c.lineWidth = 2;
      c.beginPath(); c.arc(tx, ty, 6, 0, Math.PI * 2); c.fill(); c.stroke();
    }
    // Spielerpfeil
    c.translate(x + size / 2, y + size / 2);
    c.rotate(p.angle ?? world.player.angle);
    c.fillStyle = '#fff'; c.strokeStyle = '#000'; c.lineWidth = 1.5;
    c.beginPath(); c.moveTo(8, 0); c.lineTo(-6, -5.5); c.lineTo(-3, 0); c.lineTo(-6, 5.5); c.closePath(); c.fill(); c.stroke();
    c.restore();
    c.strokeStyle = 'rgba(255,255,255,0.25)'; c.lineWidth = 2; rr(c, x, y, size, size, 12); c.stroke();
    this.text('N', x + size / 2, y + 16, { size: 13, align: 'center', weight: 800, color: '#ddd' });
  }

  drawTargetArrow(world, target, g) {
    const c = this.ctx, cam = world.camera;
    const scale = g.worldScale * cam.zoom / this.s;
    const sx = this.vw / 2 + (target.x - cam.x) * scale, sy = this.vh / 2 + (target.y - cam.y) * scale;
    const src = playerCar(world) ?? world.player;
    const meters = Math.round(Math.hypot(target.x - src.x, target.y - src.y) / 10);
    const mx = this.m.x + 40, my = this.m.y + 110;
    const inside = sx > mx && sx < this.vw - mx && sy > my && sy < this.vh - this.m.y - 130;
    if (inside) {
      const bob = Math.sin(world.time * 5) * 5;
      c.fillStyle = YELLOW; c.strokeStyle = '#000'; c.lineWidth = 2;
      c.beginPath(); c.moveTo(sx, sy - 34 + bob); c.lineTo(sx - 11, sy - 52 + bob); c.lineTo(sx + 11, sy - 52 + bob); c.closePath(); c.fill(); c.stroke();
      return;
    }
    const cx = this.vw / 2, cy = this.vh / 2;
    const a = Math.atan2(sy - cy, sx - cx);
    // Strahl von der Bildmitte bis zum Rand des freien Bereichs (zwischen oberem und unterem HUD).
    const L = mx, R = this.vw - mx, Tp = this.m.y + 120, B = this.vh - this.m.y - 140;
    const ux = Math.cos(a), uy = Math.sin(a);
    const k = Math.min(ux > 0 ? (R - cx) / ux : ux < 0 ? (L - cx) / ux : Infinity, uy > 0 ? (B - cy) / uy : uy < 0 ? (Tp - cy) / uy : Infinity);
    const ax = cx + ux * k, ay = cy + uy * k;
    c.save(); c.translate(ax, ay); c.rotate(a);
    c.fillStyle = YELLOW; c.strokeStyle = '#000'; c.lineWidth = 2;
    c.beginPath(); c.moveTo(18, 0); c.lineTo(-10, -13); c.lineTo(-4, 0); c.lineTo(-10, 13); c.closePath(); c.fill(); c.stroke();
    c.restore();
    this.text(`${meters} m`, ax - Math.cos(a) * 34, ay - Math.sin(a) * 30 + 6, { size: 16, align: 'center', weight: 800 });
  }

  drawBriefing() {
    const w = 820, h = 210, x = this.vw / 2 - w / 2, y = this.vh - this.m.y - h - 60;
    this.panel(x, y, w, h, 0.85);
    BRIEFING.forEach((line, i) => this.text(line, x + 28, y + 40 + i * 32, { size: i === 0 ? 18 : 21, color: i === 0 ? YELLOW : '#fff', weight: i === 0 ? 800 : 500 }));
    this.glyph('A', x + w - 150, y + h - 28);
    this.text('Auftrag starten', x + w - 132, y + h - 21, { size: 18 });
  }

  drawBigMap(world) {
    const c = this.ctx;
    c.fillStyle = 'rgba(0,0,0,0.7)'; c.fillRect(0, 0, this.vw, this.vh);
    const hh = this.vh - this.m.y * 2 - 40, k = hh / MAP_H, ww = MAP_W * k;
    const x = this.vw / 2 - ww / 2, y = this.m.y + 30;
    c.imageSmoothingEnabled = false; c.drawImage(this.minimap, x, y, ww, hh); c.imageSmoothingEnabled = true;
    const f = k / TILE;
    const obj = missionObjective(world.mission, { places: world.city.places, player: world.player, cars: world.cars });
    if (obj.target) { c.fillStyle = YELLOW; c.beginPath(); c.arc(x + obj.target.x * f, y + obj.target.y * f, 8, 0, Math.PI * 2); c.fill(); }
    const p = playerCar(world) ?? world.player;
    c.fillStyle = '#fff'; c.beginPath(); c.arc(x + p.x * f, y + p.y * f, 6, 0, Math.PI * 2); c.fill();
    this.text('Stadtplan', x, y - 8, { size: 20, weight: 800 });
    this.text(`Späti = Auftraggeber · gelb = aktuelles Ziel`, x + ww, y - 8, { size: 15, align: 'right', color: '#ccc', weight: 500 });
  }

  // --- Menüs --------------------------------------------------------------
  menu(menu, cx, y, { width = 380 } = {}) {
    const c = this.ctx;
    menu.items.forEach((it, i) => {
      const yy = y + i * 58, sel = i === menu.index, dis = it.enabled === false;
      if (sel) {
        c.fillStyle = YELLOW; rr(c, cx - width / 2, yy - 24, width, 48, 10); c.fill();
      } else { c.fillStyle = 'rgba(15,17,24,0.7)'; rr(c, cx - width / 2, yy - 24, width, 48, 10); c.fill(); }
      this.text(it.label, cx, yy + 8, { size: 24, align: 'center', weight: 700, color: sel ? '#111' : dis ? '#666' : '#eee', shadow: !sel });
    });
  }

  footerHints(items) {
    const c = this.ctx, y = this.vh - this.m.y - 10;
    c.font = `500 17px ${FONT}`;
    const gw = this.device === 'keyboard' ? 40 : 24;
    const widths = items.map(([, label]) => gw + 8 + c.measureText(label).width);
    let x = this.vw / 2 - (widths.reduce((a, b) => a + b, 0) + 36 * (items.length - 1)) / 2;
    items.forEach(([k, label], i) => {
      this.glyph(k, x + gw / 2, y, 12);
      this.text(label, x + gw + 8, y + 7, { size: 17, weight: 500 });
      x += widths[i] + 36;
    });
  }

  drawTitle(g) {
    const c = this.ctx, vw = this.vw, vh = this.vh;
    const grad = c.createLinearGradient(0, 0, 0, vh);
    grad.addColorStop(0, 'rgba(10,8,30,0.35)'); grad.addColorStop(1, 'rgba(10,8,20,0.85)');
    c.fillStyle = grad; c.fillRect(0, 0, vw, vh);
    drawSkyline(c, vw, vh);
    this.text('GTA', vw / 2, 150, { size: 64, align: 'center', weight: 900, color: '#fff' });
    this.text('BERLIN', vw / 2, 232, { size: 96, align: 'center', weight: 900, color: YELLOW });
    this.text('Kisten für den Kiez', vw / 2, 272, { size: 22, align: 'center', weight: 500, color: '#ddd' });
    this.menu(g.titleMenu, vw / 2, 350);
    this.footerHints([['A', 'Auswählen']]);
    this.text('v0.1 · Prototyp', vw - this.m.x, vh - this.m.y, { size: 14, align: 'right', color: '#999', weight: 500 });
  }

  drawPause(g) {
    const c = this.ctx;
    c.fillStyle = 'rgba(0,0,0,0.6)'; c.fillRect(0, 0, this.vw, this.vh);
    this.text('PAUSE', this.vw / 2, 170, { size: 56, align: 'center', weight: 900, color: YELLOW });
    const w = g.world;
    this.text(`Aufträge erledigt: ${w.completed}   ·   Bestzeit: ${w.bestTime ? fmtTime(w.bestTime) : '–'}`, this.vw / 2, 210, { size: 18, align: 'center', weight: 500, color: '#ccc' });
    this.menu(g.pauseMenu, this.vw / 2, 280);
    this.footerHints([['A', 'Auswählen'], ['B', 'Weiter']]);
  }

  drawResult(g) {
    const c = this.ctx, r = g.world.mission.result, vw = this.vw;
    c.fillStyle = 'rgba(0,0,0,0.55)'; c.fillRect(0, 0, vw, this.vh);
    const ok = r.success;
    this.text(ok ? 'AUFTRAG ERFÜLLT' : 'AUFTRAG GESCHEITERT', vw / 2, 190, { size: 58, align: 'center', weight: 900, color: ok ? '#6fe06a' : '#ff5b5b' });
    if (ok) {
      const lines = [
        `Zeit: ${fmtTime(r.time)}${r.newBest ? '   ★ Neue Bestzeit' : ''}`,
        `Lohn ${MISSION.reward} €  +  Zeitbonus ${r.bonus} €  −  Schaden ${r.damagePenalty} €`,
        `Ausgezahlt: ${r.reward} €`,
      ];
      lines.forEach((l, i) => this.text(l, vw / 2, 250 + i * 34, { size: i === 2 ? 26 : 20, align: 'center', weight: i === 2 ? 800 : 500, color: i === 2 ? '#8fe388' : '#eee' }));
    } else this.text(r.reason, vw / 2, 250, { size: 21, align: 'center', weight: 500 });
    this.menu(g.resultMenu, vw / 2, ok ? 400 : 330);
    if (ok) this.text('Fortschritt wird automatisch gespeichert.', vw / 2, 470, { size: 15, align: 'center', color: '#aaa', weight: 500 });
  }

  drawControls() {
    const c = this.ctx, vw = this.vw;
    c.fillStyle = 'rgba(5,6,10,0.9)'; c.fillRect(0, 0, vw, this.vh);
    this.text('STEUERUNG', vw / 2, 110, { size: 44, align: 'center', weight: 900, color: YELLOW });
    const rows = [
      ['Laufen / Lenken', 'Linker Stick', 'WASD / Pfeile'],
      ['Sprinten (zu Fuß)', 'A halten / Stick voll', 'Umschalt'],
      ['Gas / Bremse · Rückwärts', 'RT / LT', 'W / S'],
      ['Handbremse', 'RB oder B', 'Leertaste'],
      ['Einsteigen / Aussteigen', 'Y', 'F'],
      ['Aktion (Auftrag, Einladen)', 'A', 'E / Enter'],
      ['Hupe', 'X', 'H'],
      ['Stadtplan', 'Ansicht-Taste', 'M'],
      ['Pause', 'Menü-Taste', 'Esc / P'],
    ];
    const x0 = vw / 2 - 460;
    this.text('Controller', x0 + 470, 170, { size: 18, color: '#aaa', weight: 800 });
    this.text('Tastatur', x0 + 740, 170, { size: 18, color: '#aaa', weight: 800 });
    rows.forEach(([a, b, k], i) => {
      const y = 210 + i * 42;
      if (i % 2 === 0) { c.fillStyle = 'rgba(255,255,255,0.05)'; c.fillRect(x0, y - 28, 920, 42); }
      this.text(a, x0 + 16, y, { size: 20, weight: 600 });
      this.text(b, x0 + 470, y, { size: 20, weight: 500, color: '#ddd' });
      this.text(k, x0 + 740, y, { size: 20, weight: 500, color: '#ddd' });
    });
    this.footerHints([['B', 'Zurück']]);
  }

  toast(tst) {
    if (!tst) return;
    const c = this.ctx;
    c.font = `700 20px ${FONT}`;
    const w = c.measureText(tst.text).width + 40;
    this.panel(this.vw / 2 - w / 2, this.m.y, w, 42, 0.8);
    this.text(tst.text, this.vw / 2, this.m.y + 28, { size: 20, align: 'center', weight: 700, color: YELLOW });
  }
}

function drawSkyline(c, vw, vh) {
  const base = vh - 40;
  c.fillStyle = 'rgba(8,8,16,0.85)';
  c.beginPath(); c.moveTo(0, vh); c.lineTo(0, base);
  let x = 0; let seed = 7;
  const rnd = () => ((seed = (seed * 9301 + 49297) % 233280) / 233280);
  while (x < vw) { const w = 30 + rnd() * 60, h = 40 + rnd() * 110; c.lineTo(x, base - h); c.lineTo(x + w, base - h); x += w; }
  c.lineTo(vw, base); c.lineTo(vw, vh); c.closePath(); c.fill();
  // Fernsehturm-Silhouette (eigene Zeichnung)
  const tx = vw * 0.78;
  c.fillRect(tx - 5, base - 330, 10, 330);
  c.beginPath(); c.arc(tx, base - 250, 26, 0, Math.PI * 2); c.fill();
  c.fillRect(tx - 1.5, base - 400, 3, 80);
  c.fillStyle = '#ff3b30'; c.beginPath(); c.arc(tx, base - 402, 3, 0, Math.PI * 2); c.fill();
}

export function fmtTime(s) {
  const m = Math.floor(s / 60), r = Math.floor(s % 60);
  return `${m}:${String(r).padStart(2, '0')}`;
}

function rr(c, x, y, w, h, r) {
  c.beginPath();
  c.moveTo(x + r, y); c.arcTo(x + w, y, x + w, y + h, r); c.arcTo(x + w, y + h, x, y + h, r);
  c.arcTo(x, y + h, x, y, r); c.arcTo(x, y, x + w, y, r); c.closePath();
}

export { WORLD_W, WORLD_H };
