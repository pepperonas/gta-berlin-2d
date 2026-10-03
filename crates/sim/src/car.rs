//! Fahrzeuge (Port von `car.js`): Arcade-Fahrphysik für Verkehr, Parker und Räder, echte Fahrdynamik
//! (`dynamics.rs`) für das vom Spieler gefahrene Auto; Kollision gegen die Stadt und gegeneinander, Schaden.
use crate::carmodels::{Kind, car_model, kind, spec_of};
use crate::city::{CircleKind, City, Ground, Solid, WallKind};
use crate::collision::{Obb, circle_vs_obb, obb_vs_obb, obb_vs_rect, obb_vs_segment};
use crate::dynamics::{self, Assists, Body, Controls, DynState, Surface};
use crate::events::Event;
use crate::levels::LevelState;
use crate::math::sign;
use crate::traction::{Aqua, DRY, Traction};
use std::collections::HashMap;

pub const LENGTH: f64 = 42.;
pub const WIDTH: f64 = 20.;
pub const MAX_SPEED: f64 = 330.;
pub const MAX_REVERSE: f64 = 110.;
pub const ACCEL: f64 = 240.;
pub const BRAKE: f64 = 560.;
pub const HANDBRAKE: f64 = 260.;
pub const DRAG: f64 = 0.32;
pub const GRIP: f64 = 9.;
pub const HANDBRAKE_GRIP: f64 = 1.4;
pub const STEER_RATE: f64 = 2.8;
pub const HEALTH: f64 = 100.;
pub const DAMAGE_THRESHOLD: f64 = 70.;
pub const DAMAGE_FACTOR: f64 = 0.07;
pub const RESTITUTION: f64 = 0.3;
/// Poller geben schon bei langsamem Anstoßen nach (px/s ≈ 3 km/h).
pub const KNOCK_SPEED: f64 = 8.;
pub const KNOCK_SLOW: f64 = 0.96;
pub const KNOCK_DAMAGE: f64 = 0.75;
pub const SPEED_TO_KMH: f64 = 0.36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    Player,
    Npc,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Traffic,
    Player,
    Parked,
    Curb,
}

#[derive(Debug, Clone)]
pub struct Car {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub vx: f64,
    pub vy: f64,
    pub ang_vel: f64,
    pub kind: &'static str,
    pub model: Option<&'static str>,
    pub hw: f64,
    pub hh: f64,
    pub health: f64,
    pub wrecked: bool,
    pub wreck_t: f64,
    pub driver: Option<Driver>,
    pub role: Role,
    pub color: u32,
    pub controls: Controls,
    pub cargo: bool,
    pub horn: bool,
    pub horn_was: bool,
    pub skid: f64,
    pub spin: f64,
    pub level: LevelState,
    pub level_init: bool,
    pub dyn_state: Option<DynState>,
    pub esp: bool,
    pub abs: bool,
    pub traction: Traction,
    pub aqua: f64,
    pub aqua_yaw: f64,
    pub park_key: Option<(i64, i8, usize)>,
    pub ai: Option<Box<crate::traffic::Ai>>,
}

impl Car {
    pub fn new(
        id: u32,
        x: f64,
        y: f64,
        angle: f64,
        color: u32,
        role: Role,
        kind_name: &str,
    ) -> Self {
        let k = kind(kind_name);
        Self {
            id,
            x,
            y,
            angle,
            vx: 0.,
            vy: 0.,
            ang_vel: 0.,
            kind: k.name,
            model: None,
            hw: k.l / 2.,
            hh: k.w / 2.,
            health: HEALTH,
            wrecked: false,
            wreck_t: 0.,
            driver: None,
            role,
            color,
            controls: Controls::default(),
            cargo: false,
            horn: false,
            horn_was: false,
            skid: 0.,
            spin: 0.,
            level: LevelState::default(),
            level_init: false,
            dyn_state: None,
            esp: true,
            abs: true,
            traction: DRY,
            aqua: 0.,
            aqua_yaw: 0.,
            park_key: None,
            ai: None,
        }
    }
    pub fn kind_info(&self) -> &'static Kind {
        kind(self.kind)
    }
    pub fn obb(&self) -> Obb {
        Obb {
            x: self.x,
            y: self.y,
            angle: self.angle,
            hw: self.hw,
            hh: self.hh,
        }
    }
    pub fn forward_speed(&self) -> f64 {
        self.vx * self.angle.cos() + self.vy * self.angle.sin()
    }
    pub fn speed(&self) -> f64 {
        self.vx.hypot(self.vy)
    }
    pub fn lvl(&self) -> i8 {
        self.level.lvl
    }
    /// Modell (Fahrverhalten, Name): fest, nach Art oder aus der Nummer.
    pub fn model_name(&self) -> &'static str {
        car_model(self.id, self.kind, self.role == Role::Player, self.model)
    }
}

/// Untergrund → Fahrwerte (car.js `SURFACE`).
pub fn surface_of(g: Ground) -> Surface {
    match g {
        Ground::Cobble => Surface {
            drag: 1.25,
            grip: 0.85,
            top: 0.92,
        },
        Ground::Sidewalk => Surface {
            drag: 1.3,
            grip: 0.95,
            top: 0.9,
        },
        Ground::Grass => Surface {
            drag: 3.2,
            grip: 0.55,
            top: 0.5,
        },
        Ground::Water => Surface {
            drag: 4.,
            grip: 0.4,
            top: 0.3,
        },
        Ground::Road | Ground::Plaza | Ground::Building => Surface {
            drag: 1.,
            grip: 1.,
            top: 1.,
        },
    }
}

/// Ein Schritt Fahrphysik. `ground` = Untergrund unter dem Auto (None = Straße).
pub fn step_car(car: &mut Car, dt: f64, ground: Option<Ground>) {
    let ctl = if car.wrecked {
        Controls {
            handbrake: true,
            ..Default::default()
        }
    } else {
        car.controls
    };
    let surf = surface_of(ground.unwrap_or(Ground::Road));
    let tr = car.traction;
    let aq = car.aqua > 0.;
    let k_brake = tr.brake * if aq { Aqua::BRAKE } else { 1. };
    let k_lat = tr.lat * if aq { Aqua::LAT } else { 1. };
    let k_steer = tr.steer * if aq { Aqua::STEER } else { 1. };
    car.spin = 0.;
    let info = car.kind_info();
    // Der Spieler fährt mit echter Fahrdynamik; Verkehr, geparkte und geschobene Autos sowie Räder arcadig.
    if car.driver == Some(Driver::Player) && info.top.is_none() && !car.wrecked {
        let spec = spec_of(car.model_name());
        let mut body = Body {
            x: car.x,
            y: car.y,
            angle: car.angle,
            vx: car.vx,
            vy: car.vy,
            ang_vel: car.ang_vel,
        };
        let d = car.dyn_state.get_or_insert_with(DynState::default);
        let out = dynamics::step_dynamics(
            &mut body,
            d,
            spec,
            dt,
            surf,
            tr,
            ctl,
            Assists {
                esp: car.esp,
                abs: car.abs,
                aqua: aq,
                aqua_yaw: car.aqua_yaw,
            },
        );
        (car.x, car.y, car.angle, car.vx, car.vy, car.ang_vel) =
            (body.x, body.y, body.angle, body.vx, body.vy, body.ang_vel);
        car.spin = out.spin;
        car.skid = out.skid;
        if car.aqua > 0. {
            car.aqua = (car.aqua - dt).max(0.);
        }
        return;
    }
    let (mut s, mut c) = car.angle.sin_cos();
    let mut vf = car.vx * c + car.vy * s;
    let mut vr = -car.vx * s + car.vy * c;
    let pw = info.power;
    let top = info.top.unwrap_or(MAX_SPEED * (0.55 + 0.45 * pw)) * surf.top;
    if ctl.throttle > 0. && vf < top {
        let t = if vf > 0. { 1. - (vf / top) * 0.55 } else { 1.4 };
        vf += ACCEL * pw * info.accel * ctl.throttle * t * tr.accel * dt;
        car.spin = if ctl.throttle > 0.8 && tr.accel < 0.7 && vf < 150. {
            1.
        } else {
            0.
        };
    }
    if ctl.brake > 0. {
        if vf > 5. {
            vf = (vf - BRAKE * ctl.brake * k_brake * dt).max(0.);
        } else if vf > -(if info.top.is_some() { 22. } else { MAX_REVERSE }) {
            vf -= ACCEL * 0.6 * ctl.brake * dt;
        }
    }
    if ctl.handbrake {
        vf -= sign(vf) * vf.abs().min(HANDBRAKE * k_brake * dt);
    }
    vf -= vf * DRAG * surf.drag * dt;
    if ctl.throttle == 0. && ctl.brake == 0. && vf.abs() < 4. {
        vf = 0.;
    }
    let grip = (if ctl.handbrake {
        HANDBRAKE_GRIP
    } else {
        GRIP * surf.grip
    }) * k_lat;
    car.skid = if vr.abs() > 70. {
        (vr.abs() / 200.).min(1.)
    } else {
        0.
    };
    vr *= (-grip * dt).exp();
    let av = vf.abs();
    let speed_factor = (av / 80.).clamp(0., 1.) * (1. - 0.45 * (av / MAX_SPEED).clamp(0., 1.));
    let target = ctl.steer
        * STEER_RATE
        * speed_factor
        * sign(vf)
        * if ctl.handbrake { 1.35 } else { 1. }
        * k_steer
        + if aq { car.aqua_yaw } else { 0. };
    car.ang_vel += (target - car.ang_vel) * (12. * dt).min(1.);
    car.angle += car.ang_vel * dt;
    (s, c) = car.angle.sin_cos();
    car.vx = vf * c - vr * s;
    car.vy = vf * s + vr * c;
    car.x += car.vx * dt;
    car.y += car.vy * dt;
    if car.aqua > 0. {
        car.aqua = (car.aqua - dt).max(0.);
    }
}

pub fn damage(car: &mut Car, impact: f64, events: &mut Vec<Event>) {
    if impact > DAMAGE_THRESHOLD {
        car.health = (car.health - (impact - DAMAGE_THRESHOLD) * DAMAGE_FACTOR).max(0.);
        events.push(Event::Crash {
            x: car.x,
            y: car.y,
            strength: (impact / 300.).clamp(0., 1.),
            car: car.id,
        });
        if car.health <= 0. && !car.wrecked {
            car.wrecked = true;
            events.push(Event::Wreck {
                x: car.x,
                y: car.y,
                car: car.id,
            });
        }
    }
}

fn apply_impact(car: &mut Car, nx: f64, ny: f64, events: &mut Vec<Event>) -> f64 {
    let vn = car.vx * nx + car.vy * ny;
    if vn >= 0. {
        return 0.;
    }
    car.vx -= (1. + RESTITUTION) * vn * nx;
    car.vy -= (1. + RESTITUTION) * vn * ny;
    car.vx *= 0.85;
    car.vy *= 0.85;
    car.ang_vel *= 0.5;
    damage(car, -vn, events);
    -vn
}

/// Umgefahrene Poller (Schlüssel → Fallrichtung), je Welt gemerkt.
pub type Knocked = HashMap<(i64, i64), f64>;

/// Sperrt ein festes Hindernis ein Objekt der Ebene `lvl`? Stadtgrenze immer; Häuser, Bäume, Kisten und Poller am
/// Boden sperren Boden und Unterführung, nicht die Brücke; Wände (Ufer, Gleis, Geländer, Zaun) nur ihre Ebene.
pub fn blocks(knocked: &Knocked, s: &Solid, lvl: i8) -> bool {
    match s {
        Solid::Circle {
            kind: CircleKind::Barrier { key },
            ..
        } if knocked.contains_key(key) => false,
        Solid::Wall {
            kind: WallKind::Border,
            ..
        } => true,
        Solid::Wall {
            kind: WallKind::Wall,
            lvl: l,
            ..
        } => *l == lvl,
        _ => lvl <= 0,
    }
}

/// Auto gegen die statische Welt (Gebäude, Wände, Kisten, Bäume, Poller, Stadtgrenze).
pub fn collide_car_world(
    car: &mut Car,
    city: &mut City,
    knocked: &mut Knocked,
    events: &mut Vec<Event>,
) {
    for _ in 0..3 {
        let mut hit = false;
        for h in city.solids.query(&car.obb().bounds()) {
            let s = *city.solids.get(h);
            if !blocks(knocked, &s, car.lvl()) {
                continue;
            }
            let o = car.obb();
            let m = match s {
                Solid::Wall { seg, .. } => obb_vs_segment(&o, &seg),
                Solid::Circle { x, y, r, .. } => circle_vs_obb(x, y, r, &o).map(|c| c.inverted()),
                Solid::Rect(r) => obb_vs_rect(&o, &r),
            };
            let Some(m) = m else { continue };
            if let Solid::Circle {
                x,
                y,
                kind: CircleKind::Barrier { key },
                ..
            } = s
                && -(car.vx * m.nx + car.vy * m.ny) > KNOCK_SPEED
            {
                knocked.insert(key, car.vy.atan2(car.vx));
                car.vx *= KNOCK_SLOW;
                car.vy *= KNOCK_SLOW;
                car.health = (car.health - KNOCK_DAMAGE).max(0.);
                events.push(Event::Knock { x, y, car: car.id });
                continue;
            }
            car.x += m.nx * m.depth;
            car.y += m.ny * m.depth;
            apply_impact(car, m.nx, m.ny, events);
            hit = true;
        }
        if !hit {
            break;
        }
    }
}

/// Auto gegen Auto: halbe Trennung je Seite, elastischer Stoß, Schaden für beide.
pub fn collide_cars(a: &mut Car, b: &mut Car, events: &mut Vec<Event>) {
    let Some(m) = obb_vs_obb(&a.obb(), &b.obb()) else {
        return;
    };
    a.x += m.nx * m.depth / 2.;
    a.y += m.ny * m.depth / 2.;
    b.x -= m.nx * m.depth / 2.;
    b.y -= m.ny * m.depth / 2.;
    let vn = (a.vx - b.vx) * m.nx + (a.vy - b.vy) * m.ny;
    if vn >= 0. {
        return;
    }
    let j = -(1. + RESTITUTION) * vn / 2.;
    a.vx += j * m.nx;
    a.vy += j * m.ny;
    b.vx -= j * m.nx;
    b.vy -= j * m.ny;
    damage(a, -vn * 0.8, events);
    damage(b, -vn * 0.8, events);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::Segment;

    fn car() -> Car {
        Car::new(1, 0., 0., 0., 0xff0000, Role::Traffic, "car")
    }

    #[test]
    fn arcade_accelerates_brakes_and_steers() {
        let mut c = car();
        c.controls.throttle = 1.;
        for _ in 0..120 {
            step_car(&mut c, 1. / 60., None);
        }
        let v = c.forward_speed();
        assert!(v > 200. && v < MAX_SPEED, "{v}");
        c.controls = Controls {
            brake: 1.,
            ..Default::default()
        };
        for _ in 0..60 {
            step_car(&mut c, 1. / 60., None);
        }
        assert!(c.forward_speed() <= 5.);
        // Gras bremst und begrenzt das Höchsttempo
        let mut g = car();
        g.controls.throttle = 1.;
        for _ in 0..600 {
            step_car(&mut g, 1. / 60., Some(Ground::Grass));
        }
        assert!(g.forward_speed() < MAX_SPEED * 0.5);
        let mut s = car();
        s.vx = 150.;
        s.controls = Controls {
            throttle: 0.3,
            steer: 1.,
            ..Default::default()
        };
        for _ in 0..30 {
            step_car(&mut s, 1. / 60., None);
        }
        assert!(s.angle > 0.2);
    }

    #[test]
    fn crash_damages_and_wrecks() {
        let mut c = car();
        let mut ev = Vec::new();
        damage(&mut c, 60., &mut ev);
        assert!(ev.is_empty() && c.health == HEALTH);
        damage(&mut c, 300., &mut ev);
        assert!((c.health - (100. - 230. * 0.07)).abs() < 1e-9);
        for _ in 0..10 {
            damage(&mut c, 400., &mut ev);
        }
        assert!(
            c.wrecked
                && ev
                    .iter()
                    .filter(|e| matches!(e, Event::Wreck { .. }))
                    .count()
                    == 1
        );
    }

    #[test]
    fn car_against_car_is_symmetric() {
        let mut a = car();
        let mut b = Car::new(2, 40., 0., 0., 0, Role::Traffic, "car");
        a.vx = 200.;
        let mut ev = Vec::new();
        collide_cars(&mut a, &mut b, &mut ev);
        assert!((a.x + 1.).abs() < 1e-9 && (b.x - 41.).abs() < 1e-9);
        assert!((a.vx + b.vx - 200.).abs() < 1e-9, "Impuls bleibt erhalten");
        assert!(b.vx > 0. && a.vx < 200.);
    }

    #[test]
    fn walls_block_only_their_level() {
        let k = Knocked::new();
        let wall = Solid::Wall {
            seg: Segment {
                ax: 0.,
                ay: 0.,
                bx: 1.,
                by: 0.,
            },
            kind: WallKind::Wall,
            sub: crate::city::WallSub::Quay,
            lvl: 0,
        };
        let border = Solid::Wall {
            seg: Segment {
                ax: 0.,
                ay: 0.,
                bx: 1.,
                by: 0.,
            },
            kind: WallKind::Border,
            sub: crate::city::WallSub::Other,
            lvl: 0,
        };
        assert!(blocks(&k, &wall, 0) && !blocks(&k, &wall, 1) && blocks(&k, &border, 1));
        let post = Solid::Circle {
            x: 0.,
            y: 0.,
            r: 1.5,
            kind: CircleKind::Barrier { key: (0, 0) },
        };
        let mut k2 = Knocked::new();
        k2.insert((0, 0), 0.);
        assert!(blocks(&k, &post, 0) && !blocks(&k2, &post, 0) && !blocks(&k, &post, 1));
    }
}
