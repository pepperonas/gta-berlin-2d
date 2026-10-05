// Rechte Maustaste und Waffenrad (rein, ohne DOM – main.js füttert Zeitpunkte und Mausposition, hud.js zeichnet).
//  – kurz tippen: in ein Auto ein- bzw. aussteigen (wie F / Y);
//  – gedrückt halten (zu Fuß): das Waffenrad öffnet sich genau am Mauszeiger (am Bildrand so weit hereingerückt, dass es
//    ganz sichtbar ist). Gewählt ist, worauf der Zeiger zeigt – schon ein kleiner Ruck in Richtung eines Felds reicht,
//    und alles jenseits des Rings zählt ebenfalls. Loslassen oder Linksklick nimmt die Waffe. In der Mitte (Totzone)
//    bleibt die zuletzt gezeigte Wahl. Solange das Rad offen ist, läuft das Spiel langsamer (WHEEL.slow).
//  Am Controller dasselbe mit LB: tippen = vorige Waffe, halten = Rad, rechter Stick wählt.

export const WHEEL = {
  hold: 0.22,    // s bis das Rad aufgeht (kürzer = Tippen)
  dead: 14,      // px (HUD-Einheiten) Totzone um die Mitte des Rads
  slow: 0.3,     // Spieltempo bei offenem Rad
  radius: 190,   // Außenradius (HUD-Einheiten)
  inner: 72,     // Innenradius
  stickDead: 0.45, // Controller: Stickausschlag, ab dem gewählt wird
  ease: 0.12,    // s: Zeitlupe und Einblenden
};

// Segment zur Richtung (dx, dy) bei n Waffen: 0 oben, im Uhrzeigersinn; -1 in der Totzone
export function wheelSlot(dx, dy, n, dead = WHEEL.dead) {
  if (!(n > 0) || Math.hypot(dx, dy) < dead) return -1;
  const a = (Math.atan2(dx, -dy) + 2 * Math.PI) % (2 * Math.PI); // 0 = oben, wächst im Uhrzeigersinn
  const step = 2 * Math.PI / n;
  return Math.floor((a + step / 2) / step) % n;
}

// Mitte eines Segments (Winkel wie wheelSlot) → Einheitsvektor im Bild (x rechts, y unten)
export function slotDir(i, n) {
  const a = i * 2 * Math.PI / n;
  return { x: Math.sin(a), y: -Math.cos(a) };
}

// Zustandsautomat einer Rad-Taste (rechte Maustaste oder LB am Controller).
// Rückgaben der Methoden: { tap? (kurz getippt), pick? (Index), opened?, closed? }
//  – Maus: move(x, y) mit dem echten Zeiger (HUD-Punkte); gewählt ist das Feld in seiner Richtung ab der Radmitte.
//    place(cx, cy) legt die Mitte fest (main.js: an den Zeiger, im Bild gehalten); ohne place zählt die Druckstelle.
//  – Controller: aim(x, y) mit dem Stick (-1…1), Totzone WHEEL.stickDead; losgelassener Stick behält die Wahl.
//  – nudge(±1): Mausrad dreht die Wahl weiter; choose(i): Zifferntaste wählt und schließt; cancel(): ohne Wahl zu.
//  – sync(held): ist die Taste laut Gerät nicht mehr gedrückt (Loslassen außerhalb des Fensters, verlorenes Ereignis),
//    wird wie beim Loslassen entschieden – so bleibt das Rad nie hängen.
export function createRightButton(opts = {}) {
  const cfg = { ...WHEEL, ...opts };
  const st = { down: false, t0: 0, x0: 0, y0: 0, lx: 0, ly: 0, vx: 0, vy: 0, open: false, hover: -1, n: 0, openedAt: 0, cx: null, cy: null };
  const hoverFrom = (dx, dy, dead) => { const i = wheelSlot(dx, dy, st.n, dead); if (i >= 0) st.hover = i; };
  return {
    state: st,
    get open() { return st.open; },
    get down() { return st.down; },
    press(t, x = 0, y = 0) { Object.assign(st, { down: true, t0: t, x0: x, y0: y, lx: x, ly: y, vx: 0, vy: 0, open: false, hover: -1, cx: null, cy: null }); return {}; },
    // jeden Frame: canOpen = zu Fuß im Spiel; current = gewählte Waffe (Startwert der Anzeige), n = Anzahl Waffen
    tick(t, canOpen, current, n) {
      if (st.open && !canOpen) { st.open = false; st.down = false; return { closed: true }; } // eingestiegen, K. o. …
      if (st.down && !st.open && canOpen && t - st.t0 >= cfg.hold - 1e-9) {
        Object.assign(st, { open: true, hover: current, n, vx: 0, vy: 0, openedAt: t, cx: st.lx, cy: st.ly });
        return { opened: true };
      }
      return {};
    },
    // Mitte des offenen Rads (HUD-Punkte); der Zeiger zählt ab hier
    place(cx, cy) { if (st.open) { st.cx = cx; st.cy = cy; this.move(st.lx, st.ly); } return {}; },
    move(x, y) {
      st.lx = x; st.ly = y;
      if (!st.open) return {};
      st.vx = x - st.cx; st.vy = y - st.cy;
      hoverFrom(st.vx, st.vy, cfg.dead);
      return {};
    },
    aim(x, y) {
      if (!st.open) return {};
      if (Math.hypot(x, y) < cfg.stickDead) return {};
      st.vx = x * cfg.radius; st.vy = y * cfg.radius;
      hoverFrom(x, y, 0);
      return {};
    },
    nudge(d) { if (st.open && st.n) { st.hover = ((st.hover < 0 ? 0 : st.hover) + Math.sign(d) + st.n) % st.n; st.vx = st.vy = 0; } return {}; },
    choose(i) { if (!st.open || !(i >= 0 && i < st.n)) return {}; st.open = false; st.down = false; return { pick: i, closed: true }; },
    release(t) {
      if (!st.down) return {};
      st.down = false;
      if (st.open) { st.open = false; return st.hover >= 0 ? { pick: st.hover, closed: true } : { closed: true }; }
      return t - st.t0 < cfg.hold ? { tap: true } : {}; // getippt: nie Ein-/Aussteigen (das geht nur per Taste)
    },
    sync(t, held) { return st.down && !held ? this.release(t) : {}; },
    cancel() { const was = st.open; st.down = false; st.open = false; return was ? { closed: true, cancelled: true } : {}; },
  };
}

// Zeitlupe weich ein- und ausblenden: aktueller Faktor → Ziel (1 oder WHEEL.slow) in etwa WHEEL.ease Sekunden
export function easeTimeScale(cur, open, dt) {
  const target = open ? WHEEL.slow : 1;
  const k = Math.min(1, dt / WHEEL.ease);
  const next = cur + (target - cur) * k;
  return Math.abs(next - target) < 0.005 ? target : next;
}

// Waffensymbole (Vektor, Canvas 2D): Mitte (x, y), Größe s (etwa Breite), Farbe col
export function drawWeaponIcon(c, id, x, y, s, col = '#fff') {
  c.save();
  c.translate(x, y);
  const k = s / 100; c.scale(k, k);
  c.fillStyle = col; c.strokeStyle = col; c.lineJoin = 'round'; c.lineCap = 'round';
  const poly = (pts) => { c.beginPath(); c.moveTo(pts[0], pts[1]); for (let i = 2; i < pts.length; i += 2) c.lineTo(pts[i], pts[i + 1]); c.closePath(); c.fill(); };
  const rrect = (x0, y0, w, h, r) => { c.beginPath(); c.moveTo(x0 + r, y0); c.arcTo(x0 + w, y0, x0 + w, y0 + h, r); c.arcTo(x0 + w, y0 + h, x0, y0 + h, r); c.arcTo(x0, y0 + h, x0, y0, r); c.arcTo(x0, y0, x0 + w, y0, r); c.closePath(); c.fill(); };
  switch (id) {
    case 'fists': { // Faust: vier Fingerknöchel, Daumen quer
      rrect(-30, -22, 58, 46, 12);
      c.globalCompositeOperation = 'destination-out'; c.lineWidth = 3.5;
      for (const fx of [-15, -1, 13]) { c.beginPath(); c.moveTo(fx, -22); c.lineTo(fx, -4); c.stroke(); }
      c.beginPath(); c.moveTo(-24, 8); c.lineTo(12, 8); c.stroke();
      c.globalCompositeOperation = 'source-over';
      rrect(-26, 20, 30, 16, 6);
      break;
    }
    case 'bat': // Schläger: schräg, zum Griff schmaler, Knauf
      c.rotate(-0.6);
      poly([-46, -3, 14, -9, 46, -8, 50, 0, 46, 8, 14, 9, -46, 3]);
      c.beginPath(); c.arc(-48, 0, 6, 0, Math.PI * 2); c.fill();
      break;
    case 'knife': // Messer: Klinge mit Spitze, Parierstange, Griff
      c.rotate(-0.6);
      poly([-4, -8, 38, -8, 52, 0, 38, 6, -4, 6]);
      rrect(-10, -13, 6, 25, 2);
      rrect(-44, -7, 34, 13, 5);
      break;
    case 'pistol': // Pistole: Schlitten, Griff schräg, Abzugsbügel
      rrect(-38, -24, 76, 18, 4);
      poly([-30, -8, -6, -8, -12, 30, -34, 30]);
      c.lineWidth = 5; c.beginPath(); c.arc(-1, -2, 8, 0.2, Math.PI - 0.2); c.stroke();
      break;
    case 'smg': // MP: Gehäuse, Lauf, langes Magazin, Klappschaft
      rrect(-34, -18, 58, 20, 4);
      rrect(24, -13, 22, 8, 2);
      poly([-2, 2, 10, 2, 8, 38, -4, 38]);
      poly([-24, 2, -12, 2, -16, 22, -28, 22]);
      c.lineWidth = 5; c.beginPath(); c.moveTo(-34, -12); c.lineTo(-54, -10); c.lineTo(-54, 6); c.stroke();
      break;
    case 'shotgun': // Flinte: langer Lauf, Pumpe, Schaft
      rrect(-8, -14, 60, 8, 3);
      rrect(4, -5, 26, 9, 3);
      poly([-8, -16, -24, -16, -56, -4, -56, 10, -42, 10, -14, -2, -8, -2]);
      break;
    default:
      c.beginPath(); c.arc(0, 0, 20, 0, Math.PI * 2); c.fill();
  }
  c.restore();
}
