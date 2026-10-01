// HUD, Menüs und Overlays. Grundformat ist 16:9 (1280 × 720 virtuelle Punkte): Das Raster wird so skaliert, dass diese
// Fläche in jedes Fenster ganz hineinpasst (Maßstab = min(Breite/1280, Höhe/720)); ist das Fenster breiter oder höher,
// wächst die virtuelle Fläche mit (vw ≥ 1280, vh ≥ 720). Das Spiel-HUD hängt an den Fensterrändern (5 % Title-Safe-Rand,
// TV), Menübildschirme liegen in einem zentrierten 1280 × 720-Rahmen (inFrame) – so wird nichts abgeschnitten.
import { makeCanvas } from './lighting.js';
import { specOf, specLine, vehicleName, DRIVE_LABEL } from './carmodels.js';
import { SPEED_TO_KMH, MISSION, CAR, PLAYER } from './config.js';
import { stationById, boardable, entranceNear, STATION } from './station.js';
import { locationName, nearestPoi } from './map.js';
import { formatClock, SUNRISE } from './daylight.js';
import { WEAPONS, PLAYER_HP } from './combat.js';
const WEAPON_NAMES = Object.fromEntries(WEAPONS.map((w) => [w.id, w.name]));
import { WHEEL, slotDir, drawWeaponIcon } from './weaponwheel.js';
import { CONSOLE } from './console.js';
import { STAT_SECTIONS, formatStat, accuracy } from './stats.js';
import { dayName } from './rhythm.js';
import { WX_ICON, WX_LABEL, temperatureAt } from './weather.js';
import { undelta } from './geom.js';
import { mapLabels, prepareStreets } from './maplabels.js';
import { pathOf, ringPath, POI_STYLE } from './render.js';
import { AREA_KIND } from './citycodes.js';
import { VERSION } from './version.js';
import { missionObjective, BRIEFING } from './mission.js';
import { playerCar, speedOf, VEH_INFO_S } from './world.js';
import { vehicleState } from './ride.js';
import { roadWarning } from './traction.js';

const MINI_AREA = { [AREA_KIND.rail]: '#4a4640', [AREA_KIND.allotments]: '#35602c', [AREA_KIND.cemetery]: '#2f5a2a', [AREA_KIND.grass]: '#2f5a2a', [AREA_KIND.pitch]: '#2f5a2a', [AREA_KIND.sand]: '#6b6040', [AREA_KIND.wood]: '#284d22', [AREA_KIND.bridge]: '#4a4540' };
const POI_LABEL = { ubahn: 'U-Bahnhof', sbahn: 'S-Bahnhof', bahn: 'Bahnhof', bus: 'Bushaltestelle', mall: 'Einkaufszentrum',
  supermarket: 'Markt', shop: 'Laden', food: 'Essen', drink: 'Bar', cafe: 'Café', service: 'Service', culture: 'Kultur', hotel: 'Hotel' };
const FONT = 'Segoe UI, system-ui, -apple-system, sans-serif';
// Spiel-HUD: schmale technische Schrift (Xbox/Windows: Bahnschrift, Mac: DIN Alternate), sonst die Systemschrift
const HUD_FONT = `Bahnschrift, 'DIN Alternate', 'Roboto Condensed', 'Arial Narrow', ${FONT}`;
const YELLOW = '#ffd33d';
const GLYPH = { A: '#3fb54a', B: '#e2383f', X: '#2f7fe0', Y: '#f2b705' };
// Grundformat 16:9 in virtuellen HUD-Punkten
export const BASE = { w: 1280, h: 720 };

const KEYS = { A: 'E', B: 'Esc', X: 'H', Y: 'F', RB: 'Leer', LT: 'S', RT: 'W', MENU: 'Esc', VIEW: 'M' };

// Minikarte: Vorrat ±4000 Welt-px (±400 m, doppelt so groß wie der Ausschnitt), neu gezeichnet, wenn der Spieler
// 160 m vom Vorratsmittelpunkt weg ist (dann ist der Ausschnitt noch ganz im Vorrat)
const MINI_HALF = 4000, MINI_MOVE = 1600;
function drawMiniLayers(c, city, x, y, R, q) {
  city.render.query({ x: x - R, y: y - R, w: 2 * R, h: 2 * R }, q);
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
}

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

  // Spiel-HUD-Schrift ohne Kasten: schwarze, runde Kontur unter der Füllung – auf jedem Hintergrund lesbar
  otext(str, x, y, { size = 18, color = '#fff', align = 'left', weight = 700, base = 'alphabetic', italic = false, spacing = 0, outline = 0.85 } = {}) {
    const c = this.ctx;
    c.font = `${italic ? 'italic ' : ''}${weight} ${size}px ${HUD_FONT}`; c.textAlign = align; c.textBaseline = base;
    const sp = spacing && 'letterSpacing' in c;
    if (sp) c.letterSpacing = `${spacing}px`;
    c.lineJoin = 'round'; c.miterLimit = 2;
    c.lineWidth = Math.max(2.5, size * 0.2); c.strokeStyle = `rgba(0,0,0,${outline})`; c.strokeText(str, x, y);
    c.fillStyle = color; c.fillText(str, x, y);
    const w = c.measureText(str).width;
    if (sp) c.letterSpacing = '0px';
    return w;
  }

  // weicher dunkler Schleier hinter Schrift (Verlauf zu allen Seiten, keine Kante) – nur wo Text auf hellem Grund steht
  haze(x, y, w, h, a = 0.4) {
    const c = this.ctx, g = c.createLinearGradient(x, 0, x + w, 0);
    g.addColorStop(0, 'rgba(0,0,0,0)'); g.addColorStop(0.18, `rgba(0,0,0,${a})`); g.addColorStop(0.82, `rgba(0,0,0,${a})`); g.addColorStop(1, 'rgba(0,0,0,0)');
    c.fillStyle = g; c.fillRect(x, y, w, h);
  }

  // Waffensymbol mit Kontur als zwischengespeichertes Bild (je Waffe und Maßstab); ohne Zeichenfläche direkt
  weaponIcon(id, x, y, size) {
    const c = this.ctx, s = this.s ?? 1, key = `${id}|${size}|${s}`;
    this._icons ??= new Map();
    let img = this._icons.get(key);
    if (img === undefined) {
      img = null;
      try {
        const N = Math.ceil(size * 1.3 * s), mk = () => { const cv = makeCanvas(N, N); return [cv, cv.getContext('2d')]; };
        const [a, ga] = mk(), [b, gb] = mk(), [o, go] = mk();
        if (ga?.drawImage) {
          drawWeaponIcon(ga, id, N / 2, N / 2, size * s, '#fff');
          gb.drawImage(a, 0, 0); gb.globalCompositeOperation = 'source-in'; gb.fillStyle = 'rgba(0,0,0,0.85)'; gb.fillRect(0, 0, N, N);
          const d = Math.max(1.5, 2 * s);
          for (let i = 0; i < 8; i++) go.drawImage(b, Math.cos(i * Math.PI / 4) * d, Math.sin(i * Math.PI / 4) * d);
          go.drawImage(a, 0, 0);
          img = { canvas: o, N };
        }
      } catch { img = null; }
      this._icons.set(key, img);
    }
    if (img) { const h = img.N / s; c.drawImage(img.canvas, x - h / 2, y - h / 2, h, h); } else drawWeaponIcon(c, id, x, y, size, '#fff');
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
    c.font = `700 19px ${HUD_FONT}`;
    const body = mt ? (mt[2] ? `halten: ${mt[3]}` : mt[3]) : str;
    const tw = c.measureText(body).width;
    const gw = mt ? (this.device === 'keyboard' ? 40 : 26) : 0;
    const w = tw + gw + (mt ? 10 : 0);
    this.haze(cx - w / 2 - 70, y - 19, w + 140, 38, 0.42);
    let x = cx - w / 2;
    if (mt) { this.glyph(mt[1], x + gw / 2, y, 11); x += gw + 10; }
    this.otext(body, x, y + 7, { size: 19 });
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

  // Befehlszeile: Eingabe unten links mit Vorschau (grau) und blinkender Marke, darüber die Vorschläge (gewählter hell,
  // Art/Hilfe rechts), darüber die letzten Meldungen. Geschlossen: nur frische Meldungen (verblassen nach CONSOLE.logTime).
  drawConsole(con, now, world = null) {
    // rechts neben der Minikarte, links vor der Waffen-/Autoanzeige (Stand des letzten Bildes), sonst am linken Rand
    const c = this.ctx, m = this.m, L = this.layout ?? {}, mm = L.minimap;
    const x = mm && mm.y + mm.h > this.vh / 2 ? mm.x + mm.w + 16 : m.x;
    const right = Math.min(...[L.weapon, L.car].filter((r) => r && r.y + r.h > this.vh / 2).map((r) => r.x - 16), this.vw - m.x);
    const w = Math.max(320, Math.min(760, right - x));
    let y = this.vh - m.y - 44;
    const lines = con.log.filter((l) => con.open || now - l.t < CONSOLE.logTime);
    if (!con.open && !lines.length) return;
    c.save();
    if (con.open) {
      this.panel(x, y, w, 40, 0.88);
      const fs = 19;
      const tw = this.text('>', x + 14, y + 27, { size: fs, weight: 800, color: YELLOW, shadow: false });
      const tx = x + 26 + tw;
      const typed = this.text(con.text, tx, y + 27, { size: fs, weight: 600, shadow: false });
      if (con.sugg?.ghost && con.sel < 0) this.text(con.sugg.ghost, tx + typed, y + 27, { size: fs, weight: 600, color: 'rgba(255,255,255,0.35)', shadow: false });
      if (Math.floor(now * 2) % 2 === 0) { c.fillStyle = YELLOW; c.fillRect(tx + typed + 1, y + 10, 2, 22); }
      if (!con.text) this.text('z. B. wetter oder spawn musclecar · Enter bestätigt · Esc schließt', tx + 8, y + 27, { size: 14, weight: 500, color: 'rgba(255,255,255,0.4)', shadow: false });
      if (con.weatherPanel) {
        const rows = ['Wettertyp', 'Temperatur', 'Schneedecke', 'Straßennässe', 'Glätte'];
        const values = world ? [world.forceWeather == null ? 'Natürlich' : WX_LABEL[world.forceWeather],
          `${(world.forceTemp ?? temperatureAt(world.seed, world.dayCount, world.clock, world.forceWeather)).toFixed(1).replace('.', ',')} °C${world.forceTemp == null ? ' · natürlich' : ''}`,
          `${Math.round((world.snow ?? 0) * 100)} %`, `${Math.round((world.wet ?? 0) * 100)} %`, `${Math.round((world.ice ?? 0) * 100)} %`] : ['–', '–', '–', '–', '–'];
        const rh = 30, h = rows.length * rh + 44;
        y -= h + 6;
        this.panel(x, y, w, h, 0.94);
        this.text('WETTER EINSTELLEN', x + 16, y + 22, { size: 14, weight: 800, color: YELLOW, shadow: false });
        rows.forEach((label, i) => {
          const yy = y + 32 + i * rh;
          if (i === con.weatherRow) { c.fillStyle = 'rgba(255,211,61,0.2)'; rr(c, x + 7, yy, w - 14, rh - 1, 5); c.fill(); }
          this.text(label, x + 18, yy + 20, { size: 15, weight: i === con.weatherRow ? 800 : 600, color: '#fff', shadow: false });
          this.text(`‹  ${values[i]}  ›`, x + w - 18, yy + 20, { size: 15, weight: i === con.weatherRow ? 800 : 600, color: i === con.weatherRow ? YELLOW : '#ddd', align: 'right', shadow: false });
        });
        this.text('↑↓ auswählen · ←→ ändern · Enter fertig · Esc zurück', x + 16, y + h - 8, { size: 12, weight: 500, color: '#aaa', shadow: false });
      }
      // Hilfezeile: Aufbau und Zweck des Befehls, den man gerade tippt
      if (!con.weatherPanel && con.sugg?.help && con.text) { y -= 26; this.text(con.sugg.help, x + 8, y + 18, { size: 14, weight: 600, color: 'rgba(255,211,61,0.85)' }); this.counts = this.counts ?? {}; this.counts.consoleHelp = con.sugg.help; }
      const items = con.weatherPanel ? [] : con.sugg?.items ?? [];
      if (items.length) {
        const rh = 28, h = items.length * rh + 12;
        y -= h + 6;
        this.panel(x, y, w, h, 0.9);
        items.forEach((it, i) => {
          const yy = y + 6 + i * rh;
          if (i === con.sel || (con.sel < 0 && i === 0)) { c.fillStyle = i === con.sel ? 'rgba(255,211,61,0.25)' : 'rgba(255,255,255,0.06)'; rr(c, x + 6, yy, w - 12, rh - 2, 6); c.fill(); }
          this.addHit({ kind: 'sugg', i, x: x + 6, y: yy, w: w - 12, h: rh - 2 }); // anklickbar
          this.text(it.label, x + 18, yy + 19, { size: 16, weight: i === con.sel ? 800 : 600, color: i === con.sel ? YELLOW : it.hint === 'zuletzt' ? '#cfd8e6' : '#fff', shadow: false });
          if (it.hint) this.text(it.hint, x + w - 16, yy + 19, { size: 13, weight: 500, align: 'right', color: '#aaa', shadow: false });
        });
        this.counts = this.counts ?? {}; this.counts.suggestions = items.length;
      }
    }
    for (let i = lines.length - 1; i >= 0; i--) {
      const l = lines[i], a = con.open ? 1 : Math.max(0, 1 - (now - l.t) / CONSOLE.logTime);
      y -= 24;
      this.text(l.text, x + 6, y + 16, { size: 15, weight: l.text.startsWith('>') ? 500 : 700, color: l.ok ? `rgba(255,255,255,${a})` : `rgba(255,128,128,${a})` });
    }
    c.restore();
  }

  // Bildrate, Zeichenzeit und Qualitätsstufe oben in der Mitte (Konsole: fps)
  drawFps(fps, ms, quality, res = 1) {
    const x = this.vw / 2 - 140, y = 10;
    this.panel(x, y, 280, 30, 0.7);
    this.text(`${Math.round(fps)} fps · ${ms.toFixed(1)} ms · ${quality === 'high' ? 'hoch' : 'niedrig'} · ${Math.round(res * 100)} %`, this.vw / 2, y + 21, { size: 14, weight: 700, align: 'center', color: fps >= 50 ? '#8f8' : fps >= 30 ? YELLOW : '#f88', shadow: false });
  }

  // Statistik: Abschnitte in drei Spalten (Unterwegs + Verkehr | Kampf + Aufträge | Nahverkehr), je Zeile „dieses Spiel“ und „insgesamt“; darunter die Waffen
  // (Schüsse/Schläge, Treffer, Quote, Tote). inGame: Spielwelt dahinter.
  drawStats(stats, inGame) {
    this.fillScreen(inGame ? 'rgba(5,6,10,0.82)' : 'rgba(5,6,10,0.94)');
    this.inFrame(() => {
      const c = this.ctx, vw = this.vw, g = stats.game, t = stats.total;
      this.text('STATISTIK', vw / 2, 58, { size: 36, align: 'center', weight: 900, color: YELLOW });
      const sec = (name) => STAT_SECTIONS.find(([n]) => n === name);
      const cols = [[sec('Unterwegs'), sec('Verkehr')], [sec('Kampf'), sec('Aufträge')], [sec('Nahverkehr')]];
      const colW = 370, gap = 20, x0 = vw / 2 - (cols.length * colW + (cols.length - 1) * gap) / 2, vOff = 112;
      let rows = 0, bottom = 0;
      cols.forEach((secs, ci) => {
        const x = x0 + ci * (colW + gap);
        let y = 96;
        this.text('dieses Spiel', x + colW - vOff, y, { size: 13, align: 'right', color: '#999', weight: 700 });
        this.text('insgesamt', x + colW - 8, y, { size: 13, align: 'right', color: '#999', weight: 700 });
        for (const [title, list] of secs) {
          y += 28; this.text(title.toUpperCase(), x, y, { size: 15, weight: 800, color: YELLOW });
          for (const [k, label, fmt] of list) {
            y += 22;
            if (rows++ % 2 === 0) { c.fillStyle = 'rgba(255,255,255,0.04)'; c.fillRect(x - 6, y - 16, colW + 12, 22); }
            this.text(label, x, y, { size: 15, weight: 500, shadow: false });
            this.text(formatStat(g[k] ?? 0, fmt), x + colW - vOff, y, { size: 15, weight: 700, align: 'right', shadow: false });
            this.text(formatStat(t[k] ?? 0, fmt), x + colW - 8, y, { size: 15, weight: 700, align: 'right', color: '#ccc', shadow: false });
          }
        }
        bottom = Math.max(bottom, y);
      });
      // Waffen: direkt unter den Spalten (alles passt in den 720er-Rahmen)
      let y = bottom + 38;
      const wx = vw / 2 - 560;
      this.text('WAFFEN', wx, y, { size: 15, weight: 800, color: YELLOW });
      const heads = ['Schüsse / Schläge', 'Kugeln', 'Treffer', 'Quote', 'Tote'];
      heads.forEach((h, i) => this.text(h, wx + 330 + i * 150, y, { size: 13, align: 'right', color: '#999', weight: 700 }));
      this.text('(dieses Spiel / insgesamt)', wx + 1090, y, { size: 12, align: 'right', color: '#777', weight: 600 });
      for (const [id, wg] of Object.entries(g.weapons)) {
        const wt = t.weapons[id];
        y += 21;
        drawWeaponIcon(c, id, wx + 12, y - 5, 20, '#ddd');
        this.text(WEAPON_NAMES[id] ?? id, wx + 32, y, { size: 15, weight: 600, shadow: false });
        const cells = [[wg.shots, wt.shots], [wg.bullets, wt.bullets], [wg.hits, wt.hits], [`${accuracy(wg)} %`, `${accuracy(wt)} %`], [wg.kills, wt.kills]];
        cells.forEach(([a, b], i) => this.text(`${typeof a === 'number' ? a.toLocaleString('de-DE') : a} / ${typeof b === 'number' ? b.toLocaleString('de-DE') : b}`, wx + 330 + i * 150, y, { size: 15, weight: 600, align: 'right', shadow: false }));
      }
      this.counts = this.counts ?? {}; this.counts.statsBottom = y + 30;
      this.text('Esc / B: zurück', vw / 2, y + 30, { size: 14, align: 'center', color: '#888', weight: 600 });
      this.counts = this.counts ?? {}; this.counts.statRows = rows;
    });
  }

  // Waffenrad (rechte Maustaste halten): runde Segmente mit Symbolen, das gezeigte hell und etwas größer, in der Mitte
  // Name und Munition. hover = Index des gezeigten Segments.
  // opt: { vx, vy (Zeiger ab der Mitte, HUD-Einheiten), age (s seit dem Öffnen), pad (Controller) }
  drawWeaponWheel(p, hover, opt = {}) {
    const c = this.ctx, cx = opt.cx ?? this.vw / 2, cy = opt.cy ?? this.vh / 2, n = WEAPONS.length; // Maus: am Zeiger
    const R = WHEEL.radius, r0 = WHEEL.inner, gap = 0.035, step = 2 * Math.PI / n;
    // Einblenden: kurz wachsen und aufhellen (ohne Bewegungswunsch sofort)
    const u = Math.min(1, Math.max(0, (opt.age ?? 1) / (WHEEL.ease * 1.4))), e = 1 - (1 - u) ** 3;
    c.save();
    c.globalAlpha = 0.35 + 0.65 * e;
    c.fillStyle = `rgba(8,10,14,${(0.35 * e).toFixed(3)})`; c.fillRect(0, 0, this.vw, this.vh); // Welt dahinter abgedunkelt
    c.translate(cx, cy); c.scale(0.86 + 0.14 * e, 0.86 + 0.14 * e); c.translate(-cx, -cy);
    let icons = 0;
    for (let i = 0; i < n; i++) {
      const on = i === hover, sel = i === (p.weapon ?? 0), mid = i * step - Math.PI / 2; // 0 oben
      const a0 = mid - step / 2 + gap, a1 = mid + step / 2 - gap, Ro = on ? R + 10 : R;
      c.beginPath(); c.arc(cx, cy, Ro, a0, a1); c.arc(cx, cy, r0, a1, a0, true); c.closePath();
      c.fillStyle = on ? 'rgba(255,211,61,0.92)' : sel ? 'rgba(46,50,60,0.9)' : 'rgba(20,23,30,0.82)'; c.fill();
      c.lineWidth = on ? 3 : 1.5; c.strokeStyle = on ? '#fff3c4' : sel ? YELLOW : 'rgba(255,255,255,0.18)'; c.stroke();
      const d = slotDir(i, n), rm = (r0 + Ro) / 2, wp = WEAPONS[i];
      drawWeaponIcon(c, wp.id, cx + d.x * rm, cy + d.y * rm - 6, on ? 86 : 72, on ? '#1a1c22' : sel ? YELLOW : '#e8e8e8');
      // Munition unter dem Symbol (Schusswaffen), die Taste daneben
      const col = on ? 'rgba(26,28,34,0.85)' : 'rgba(255,255,255,0.6)';
      if (!wp.melee) this.text(`${p.mag?.[i] ?? wp.mag}/${wp.mag}`, cx + d.x * rm, cy + d.y * rm + 30, { size: 12, weight: 800, align: 'center', color: (p.mag?.[i] ?? 1) === 0 && !on ? '#ff8080' : col, shadow: false });
      if (!opt.pad) this.text(`${i + 1}`, cx + d.x * (Ro - 14), cy + d.y * (Ro - 14) + 5, { size: 11, weight: 800, align: 'center', color: on ? 'rgba(26,28,34,0.6)' : 'rgba(255,255,255,0.35)', shadow: false });
      icons++;
    }
    this.counts = this.counts ?? {}; this.counts.wheelIcons = (this.counts.wheelIcons ?? 0) + icons;
    // Mitte: Name und Munition der gezeigten Waffe
    const wp = WEAPONS[hover >= 0 ? hover : p.weapon ?? 0], k = WEAPONS.indexOf(wp);
    c.beginPath(); c.arc(cx, cy, r0 - 8, 0, Math.PI * 2); c.fillStyle = 'rgba(12,14,19,0.9)'; c.fill();
    // Zeiger: wohin die Maus/der Stick gerade zeigt (auf den Innenkreis begrenzt)
    const vl = Math.hypot(opt.vx ?? 0, opt.vy ?? 0);
    if (opt.pad && vl > 2) { // Maus: der echte Zeiger zeigt selbst
      const L = Math.min(r0 - 12, vl * (r0 - 12) / WHEEL.radius * 1.6), ux = opt.vx / vl, uy = opt.vy / vl;
      c.strokeStyle = 'rgba(255,211,61,0.55)'; c.lineWidth = 3; c.lineCap = 'round';
      c.beginPath(); c.moveTo(cx + ux * 8, cy + uy * 8); c.lineTo(cx + ux * L, cy + uy * L); c.stroke();
      c.fillStyle = YELLOW; c.beginPath(); c.arc(cx + ux * L, cy + uy * L, 4.5, 0, Math.PI * 2); c.fill();
      this.counts.wheelPointer = true;
    }
    this.text(wp.name.toUpperCase(), cx, cy - 4, { size: wp.name.length > 12 ? 12 : 15, weight: 800, align: 'center', color: YELLOW });
    this.text(wp.melee ? 'Nahkampf' : `${p.mag?.[k] ?? wp.mag} / ${wp.mag}`, cx, cy + 20, { size: 15, weight: 700, align: 'center', color: '#ddd' });
    c.restore();
    // Bedienhinweis unter dem Rad
    const hint = opt.pad ? 'Rechter Stick wählt · LB loslassen nimmt die Waffe · B bricht ab' : 'Auf die Waffe zeigen · Loslassen oder Klick nimmt sie · Mausrad oder 1–6 · Esc bricht ab';
    const hx = Math.max(Math.min(cx, this.vw - 380), Math.min(380, this.vw / 2)); // Hinweis bleibt ganz im Bild
    this.text(hint, hx, Math.min(cy + R + 44, this.vh - 12), { size: 14, weight: 600, align: 'center', color: 'rgba(255,255,255,0.75)' });
    this.layout = { ...(this.layout ?? {}), wheel: { cx, cy, R, r0, hover } };
  }

  // Unten rechts zu Fuß (ohne Kasten): Waffenname, Magazin, Symbol mit Kontur, Waffenleiste
  drawWeaponPanel(p) {
    const c = this.ctx, m = this.m;
    const w = 220, h = 70, x = this.vw - m.x - w, y = this.vh - m.y - h;
    this.layout = { ...(this.layout ?? {}), weapon: { x, y, w, h } };
    const wp = WEAPONS[p.weapon ?? 0], right = x + w - 70;
    this.weaponIcon(wp.id, x + w - 30, y + 34, 50);
    this.otext(wp.name.toUpperCase(), right, y + 16, { size: 13, weight: 700, color: YELLOW, align: 'right', spacing: 1.5 });
    if (wp.melee) this.otext('NAHKAMPF', right, y + 46, { size: 20, weight: 800, align: 'right', color: '#e8e8e8' });
    else if (p.reloadT > 0) {
      const u = 1 - p.reloadT / wp.reload;
      this.otext('NACHLADEN', right, y + 40, { size: 14, weight: 800, align: 'right', color: '#ddd', spacing: 1 });
      c.fillStyle = 'rgba(0,0,0,0.55)'; c.fillRect(right - 111, y + 47, 112, 6);
      c.fillStyle = YELLOW; c.fillRect(right - 110, y + 48, 110 * u, 4);
    } else {
      const mag = p.mag?.[p.weapon] ?? 0;
      const tw = this.otext(`/ ${wp.mag}`, right, y + 48, { size: 15, weight: 700, align: 'right', color: '#c8c8c8' });
      this.otext(`${mag}`, right - tw - 5, y + 48, { size: 32, weight: 800, align: 'right', color: mag <= wp.mag * 0.25 ? '#ff7a70' : '#fff' });
    }
    // Waffenleiste: kleine Striche, der gewählte gelb
    for (let i = 0; i < WEAPONS.length; i++) {
      const bx = right - (WEAPONS.length - 1 - i) * 14 - 10;
      c.fillStyle = 'rgba(0,0,0,0.7)'; c.fillRect(bx - 1, y + 59, 12, 5);
      c.fillStyle = i === p.weapon ? YELLOW : 'rgba(255,255,255,0.45)'; c.fillRect(bx, y + 60, 10, 3);
    }
  }

  // Lebensleiste unter der Minikarte (wie im Genre üblich): schmal, grün → gelb → rot, blinkt bei wenig Leben
  drawHealth(p, x, y, w, t) {
    const c = this.ctx, hp = Math.max(0, Math.min(1, (p.hp ?? PLAYER_HP) / PLAYER_HP));
    c.fillStyle = 'rgba(0,0,0,0.6)'; c.fillRect(x, y, w, 6);
    c.fillStyle = 'rgba(70,160,80,0.25)'; c.fillRect(x + 1, y + 1, w - 2, 4);
    const low = hp < 0.25 && Math.floor(t * 3) % 2 === 0;
    c.fillStyle = low ? '#ff6a5a' : hp > 0.6 ? '#5fd35f' : hp > 0.3 ? '#e8c440' : '#e8483c';
    if (hp > 0) c.fillRect(x + 1, y + 1, (w - 2) * hp, 4);
  }

  // Tacho unten rechts: Drehzahlbogen (270°, roter Bereich, Striche je 1000/2000 U/min), km/h groß in der Mitte,
  // Gang in der Lücke unten; links daneben kleine Anzeigen (Antrieb, ESP, Kisten, Schrott). Ohne Kasten.
  drawSpeedo(world, car, eng) {
    const c = this.ctx, m = this.m, R = 54;
    const cx = this.vw - m.x - R - 2, cy = this.vh - m.y - R - 14;
    const x = cx - R - 84, y = cy - R - 6, w = this.vw - m.x - x, h = 2 * R + 20;
    this.layout = { ...(this.layout ?? {}), car: { x, y, w, h } };
    const kmh = Math.round(speedOf(car) * SPEED_TO_KMH);
    const a0 = Math.PI * 0.75, span = Math.PI * 1.5, arc = (r, f0, f1) => { c.beginPath(); c.arc(cx, cy, r, a0 + span * f0, a0 + span * f1); c.stroke(); };
    // weicher dunkler Grund für die Lesbarkeit, ohne Kante
    const bg = c.createRadialGradient(cx, cy, R * 0.3, cx, cy, R * 1.3);
    bg.addColorStop(0, 'rgba(0,0,0,0.42)'); bg.addColorStop(0.75, 'rgba(0,0,0,0.3)'); bg.addColorStop(1, 'rgba(0,0,0,0)');
    c.fillStyle = bg; c.beginPath(); c.arc(cx, cy, R * 1.3, 0, Math.PI * 2); c.fill();
    c.lineCap = 'butt';
    const red = eng?.red ?? 0, redFrom = eng?.electric ? 1.01 : 0.86;
    let f = eng && red > 0 ? Math.max(0, Math.min(1, eng.rpm / red)) : Math.min(1, kmh / (car.top ? car.top * SPEED_TO_KMH : 60));
    c.lineWidth = 5; c.strokeStyle = 'rgba(255,255,255,0.14)'; arc(R, 0, 1);
    if (eng && redFrom < 1) { c.strokeStyle = 'rgba(255,70,55,0.6)'; arc(R, redFrom, 1); }
    // Striche und Zahlen (×1000 U/min)
    if (eng && red > 0) {
      const step = red > 9500 ? 2000 : 1000;
      c.lineWidth = 1.5;
      c.font = `700 9px ${HUD_FONT}`; c.textAlign = 'center'; c.textBaseline = 'middle';
      for (let r = 0; r <= red + 1; r += step) {
        const q = r / red, a = a0 + span * q, ca = Math.cos(a), sa = Math.sin(a), hot = q >= redFrom;
        c.strokeStyle = hot ? 'rgba(255,110,95,0.9)' : 'rgba(255,255,255,0.55)';
        c.beginPath(); c.moveTo(cx + ca * (R - 9), cy + sa * (R - 9)); c.lineTo(cx + ca * (R - 4), cy + sa * (R - 4)); c.stroke();
        c.fillStyle = hot ? 'rgba(255,120,105,0.9)' : 'rgba(255,255,255,0.55)'; c.fillText(`${r / 1000}`, cx + ca * (R - 17), cy + sa * (R - 17));
      }
    }
    // Füllung: dezenter Schein + heller Bogen, im roten Bereich rot
    const hot = f >= redFrom, col = hot ? '#ff5040' : '#ffffff';
    c.lineWidth = 11; c.strokeStyle = hot ? 'rgba(255,70,50,0.18)' : 'rgba(255,255,255,0.1)'; arc(R, 0, f);
    c.lineWidth = 5; c.strokeStyle = col; arc(R, 0, f);
    const ae = a0 + span * f;
    c.lineWidth = 3; c.strokeStyle = hot ? '#ff5040' : YELLOW;
    c.beginPath(); c.moveTo(cx + Math.cos(ae) * (R - 11), cy + Math.sin(ae) * (R - 11)); c.lineTo(cx + Math.cos(ae) * (R + 4), cy + Math.sin(ae) * (R + 4)); c.stroke();
    // Tempo und Gang
    this.otext(`${kmh}`, cx, cy + 9, { size: 32, weight: 800, align: 'center' });
    this.otext('KM/H', cx, cy + 23, { size: 9, weight: 700, align: 'center', color: '#bdbdbd', spacing: 1.5 });
    if (eng) this.otext(eng.electric && eng.gear !== 'R' ? 'D' : `${eng.gear}`, cx, cy + R - 2, { size: 17, weight: 800, align: 'center', color: eng.gear === 'R' ? '#9ad0ff' : hot ? '#ff6a5a' : YELLOW });
    // Fahrzeugzustand: dünner Bogen innen (unten links → unten rechts), nur wenn beschädigt
    const hp = Math.max(0, car.health / CAR.health);
    if (hp < 0.999) {
      c.lineWidth = 2.5; c.strokeStyle = 'rgba(0,0,0,0.5)'; c.beginPath(); c.arc(cx, cy, R + 8, a0, a0 + span * 0.25); c.stroke();
      c.strokeStyle = hp > 0.6 ? '#5fd35f' : hp > 0.3 ? '#e8c440' : '#e8483c';
      if (hp > 0) { c.beginPath(); c.arc(cx, cy, R + 8, a0, a0 + span * 0.25 * hp); c.stroke(); }
    }
    // kleine Anzeigen links vom Tacho
    const lx = cx - R - 14, tags = [];
    if (car.wrecked) tags.push(['SCHROTT', '#ff6a5a']);
    if (car.cargo) tags.push(['▣ KISTEN', '#e6b460']);
    if (car.dyn) {
      const sp = specOf(car), espOn = world.esp !== false;
      const regulating = espOn && car.dyn.esp && Math.floor((world.time ?? 0) * 8) % 2 === 0;
      tags.push([regulating ? 'ESP REGELT' : espOn ? 'ESP AN' : 'ESP AUS', regulating || !espOn ? '#ffb020' : '#8ed49b']);
      tags.push([world.abs === false ? 'ABS AUS' : 'ABS AN', world.abs === false ? '#ffb020' : '#8ed49b']);
      tags.push([DRIVE_LABEL[sp.drive], 'rgba(255,255,255,0.7)']);
    }
    tags.forEach(([t, col], i) => this.otext(t, lx, cy + R - 4 - (tags.length - 1 - i) * 17, { size: 13, weight: 800, align: 'right', color: col, spacing: 1 }));
  }

  // Nach dem Einsteigen: Modellname groß, Bauart/Technik klein darüber dem Tacho – blendet ein und wieder aus
  drawVehInfo(world, car) {
    const vi = world.vehInfo;
    if (!vi || vi.carId !== car.id) return;
    const t = vi.t, a = Math.max(0, Math.min(1, t / 0.35, (VEH_INFO_S - t) / 0.9));
    if (a <= 0.01) return;
    const c = this.ctx, L = this.layout ?? {}, top = Math.min(L.car?.y ?? this.vh, L.roadWarn?.y ?? Infinity);
    const x = this.vw - this.m.x + (1 - Math.min(1, t / 0.35)) * 24, y = top - 12;
    c.save(); c.globalAlpha = a;
    const nm = vehicleName(car);
    this.otext(specLine(car).toUpperCase(), x, y, { size: 12, weight: 700, align: 'right', color: '#d8d8d8', spacing: 1.2 });
    this.otext(nm.full, x, y - 20, { size: 30, weight: 800, align: 'right', italic: true, outline: 0.9 });
    c.restore();
  }

  // Treffer: roter Rand, K. o.: Bild dunkelrot mit Schrift
  drawHurt(p) {
    const c = this.ctx, vw = this.vw, vh = this.vh;
    const a = p.dead ? Math.min(0.55, (p.deadT ?? 0) * 0.4) : (p.hurtFlash ?? 0) * 0.45;
    if (a > 0.01) {
      const g = c.createRadialGradient(vw / 2, vh / 2, Math.min(vw, vh) * 0.3, vw / 2, vh / 2, Math.max(vw, vh) * 0.75);
      g.addColorStop(0, 'rgba(120,0,0,0)'); g.addColorStop(1, `rgba(150,0,0,${a})`);
      c.fillStyle = g; c.fillRect(0, 0, vw, vh);
    }
    if (p.dead) {
      this.text('K. O.', vw / 2, vh / 2 - 10, { size: 72, weight: 900, color: '#ff4d4d', align: 'center' });
      this.text('Du wachst gleich im nächsten Krankenhaus auf …', vw / 2, vh / 2 + 34, { size: 20, align: 'center', color: '#eee' });
    }
  }

  drawGameplay(world, g) {
    const c = this.ctx, m = this.m, vw = this.vw, vh = this.vh;
    const car = playerCar(world);
    const ctx = { places: world.city.places, player: world.player, cars: world.cars };
    const obj = missionObjective(world.mission, ctx);

    // Oben links: Ort, Geld, Tag/Uhrzeit/Wetter, Ort in der Nähe – Schrift mit Kontur, ohne Kasten
    const stn = world.player.inside && stationById(world.city, world.player.inside.id);
    this.otext(stn ? `${stn.sbahn ? 'S' : 'U'}-Bahnhof ${stn.name} · ${stn.lines.join(' ')}` : locationName(world.city, world.player.x, world.player.y), m.x, m.y + 16, { size: 19, weight: 700 });
    const mw = this.otext(`${world.money.toLocaleString('de-DE')} €`, m.x, m.y + 42, { size: 22, color: '#7fe07a', weight: 800 });
    const night = world.clock >= 1230 || world.clock < SUNRISE;
    const wx = world.weather, icon = wx && wx.kind !== 'clear' && (wx.cloud > 0.3 || wx.fog > 0.3) ? WX_ICON[wx.kind] : night ? '☾' : '☀';
    this.otext(`${icon} ${dayName(world.day ?? 4)} ${formatClock(world.clock)} · ${Math.round(world.temp ?? 0) || 0} °C`, m.x + mw + 14, m.y + 41, { size: 15, color: night ? '#c3cdff' : '#ffe39a', weight: 700 });
    // Geschäft/Lokal/Haltestelle in unmittelbarer Nähe
    const here = playerCar(world) ?? world.player;
    const poi = nearestPoi(world.city, here.x, here.y, car ? 120 : 180);
    if (poi) this.otext(`${POI_LABEL[poi.cat]}: ${poi.name}`, m.x, m.y + 62, { size: 13, color: '#d4d4d4', weight: 600 });

    // Oben rechts: Missionsziel + Zeit, rechtsbündig, gelbe Kennung mit Strich
    const ms = world.mission.state;
    if (obj.text) {
      const timed = ms === 'toPickup' || ms === 'toDropoff', R = vw - m.x;
      c.font = `700 18px ${HUD_FONT}`;
      const w = Math.min(vw / 2 - m.x, Math.max(340, c.measureText(obj.text).width + 8)), h = timed ? 76 : 44;
      this.layout = { ...(this.layout ?? {}), mission: { x: R - w, y: m.y, w, h } };
      const lw = this.otext('AUFTRAG', R, m.y + 11, { size: 11, color: YELLOW, weight: 800, align: 'right', spacing: 2.5 });
      c.fillStyle = 'rgba(0,0,0,0.6)'; c.fillRect(R - lw - 37, m.y + 5, 28, 4);
      c.fillStyle = YELLOW; c.fillRect(R - lw - 36, m.y + 6, 26, 2);
      this.otext(obj.text, R, m.y + 34, { size: 18, align: 'right' });
      if (timed) {
        const tm = world.mission.timer, urgent = tm < 15, blink = urgent && Math.floor(world.time * 4) % 2 === 0;
        const tw = this.otext(fmtTime(tm), R, m.y + 66, { size: 26, align: 'right', weight: 800, color: blink ? '#ff4d4d' : urgent ? '#ff8080' : '#fff' });
        this.otext(ms === 'toDropoff' ? 'Kisten geladen ✓' : 'Zeit bis Ladenschluss', R - tw - 12, m.y + 64, { size: 13, align: 'right', color: '#cfcfcf', weight: 600 });
      }
    }

    // Oben mittig: Fahrgast-/Fahrerleiste (Linie, Ziel, nächster Halt; als Fahrer Tempo und Türen)
    this.drawRideBar(world);

    // Unten links: Minikarte mit Lebensleiste darunter (beim Briefing ausgeblendet; unter Tage gedämpft)
    if (ms !== 'briefing') {
      const dim = (world.underground ?? 0) > 0.5, S = 176, my = vh - m.y - S - 10;
      if (dim) { c.save(); c.globalAlpha = 0.6; }
      this.drawMinimap(world, obj.target, m.x, my, S);
      if (dim) c.restore();
      this.drawHealth(world.player, m.x, my + S + 4, S, world.time ?? 0);
      this.layout.minimap = { x: m.x, y: my, w: S, h: S + 10 };
    }

    // Unten rechts: Tacho mit Drehzahl, darüber beim Einsteigen Name und Technik
    if (car) this.drawSpeedo(world, car, g.engine ?? null);

    // Warnschild (Wetter an der Stelle): über dem Tacho, als Zugführer unter der Fahrerleiste
    const warn = roadWarning(world);
    if (warn) {
      const L = this.layout ?? {};
      c.font = `800 14px ${HUD_FONT}`;
      const w = Math.max(150, c.measureText(warn).width + 44), h = 26;
      const x = car ? vw - m.x - w : (L.rideBar ? L.rideBar.x + L.rideBar.w - w : vw - m.x - w);
      const y = car ? L.car.y - h - 8 : (L.rideBar ? L.rideBar.y + L.rideBar.h + 8 : m.y);
      const blink = warn === 'Aquaplaning!' && Math.floor(world.time * 6) % 2 === 0;
      this.layout = { ...L, roadWarn: { x, y, w, h } };
      c.fillStyle = blink ? 'rgba(255,80,60,0.88)' : 'rgba(255,190,40,0.88)'; rr(c, x, y, w, h, 4); c.fill();
      this.text('⚠', x + 12, y + 19, { size: 14, weight: 800, color: '#1a1a1a', shadow: false });
      this.text(warn, x + 32, y + 18, { size: 14, weight: 800, color: '#1a1a1a', shadow: false });
    } else if (this.layout?.roadWarn) { const { roadWarn, ...rest } = this.layout; this.layout = rest; }
    if (car) this.drawVehInfo(world, car);
    if (!car && !world.player.ride) this.drawWeaponPanel(world.player); // im Zug keine Waffe
    this.drawHurt(world.player);

    // Richtungspfeil zum Ziel (am Bildschirmrand, wenn außer Sicht)
    if (obj.target) this.drawTargetArrow(world, obj.target, g);

    // Hinweise unten mittig
    const mission = world.mission;
    let hint = mission.prompt, hintY = vh - m.y - 24;
    // längere Meldungen über den Eckfeldern (Waffe/Tacho unten rechts), damit sie nicht überlappen
    if (!hint && world.notice) { hint = world.notice.text; hintY = vh - m.y - 120; }
    if (!hint && stn) { // im U-Bahnhof: Einsteigen am Bahnsteig, sonst Hinweis auf die Treppen
      const b = boardable(world, stn, world.player.x, world.player.y);
      hint = b ? `G: Einsteigen ${b.train.line} → ${b.train.dest}` : null;
    }
    if (!hint && !world.player.inCar && !stn) {
      const near = world.cars.some((cc) => !cc.wrecked && Math.hypot(cc.x - world.player.x, cc.y - world.player.y) < PLAYER.enterDist);
      const bike = !near && (world.bikes ?? []).find((b) => (b.state === 'ride' || b.state === 'lying') && Math.hypot(b.x - world.player.x, b.y - world.player.y) < 34);
      if (near) hint = 'Y: Einsteigen';
      else if (bike) hint = bike.state === 'ride' ? 'Y: Rad kapern' : 'Y: Aufs Rad';
    }
    if (!hint && !world.player.inCar && !stn && !world.player.ride && (world.player.lvl ?? 0) === 0) { // U-Bahn-Eingang in der Nähe
      const ne = entranceNear(world._stNear, world.player.x, world.player.y, STATION.reach);
      if (ne) hint = `A: Hinunter zur ${ne.stn.sbahn ? 'S' : 'U'}-Bahn ${ne.stn.name}`;
    }
    if (!hint && car && speedOf(car) < 20 && !car.wrecked && g.hintT < 12) hint = car.top || car.hw < 15 ? 'Y: Absteigen' : 'Y: Aussteigen'; // Rad, Motorrad, Roller
    if (!hint && car && car.wrecked) hint = 'Y: Aussteigen – das Auto ist Schrott';
    if (hint && !g.console?.open) this.prompt(hint, vw / 2, hintY); // offene Befehlszeile verdeckt sonst den Hinweis
    if (mission.load > 0 && mission.state === 'toPickup') {
      const w = 260, x = vw / 2 - w / 2, y = vh - m.y - 66;
      c.fillStyle = 'rgba(0,0,0,0.6)'; c.fillRect(x - 1, y - 1, w + 2, 8);
      c.fillStyle = YELLOW; c.fillRect(x, y, w * Math.min(1, mission.load / MISSION.loadTime), 6);
    }
    if (mission.state === 'briefing') this.drawBriefing();
    if (world.loading) this.drawLoading(world);
  }

  // Fahrgast-/Fahrerleiste oben mittig; weicht links aus, wenn rechts das Auftragsfeld steht
  drawRideBar(world) {
    const r = world.player.ride;
    if (!r) return;
    const c = this.ctx, w = 440, h = r.kind === 'driver' ? 92 : 64, y = this.m.y, mis = this.layout?.mission;
    let x = this.vw / 2 - w / 2;
    if (mis && mis.y < y + h && mis.y + mis.h > y) x = Math.min(x, mis.x - 16 - w);
    this.layout = { ...(this.layout ?? {}), rideBar: { x, y, w, h } };
    const st = vehicleState(world, r.ref);
    this.panel(x, y, w, h);
    const col = st?.p.color ? `#${String(st.p.color).replace('#', '')}` : YELLOW;
    c.fillStyle = col; rr(c, x + 14, y + 12, 58, 26, 6); c.fill();
    this.text(r.line, x + 43, y + 31, { size: 16, weight: 900, align: 'center', color: st?.p.color ? '#fff' : '#111', shadow: false });
    this.text(`→ ${String(r.dest ?? '').replace(/^[SU]\s+/, '')}`, x + 84, y + 31, { size: 16, weight: 700 });
    const next = st ? st.p.stopNames[Math.min(st.p.stopNames.length - 1, st.stop)] ?? '' : '';
    const line2 = st?.dwelling ? `Hält: ${next}` : `Nächster Halt: ${next}`;
    this.text(line2.replace(/\s\(.*\)$/, ''), x + 14, y + 56, { size: 15, weight: 600, color: '#ddd' });
    if (r.kind === 'passenger') this.text(st?.underground && !st.dwelling ? 'Aussteigen nur am Bahnsteig' : 'G: aussteigen', x + w - 14, y + 56, { size: 13, weight: 700, align: 'right', color: '#aaa' });
    if (r.kind === 'driver' && world.playerTrain) {
      const t = world.playerTrain, kmh = Math.round(t.v * 0.36);
      this.text(`${kmh} km/h`, x + 14, y + 82, { size: 18, weight: 800, color: t.blocked ? '#ff8080' : '#fff' });
      const doors = t.drive.doors === 'open' ? 'Türen offen – E/A schließen' : t.atStop ? 'E/A: Türen öffnen' : t.blocked ? 'Zug voraus' : 'W/RT Gas · S/LT Bremse · Leertaste/B Notbremse';
      this.text(doors, x + w - 14, y + 82, { size: 13, weight: 700, align: 'right', color: t.atStop ? YELLOW : '#aaa' });
    }
    this.counts = this.counts ?? {}; this.counts.rideBar = true;
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
    const R = 2200;
    c.save();
    rr(c, x, y, size, size, 6); c.fillStyle = '#3a3d44'; c.fill(); c.clip();
    // Kartengrund (Flächen, Häuser, Wasser, Straßen, Grenze): aus einem selten neu gezeichneten Vorrat doppelter Größe
    // kopiert – statt in jedem Bild hunderte Formen zu füllen
    const base = this.miniBase(world, p, size);
    if (base) {
      const k = base.px / (2 * MINI_HALF); // Vorrat-Pixel je Welt-px
      c.drawImage(base.canvas, (p.x - 2000 - (base.cx - MINI_HALF)) * k, (p.y - 2000 - (base.cy - MINI_HALF)) * k, 4000 * k, 4000 * k, x, y, size, size);
    } else {
      c.save();
      c.transform(zoom, 0, 0, zoom, x + size / 2 - p.x * zoom, y + size / 2 - p.y * zoom);
      drawMiniLayers(c, city, p.x, p.y, R, this._mq ??= []);
      c.restore();
    }
    // Bahnhöfe auf der Minikarte
    for (const q of city.poiHash.query({ x: p.x - R, y: p.y - R, w: 2 * R, h: 2 * R }, [])) {
      if (q.cat !== 'ubahn' && q.cat !== 'sbahn') continue;
      const mx = x + size / 2 + (q.x - p.x) * zoom, my = y + size / 2 + (q.y - p.y) * zoom;
      this.stationIcon(q.cat, mx, my, 6);
    }
    const toMini = (wx, wy) => [x + size / 2 + (wx - p.x) * zoom, y + size / 2 + (wy - p.y) * zoom];
    // Eingänge der begehbaren Bahnhöfe (Treppen hinunter): kleine Symbole mit weißem Rand
    for (const stn of world._stNear ?? []) for (const ex of stn.exits) {
      const [mx, my] = toMini(ex.x, ex.y);
      c.fillStyle = '#fff';
      if (stn.sbahn) { c.beginPath(); c.arc(mx, my, 5, 0, Math.PI * 2); c.fill(); } else c.fillRect(mx - 5, my - 5, 10, 10);
      this.stationIcon(stn.sbahn ? 'sbahn' : 'ubahn', mx, my, 4);
    }
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
    c.strokeStyle = 'rgba(0,0,0,0.7)'; c.lineWidth = 3; rr(c, x - 1, y - 1, size + 2, size + 2, 7); c.stroke();
    c.strokeStyle = 'rgba(255,255,255,0.18)'; c.lineWidth = 1; rr(c, x + 0.5, y + 0.5, size - 1, size - 1, 6); c.stroke();
    this.otext('N', x + size / 2, y + 15, { size: 12, align: 'center', weight: 800, color: '#e6e6e6' });
  }

  // Vorrat für den Kartengrund der Minikarte: ±MINI_HALF Welt-px um (cx, cy), neu bei Bewegung über MINI_MOVE, anderer
  // Skalierung oder (höchstens alle 30 Bilder) nachgeladenen Kacheln. null ohne Zeichenfläche (Tests) → direkt zeichnen.
  miniBase(world, p, size) {
    const city = world.city, s = this.s ?? 1, m = this._mini;
    this._miniT = (this._miniT ?? 0) + 1;
    const stale = !m || m.city !== city || m.s !== s || m.size !== size || Math.abs(p.x - m.cx) > MINI_MOVE || Math.abs(p.y - m.cy) > MINI_MOVE
      || (m.gen !== city.gen && this._miniT - m.t > 30);
    if (!stale) return m;
    const px = Math.ceil(2 * MINI_HALF * (size / 4000) * s);
    let canvas = m?.canvas?.width === px ? m.canvas : null;
    try { canvas ??= makeCanvas(px, px); } catch { canvas = null; }
    const g = canvas?.getContext?.('2d');
    if (!g || !g.drawImage) return (this._mini = null);
    const k = px / (2 * MINI_HALF), cx = Math.round(p.x), cy = Math.round(p.y);
    g.setTransform(1, 0, 0, 1, 0, 0); g.fillStyle = '#3a3d44'; g.fillRect(0, 0, px, px);
    g.setTransform(k, 0, 0, k, -(cx - MINI_HALF) * k, -(cy - MINI_HALF) * k);
    drawMiniLayers(g, city, cx, cy, MINI_HALF + 200, this._mq ??= []);
    return (this._mini = { city, s, size, cx, cy, px, canvas, gen: city.gen, t: this._miniT });
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
    // unten über dem Warnschild enden (steht über dem Tacho), sonst liegt die Beschriftung darin
    const warnTop = this.layout?.roadWarn ? this.layout.roadWarn.y - 30 : Infinity;
    const L = mx, R = this.vw - mx, Tp = this.m.y + 120, B = Math.min(this.vh - this.m.y - 140, warnTop);
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

  drawControls(g) {
    this.fillScreen('rgba(5,6,10,0.9)');
    this.inFrame(() => this.controlsContent(g?.settings?.controls ?? 'diablo'));
  }

  controlsContent(scheme = 'diablo') {
    const c = this.ctx, vw = this.vw, d = scheme === 'diablo';
    this.text('STEUERUNG', vw / 2, 110, { size: 44, align: 'center', weight: 900, color: YELLOW });
    this.text(`Zu Fuß am PC:  ‹ ${d ? 'Diablo (Klick)' : 'Klassisch (WASD)'} ›   ← / → wechselt`, vw / 2, 146, { size: 18, align: 'center', weight: 700, color: '#ddd' });
    const rows = [
      ['Laufen / Lenken', 'Linker Stick', d ? 'Linksklick (Boden) · WASD' : 'WASD / Pfeile'],
      ['Angreifen · Tritt (zu Fuß)', 'RT · B', d ? 'Linksklick (Person) · Strg + Klick · rechte Maus tippen' : 'linke Maus · V'],
      ['Sprinten · langsam gehen', 'A halten · Stick halb', 'Umschalt · Alt'],
      ['Gas / Bremse · Rückwärts', 'RT / LT', 'W / S'],
      ['Handbremse', 'RB oder B', 'Leertaste'],
      ['ESP umschalten (im Auto)', '–', 'X'],
      ['ABS umschalten (im Auto)', '–', 'Y'],
      ['Einsteigen / Aussteigen', 'Y', d ? 'Linksklick (Auto) · F' : 'F / rechte Maus tippen'],
      ['Mitfahren (Bus, Tram, S/U-Bahn)', 'Steuerkreuz unten', 'G'],
      ['Waffenrad (zu Fuß)', 'LB halten, rechter Stick', 'rechte Maus halten'],
      ['Aktion (Auftrag, Einladen, Türen/Wenden)', 'A', 'E'],
      ['Befehlszeile (Zeit, Wetter, Teleport …)', '–', 'Enter'],
      ['Zoom (zu Fuß) · Hupe', '– · X', d ? 'Mausrad · H' : '– · H'],
      ['Stadtplan', 'Ansicht-Taste', 'M'],
      ['Pause', 'Menü-Taste', 'Esc / P'],
      ['Menüs · Stadtplan', 'Steuerkreuz, A / B', 'Maus: zeigen + klicken'],
    ];
    const x0 = vw / 2 - 460;
    this.text('Controller', x0 + 470, 170, { size: 18, color: '#aaa', weight: 800 });
    this.text('Tastatur', x0 + 740, 170, { size: 18, color: '#aaa', weight: 800 });
    // Die 16 Zeilen enden bei y=615; darunter bleibt Luft zur Fußzeile (Fuß-Klickfläche beginnt bei 654).
    const rowH = 27;
    rows.forEach(([a, b, k], i) => {
      const y = 210 + i * rowH;
      if (i % 2 === 0) { c.fillStyle = 'rgba(255,255,255,0.05)'; c.fillRect(x0, y - (rowH - 12), 920, rowH); }
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
