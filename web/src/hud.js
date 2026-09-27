// HUD, Menüs und Overlays. Grundformat ist 16:9 (1280 × 720 virtuelle Punkte): Das Raster wird so skaliert, dass diese
// Fläche in jedes Fenster ganz hineinpasst (Maßstab = min(Breite/1280, Höhe/720)); ist das Fenster breiter oder höher,
// wächst die virtuelle Fläche mit (vw ≥ 1280, vh ≥ 720). Das Spiel-HUD hängt an den Fensterrändern (5 % Title-Safe-Rand,
// TV), Menübildschirme liegen in einem zentrierten 1280 × 720-Rahmen (inFrame) – so wird nichts abgeschnitten.
import { SPEED_TO_KMH, MISSION, CAR, PLAYER } from './config.js';
import { locationName, nearestPoi } from './map.js';
import { formatClock, SUNRISE } from './daylight.js';
import { undelta } from './geom.js';
import { mapLabels, prepareStreets } from './maplabels.js';
import { pathOf, ringPath, POI_STYLE } from './render.js';
import { AREA_KIND } from './citycodes.js';
import { VERSION } from './version.js';
import { missionObjective, BRIEFING } from './mission.js';
import { playerCar, speedOf } from './world.js';

const MINI_AREA = { [AREA_KIND.rail]: '#4a4640', [AREA_KIND.allotments]: '#35602c', [AREA_KIND.cemetery]: '#2f5a2a', [AREA_KIND.grass]: '#2f5a2a', [AREA_KIND.pitch]: '#2f5a2a', [AREA_KIND.sand]: '#6b6040', [AREA_KIND.wood]: '#284d22' };
const POI_LABEL = { ubahn: 'U-Bahnhof', sbahn: 'S-Bahnhof', bahn: 'Bahnhof', bus: 'Bushaltestelle', mall: 'Einkaufszentrum',
  supermarket: 'Markt', shop: 'Laden', food: 'Essen', drink: 'Bar', cafe: 'Café', service: 'Service', culture: 'Kultur', hotel: 'Hotel' };
const FONT = 'Segoe UI, system-ui, -apple-system, sans-serif';
const YELLOW = '#ffd33d';
const GLYPH = { A: '#3fb54a', B: '#e2383f', X: '#2f7fe0', Y: '#f2b705' };
// Grundformat 16:9 in virtuellen HUD-Punkten
export const BASE = { w: 1280, h: 720 };

const KEYS = { A: 'E', B: 'Esc', X: 'H', Y: 'F', RB: 'Leer', LT: 'S', RT: 'W', MENU: 'Esc', VIEW: 'M' };

export class Hud {
  constructor(ctx) { this.ctx = ctx; this.overview = null; this.device = 'gamepad'; }

  begin(W, H) {
    this.s = Math.min(W / BASE.w, H / BASE.h); this.vw = W / this.s; this.vh = H / this.s;
    this.fx = 0; this.fy = 0; // Versatz des 16:9-Rahmens (nur innerhalb von inFrame ≠ 0)
    this.m = { x: this.vw * 0.05, y: this.vh * 0.05 };
    this.ctx.setTransform(this.s, 0, 0, this.s, 0, 0);
    this.fullW = this.vw; this.fullH = this.vh;
    this.hits = []; // anklickbare Flächen dieses Bildes (virtuelle HUD-Koordinaten des Fensters), für die Maus in main.js
  }

  // Zeichnet fn im zentrierten 16:9-Rahmen (1280 × 720, eigener Title-Safe-Rand); Klickflächen bleiben Fensterkoordinaten.
  inFrame(fn) {
    const keep = { vw: this.vw, vh: this.vh, m: this.m, fx: this.fx, fy: this.fy };
    this.fx = (this.vw - BASE.w) / 2; this.fy = (this.vh - BASE.h) / 2;
    this.vw = BASE.w; this.vh = BASE.h; this.m = { x: BASE.w * 0.05, y: BASE.h * 0.05 };
    this.ctx.save(); this.ctx.translate(this.fx, this.fy);
    try { fn(); } finally { this.ctx.restore(); Object.assign(this, keep); }
  }

  // ganzes Fenster abdunkeln (auch außerhalb des Rahmens)
  fillScreen(color) { const c = this.ctx; c.fillStyle = color; c.fillRect(-this.fx, -this.fy, this.fullW ?? this.vw, this.fullH ?? this.vh); }

  addHit(h) { this.hits.push({ ...h, x: h.x + this.fx, y: h.y + this.fy }); }

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

  // Stadtplan (ganz Berlin) aus overview.json: je Schicht ein Pfad in Weltkoordinaten, einmal gebaut.
  buildOverview(city) {
    const ov = city.overview;
    if (!ov) return null;
    const ring = (p, d) => { const r = undelta(d); p.moveTo(r[0], r[1]); for (let i = 2; i < r.length; i += 2) p.lineTo(r[i], r[i + 1]); p.closePath(); };
    const line = (p, d) => { const r = undelta(d); p.moveTo(r[0], r[1]); for (let i = 2; i < r.length; i += 2) p.lineTo(r[i], r[i + 1]); };
    const areas = new Map();
    for (const [k, rings] of ov.areas) { let p = areas.get(k); if (!p) areas.set(k, p = new Path2D()); for (const [, d] of rings) ring(p, d); }
    const water = new Path2D(); for (const rings of ov.water) for (const [, d] of rings) ring(water, d);
    const roads = [new Path2D(), new Path2D(), new Path2D(), new Path2D()]; // Autobahn · Hauptstraße · Nebenstraße · Wohnstraße
    for (const [cls, , d] of ov.roads) line(roads[cls <= 2 ? 0 : cls <= 4 ? 1 : cls <= 5 ? 2 : 3], d);
    const rails = new Path2D(); for (const d of ov.rails) line(rails, d);
    const border = ringPath(city.border);
    const bezirke = new Path2D(); for (const b of city.bezirke) for (const r of b.rings) { bezirke.moveTo(r[0], r[1]); for (let i = 2; i < r.length; i += 2) bezirke.lineTo(r[i], r[i + 1]); bezirke.closePath(); }
    const outside = new Path2D(); outside.rect(-1e6, -1e6, city.width + 2e6, city.height + 2e6); outside.addPath(border);
    this.overview = { areas, water, roads, rails, border, bezirke, outside, stations: ov.stations,
      labelData: { labels: ov.labels, ortsteile: ov.ortsteile ?? [], kieze: ov.kieze ?? [], stations: ov.stations, streets: prepareStreets(ov.roads, ov.names ?? [], undelta) } };
    return this.overview;
  }

  // Schrift mit dunklem Rand (Beschriftung auf dem Stadtplan), optional gedreht
  haloText({ text, x, y, angle = 0, size, weight, color, italic }) {
    const c = this.ctx;
    c.save(); c.translate(x, y); if (angle) c.rotate(angle);
    c.font = `${italic ? 'italic ' : ''}${weight} ${size}px ${FONT}`; c.textAlign = 'center'; c.textBaseline = 'middle';
    c.lineJoin = 'round'; c.lineWidth = 3.2; c.strokeStyle = 'rgba(12,13,17,0.9)'; c.strokeText(text, 0, 0.5);
    c.fillStyle = color; c.fillText(text, 0, 0.5);
    c.restore();
  }

  // Ansicht des Stadtplans: Zoom (1 = ganz Berlin) und Mittelpunkt in Weltkoordinaten; Mausrad/Ziehen in main.js.
  zoomBigMap(factor, vx, vy) {
    const m = this.bigMap, v = this.mapView;
    if (!m || !v) return;
    const wx = (vx - m.ox) / m.f, wy = (vy - m.oy) / m.f;
    v.z = Math.min(64, Math.max(1, v.z * factor)); // 1 = ganz Berlin, 64 ≈ 1 m je Bildpunkt (Straßennamen)
    const f = m.f0 * v.z;
    v.cx = wx - (vx - m.x - m.w / 2) / f; v.cy = wy - (vy - m.y - m.h / 2) / f;
  }
  panBigMap(dx, dy) { if (this.bigMap && this.mapView) { this.mapView.cx -= dx / this.bigMap.f; this.mapView.cy -= dy / this.bigMap.f; } }

  drawGameplay(world, g) {
    const c = this.ctx, m = this.m, vw = this.vw, vh = this.vh;
    const car = playerCar(world);
    const ctx = { places: world.city.places, player: world.player, cars: world.cars };
    const obj = missionObjective(world.mission, ctx);

    // Oben links: Ort + Geld
    this.text(locationName(world.city, world.player.x, world.player.y), m.x, m.y + 22, { size: 22, weight: 700 });
    const mw = this.text(`${world.money.toLocaleString('de-DE')} €`, m.x, m.y + 52, { size: 26, color: '#8fe388', weight: 800 });
    const night = world.clock >= 1230 || world.clock < SUNRISE;
    this.text(`${night ? '☾' : '☀'} ${formatClock(world.clock)}`, m.x + mw + 18, m.y + 52, { size: 20, color: night ? '#b9c6ff' : '#ffe08a', weight: 700 });
    // Geschäft/Lokal/Haltestelle in unmittelbarer Nähe
    const here = playerCar(world) ?? world.player;
    const poi = nearestPoi(world.city, here.x, here.y, car ? 120 : 180);
    if (poi) this.text(`${POI_LABEL[poi.cat]}: ${poi.name}`, m.x, m.y + 78, { size: 16, color: '#d8d8d8', weight: 600 });

    // Oben rechts: Missionsziel + Zeit
    const ms = world.mission.state;
    if (obj.text) {
      const w = 420;
      this.layout = { ...(this.layout ?? {}), mission: { x: vw - m.x - w, y: m.y, w, h: ms === 'toPickup' || ms === 'toDropoff' ? 92 : 58 } };
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
      this.layout = { ...(this.layout ?? {}), car: { x, y, w, h } };
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
    if (world.loading) this.drawLoading(world);
  }

  // Welt wartet auf Kacheln: Fortschritt, oder klarer Hinweis, wenn der Spielserver nicht antwortet.
  drawLoading(world) {
    const vw = this.vw, vh = this.vh, st = world.city.status(world.camera.x, world.camera.y);
    const off = st.failed > 0;
    const w = off ? 620 : 340, h = off ? 86 : 56;
    this.panel(vw / 2 - w / 2, vh / 2 - h / 2, w, h, 0.85);
    if (off) {
      this.text('Keine Verbindung zum Spielserver', vw / 2, vh / 2 - 8, { size: 22, align: 'center', weight: 800, color: '#ff8a80' });
      this.text(`Läuft der Server noch (gta2d)? Neuer Versuch in ${Math.ceil(st.retryIn / 1000)} s …`, vw / 2, vh / 2 + 24, { size: 17, align: 'center', weight: 500, color: '#ddd' });
    } else this.text(`Lade Stadtteil … ${st.ready} / ${st.needed}`, vw / 2, vh / 2 + 8, { size: 22, align: 'center', weight: 700 });
  }

  drawMinimap(world, target, x, y, size) {
    this.layout = { ...(this.layout ?? {}), minimap: { x, y, w: size, h: size } };
    const c = this.ctx, p = playerCar(world) ?? world.player, city = world.city;
    const zoom = size / 4000; // Minikarte zeigt ~400 m
    c.save();
    rr(c, x, y, size, size, 12); c.fillStyle = '#3a3d44'; c.fill(); c.clip();
    c.save();
    c.transform(zoom, 0, 0, zoom, x + size / 2 - p.x * zoom, y + size / 2 - p.y * zoom);
    const R = 2200;
    const q = city.render.query({ x: p.x - R, y: p.y - R, w: 2 * R, h: 2 * R }, this._mq ??= []);
    for (const f of q) if (f.layer === 'area' && f.kind !== AREA_KIND.plaza) { c.fillStyle = MINI_AREA[f.kind] ?? '#2f5a2a'; c.fill(pathOf(f), 'evenodd'); }
    c.fillStyle = '#2b2d33';
    for (const f of q) if (f.layer === 'building') c.fill(pathOf(f), 'evenodd');
    c.fillStyle = '#1f4f78';
    for (const f of q) if (f.layer === 'water') c.fill(pathOf(f), 'evenodd');
    c.lineCap = 'round'; c.lineJoin = 'round';
    for (const f of q) if (f.layer === 'edge' && f.cls <= 9) {
      c.strokeStyle = f.cls <= 4 ? '#b9a66a' : '#8d919a'; c.lineWidth = Math.max(f.w, 28); c.stroke(pathOf(f));
    }
    c.strokeStyle = '#ffd33d'; c.lineWidth = 30; c.stroke(city._borderPath ??= ringPath(city.border));
    c.restore();
    // Bahnhöfe auf der Minikarte
    for (const q of city.poiHash.query({ x: p.x - R, y: p.y - R, w: 2 * R, h: 2 * R }, [])) {
      if (q.cat !== 'ubahn' && q.cat !== 'sbahn') continue;
      const mx = x + size / 2 + (q.x - p.x) * zoom, my = y + size / 2 + (q.y - p.y) * zoom;
      this.stationIcon(q.cat, mx, my, 6);
    }
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

  stationIcon(cat, x, y, r) {
    const c = this.ctx, st = POI_STYLE[cat];
    c.fillStyle = st.bg;
    if (cat === 'ubahn') c.fillRect(x - r, y - r, 2 * r, 2 * r); else { c.beginPath(); c.arc(x, y, r, 0, Math.PI * 2); c.fill(); }
    c.fillStyle = st.fg; c.font = `800 ${Math.round(r * 1.5)}px ${FONT}`; c.textAlign = 'center'; c.textBaseline = 'middle';
    c.fillText(st.glyph, x, y + 0.5);
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
    const c = this.ctx, city = world.city;
    c.fillStyle = 'rgba(12,13,16,0.94)'; c.fillRect(0, 0, this.vw, this.vh);
    const x = this.m.x, y = this.m.y + 30, w = this.vw - 2 * this.m.x, h = this.vh - this.m.y * 2 - 60;
    const p = playerCar(world) ?? world.player;
    const v = (this.mapView ??= { z: 1, cx: city.width / 2, cy: city.height / 2 });
    const f0 = Math.min(w / city.width, h / city.height), f = f0 * v.z;
    // nicht über den Rand von Berlin hinausschieben
    const hw = w / 2 / f, hh = h / 2 / f;
    v.cx = hw * 2 >= city.width ? city.width / 2 : Math.min(city.width - hw, Math.max(hw, v.cx));
    v.cy = hh * 2 >= city.height ? city.height / 2 : Math.min(city.height - hh, Math.max(hh, v.cy));
    const ox = x + w / 2 - v.cx * f, oy = y + h / 2 - v.cy * f;
    this.bigMap = { x, y, w, h, f, f0, ox, oy }; // für Mausklicks (virtuelle HUD-Koordinaten)
    this.addHit({ kind: 'map', x, y, w, h });
    const ov = this.overview ?? this.buildOverview(city);
    c.save();
    rr(c, x, y, w, h, 10); c.clip();
    c.fillStyle = '#3a3d44'; c.fillRect(x, y, w, h);
    const W = (pxOnScreen) => pxOnScreen / f; // Linienbreite in Bildschirmpunkten
    if (ov) {
      c.save(); c.transform(f, 0, 0, f, ox, oy);
      for (const [k, path] of ov.areas) { c.fillStyle = MINI_AREA[k] ?? '#2f5a2a'; c.fill(path); }
      c.fillStyle = '#1f4f78'; c.fill(ov.water);
      c.lineCap = 'round'; c.lineJoin = 'round';
      c.strokeStyle = '#6b6f78'; c.lineWidth = W(v.z >= 4 ? 1.4 : 0.6); c.stroke(ov.roads[3]);
      c.strokeStyle = '#8d919a'; c.lineWidth = W(v.z >= 4 ? 2.2 : 1.1); c.stroke(ov.roads[2]);
      c.strokeStyle = '#b9a66a'; c.lineWidth = W(v.z >= 4 ? 3.2 : 1.6); c.stroke(ov.roads[1]);
      c.strokeStyle = '#e0a84a'; c.lineWidth = W(v.z >= 4 ? 4 : 2.2); c.stroke(ov.roads[0]);
      c.strokeStyle = 'rgba(40,36,32,0.9)'; c.lineWidth = W(1.2); c.stroke(ov.rails);
      c.fillStyle = 'rgba(0,0,0,0.55)'; c.fill(ov.outside, 'evenodd');
      c.strokeStyle = 'rgba(255,255,255,0.35)'; c.lineWidth = W(1); c.stroke(ov.bezirke);
      c.strokeStyle = '#ffd33d'; c.lineWidth = W(2.5); c.stroke(ov.border);
      c.restore();
      const S = (wx, wy) => [ox + wx * f, oy + wy * f];
      for (const [qx, qy, cat] of ov.stations) {
        const [sx, sy] = S(qx, qy);
        if (sx < x - 20 || sx > x + w + 20 || sy < y - 20 || sy > y + h + 20) continue;
        this.stationIcon(cat, sx, sy, v.z >= 3 ? 6 : 3.5);
      }
      // Beschriftung je Maßstab: Bezirke → Ortsteile → Kieze und Bahnhöfe → Straßennamen (maplabels.js)
      const mpp = 1 / (f * city.scale);
      const measure = (t, st) => { c.font = `${st.italic ? 'italic ' : ''}${st.weight} ${st.size}px ${FONT}`; return c.measureText(t).width; };
      // nur neu setzen, wenn sich die Ansicht geändert hat (sonst die gemerkten Beschriftungen zeichnen)
      const key = `${f}|${ox}|${oy}|${w}|${h}`;
      if (ov.labelKey !== key) {
        const hint = { x0: x + 10, y0: y + h - 40, x1: x + 10 + 470, y1: y + h - 10 }; // Hinweisleiste unten links
        ov.labels = mapLabels(ov.labelData, { f, ox, oy, x, y, w, h, mpp }, measure, [hint]);
        ov.labelKey = key;
      }
      for (const l of ov.labels) this.haloText(l);
    } else this.text('Stadtplan lädt …', x + w / 2, y + h / 2, { size: 22, align: 'center', weight: 700 });
    const obj = missionObjective(world.mission, { places: city.places, player: world.player, cars: world.cars });
    const dot = (wx, wy, r, fill) => { c.fillStyle = fill; c.strokeStyle = '#000'; c.lineWidth = 2; c.beginPath(); c.arc(ox + wx * f, oy + wy * f, r, 0, Math.PI * 2); c.fill(); c.stroke(); };
    dot(city.places.giver.x, city.places.giver.y, 5, '#e03b3b');
    if (obj.target) dot(obj.target.x, obj.target.y, 8, YELLOW);
    dot(p.x, p.y, 6, '#fff');
    c.restore();
    this.text('Stadtplan · Berlin', x, y - 8, { size: 20, weight: 800 });
    this.text('rot = Späti · gelb = Ziel · weiß = du · U/S = Bahnhof', x + w, y - 8, { size: 15, align: 'right', color: '#ccc', weight: 500 });
    this.text(city.attribution, x + w, y + h + 20, { size: 12, align: 'right', color: '#aaa', weight: 500 });
    this.panel(x + 10, y + h - 40, 470, 30, 0.75);
    this.text('Mausrad: zoomen · Ziehen: verschieben · Klick: dorthin teleportieren', x + 22, y + h - 20, { size: 14, color: '#eee', weight: 600 });
  }

  // Bestätigung vor dem Teleport; die Knopfflächen merkt sich der HUD für Mausklicks.
  drawTeleportDialog(spot) {
    const c = this.ctx, w = 560, h = 190, x = this.vw / 2 - w / 2, y = this.vh / 2 - h / 2;
    c.fillStyle = 'rgba(0,0,0,0.45)'; c.fillRect(0, 0, this.vw, this.vh);
    this.panel(x, y, w, h, 0.92);
    this.text('HIERHIN TELEPORTIEREN?', this.vw / 2, y + 44, { size: 22, align: 'center', weight: 800, color: YELLOW });
    this.text(spot.pending ? 'Lade den Stadtteil …' : spot.name, this.vw / 2, y + 84, { size: 20, align: 'center', weight: 600, color: spot.pending ? '#bbb' : '#fff' });
    const bw = 200, bh = 46, by = y + h - bh - 22;
    this.dialogButtons = { yes: { x: this.vw / 2 - bw - 12, y: by, w: bw, h: bh }, no: { x: this.vw / 2 + 12, y: by, w: bw, h: bh } };
    this.hits = [{ kind: 'dialog', yes: true, ...this.dialogButtons.yes }, { kind: 'dialog', yes: false, ...this.dialogButtons.no }]; // der Dialog liegt über allem
    for (const [k, label, glyph, bg] of [['yes', 'Ja', 'A', YELLOW], ['no', 'Nein', 'B', 'rgba(255,255,255,0.12)']]) {
      const b = this.dialogButtons[k];
      c.fillStyle = bg; rr(c, b.x, b.y, b.w, b.h, 10); c.fill();
      const gw = this.glyph(glyph, b.x + 34, b.y + b.h / 2, 12);
      this.text(label, b.x + 34 + gw / 2 + 12, b.y + b.h / 2 + 8, { size: 20, weight: 800, color: k === 'yes' ? '#111' : '#eee', shadow: k !== 'yes' });
    }
  }

  // --- Menüs --------------------------------------------------------------
  menu(menu, cx, y, { width = 380 } = {}) {
    const c = this.ctx;
    menu.items.forEach((it, i) => {
      const yy = y + i * 58, sel = i === menu.index, dis = it.enabled === false;
      if (!dis) this.addHit({ kind: 'menu', menu, i, x: cx - width / 2, y: yy - 24, w: width, h: 48 });
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
      this.addHit({ kind: 'key', key: k, x: x - 8, y: y - 20, w: widths[i] + 16, h: 36 });
      this.glyph(k, x + gw / 2, y, 12);
      this.text(label, x + gw + 8, y + 7, { size: 17, weight: 500 });
      x += widths[i] + 36;
    });
  }

  drawTitle(g) {
    const c = this.ctx;
    const grad = c.createLinearGradient(0, 0, 0, this.vh);
    grad.addColorStop(0, 'rgba(10,8,30,0.35)'); grad.addColorStop(1, 'rgba(10,8,20,0.85)');
    c.fillStyle = grad; c.fillRect(0, 0, this.vw, this.vh);
    drawSkyline(c, this.vw, this.vh);
    this.text(`v${VERSION} · Prototyp`, this.vw - this.m.x, this.vh - this.m.y, { size: 14, align: 'right', color: '#999', weight: 500 });
    this.text('Kartendaten © OpenStreetMap-Mitwirkende (ODbL)', this.m.x, this.vh - this.m.y, { size: 12, color: '#999', weight: 500 });
    this.inFrame(() => this.titleContent(g));
  }

  titleContent(g) {
    const vw = this.vw;
    this.text('GTA', vw / 2, 150, { size: 64, align: 'center', weight: 900, color: '#fff' });
    this.text('BERLIN', vw / 2, 232, { size: 96, align: 'center', weight: 900, color: YELLOW });
    this.text('Kisten für den Kiez', vw / 2, 272, { size: 22, align: 'center', weight: 500, color: '#ddd' });
    if (g.city) {
      this.menu(g.titleMenu, vw / 2, 350);
      this.footerHints([['A', 'Auswählen']]);
    } else this.text(g.loadError ? `Karte nicht ladbar: ${g.loadError}` : 'Lade Berlin …', vw / 2, 380, { size: 22, align: 'center', weight: 600, color: g.loadError ? '#ff8080' : '#ddd' });
  }

  drawPause(g) {
    this.fillScreen('rgba(0,0,0,0.6)');
    this.inFrame(() => this.pauseContent(g));
  }

  pauseContent(g) {
    this.text('PAUSE', this.vw / 2, 170, { size: 56, align: 'center', weight: 900, color: YELLOW });
    const w = g.world;
    this.text(`Aufträge erledigt: ${w.completed}   ·   Bestzeit: ${w.bestTime ? fmtTime(w.bestTime) : '–'}`, this.vw / 2, 210, { size: 18, align: 'center', weight: 500, color: '#ccc' });
    this.menu(g.pauseMenu, this.vw / 2, 280);
    this.footerHints([['A', 'Auswählen'], ['B', 'Weiter']]);
  }

  drawResult(g) {
    this.fillScreen('rgba(0,0,0,0.55)');
    this.inFrame(() => this.resultContent(g));
  }

  resultContent(g) {
    const r = g.world.mission.result, vw = this.vw;
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
    this.fillScreen('rgba(5,6,10,0.9)');
    this.inFrame(() => this.controlsContent());
  }

  controlsContent() {
    const c = this.ctx, vw = this.vw;
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
      ['Menüs · Stadtplan', 'Steuerkreuz, A / B', 'Maus: zeigen + klicken'],
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

