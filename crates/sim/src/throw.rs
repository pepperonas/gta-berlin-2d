//! Wurfwaffen: Handgranate und Molotow-Cocktail.
//!
//! Ein Wurf fliegt im Bogen (Höhe `z` in px über dem Boden) zum gezielten Punkt. Hauswände halten ihn auf (die
//! Granate prallt zurück, der Molotow zerschellt), niedrige Zäune und Geländer überfliegt er, Autos nur, solange er
//! hoch genug ist. Die Granate rollt nach der Landung aus und explodiert nach `FUSE_S` mit derselben Druckwelle wie
//! ein Fahrzeug ([`World::blast`]). Der Molotow zerschellt bei der ersten Berührung und hinterlässt eine brennende
//! Fläche ([`Flame`]), die Menschen und den Spieler verbrennt und Autos so lange beschädigt, bis sie zum Wrack werden
//! (und dann wie jedes Wrack ausbrennen und explodieren). Alles deterministisch, ohne Welt-Zufall.

use crate::combat::{Target, cast_ray, hurt_bike, hurt_ped, hurt_player};
use crate::events::Event;
use crate::pedestrians::PedState;
use crate::world::World;

/// Schwerkraft der Würfe (px/s²; spielerisch, nicht 98)
pub const GRAVITY: f64 = 320.;
/// Zündzeit der Handgranate ab dem Wurf
pub const FUSE_S: f64 = 2.4;
/// Stärke der Granaten-Druckwelle (Pkw-Explosion = 1)
pub const GRENADE_K: f64 = 0.85;
/// Über Autos fliegt ein Wurf erst ab dieser Höhe (px)
pub const OVER_CAR: f64 = 16.;
/// Feuer des Molotows: Radius (px), Brenndauer (s)
pub const FLAME_R: f64 = 34.;
pub const FLAME_S: f64 = 8.;
/// Brandschaden je Sekunde: Menschen, Spieler, Fahrzeuge
pub const FLAME_PED: f64 = 30.;
pub const FLAME_PLAYER: f64 = 16.;
pub const FLAME_CAR: f64 = 14.;
/// Schaden wird in Takten verteilt (sonst spritzt jedes Bild Blut)
const TICK: f64 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Grenade,
    Molotov,
}
impl Kind {
    pub fn of(weapon: &str) -> Option<Kind> {
        match weapon {
            "grenade" => Some(Kind::Grenade),
            "molotov" => Some(Kind::Molotov),
            _ => None,
        }
    }
}

/// Ein fliegender (oder rollender) Wurfkörper.
#[derive(Debug, Clone, PartialEq)]
pub struct Thrown {
    pub id: u32,
    pub kind: Kind,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub lvl: i8,
    /// Sekunden seit dem Wurf
    pub t: f64,
    /// Drehung (nur Darstellung)
    pub spin: f64,
    /// Sitz des Werfers (0 = Spieler 1, 1 = Spieler 2)
    pub by: u8,
}

/// Brennende Fläche eines Molotows.
#[derive(Debug, Clone, PartialEq)]
pub struct Flame {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub lvl: i8,
    /// Sekunden seit dem Zerschellen
    pub t: f64,
    pub by: u8,
    tick: f64,
}
impl Flame {
    /// Radius jetzt: breitet sich in 0,4 s aus, schrumpft in der letzten Sekunde
    pub fn r(&self) -> f64 {
        let grow = (self.t / 0.4).min(1.);
        let fade = ((FLAME_S - self.t) / 1.).clamp(0., 1.);
        FLAME_R * (0.45 + 0.55 * grow) * (0.3 + 0.7 * fade)
    }
}

/// Ausholen: so lange dauert ein ganzes Hin und Her der Wurfweite (s)
pub const CHARGE_PERIOD: f64 = 1.6;
/// kürzeste Wurfweite beim Ausholen (Anteil der größten)
pub const CHARGE_MIN: f64 = 0.25;

/// Wurfweite nach `t` Sekunden Ausholen: pendelt gleichmäßig zwischen `CHARGE_MIN · range` (beim Drücken) und
/// `range` (nach einer halben Periode) hin und her, solange die Taste gehalten wird.
pub fn charge_reach(range: f64, t: f64) -> f64 {
    let u = (t.max(0.) / (CHARGE_PERIOD / 2.)).rem_euclid(2.);
    let tri = if u <= 1. { u } else { 2. - u };
    range * (CHARGE_MIN + (1. - CHARGE_MIN) * tri)
}

/// Anfangsgeschwindigkeit für einen Wurf über die Strecke `d` (px): Flugzeit wächst mit der Weite.
pub fn launch(d: f64) -> (f64, f64) {
    let t = (0.35 + d / 700.).clamp(0.4, 1.1);
    (d / t, GRAVITY * t / 2.)
}

impl World {
    /// Wurfkörper in der Hand der Spielfigur (aktiver Sitz), abgeworfen in Richtung `ang` über die Strecke `d`.
    fn launch_state(&self, kind: Kind, ang: f64, d: f64) -> Thrown {
        let (vh, vz) = launch(d);
        let p = &self.player;
        Thrown {
            id: 0,
            kind,
            x: p.x + ang.cos() * 8.,
            y: p.y + ang.sin() * 8.,
            z: 12.,
            vx: ang.cos() * vh,
            vy: ang.sin() * vh,
            vz,
            lvl: p.level.lvl,
            t: 0.,
            spin: 0.,
            by: self.seat_index(),
        }
    }

    /// Wurf der Spielfigur (auf dem gerade aktiven Sitz) in Richtung `ang` über die Strecke `d`.
    pub fn throw(&mut self, kind: Kind, ang: f64, d: f64) {
        self.thrown_seq += 1;
        let g = Thrown {
            id: self.thrown_seq,
            ..self.launch_state(kind, ang, d)
        };
        let (x, y) = (g.x, g.y);
        self.thrown.push(g);
        self.events.push(Event::Throw {
            x,
            y,
            weapon: match kind {
                Kind::Grenade => "grenade",
                Kind::Molotov => "molotov",
            },
        });
    }

    /// Vorschau eines Wurfs (Bogen beim Ausholen): Punkte (x, y, Höhe) je Simulationsschritt, genau wie der echte
    /// Flug, bis zur ersten Berührung – Boden oder ein Hindernis, das ihn aufhält (dort endet der Bogen).
    pub fn throw_preview(&mut self, kind: Kind, ang: f64, d: f64) -> Vec<(f64, f64, f64)> {
        let dt = crate::world::DT;
        let mut g = self.launch_state(kind, ang, d);
        let mut out = vec![(g.x, g.y, g.z)];
        for _ in 0..240 {
            let sp = g.vx.hypot(g.vy);
            let a = g.vy.atan2(g.vx);
            let r = cast_ray(self, g.x, g.y, a, sp * dt + 3., g.lvl);
            let blocked = match r.hit {
                Some(Target::Wall) => true,
                Some(Target::Car(_)) => g.z < OVER_CAR,
                _ => false,
            };
            if blocked {
                out.push((r.x - a.cos() * 3., r.y - a.sin() * 3., g.z));
                break;
            }
            g.x += g.vx * dt;
            g.y += g.vy * dt;
            g.vz -= GRAVITY * dt;
            g.z += g.vz * dt;
            if g.z <= 0. {
                out.push((g.x, g.y, 0.));
                break;
            }
            out.push((g.x, g.y, g.z));
        }
        out
    }

    /// Ein Schritt aller Würfe und Feuer.
    pub(crate) fn update_thrown(&mut self, dt: f64) {
        if self.thrown.is_empty() && self.flames.is_empty() {
            return;
        }
        let mut boom = Vec::new();
        let mut shatter = Vec::new();
        for i in 0..self.thrown.len() {
            let mut g = self.thrown[i].clone();
            g.t += dt;
            let sp = g.vx.hypot(g.vy);
            g.spin += sp * dt * 0.08;
            // horizontale Bewegung mit Hindernissen
            if sp > 0.5 {
                let len = sp * dt;
                let ang = g.vy.atan2(g.vx);
                let r = cast_ray(self, g.x, g.y, ang, len + 3., g.lvl);
                let blocked = match r.hit {
                    Some(Target::Wall) => true,
                    Some(Target::Car(_)) => g.z < OVER_CAR,
                    _ => false,
                };
                if blocked {
                    match g.kind {
                        Kind::Molotov => {
                            g.x = r.x - ang.cos() * 3.;
                            g.y = r.y - ang.sin() * 3.;
                            shatter.push(i);
                            self.thrown[i] = g;
                            continue;
                        }
                        Kind::Grenade => {
                            self.events.push(Event::Bounce {
                                x: r.x,
                                y: r.y,
                                strength: (sp / 400.).min(1.),
                            });
                            g.vx = -g.vx * 0.35;
                            g.vy = -g.vy * 0.35;
                        }
                    }
                } else {
                    g.x += g.vx * dt;
                    g.y += g.vy * dt;
                }
            }
            // Höhe
            if g.z > 0. || g.vz > 0. {
                g.vz -= GRAVITY * dt;
                g.z += g.vz * dt;
                if g.z <= 0. {
                    g.z = 0.;
                    match g.kind {
                        Kind::Molotov => {
                            shatter.push(i);
                            self.thrown[i] = g;
                            continue;
                        }
                        Kind::Grenade => {
                            let hit = -g.vz;
                            if hit > 40. {
                                self.events.push(Event::Bounce {
                                    x: g.x,
                                    y: g.y,
                                    strength: (hit / 300.).min(1.),
                                });
                                g.vz = hit * 0.3;
                                g.vx *= 0.6;
                                g.vy *= 0.6;
                            } else {
                                g.vz = 0.;
                            }
                        }
                    }
                }
            } else {
                // am Boden: ausrollen
                let f = (-4. * dt).exp();
                g.vx *= f;
                g.vy *= f;
            }
            if g.kind == Kind::Grenade && g.t >= FUSE_S {
                boom.push(i);
            }
            self.thrown[i] = g;
        }
        // Molotow: Feuer am Boden
        for &i in &shatter {
            let g = self.thrown[i].clone();
            self.events.push(Event::Shatter {
                x: g.x,
                y: g.y,
                r: FLAME_R,
            });
            self.thrown_seq += 1;
            self.flames.push(Flame {
                id: self.thrown_seq,
                x: g.x,
                y: g.y,
                lvl: g.lvl,
                t: 0.,
                by: g.by,
                tick: 0.,
            });
        }
        // Granate: Explosion
        for &i in &boom {
            let g = self.thrown[i].clone();
            self.events.push(Event::Explosion {
                x: g.x,
                y: g.y,
                car: None,
                strength: GRENADE_K * 0.75,
            });
            self.blast(g.x, g.y, g.lvl, GRENADE_K, None, Some(g.by));
        }
        if !boom.is_empty() || !shatter.is_empty() {
            let mut k = 0;
            self.thrown.retain(|_| {
                let keep = !boom.contains(&k) && !shatter.contains(&k);
                k += 1;
                keep
            });
        }
        self.update_flames(dt);
    }

    fn update_flames(&mut self, dt: f64) {
        for f in &mut self.flames {
            f.t += dt;
            f.tick += dt;
        }
        for i in 0..self.flames.len() {
            if self.flames[i].tick < TICK {
                continue;
            }
            self.flames[i].tick -= TICK;
            let (x, y, lvl, r, by) = {
                let f = &self.flames[i];
                (f.x, f.y, f.lvl, f.r(), f.by)
            };
            for q in 0..self.peds.len() {
                let p = &self.peds[q];
                if p.state == PedState::Dead || p.level.lvl != lvl {
                    continue;
                }
                if (p.x - x).hypot(p.y - y) < r + 4. {
                    hurt_ped(self, q, FLAME_PED * TICK, (x, y), false, "molotov", true);
                }
            }
            for b in (0..self.bikes.len()).rev() {
                let k = &self.bikes[b];
                if k.state == crate::bikes::State::Ride
                    && k.level.lvl == lvl
                    && (k.x - x).hypot(k.y - y) < r
                {
                    hurt_bike(self, b, FLAME_PED * TICK, (x, y), false, "molotov", true);
                }
            }
            for c in &mut self.cars {
                if c.wrecked || c.lvl() != lvl || (c.x - x).hypot(c.y - y) >= r + c.hw * 0.6 {
                    continue;
                }
                c.health = (c.health - FLAME_CAR * TICK).max(0.);
                if c.health <= 0. {
                    c.wrecked = true;
                    self.events.push(Event::Wreck {
                        x: c.x,
                        y: c.y,
                        car: c.id,
                        player: true,
                    });
                }
            }
            // Spieler (auch der Werfer selbst; der Partner nicht – kein Eigenbeschuss)
            let me = self.seat_index();
            let burn = |w: &mut World| {
                let p = &w.player;
                if p.in_car.is_none()
                    && p.ride.is_none()
                    && p.level.lvl == lvl
                    && (p.x - x).hypot(p.y - y) < r + 3.
                {
                    hurt_player(w, FLAME_PLAYER * TICK, (x, y));
                }
            };
            if by == me {
                burn(self);
            } else {
                self.with_p2(burn);
            }
        }
        self.flames.retain(|f| f.t < FLAME_S);
    }
}
