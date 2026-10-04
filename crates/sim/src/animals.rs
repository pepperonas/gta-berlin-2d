//! Tiere der Stadt (Port von `animals.js`): Taubenschwärme picken auf Plätzen und vor Imbissen und flattern auf, wenn
//! jemand zu nah kommt, ein Auto vorbeirast oder ein Schuss fällt; Enten paddeln in Ufernähe und schwimmen weg, wenn
//! man am Ufer auf sie zugeht. Orte sind deterministisch (Ort-Hash), Bewegungen laufen über den Welt-Zufall. Neue Tiere
//! entstehen nur außer Sicht, Aufgeflogene verschwinden, sobald sie aus dem Bild sind.
use crate::city::{City, Ground, PolyKind};
use crate::collision::Rect;
use crate::life::{AREA_PLAZA, LifeCache, area_bbox, free_point, h};
use crate::math::Rng;

pub const RADIUS: f64 = 1500.;
pub const EVERY: f64 = 0.5;
pub const MAX_FLOCKS: usize = 14;
pub const MAX_DUCKS: usize = 6;
pub const SCARE_PERSON: f64 = 55.;
pub const SCARE_CAR: f64 = 50.;
pub const SCARE_SHOT: f64 = 450.;
pub const FLY: (f64, f64) = (95., 140.);
pub const FLY_TIME: (f64, f64) = (2.5, 5.);
pub const CLIMB: f64 = 40.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pigeon,
    Duck,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Peck,
    Swim,
    Flee,
    Fly,
    Land,
}

/// Wo ein Schwarm sein will.
#[derive(Debug, Clone, PartialEq)]
pub struct Spot {
    pub key: String,
    pub kind: Kind,
    pub x: f64,
    pub y: f64,
    pub n: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Animal {
    pub kind: Kind,
    pub key: String,
    pub x: f64,
    pub y: f64,
    /// Flughöhe (px)
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub facing: f64,
    pub state: State,
    pub t: f64,
    pub hx: f64,
    pub hy: f64,
    pub seed: u32,
    pub flap: f64,
    pub hop: f64,
}

type Rings = Vec<Vec<(f64, f64)>>;

fn ha(n: &[f64]) -> f64 {
    h(11., n)
}

/// Wo gerade Tauben bzw. Enten sein wollen.
pub fn animal_spots(
    city: &mut City,
    cache: &mut LifeCache,
    cx: f64,
    cy: f64,
    radius: f64,
) -> Vec<Spot> {
    let mut out = Vec::new();
    let bx = Rect::around(cx, cy, radius);
    let pois: Vec<(f64, f64)> = city
        .pois
        .query(&bx)
        .into_iter()
        .map(|i| city.pois.get(i))
        .filter(|q| matches!(q.cat, "food" | "cafe" | "ubahn" | "sbahn"))
        .map(|q| (q.x, q.y))
        .collect();
    for (x, y) in pois {
        if ha(&[x, y]) > 0.3 {
            continue;
        }
        if let Some(f) = cache.front(city, x, y) {
            out.push(Spot {
                key: format!("t{},{}", x.round(), y.round()),
                kind: Kind::Pigeon,
                x: f.x + f.ux * 25.,
                y: f.y + f.uy * 25.,
                n: 4 + (ha(&[y, x]) * 6.).floor() as usize,
            });
        }
    }
    let polys: Vec<(PolyKind, Rings)> = city
        .polys
        .query(&bx)
        .into_iter()
        .map(|i| city.polys.get(i))
        .filter(|p| {
            matches!(
                p.kind,
                PolyKind::Water
                    | PolyKind::Area {
                        kind: AREA_PLAZA,
                        ..
                    }
            )
        })
        .map(|p| (p.kind, p.rings.clone()))
        .collect();
    for (kind, rings) in polys {
        let Some(b) = area_bbox(&rings) else { continue };
        if kind != PolyKind::Water {
            if b.w * b.h < 90_000. {
                continue;
            }
            if let Some((x, y)) = free_point(city, &b, &rings, 10) {
                out.push(Spot {
                    key: format!("p{},{}", b.x.round(), b.y.round()),
                    kind: Kind::Pigeon,
                    x,
                    y,
                    n: 5 + (ha(&[b.x, b.y]) * 8.).floor() as usize,
                });
            }
            continue;
        }
        if b.w * b.h < 250_000. {
            continue;
        }
        // ufernah: Punkte an den Ringecken, ein paar Meter ins Wasser versetzt
        let ring = &rings[0];
        let m = ring.len();
        let groups = (m / 40 + 1).min(4);
        for g in 0..groups {
            let i = (ha(&[b.x, b.y, g as f64]) * m as f64).floor() as usize % m.max(1);
            let (x0, y0) = ring[i];
            if (x0 - cx).abs() > radius || (y0 - cy).abs() > radius {
                continue;
            }
            'dist: for d in [60., 90., 130.] {
                for a in 0..8 {
                    let ang = a as f64 * 0.785;
                    let (x, y) = (x0 + ang.cos() * d, y0 + ang.sin() * d);
                    if city.surface_at(x, y, None) == Ground::Water {
                        out.push(Spot {
                            key: format!("d{},{}", x0.round(), y0.round()),
                            kind: Kind::Duck,
                            x,
                            y,
                            n: 2 + (ha(&[x0, y0]) * 4.).floor() as usize,
                        });
                        break 'dist;
                    }
                }
            }
        }
    }
    out
}

pub fn make_bird(rng: &mut Rng, s: &Spot, i: usize) -> Animal {
    let a = rng.float() * std::f64::consts::TAU;
    let r = if s.kind == Kind::Duck {
        10. + i as f64 * 9.
    } else {
        6. + rng.float() * 22.
    };
    let facing = rng.float() * (628. / 100.); // wie animals.js (nicht TAU)
    let t = rng.float() * 2.;
    let seed = (rng.float() * 1e6).floor() as u32;
    Animal {
        kind: s.kind,
        key: s.key.clone(),
        x: s.x + a.cos() * r,
        y: s.y + a.sin() * r,
        z: 0.,
        vx: 0.,
        vy: 0.,
        facing,
        state: if s.kind == Kind::Duck {
            State::Swim
        } else {
            State::Peck
        },
        t,
        hx: s.x,
        hy: s.y,
        seed,
        flap: 0.,
        hop: 0.,
    }
}

fn fly_off(rng: &mut Rng, a: &mut Animal, fx: f64, fy: f64) {
    if a.kind == Kind::Duck {
        let ang = (a.y - fy).atan2(a.x - fx) + (rng.float() - 0.5) * 0.8;
        a.state = State::Flee;
        a.t = 2. + rng.float() * 1.5;
        (a.vx, a.vy, a.facing) = (ang.cos() * 38., ang.sin() * 38., ang);
        return;
    }
    let ang = (a.y - fy).atan2(a.x - fx) + (rng.float() - 0.5) * 1.4;
    let v = FLY.0 + rng.float() * (FLY.1 - FLY.0);
    a.state = State::Fly;
    a.t = FLY_TIME.0 + rng.float() * (FLY_TIME.1 - FLY_TIME.0);
    (a.vx, a.vy, a.facing) = (ang.cos() * v, ang.sin() * v, ang);
}

/// Bedrohung: Ort und Radius (Spielfigur zu Fuß, schnelle Autos, Fliehende); Schüsse/Hupen getrennt.
pub type Threat = (f64, f64, f64);

pub fn update_animals(
    animals: &mut [Animal],
    city: &mut City,
    rng: &mut Rng,
    threats: &[Threat],
    shots: &[(f64, f64)],
    dt: f64,
) {
    for a in animals.iter_mut() {
        a.flap += dt;
        match a.state {
            State::Peck | State::Swim => {
                let scare = shots
                    .iter()
                    .find(|s| (s.0 - a.x).hypot(s.1 - a.y) < SCARE_SHOT)
                    .copied()
                    .or_else(|| {
                        threats
                            .iter()
                            .find(|t| (t.0 - a.x).hypot(t.1 - a.y) < t.2)
                            .map(|t| (t.0, t.1))
                    });
                if let Some((fx, fy)) = scare {
                    fly_off(rng, a, fx, fy);
                    continue;
                }
                let duck = a.kind == Kind::Duck;
                a.t -= dt;
                if a.t <= 0. {
                    // picken / paddeln: kleine Hüpfer bzw. Treiben um den Heimatpunkt
                    a.t = if duck {
                        1.5 + rng.float() * 3.
                    } else {
                        0.4 + rng.float() * 1.6
                    };
                    let back = (a.hx - a.x).hypot(a.hy - a.y) > if duck { 60. } else { 30. };
                    let ang = if back {
                        (a.hy - a.y).atan2(a.hx - a.x)
                    } else {
                        rng.float() * std::f64::consts::TAU
                    };
                    let v = if duck {
                        8. + rng.float() * 8.
                    } else {
                        14. + rng.float() * 16.
                    };
                    (a.vx, a.vy, a.facing) = (ang.cos() * v, ang.sin() * v, ang);
                    a.hop = if duck { 0. } else { 0.18 };
                }
                if !duck {
                    a.hop -= dt;
                    if a.hop <= 0. {
                        a.vx *= 0.8;
                        a.vy *= 0.8;
                    }
                }
                let (nx, ny) = (a.x + a.vx * dt, a.y + a.vy * dt);
                let ok = if duck {
                    city.surface_at(nx, ny, None) == Ground::Water
                } else {
                    city.in_building(nx, ny).is_none()
                };
                if ok {
                    (a.x, a.y) = (nx, ny);
                } else {
                    (a.vx, a.vy) = (-a.vx, -a.vy);
                }
            }
            State::Flee => {
                let (nx, ny) = (a.x + a.vx * dt, a.y + a.vy * dt);
                if city.surface_at(nx, ny, None) == Ground::Water {
                    (a.x, a.y) = (nx, ny);
                } else {
                    (a.vx, a.vy) = (-a.vx, -a.vy);
                }
                a.t -= dt;
                if a.t <= 0. {
                    a.state = State::Swim;
                    a.t = 1.;
                    (a.hx, a.hy) = (a.x, a.y);
                }
            }
            State::Fly => {
                a.x += a.vx * dt;
                a.y += a.vy * dt;
                a.z = (a.z + CLIMB * dt).min(160.);
                a.t -= dt;
                if a.t <= 0. {
                    a.state = State::Land;
                }
            }
            State::Land => {
                a.x += a.vx * 0.5 * dt;
                a.y += a.vy * 0.5 * dt;
                a.z = (a.z - 55. * dt).max(0.);
                if a.z == 0. {
                    if city.in_building(a.x, a.y).is_some()
                        || city.surface_at(a.x, a.y, None) == Ground::Water
                    {
                        a.state = State::Fly;
                        a.t = 1.;
                    } else {
                        a.state = State::Peck;
                        a.t = 1.;
                        (a.hx, a.hy) = (a.x, a.y);
                        (a.vx, a.vy) = (0., 0.);
                    }
                }
            }
        }
    }
}
