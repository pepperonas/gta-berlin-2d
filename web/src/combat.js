// Nahkampf und Schusswaffen (DOM-frei, deterministisch; Streuung aus dem Welt-Zufall).
// Die Spielfigur hat alle Waffen von Anfang an, Munition ist unbegrenzt, Magazine werden nachgeladen.
// Schüsse sind sofortige Strahlen: sie stoppen an Hauswänden, der Stadtgrenze, Bäumen und Kisten und treffen das
// erste Ziel (Passant oder Auto). Nahkampf trifft in einem Bogen vor der Figur. Treffer erzeugen Ereignisse, aus denen
// render.js Blut, Mündungsfeuer und Leuchtspuren macht und audio.js die Klänge.
import { PED, CAR } from './config.js';
import { wrapAngle } from './math.js';

// dmg je Treffer (bei der Schrotflinte je Schrotkugel); range in px (10 px = 1 m); cooldown s zwischen zwei Angriffen;
// arc: Öffnungswinkel des Nahkampfbogens; spread: Streuung (rad, Standardabweichung) bzw. Fächer der Schrotflinte.
export const WEAPONS = [
  { id: 'fists', name: 'Fäuste', melee: true, dmg: 20, range: 22, arc: 1.3, cooldown: 0.3, knock: 0.9 },
  { id: 'bat', name: 'Baseballschläger', melee: true, dmg: 38, range: 34, arc: 1.7, cooldown: 0.55, knock: 1.8 },
  { id: 'knife', name: 'Messer', melee: true, dmg: 34, range: 20, arc: 1.0, cooldown: 0.32, knock: 0.6 },
  { id: 'pistol', name: 'Pistole', dmg: 34, range: 650, cooldown: 0.26, spread: 0.025, mag: 12, reload: 1.2, pellets: 1 },
  { id: 'smg', name: 'Maschinenpistole', dmg: 17, range: 550, cooldown: 0.075, spread: 0.07, mag: 30, reload: 1.7, pellets: 1, auto: true },
  { id: 'shotgun', name: 'Schrotflinte', dmg: 14, range: 320, cooldown: 0.85, spread: 0.3, mag: 6, reload: 2.2, pellets: 8 },
];
export const KICK = { id: 'kick', name: 'Tritt', melee: true, dmg: 24, range: 26, arc: 1.1, cooldown: 0.5, knock: 2.2 };
export const PED_HP = 100;
export const CAR_BULLET_FACTOR = 0.45;     // Kugelschaden auf Autos (Anteil des Treffers)
export const ASSIST = { cone: 0.32, coneMouse: 0.1 }; // Zielhilfe: halber Kegelwinkel (rad)
export const GUNSHOT_SCARE = 420;          // so weit fliehen Passanten vor Schüssen (px)
export const BODY_KEEP = 60;               // Tote verschwinden frühestens nach 60 s (und nur außer Sicht)
export const PLAYER_HP = 100;
export const REGEN = { delay: 8, rate: 4 };   // nach 8 s ohne Treffer 4 LP/s zurück
export const FIGHTER_SHARE = 0.15;         // so viele Passanten wehren sich mit den Fäusten
export const FIGHT = { dmg: 9, cooldown: 0.9, reach: 17, giveUp: 20, far: 450 };
export const RESPAWN_DELAY = 3;            // Sekunden K. o., dann Krankenhaus
export const HOSPITAL_FEE = 0.1;           // Anteil des Geldes, der dabei verloren geht

// Wehrt sich dieser Passant? Fest je Person (aus der Nummer, nicht aus dem Welt-Zufall).
export function isFighter(ped) {
  const x = Math.sin((ped.id ?? 0) * 78.233 + 1.7) * 43758.5453;
  return x - Math.floor(x) < FIGHTER_SHARE;
}

// Treffer auf die Spielfigur (Faustschlag, Anfahren); bei 0 LP K. o. (world.js schickt sie ins Krankenhaus)
export function hurtPlayer(w, dmg, fromX, fromY) {
  const p = initCombat(w.player);
  if (p.dead || p.inCar || dmg <= 0) return;
  p.hp = Math.max(0, p.hp - dmg); p.sinceHurt = 0; p.hurtFlash = 1;
  w.events.push({ type: 'player-hurt', x: p.x, y: p.y, dmg });
  w.events.push({ type: 'blood', x: p.x, y: p.y, a: Math.atan2(p.y - fromY, p.x - fromX), n: 3 });
  if (p.hp <= 0) {
    p.dead = true; p.deadT = 0; p.fall = Math.atan2(p.y - fromY, p.x - fromX); p.attack = null; p.reloadT = 0;
    w.events.push({ type: 'wasted', x: p.x, y: p.y });
  }
}

export function initCombat(p) {
  p.hp ??= PLAYER_HP; p.sinceHurt ??= 99; p.hurtFlash ??= 0;
  p.weapon ??= 0;
  p.mag ??= WEAPONS.map((wp) => wp.mag ?? 0);
  p.cool ??= 0; p.reloadT ??= 0; p.attack ??= null;
  p.aim ??= p.angle ?? 0;
  return p;
}

export const weaponOf = (p) => WEAPONS[p.weapon ?? 0];

// --- Geometrie ---------------------------------------------------------------------------------------------------

// Strahl (ox, oy) + t·(dx, dy), |d| = 1, gegen Kreis: kleinstes t ≥ 0 oder Infinity
export function rayCircle(ox, oy, dx, dy, cx, cy, r) {
  const fx = ox - cx, fy = oy - cy, b = fx * dx + fy * dy, c = fx * fx + fy * fy - r * r;
  if (c <= 0) return 0;
  const disc = b * b - c;
  if (disc < 0 || b > 0) return Infinity;
  return -b - Math.sqrt(disc);
}

export function raySegment(ox, oy, dx, dy, s) {
  const ex = s.bx - s.ax, ey = s.by - s.ay, den = dx * ey - dy * ex;
  if (Math.abs(den) < 1e-9) return Infinity;
  const wx = s.ax - ox, wy = s.ay - oy;
  const t = (wx * ey - wy * ex) / den, u = (wx * dy - wy * dx) / den;
  return t >= 0 && u >= 0 && u <= 1 ? t : Infinity;
}

// Strahl gegen gedrehtes Rechteck (Auto: x, y, angle, hw, hh)
export function rayObb(ox, oy, dx, dy, b) {
  const c = Math.cos(b.angle), s = Math.sin(b.angle);
  const lx = (ox - b.x) * c + (oy - b.y) * s, ly = -(ox - b.x) * s + (oy - b.y) * c;
  const ldx = dx * c + dy * s, ldy = -dx * s + dy * c;
  let t0 = 0, t1 = Infinity;
  for (const [o, d, h] of [[lx, ldx, b.hw], [ly, ldy, b.hh]]) {
    if (Math.abs(d) < 1e-9) { if (o < -h || o > h) return Infinity; continue; }
    let a = (-h - o) / d, z = (h - o) / d;
    if (a > z) [a, z] = [z, a];
    t0 = Math.max(t0, a); t1 = Math.min(t1, z);
    if (t0 > t1) return Infinity;
  }
  return t0;
}

const tmp = [];
// Erster Treffer entlang eines Strahls. Hindernisse: Hauswände, Stadtgrenze, Bäume, Kisten (Zäune, Gleise, Kaikanten
// und Geländer sind niedrig – Kugeln fliegen darüber).
export function castRay(w, ox, oy, ang, range, shooter = null) {
  const dx = Math.cos(ang), dy = Math.sin(ang);
  let best = range, hit = null;
  const ex = ox + dx * range, ey = oy + dy * range;
  const box = { x: Math.min(ox, ex) - 2, y: Math.min(oy, ey) - 2, w: Math.abs(ex - ox) + 4, h: Math.abs(ey - oy) + 4 };
  for (const s of w.solids.query(box, tmp)) {
    let t;
    if (s.seg) { if (s.kind !== 'building' && s.kind !== 'border') continue; t = raySegment(ox, oy, dx, dy, s); }
    else if (s.r !== undefined) t = rayCircle(ox, oy, dx, dy, s.x, s.y, s.r);
    else t = rayObb(ox, oy, dx, dy, { x: s.x + s.w / 2, y: s.y + s.h / 2, angle: 0, hw: s.w / 2, hh: s.h / 2 });
    if (t < best) { best = t; hit = { type: 'wall' }; }
  }
  for (const ped of w.peds) {
    if (ped.state === 'dead' || ped === shooter) continue;
    const t = rayCircle(ox, oy, dx, dy, ped.x, ped.y, PED.radius + 2);
    if (t < best) { best = t; hit = { type: 'ped', obj: ped }; }
  }
  for (const car of w.cars) {
    if (car.id === w.player.inCar) continue;
    const t = rayObb(ox, oy, dx, dy, car);
    if (t < best) { best = t; hit = { type: 'car', obj: car }; }
  }
  return { t: best, x: ox + dx * best, y: oy + dy * best, hit };
}

// Zielhilfe: nächstes Ziel (Passant oder Auto mit Fahrer) in einem Kegel um die Zielrichtung, freie Sichtlinie.
// Liefert den korrigierten Winkel und das Ziel (oder den unveränderten Winkel).
export function aimAssist(w, p, ang, range, cone = ASSIST.cone) {
  let best = null, score = Infinity;
  const consider = (obj, x, y) => {
    const dx = x - p.x, dy = y - p.y, d = Math.hypot(dx, dy);
    if (d < 1 || d > range) return;
    const da = Math.abs(wrapAngle(Math.atan2(dy, dx) - ang));
    if (da > cone) return;
    const sc = da * 300 + d; // lieber nah an der Zielrichtung, dann nah an der Figur
    if (sc < score) {
      const a = Math.atan2(dy, dx), r = castRay(w, p.x, p.y, a, d + 20);
      if (r.hit && r.hit.obj === obj) { score = sc; best = { obj, ang: a }; }
    }
  };
  for (const ped of w.peds) if (ped.state !== 'dead') consider(ped, ped.x, ped.y);
  for (const car of w.cars) if (car.driver === 'npc' && !car.wrecked) consider(car, car.x, car.y);
  return best ? { ang: best.ang, target: best.obj } : { ang, target: null };
}

// Ziele im Nahkampfbogen (Passanten und Autos)
export function meltargets(w, p, ang, wp) {
  const out = [];
  for (const ped of w.peds) {
    if (ped.state === 'dead') continue;
    const dx = ped.x - p.x, dy = ped.y - p.y, d = Math.hypot(dx, dy);
    if (d > wp.range + PED.radius + 6) continue;
    if (d > 4 && Math.abs(wrapAngle(Math.atan2(dy, dx) - ang)) > wp.arc / 2) continue;
    out.push(ped);
  }
  for (const car of w.cars) {
    if (car.id === w.player.inCar) continue;
    const tx = p.x + Math.cos(ang) * wp.range, ty = p.y + Math.sin(ang) * wp.range;
    if (rayObb(p.x, p.y, Math.cos(ang), Math.sin(ang), car) <= wp.range) out.push(car);
    else if (Math.hypot(tx - car.x, ty - car.y) < CAR.width / 2) out.push(car);
  }
  return out;
}

// --- Wirkung -----------------------------------------------------------------------------------------------------

export function hurtPed(w, ped, dmg, fromX, fromY, melee = false) {
  if (ped.state === 'dead') return;
  ped.hp = (ped.hp ?? PED_HP) - dmg;
  const a = Math.atan2(ped.y - fromY, ped.x - fromX);
  w.events.push({ type: 'blood', x: ped.x, y: ped.y, a, n: melee ? 4 : 7 });
  if (ped.hp <= 0) {
    ped.state = 'dead'; ped.deadT = 0; ped.threat = { x: fromX, y: fromY }; ped.fall = a;
    w.events.push({ type: 'kill', x: ped.x, y: ped.y, id: ped.id });
  } else {
    ped.state = 'down'; ped.t = melee ? 1.4 : 2.2; ped.threat = { x: fromX, y: fromY }; ped.fall = a;
    if (isFighter(ped)) ped.angry = true; // steht auf und schlägt zurück
    if (melee) { ped.x += Math.cos(a) * 6; ped.y += Math.sin(a) * 6; }
  }
}

export function hurtCar(w, car, dmg, fromX, fromY) {
  if (car.wrecked) return;
  car.health = Math.max(0, car.health - dmg * CAR_BULLET_FACTOR);
  w.events.push({ type: 'impact', x: car.x, y: car.y, metal: true });
  if (car.driver === 'npc') car.shotAt = { x: fromX, y: fromY }; // world.js: Fahrer steigt aus und flieht
  if (car.health <= 0) { car.wrecked = true; w.events.push({ type: 'wreck', x: car.x, y: car.y, carId: car.id }); }
}

// Nahkampfschlag (Waffe oder Tritt) in Richtung ang
export function strike(w, p, wp, ang) {
  const hits = meltargets(w, p, ang, wp);
  w.events.push({ type: 'swing', x: p.x, y: p.y, weapon: wp.id, hit: hits.length > 0 });
  for (const t of hits) {
    if (t.hw !== undefined) { w.events.push({ type: 'thud', x: t.x, y: t.y }); continue; } // Auto: nur ein Scheppern
    hurtPed(w, t, wp.dmg, p.x, p.y, true);
  }
  return hits.length;
}

// Schuss (eine Salve; Schrotflinte fächert pellets Kugeln)
export function shoot(w, p, wp, ang, rng) {
  const mx = p.x + Math.cos(ang) * 10, my = p.y + Math.sin(ang) * 10; // Mündung
  const traces = [];
  for (let k = 0; k < wp.pellets; k++) {
    const off = wp.pellets > 1 ? (k / (wp.pellets - 1) - 0.5) * wp.spread + (rng() - 0.5) * 0.06 : gauss(rng) * wp.spread;
    const a = ang + off, r = castRay(w, p.x, p.y, a, wp.range); // ab Körpermitte: trifft auch aus nächster Nähe
    traces.push([r.x, r.y]);
    if (r.hit?.type === 'ped') hurtPed(w, r.hit.obj, wp.dmg, p.x, p.y);
    else if (r.hit?.type === 'car') hurtCar(w, r.hit.obj, wp.dmg, p.x, p.y);
    else if (r.hit?.type === 'wall') w.events.push({ type: 'impact', x: r.x, y: r.y });
  }
  w.events.push({ type: 'shot', x: mx, y: my, a: ang, weapon: wp.id, traces });
}

function gauss(rng) { return (rng() + rng() + rng() - 1.5) * 0.8; }

// --- Spielfigur --------------------------------------------------------------------------------------------------

// Zielrichtung aus der Eingabe: Maus (Weltpunkt) › rechter Stick › Blickrichtung
export function aimFromInput(p, input) {
  if (input.aimWorld) return { ang: Math.atan2(input.aimWorld.y - p.y, input.aimWorld.x - p.x), explicit: true, mouse: true };
  if (Math.hypot(input.aimX ?? 0, input.aimY ?? 0) > 0.35) return { ang: Math.atan2(input.aimY, input.aimX), explicit: true, mouse: false };
  return { ang: p.angle, explicit: false, mouse: false };
}

// Ein Schritt: Waffenwahl, Nachladen, Zielen, Angreifen, Treten.
export function updatePlayerCombat(w, input, dt) {
  const p = initCombat(w.player);
  p.cool = Math.max(0, p.cool - dt);
  if (p.attack && (p.attack.t -= dt) <= 0) p.attack = null;
  p.hurtFlash = Math.max(0, p.hurtFlash - dt * 2);
  if (p.dead) return;
  p.sinceHurt += dt;
  if (p.sinceHurt > REGEN.delay && p.hp < PLAYER_HP) p.hp = Math.min(PLAYER_HP, p.hp + REGEN.rate * dt);
  if (p.inCar) return;
  const n = WEAPONS.length;
  let sel = p.weapon;
  if (input.weaponSlot >= 1 && input.weaponSlot <= n) sel = input.weaponSlot - 1;
  if (input.weaponNext) sel = (sel + 1) % n;
  if (input.weaponPrev) sel = (sel + n - 1) % n;
  if (sel !== p.weapon) { p.weapon = sel; p.reloadT = 0; p.cool = Math.max(p.cool, 0.15); w.events.push({ type: 'weapon', weapon: WEAPONS[sel].id }); }
  const wp = WEAPONS[p.weapon];
  // Nachladen
  if (p.reloadT > 0) {
    p.reloadT -= dt;
    if (p.reloadT <= 0) { p.reloadT = 0; p.mag[p.weapon] = wp.mag; w.events.push({ type: 'reloaded', weapon: wp.id }); }
  } else if (!wp.melee && ((input.reload && p.mag[p.weapon] < wp.mag) || p.mag[p.weapon] <= 0)) {
    p.reloadT = wp.reload; w.events.push({ type: 'reload', weapon: wp.id });
  }
  if (p.stun > 0) return;
  // Zielen
  const aim = aimFromInput(p, input);
  let ang = aim.ang;
  // Einzelfeuer (Pistole, Schrotflinte) je Druck, Dauerfeuer (MP) und Nahkampf solange gehalten
  const attacking = wp.auto || wp.melee ? !!input.fire : !!input.firePressed;
  if (aim.explicit || input.fire || input.kick) {
    const range = wp.melee ? 60 : wp.range;
    ang = aimAssist(w, p, ang, range, aim.mouse ? ASSIST.coneMouse : ASSIST.cone).ang;
    p.angle = ang; // Figur schaut in Zielrichtung
  }
  p.aim = ang;
  if (input.kick && p.cool <= 0) {
    p.cool = KICK.cooldown; p.attack = { kind: 'kick', t: 0.28 };
    strike(w, p, KICK, ang);
    return;
  }
  if (!attacking || p.cool > 0) return;
  if (wp.melee) {
    p.cool = wp.cooldown; p.attack = { kind: 'swing', t: 0.22, weapon: wp.id };
    strike(w, p, wp, ang);
  } else if (p.reloadT <= 0 && p.mag[p.weapon] > 0) {
    p.cool = wp.cooldown; p.attack = { kind: 'shot', t: 0.08, weapon: wp.id };
    p.mag[p.weapon]--;
    shoot(w, p, wp, ang, w.rng);
  }
}

// --- Passanten, die sich wehren ---------------------------------------------------------------------------------

export function startFight(ped) {
  if (ped.state === 'dead' || ped.state === 'down') { ped.angry = true; return; }
  ped.state = 'fight'; ped.fightT = 0; ped.hitCd = 0.4; ped.angry = false;
}

// Ein Schritt im Zustand 'fight' (aus pedestrians.js): zur Spielfigur laufen, zuschlagen, irgendwann aufgeben.
export function updateFight(ped, world, dt, move) {
  const p = world.player;
  ped.fightT = (ped.fightT ?? 0) + dt;
  ped.punch = Math.max(0, (ped.punch ?? 0) - dt);
  const dx = p.x - ped.x, dy = p.y - ped.y, d = Math.hypot(dx, dy);
  if (p.dead || p.inCar || d > FIGHT.far || ped.fightT > FIGHT.giveUp) return false; // aufgeben
  ped.facing = Math.atan2(dy, dx);
  if (d > FIGHT.reach) { const v = Math.min(d - FIGHT.reach + 1, PED.run * 0.85 * dt); move(ped, dx / d * v, dy / d * v, world); }
  ped.hitCd = (ped.hitCd ?? 0) - dt;
  if (d <= FIGHT.reach + 2 && ped.hitCd <= 0) {
    ped.hitCd = FIGHT.cooldown; ped.punch = 0.22;
    world.events.push({ type: 'swing', x: ped.x, y: ped.y, weapon: 'fists', hit: true, npc: true });
    hurtPlayer(world, FIGHT.dmg, ped.x, ped.y);
  }
  return true;
}
