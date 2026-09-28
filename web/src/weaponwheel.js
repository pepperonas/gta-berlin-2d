// Rechte Maustaste und Waffenrad (rein, ohne DOM – main.js füttert Zeitpunkte und Mausposition, hud.js zeichnet).
//  – kurz tippen: in ein Auto ein- bzw. aussteigen (wie F / Y);
//  – gedrückt halten (zu Fuß): das Waffenrad öffnet sich, die Richtung der Maus ab der Stelle des Drucks wählt ein
//    Segment, Loslassen nimmt die Waffe. In der Mitte (Totzone) bleibt die zuletzt gezeigte Wahl – wer nur kurz die Maus
//    zurückzieht, verliert sie nicht. Solange das Rad offen ist, läuft das Spiel langsamer (WHEEL.slow).

export const WHEEL = {
  hold: 0.22,    // s bis das Rad aufgeht (kürzer = Tippen)
  dead: 18,      // px (HUD-Einheiten) Totzone um den Druckpunkt
  slow: 0.3,     // Spieltempo bei offenem Rad
  radius: 190,   // Außenradius (HUD-Einheiten)
  inner: 72,     // Innenradius
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

// Zustandsautomat der rechten Taste. Rückgaben der Methoden: { enterExit?, pick? (Index), opened?, closed? }
export function createRightButton(opts = {}) {
  const cfg = { ...WHEEL, ...opts };
  const st = { down: false, t0: 0, x0: 0, y0: 0, open: false, hover: -1, n: 0 };
  return {
    state: st,
    get open() { return st.open; },
    press(t, x, y) { Object.assign(st, { down: true, t0: t, x0: x, y0: y, open: false, hover: -1 }); return {}; },
    // jeden Frame: canOpen = zu Fuß im Spiel; current = gewählte Waffe (Startwert der Anzeige), n = Anzahl Waffen
    tick(t, canOpen, current, n) {
      if (st.open && !canOpen) { st.open = false; st.down = false; return { closed: true }; } // eingestiegen, K. o. …
      if (st.down && !st.open && canOpen && t - st.t0 >= cfg.hold - 1e-9) { st.open = true; st.hover = current; st.n = n; return { opened: true }; }
      return {};
    },
    move(x, y) {
      if (!st.open) return {};
      const i = wheelSlot(x - st.x0, y - st.y0, st.n, cfg.dead);
      if (i >= 0) st.hover = i;
      return {};
    },
    release(t) {
      if (!st.down) return {};
      st.down = false;
      if (st.open) { st.open = false; return st.hover >= 0 ? { pick: st.hover, closed: true } : { closed: true }; }
      return t - st.t0 < cfg.hold ? { enterExit: true } : {};
    },
    cancel() { const was = st.open; st.down = false; st.open = false; return was ? { closed: true } : {}; },
  };
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
