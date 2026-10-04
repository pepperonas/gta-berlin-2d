//! Nahkampf und Schusswaffen (Port von `combat.js`, deterministisch; Streuung aus dem Welt-Zufall).
//! Die Spielfigur hat alle Waffen von Anfang an, Munition ist unbegrenzt, Magazine werden nachgeladen. Schüsse sind
//! sofortige Strahlen: Sie stoppen an Hauswänden, der Stadtgrenze, Bäumen und Kisten und treffen das erste Ziel
//! (Passant oder Auto). Nahkampf trifft in einem Bogen vor der Figur. Etwa 15 % der Passanten wehren sich.
use crate::car::{Car, Driver, blocks};
use crate::city::{Solid, WallKind};
use crate::collision::{Obb, Rect, Segment};
use crate::events::Event;
use crate::math::{Rng, wrap_angle};
use crate::pedestrians::{Ped, PedState, RADIUS as PED_RADIUS};
use crate::world::World;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weapon {
    pub id: &'static str,
    pub name: &'static str,
    pub melee: bool,
    /// Schaden je Treffer (Schrotflinte: je Kugel)
    pub dmg: f64,
    /// Reichweite (px, 10 px = 1 m)
    pub range: f64,
    /// Öffnungswinkel des Nahkampfbogens
    pub arc: f64,
    /// Sekunden zwischen zwei Angriffen
    pub cooldown: f64,
    /// Streuung (rad) bzw. Fächer der Schrotflinte
    pub spread: f64,
    pub mag: u32,
    pub reload: f64,
    pub pellets: u32,
    /// Dauerfeuer, solange gehalten
    pub auto: bool,
}
const fn melee(
    id: &'static str,
    name: &'static str,
    dmg: f64,
    range: f64,
    arc: f64,
    cooldown: f64,
) -> Weapon {
    Weapon {
        id,
        name,
        melee: true,
        dmg,
        range,
        arc,
        cooldown,
        spread: 0.,
        mag: 0,
        reload: 0.,
        pellets: 1,
        auto: false,
    }
}
#[allow(clippy::too_many_arguments)]
const fn gun(
    id: &'static str,
    name: &'static str,
    dmg: f64,
    range: f64,
    cooldown: f64,
    spread: f64,
    mag: u32,
    reload: f64,
    pellets: u32,
    auto: bool,
) -> Weapon {
    Weapon {
        id,
        name,
        melee: false,
        dmg,
        range,
        arc: 0.,
        cooldown,
        spread,
        mag,
        reload,
        pellets,
        auto,
    }
}
pub const WEAPONS: [Weapon; 6] = [
    melee("fists", "Fäuste", 20., 22., 1.3, 0.3),
    melee("bat", "Baseballschläger", 38., 34., 1.7, 0.55),
    melee("knife", "Messer", 34., 20., 1.0, 0.32),
    gun(
        "pistol", "Pistole", 34., 650., 0.26, 0.025, 12, 1.2, 1, false,
    ),
    gun(
        "smg",
        "Maschinenpistole",
        17.,
        550.,
        0.075,
        0.07,
        30,
        1.7,
        1,
        true,
    ),
    gun(
        "shotgun",
        "Schrotflinte",
        14.,
        320.,
        0.85,
        0.3,
        6,
        2.2,
        8,
        false,
    ),
];
pub const KICK: Weapon = melee("kick", "Tritt", 24., 26., 1.1, 0.5);
pub const PED_HP: f64 = 100.;
/// Kugelschaden auf Autos (Anteil des Treffers)
pub const CAR_BULLET_FACTOR: f64 = 0.45;
/// Zielhilfe am Controller: halber Kegelwinkel (rad)
pub const ASSIST_CONE: f64 = 0.32;
/// so weit fliehen Passanten vor Schüssen (px)
pub const GUNSHOT_SCARE: f64 = 420.;
pub const PLAYER_HP: f64 = 100.;
/// nach 8 s ohne Treffer 4 LP/s zurück
pub const REGEN_DELAY: f64 = 8.;
pub const REGEN_RATE: f64 = 4.;
pub const FIGHTER_SHARE: f64 = 0.15;
pub const FIGHT_DMG: f64 = 9.;
pub const FIGHT_COOLDOWN: f64 = 0.9;
pub const FIGHT_REACH: f64 = 17.;
pub const FIGHT_GIVE_UP: f64 = 20.;
pub const FIGHT_FAR: f64 = 450.;
/// Sekunden K. o., dann Krankenhaus
pub const RESPAWN_DELAY: f64 = 3.;
/// Anteil des Geldes, der dabei verloren geht
pub const HOSPITAL_FEE: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackKind {
    Kick,
    Swing,
    Shot,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attack {
    pub kind: AttackKind,
    pub t: f64,
    pub weapon: &'static str,
}

/// Kampfzustand der Spielfigur.
#[derive(Debug, Clone, PartialEq)]
pub struct Combat {
    pub hp: f64,
    pub since_hurt: f64,
    pub hurt_flash: f64,
    pub weapon: usize,
    pub mag: [u32; 6],
    pub cool: f64,
    pub reload_t: f64,
    pub attack: Option<Attack>,
    pub aim: f64,
    pub dead: bool,
    pub dead_t: f64,
    pub fall: f64,
    /// schon ins Krankenhaus gebracht, wartet auf die Kacheln dort
    pub moved: bool,
}
impl Default for Combat {
    fn default() -> Self {
        Self {
            hp: PLAYER_HP,
            since_hurt: 99.,
            hurt_flash: 0.,
            weapon: 0,
            mag: WEAPONS.map(|w| w.mag),
            cool: 0.,
            reload_t: 0.,
            attack: None,
            aim: 0.,
            dead: false,
            dead_t: 0.,
            fall: 0.,
            moved: false,
        }
    }
}
impl Combat {
    pub fn weapon(&self) -> &'static Weapon {
        &WEAPONS[self.weapon]
    }
}

/// Eingaben des Kampfs (input.js): Feuer gehalten/gedrückt, Tritt, Nachladen, Waffenwahl, Zielen.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CombatInput {
    pub fire: bool,
    pub fire_pressed: bool,
    pub kick: bool,
    pub reload: bool,
    pub weapon_next: bool,
    pub weapon_prev: bool,
    /// 1…6 wählt direkt, 0 = keine Wahl
    pub weapon_slot: u8,
    /// rechter Stick
    pub aim_x: f64,
    pub aim_y: f64,
    /// Maus: Zielpunkt auf dem Boden (Kartenpixel); hat Vorrang vor dem Stick
    pub aim_world: Option<(f64, f64)>,
}

/// Wehrt sich dieser Passant? Fest je Person (aus der Nummer, nicht aus dem Welt-Zufall).
pub fn is_fighter(id: u32) -> bool {
    let x = (id as f64 * 78.233 + 1.7).sin() * 43758.5453;
    x - x.floor() < FIGHTER_SHARE
}

// --- Geometrie ---------------------------------------------------------------------------------------------------

/// Strahl (o) + t·(d), |d| = 1, gegen Kreis: kleinstes t ≥ 0 oder ∞.
pub fn ray_circle(ox: f64, oy: f64, dx: f64, dy: f64, cx: f64, cy: f64, r: f64) -> f64 {
    let (fx, fy) = (ox - cx, oy - cy);
    let b = fx * dx + fy * dy;
    let c = fx * fx + fy * fy - r * r;
    if c <= 0. {
        return 0.;
    }
    let disc = b * b - c;
    if disc < 0. || b > 0. {
        return f64::INFINITY;
    }
    -b - disc.sqrt()
}
pub fn ray_segment(ox: f64, oy: f64, dx: f64, dy: f64, s: &Segment) -> f64 {
    let (ex, ey) = (s.bx - s.ax, s.by - s.ay);
    let den = dx * ey - dy * ex;
    if den.abs() < 1e-9 {
        return f64::INFINITY;
    }
    let (wx, wy) = (s.ax - ox, s.ay - oy);
    let t = (wx * ey - wy * ex) / den;
    let u = (wx * dy - wy * dx) / den;
    if t >= 0. && (0. ..=1.).contains(&u) {
        t
    } else {
        f64::INFINITY
    }
}
/// Strahl gegen gedrehtes Rechteck.
pub fn ray_obb(ox: f64, oy: f64, dx: f64, dy: f64, b: &Obb) -> f64 {
    let (c, s) = (b.angle.cos(), b.angle.sin());
    let (lx, ly) = (
        (ox - b.x) * c + (oy - b.y) * s,
        -(ox - b.x) * s + (oy - b.y) * c,
    );
    let (ldx, ldy) = (dx * c + dy * s, -dx * s + dy * c);
    let (mut t0, mut t1) = (0f64, f64::INFINITY);
    for (o, d, h) in [(lx, ldx, b.hw), (ly, ldy, b.hh)] {
        if d.abs() < 1e-9 {
            if o < -h || o > h {
                return f64::INFINITY;
            }
            continue;
        }
        let (mut a, mut z) = ((-h - o) / d, (h - o) / d);
        if a > z {
            std::mem::swap(&mut a, &mut z);
        }
        t0 = t0.max(a);
        t1 = t1.min(z);
        if t0 > t1 {
            return f64::INFINITY;
        }
    }
    t0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    Wall,
    Ped(usize),
    Car(usize),
    /// fahrender Radfahrer: ein Treffer holt ihn vom Rad
    Bike(usize),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub hit: Option<Target>,
}

/// Erster Treffer entlang eines Strahls. Hindernisse: Hauswände, Stadtgrenze, Bäume, Kisten (Zäune, Gleise, Kaikanten
/// und Geländer sind niedrig – Kugeln fliegen darüber). Ziele auf anderer Ebene trifft der Schütze nicht.
pub fn cast_ray(w: &mut World, ox: f64, oy: f64, ang: f64, range: f64, lvl: i8) -> RayHit {
    let (dx, dy) = (ang.cos(), ang.sin());
    let mut best = range;
    let mut hit = None;
    let (ex, ey) = (ox + dx * range, oy + dy * range);
    let bx = Rect {
        x: ox.min(ex) - 2.,
        y: oy.min(ey) - 2.,
        w: (ex - ox).abs() + 4.,
        h: (ey - oy).abs() + 4.,
    };
    for h in w.city.solids.query(&bx) {
        let s = *w.city.solids.get(h);
        if !blocks(&w.knocked, &s, lvl) {
            continue;
        }
        let t = match s {
            Solid::Wall { seg, kind, .. } => {
                if kind == WallKind::Wall {
                    continue;
                }
                ray_segment(ox, oy, dx, dy, &seg)
            }
            Solid::Circle { x, y, r, .. } => ray_circle(ox, oy, dx, dy, x, y, r),
            Solid::Rect(r) => ray_obb(
                ox,
                oy,
                dx,
                dy,
                &Obb {
                    x: r.x + r.w / 2.,
                    y: r.y + r.h / 2.,
                    angle: 0.,
                    hw: r.w / 2.,
                    hh: r.h / 2.,
                },
            ),
        };
        if t < best {
            best = t;
            hit = Some(Target::Wall);
        }
    }
    for (i, p) in w.peds.iter().enumerate() {
        if p.state == PedState::Dead || p.level.lvl != lvl {
            continue;
        }
        let t = ray_circle(ox, oy, dx, dy, p.x, p.y, PED_RADIUS + 2.);
        if t < best {
            best = t;
            hit = Some(Target::Ped(i));
        }
    }
    for (i, c) in w.cars.iter().enumerate() {
        if Some(c.id) == w.player.in_car || c.lvl() != lvl {
            continue;
        }
        let t = ray_obb(ox, oy, dx, dy, &c.obb());
        if t < best {
            best = t;
            hit = Some(Target::Car(i));
        }
    }
    for (i, b) in w.bikes.iter().enumerate() {
        if b.state != crate::bikes::State::Ride || b.level.lvl != lvl {
            continue;
        }
        let t = ray_circle(ox, oy, dx, dy, b.x, b.y, crate::bikes::RADIUS + 2.);
        if t < best {
            best = t;
            hit = Some(Target::Bike(i));
        }
    }
    RayHit {
        t: best,
        x: ox + dx * best,
        y: oy + dy * best,
        hit,
    }
}

/// Zielhilfe (Controller): nächstes Ziel in einem Kegel um die Zielrichtung mit freier Sichtlinie.
pub fn aim_assist(w: &mut World, ang: f64, range: f64, cone: f64) -> f64 {
    let (px, py, lvl) = (w.player.x, w.player.y, w.player.level.lvl);
    let mut cands: Vec<(f64, f64, Target)> = Vec::new();
    let mut consider = |x: f64, y: f64, t: Target| {
        let (dx, dy) = (x - px, y - py);
        let d = dx.hypot(dy);
        if d < 1. || d > range {
            return;
        }
        let da = wrap_angle(dy.atan2(dx) - ang).abs();
        if da <= cone {
            cands.push((da * 300. + d, dy.atan2(dx), t));
        }
    };
    for (i, p) in w.peds.iter().enumerate() {
        if p.state != PedState::Dead {
            consider(p.x, p.y, Target::Ped(i));
        }
    }
    for (i, c) in w.cars.iter().enumerate() {
        if c.driver == Some(Driver::Npc) && !c.wrecked {
            consider(c.x, c.y, Target::Car(i));
        }
    }
    for (i, b) in w.bikes.iter().enumerate() {
        if b.state == crate::bikes::State::Ride {
            consider(b.x, b.y, Target::Bike(i));
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, a, t) in cands {
        let (x, y) = match t {
            Target::Ped(i) => (w.peds[i].x, w.peds[i].y),
            Target::Car(i) => (w.cars[i].x, w.cars[i].y),
            Target::Bike(i) => (w.bikes[i].x, w.bikes[i].y),
            Target::Wall => continue,
        };
        let d = (x - px).hypot(y - py);
        if cast_ray(w, px, py, a, d + 20., lvl).hit == Some(t) {
            return a;
        }
    }
    ang
}

/// Was liegt unter dem Mauszeiger? Lebende Person (Körper + 4 px) vor Auto (gedrehtes Rechteck + 2 px); liefert
/// dessen Mitte (combat.js pickTarget).
pub fn pick_target(w: &World, x: f64, y: f64) -> Option<(f64, f64)> {
    let lvl = w.player.level.lvl;
    let ped = w
        .peds
        .iter()
        .filter(|p| p.state != PedState::Dead && p.level.lvl == lvl)
        .map(|p| ((p.x - x).hypot(p.y - y), p))
        .filter(|(d, _)| *d < PED_RADIUS + 4.)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, p)) = ped {
        return Some((p.x, p.y));
    }
    let bike = w
        .bikes
        .iter()
        .filter(|b| b.state != crate::bikes::State::Gone && b.level.lvl == lvl)
        .map(|b| ((b.x - x).hypot(b.y - y), b))
        .filter(|(d, _)| *d < crate::bikes::RADIUS + 5.)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, b)) = bike {
        return Some((b.x, b.y));
    }
    w.cars
        .iter()
        .filter(|c| c.lvl() == lvl && Some(c.id) != w.player.in_car)
        .find(|c| {
            let (s, co) = c.angle.sin_cos();
            let (dx, dy) = (x - c.x, y - c.y);
            (dx * co + dy * s).abs() < c.hw + 2. && (-dx * s + dy * co).abs() < c.hh + 2.
        })
        .map(|c| (c.x, c.y))
}

/// Ziele im Nahkampfbogen (Passanten und Autos).
fn melee_targets(w: &World, ang: f64, wp: &Weapon) -> Vec<Target> {
    let p = &w.player;
    let lvl = p.level.lvl;
    let mut out = Vec::new();
    for (i, q) in w.peds.iter().enumerate() {
        if q.state == PedState::Dead || q.level.lvl != lvl {
            continue;
        }
        let (dx, dy) = (q.x - p.x, q.y - p.y);
        let d = dx.hypot(dy);
        if d > wp.range + PED_RADIUS + 6. {
            continue;
        }
        if d > 4. && wrap_angle(dy.atan2(dx) - ang).abs() > wp.arc / 2. {
            continue;
        }
        out.push(Target::Ped(i));
    }
    for (i, b) in w.bikes.iter().enumerate() {
        if b.state != crate::bikes::State::Ride || b.level.lvl != lvl {
            continue;
        }
        let (dx, dy) = (b.x - p.x, b.y - p.y);
        let d = dx.hypot(dy);
        if d > wp.range + crate::bikes::RADIUS + 6. {
            continue;
        }
        if d > 4. && wrap_angle(dy.atan2(dx) - ang).abs() > wp.arc / 2. {
            continue;
        }
        out.push(Target::Bike(i));
    }
    for (i, c) in w.cars.iter().enumerate() {
        if Some(c.id) == p.in_car || c.lvl() != lvl {
            continue;
        }
        let (tx, ty) = (p.x + ang.cos() * wp.range, p.y + ang.sin() * wp.range);
        if ray_obb(p.x, p.y, ang.cos(), ang.sin(), &c.obb()) <= wp.range
            || (tx - c.x).hypot(ty - c.y) < c.hh
        {
            out.push(Target::Car(i));
        }
    }
    out
}

// --- Wirkung -----------------------------------------------------------------------------------------------------

/// Treffer auf einen Passanten; `player` = vom Spieler (Statistik).
pub fn hurt_ped(
    w: &mut World,
    i: usize,
    dmg: f64,
    from: (f64, f64),
    melee: bool,
    weapon: &'static str,
    player: bool,
) {
    let q: &mut Ped = &mut w.peds[i];
    if q.state == PedState::Dead {
        return;
    }
    q.hp -= dmg;
    let a = (q.y - from.1).atan2(q.x - from.0);
    w.events.push(Event::Blood {
        x: q.x,
        y: q.y,
        a,
        n: if melee { 4 } else { 7 },
    });
    q.threat = from;
    if q.hp <= 0. {
        q.state = PedState::Dead;
        q.dead_t = 0.;
        q.fall = a;
        w.events.push(Event::Kill {
            x: q.x,
            y: q.y,
            weapon,
            player,
        });
    } else {
        // Verletzte bleiben auf den Beinen: Zivilisten fliehen, Kämpfer gehen in den Gegenangriff
        if is_fighter(q.id) {
            start_fight(q);
            q.hit_cd = 0.5;
        } else {
            q.state = PedState::Flee;
            q.t = if melee { 1.4 } else { 2.2 };
        }
        if melee {
            q.x += a.cos() * 6.;
            q.y += a.sin() * 6.;
        }
    }
}

/// Radfahrer getroffen: er stürzt vom Rad (das liegen bleibt) und nimmt den Treffer als Person.
pub fn hurt_bike(
    w: &mut World,
    i: usize,
    dmg: f64,
    from: (f64, f64),
    melee: bool,
    weapon: &'static str,
    player: bool,
) {
    if w.bikes[i].state != crate::bikes::State::Ride {
        return;
    }
    let (x, y) = (w.bikes[i].x, w.bikes[i].y);
    let rider = w.dismount(i, from, true);
    w.events.push(Event::BikeDown { x, y, player });
    if let Some(k) = rider {
        hurt_ped(w, k, dmg, from, melee, weapon, player);
    }
}

pub fn hurt_car(w: &mut World, i: usize, dmg: f64, from: (f64, f64)) {
    let c: &mut Car = &mut w.cars[i];
    if c.wrecked {
        return;
    }
    c.health = (c.health - dmg * CAR_BULLET_FACTOR).max(0.);
    w.events.push(Event::Impact {
        x: c.x,
        y: c.y,
        metal: true,
    });
    if c.driver == Some(Driver::Npc) {
        c.shot_at = Some(from); // world.rs: Fahrer steigt aus und flieht
    }
    if c.health <= 0. {
        c.wrecked = true;
        w.events.push(Event::Wreck {
            x: c.x,
            y: c.y,
            car: c.id,
            player: true,
        });
    }
}

/// Treffer auf die Spielfigur (Faustschlag); bei 0 LP K. o. (world.rs schickt sie ins Krankenhaus).
pub fn hurt_player(w: &mut World, dmg: f64, from: (f64, f64)) {
    let p = &mut w.player;
    if p.combat.dead || p.in_car.is_some() || dmg <= 0. || w.god {
        return;
    }
    let c = &mut p.combat;
    c.hp = (c.hp - dmg).max(0.);
    c.since_hurt = 0.;
    c.hurt_flash = 1.;
    let a = (p.y - from.1).atan2(p.x - from.0);
    w.events.push(Event::PlayerHurt {
        x: p.x,
        y: p.y,
        dmg,
    });
    w.events.push(Event::Blood {
        x: p.x,
        y: p.y,
        a,
        n: 3,
    });
    if c.hp <= 0. {
        c.dead = true;
        c.dead_t = 0.;
        c.fall = a;
        c.attack = None;
        c.reload_t = 0.;
        c.moved = false;
        w.events.push(Event::Wasted { x: p.x, y: p.y });
    }
}

/// Nahkampfschlag (Waffe oder Tritt) in Richtung `ang`.
pub fn strike(w: &mut World, wp: &Weapon, ang: f64) -> usize {
    let hits = melee_targets(w, ang, wp);
    let (px, py) = (w.player.x, w.player.y);
    w.events.push(Event::Swing {
        x: px,
        y: py,
        weapon: wp.id,
        hit: !hits.is_empty(),
        npc: false,
    });
    for &t in &hits {
        w.events.push(Event::WeaponHit {
            weapon: wp.id,
            car: matches!(t, Target::Car(_)),
        });
        match t {
            Target::Car(i) => {
                let (x, y) = (w.cars[i].x, w.cars[i].y);
                w.events.push(Event::Thud { x, y });
            }
            Target::Ped(i) => hurt_ped(w, i, wp.dmg, (px, py), true, wp.id, true),
            Target::Bike(i) => hurt_bike(w, i, wp.dmg, (px, py), true, wp.id, true),
            Target::Wall => {}
        }
    }
    hits.len()
}

/// Glockenähnliche Streuung aus drei Zufallszahlen.
fn gauss(rng: &mut Rng) -> f64 {
    (rng.float() + rng.float() + rng.float() - 1.5) * 0.8
}

/// Schuss (eine Salve; die Schrotflinte fächert `pellets` Kugeln).
pub fn shoot(w: &mut World, wp: &Weapon, ang: f64, spread_k: f64) {
    let (px, py, lvl) = (w.player.x, w.player.y, w.player.level.lvl);
    let mut traces = Vec::with_capacity(wp.pellets as usize);
    for k in 0..wp.pellets {
        let off = if wp.pellets > 1 {
            (k as f64 / (wp.pellets - 1) as f64 - 0.5) * wp.spread + (w.rng.float() - 0.5) * 0.06
        } else {
            gauss(&mut w.rng) * wp.spread * spread_k
        };
        let r = cast_ray(w, px, py, ang + off, wp.range, lvl);
        traces.push((r.x, r.y));
        match r.hit {
            Some(Target::Ped(i)) => {
                w.events.push(Event::WeaponHit {
                    weapon: wp.id,
                    car: false,
                });
                hurt_ped(w, i, wp.dmg, (px, py), false, wp.id, true);
            }
            Some(Target::Car(i)) => {
                w.events.push(Event::WeaponHit {
                    weapon: wp.id,
                    car: true,
                });
                hurt_car(w, i, wp.dmg, (px, py));
            }
            Some(Target::Bike(i)) => {
                w.events.push(Event::WeaponHit {
                    weapon: wp.id,
                    car: false,
                });
                hurt_bike(w, i, wp.dmg, (px, py), false, wp.id, true);
            }
            Some(Target::Wall) => w.events.push(Event::Impact {
                x: r.x,
                y: r.y,
                metal: false,
            }),
            None => {}
        }
    }
    w.events.push(Event::Shot {
        x: px + ang.cos() * 10.,
        y: py + ang.sin() * 10.,
        a: ang,
        weapon: wp.id,
        traces,
    });
}

/// Streuung nach Tempo der Figur: Stand ruhiger, joggen und sprinten deutlich unruhiger.
pub fn spread_factor(move_speed: f64) -> f64 {
    if move_speed < 1. {
        0.7
    } else if move_speed <= crate::world::WALK + 1. {
        1.
    } else if move_speed <= crate::world::JOG + 1. {
        1.6
    } else {
        2.5
    }
}

/// Ein Schritt: Waffenwahl, Nachladen, Zielen, Angreifen, Treten.
pub fn update_player_combat(w: &mut World, input: &CombatInput, dt: f64) {
    let c = &mut w.player.combat;
    c.cool = (c.cool - dt).max(0.);
    if let Some(a) = c.attack.as_mut() {
        a.t -= dt;
        if a.t <= 0. {
            c.attack = None;
        }
    }
    c.hurt_flash = (c.hurt_flash - dt * 2.).max(0.);
    if c.dead {
        return;
    }
    c.since_hurt += dt;
    if c.since_hurt > REGEN_DELAY && c.hp < PLAYER_HP {
        c.hp = (c.hp + REGEN_RATE * dt).min(PLAYER_HP);
    }
    if w.player.in_car.is_some() {
        return;
    }
    let n = WEAPONS.len();
    let mut sel = c.weapon;
    if (1..=n as u8).contains(&input.weapon_slot) {
        sel = input.weapon_slot as usize - 1;
    }
    if input.weapon_next {
        sel = (sel + 1) % n;
    }
    if input.weapon_prev {
        sel = (sel + n - 1) % n;
    }
    if sel != c.weapon {
        c.weapon = sel;
        c.reload_t = 0.;
        c.cool = c.cool.max(0.15);
        w.events.push(Event::WeaponSwitch {
            weapon: WEAPONS[sel].id,
        });
    }
    let wp = WEAPONS[c.weapon];
    // Nachladen
    if c.reload_t > 0. {
        c.reload_t -= dt;
        if c.reload_t <= 0. {
            c.reload_t = 0.;
            c.mag[c.weapon] = wp.mag;
            w.events.push(Event::Reloaded { weapon: wp.id });
        }
    } else if !wp.melee && ((input.reload && c.mag[c.weapon] < wp.mag) || c.mag[c.weapon] == 0) {
        c.reload_t = wp.reload;
        w.events.push(Event::Reload { weapon: wp.id });
    }
    if w.player.stun > 0. {
        return;
    }
    // Zielen: Maus › rechter Stick › Blickrichtung; die Maus rastet nur auf dem Ziel unter dem Zeiger ein, am Stick
    // (und beim Angriff ohne Maus) hilft die Zielhilfe
    let (px, py) = (w.player.x, w.player.y);
    let mouse = input
        .aim_world
        .map(|(x, y)| pick_target(w, x, y).unwrap_or((x, y)));
    let explicit = mouse.is_some() || input.aim_x.hypot(input.aim_y) > 0.35;
    let mut ang = match mouse {
        Some((x, y)) => (y - py).atan2(x - px),
        None if explicit => input.aim_y.atan2(input.aim_x),
        None => w.player.angle,
    };
    // Einzelfeuer (Pistole, Schrotflinte) je Druck, Dauerfeuer (MP) und Nahkampf solange gehalten
    let attacking = if wp.auto || wp.melee {
        input.fire
    } else {
        input.fire_pressed
    };
    if explicit || input.fire || input.kick {
        if mouse.is_none() {
            let range = if wp.melee { 60. } else { wp.range };
            ang = aim_assist(w, ang, range, ASSIST_CONE);
        }
        w.player.angle = ang;
    }
    let c = &mut w.player.combat;
    c.aim = ang;
    if input.kick && c.cool <= 0. {
        c.cool = KICK.cooldown;
        c.attack = Some(Attack {
            kind: AttackKind::Kick,
            t: 0.28,
            weapon: KICK.id,
        });
        strike(w, &KICK, ang);
        return;
    }
    if !attacking || c.cool > 0. {
        return;
    }
    if wp.melee {
        c.cool = wp.cooldown;
        c.attack = Some(Attack {
            kind: AttackKind::Swing,
            t: 0.22,
            weapon: wp.id,
        });
        strike(w, &wp, ang);
    } else if c.reload_t <= 0. && c.mag[c.weapon] > 0 {
        c.cool = wp.cooldown;
        c.attack = Some(Attack {
            kind: AttackKind::Shot,
            t: 0.08,
            weapon: wp.id,
        });
        c.mag[c.weapon] -= 1;
        let k = spread_factor(w.player.move_speed);
        shoot(w, &wp, ang, k);
    }
}

// --- Passanten, die sich wehren ---------------------------------------------------------------------------------

pub fn start_fight(q: &mut Ped) {
    if matches!(q.state, PedState::Dead | PedState::Down) {
        return;
    }
    q.state = PedState::Fight;
    q.fight_t = 0.;
    q.hit_cd = 0.4;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rays_hit_shapes() {
        assert_eq!(ray_circle(0., 0., 1., 0., 10., 0., 2.), 8.);
        assert_eq!(ray_circle(0., 0., -1., 0., 10., 0., 2.), f64::INFINITY);
        assert_eq!(
            ray_circle(10., 0., 1., 0., 10., 0., 2.),
            0.,
            "im Kreis = sofort"
        );
        let s = Segment {
            ax: 5.,
            ay: -5.,
            bx: 5.,
            by: 5.,
        };
        assert_eq!(ray_segment(0., 0., 1., 0., &s), 5.);
        assert_eq!(ray_segment(0., 6., 1., 0., &s), f64::INFINITY);
        let b = Obb {
            x: 20.,
            y: 0.,
            angle: std::f64::consts::FRAC_PI_2,
            hw: 10.,
            hh: 4.,
        };
        // gedreht: entlang x ist das Rechteck nur 4 breit
        assert!((ray_obb(0., 0., 1., 0., &b) - 16.).abs() < 1e-9);
        assert_eq!(ray_obb(0., 30., 1., 0., &b), f64::INFINITY);
    }
    #[test]
    fn fighters_are_a_fixed_share() {
        let n = (0..10_000).filter(|&i| is_fighter(i)).count();
        assert!((1300..1700).contains(&n), "{n}");
        assert_eq!(is_fighter(42), is_fighter(42));
    }
    #[test]
    fn weapons_match_the_reference() {
        assert_eq!(
            WEAPONS.map(|w| w.id),
            ["fists", "bat", "knife", "pistol", "smg", "shotgun"]
        );
        assert!(WEAPONS[4].auto && WEAPONS[5].pellets == 8 && WEAPONS[3].mag == 12);
        let c = Combat::default();
        assert_eq!(c.mag, [0, 0, 0, 12, 30, 6]);
        assert_eq!(spread_factor(0.), 0.7);
    }
}
