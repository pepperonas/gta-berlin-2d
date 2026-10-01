// Eingabe: Tastatur + Gamepad → abstrakte Aktionen.
// Gamepad-Quellen: (1) Web-Gamepad-API (Browser), (2) Xbox-Hülle, die Windows.Gaming.Input nativ liest
// und per WebView2-Nachricht weiterreicht (die Web-Gamepad-API ist in UWP-WebView2 unzuverlässig).
import { clamp } from './math.js';

// Standard-Mapping (W3C): Tastenindizes.
export const BTN = { A: 0, B: 1, X: 2, Y: 3, LB: 4, RB: 5, LT: 6, RT: 7, VIEW: 8, MENU: 9, LS: 10, RS: 11, UP: 12, DOWN: 13, LEFT: 14, RIGHT: 15 };

export function radialDeadzone(x, y, dz = 0.22) {
  const m = Math.hypot(x, y);
  if (m < dz) return [0, 0];
  const k = Math.min(1, (m - dz) / (1 - dz)) / m;
  return [x * k, y * k];
}

// Lesedaten der Hülle ({ a, b, …, lx, ly, lt, rt }) in die Form eines Standard-Gamepads bringen.
export function fromHostReading(r) {
  const order = ['a', 'b', 'x', 'y', 'lb', 'rb', null, null, 'view', 'menu', 'ls', 'rs', 'up', 'down', 'left', 'right'];
  const buttons = order.map((k, i) => {
    if (i === 6) return { pressed: (r.lt ?? 0) > 0.3, value: r.lt ?? 0 };
    if (i === 7) return { pressed: (r.rt ?? 0) > 0.3, value: r.rt ?? 0 };
    return { pressed: !!r[k], value: r[k] ? 1 : 0 };
  });
  return { id: 'xbox-host', connected: true, mapping: 'standard', buttons, axes: [r.lx ?? 0, -(r.ly ?? 0), r.rx ?? 0, -(r.ry ?? 0)] };
}

// Rohdaten eines Gamepads → { held: {...}, axes }.
export function readPad(gp) {
  const b = (i) => !!gp.buttons[i]?.pressed;
  const v = (i) => gp.buttons[i]?.value ?? (b(i) ? 1 : 0);
  const [lx, ly] = radialDeadzone(gp.axes[0] ?? 0, gp.axes[1] ?? 0);
  const [rx, ry] = radialDeadzone(gp.axes[2] ?? 0, gp.axes[3] ?? 0);
  return {
    lx, ly, rx, ry, lt: v(BTN.LT), rt: v(BTN.RT),
    fire: v(BTN.RT) > 0.5, kick: b(BTN.B), reload: b(BTN.X), wpnNext: b(BTN.RB), wpnPrev: b(BTN.LB),
    a: b(BTN.A), b: b(BTN.B), x: b(BTN.X), y: b(BTN.Y), lb: b(BTN.LB), rb: b(BTN.RB),
    view: b(BTN.VIEW), menu: b(BTN.MENU), up: b(BTN.UP), down: b(BTN.DOWN), left: b(BTN.LEFT), right: b(BTN.RIGHT),
    rideBtn: b(BTN.DOWN),
  };
}

export function readKeys(k) {
  const any = (...codes) => codes.some((c) => k.has(c));
  const lx = (any('KeyD', 'ArrowRight') ? 1 : 0) - (any('KeyA', 'ArrowLeft') ? 1 : 0);
  const ly = (any('KeyS', 'ArrowDown') ? 1 : 0) - (any('KeyW', 'ArrowUp') ? 1 : 0);
  return {
    lx, ly,
    rt: any('KeyW', 'ArrowUp') ? 1 : 0, lt: any('KeyS', 'ArrowDown') ? 1 : 0,
    a: any('KeyE'), b: any('Escape', 'Backspace'), x: any('KeyH'), y: any('KeyF'),
    lb: false, rb: any('Space'), view: any('KeyM'), menu: any('Escape', 'KeyP'),
    up: any('ArrowUp', 'KeyW'), down: any('ArrowDown', 'KeyS'), left: any('ArrowLeft', 'KeyA'), right: any('ArrowRight', 'KeyD'),
    sprint: any('ShiftLeft', 'ShiftRight'), confirmKey: any('Enter', 'Space'),
    fire: any('ControlLeft', 'ControlRight'), kick: any('KeyV'), reload: any('KeyR'), wpnNext: any('KeyQ'),
    slot: [1, 2, 3, 4, 5, 6].find((d) => k.has('Digit' + d)) ?? 0,
    rideBtn: any('KeyG'), espToggle: any('KeyX'), absToggle: any('KeyY'),
  };
}

const EMPTY = { lx: 0, ly: 0, rx: 0, ry: 0, lt: 0, rt: 0, a: false, b: false, x: false, y: false, lb: false, rb: false, view: false, menu: false, up: false, down: false, left: false, right: false, sprint: false, confirmKey: false,
  fire: false, kick: false, reload: false, wpnNext: false, wpnPrev: false, slot: 0, rideBtn: false, espToggle: false, absToggle: false };

export function merge(a, b) {
  const out = { ...EMPTY };
  for (const k of Object.keys(EMPTY)) {
    if (typeof EMPTY[k] === 'number') out[k] = Math.abs(a[k] ?? 0) >= Math.abs(b[k] ?? 0) ? (a[k] ?? 0) : (b[k] ?? 0);
    else out[k] = !!(a[k] || b[k]);
  }
  return out;
}

// Zustandsbehaftet: Flankenerkennung (gedrückt in diesem Frame) und Menü-Wiederholung.
export class InputState {
  constructor() { this.prev = { ...EMPTY }; this.repeat = { dir: null, t: 0 }; this.lastDevice = 'keyboard'; }

  frame(raw, dt) {
    const p = this.prev;
    const edge = (k) => raw[k] && !p[k];
    const stickUp = raw.ly < -0.6, stickDown = raw.ly > 0.6, stickLeft = raw.lx < -0.6, stickRight = raw.lx > 0.6;
    let dir = null;
    if (raw.up || stickUp) dir = 'up'; else if (raw.down || stickDown) dir = 'down';
    else if (raw.left || stickLeft) dir = 'left'; else if (raw.right || stickRight) dir = 'right';
    let menuDir = null;
    if (dir !== this.repeat.dir) { this.repeat = { dir, t: 0.38 }; menuDir = dir; }
    else if (dir) { this.repeat.t -= dt; if (this.repeat.t <= 0) { this.repeat.t = 0.11; menuDir = dir; } }

    const out = {
      moveX: clamp(raw.lx, -1, 1), moveY: clamp(raw.ly, -1, 1),
      steer: clamp(raw.lx, -1, 1), throttle: raw.rt, brake: raw.lt,
      handbrake: raw.rb || raw.b, sprint: raw.sprint || raw.a, horn: raw.x || raw.ls,
      action: edge('a'), actionHeld: raw.a, enterExit: edge('y'), ride: edge('rideBtn'), espToggle: edge('espToggle'), absToggle: edge('absToggle'), pause: edge('menu'), mapToggle: edge('view'),
      menuUp: menuDir === 'up', menuDown: menuDir === 'down', menuLeft: menuDir === 'left', menuRight: menuDir === 'right',
      confirm: edge('a') || edge('confirmKey'), back: edge('b'),
      menuHover: null, menuPick: null, // Maus (setzt main.js)
      // Kampf (nur zu Fuß wirksam): RT/Strg/linke Maustaste, B/V, X/R, LB/RB/Q/Mausrad, 1–6, rechter Stick/Maus
      fire: raw.fire, firePressed: edge('fire'), kick: edge('kick'), reload: edge('reload'),
      weaponNext: edge('wpnNext'), weaponPrev: edge('wpnPrev'), weaponSlot: raw.slot !== p.slot ? raw.slot : 0,
      aimX: raw.rx, aimY: raw.ry, aimWorld: null, // Mausziel (Weltpunkt) setzt main.js
    };
    this.prev = { ...raw };
    return out;
  }
}
