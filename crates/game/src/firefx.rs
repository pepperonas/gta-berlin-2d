//! Brennende Wracks und Explosionen (Darstellung zu `sim/fire.rs`): Flammen und Rauch am brennenden Wagen, Feuerball,
//! Druckwellenring, Trümmer und Rauchsäule beim Knall, Brandfleck am Boden, Glimmen des ausgebrannten Wracks.
//! Dazu die Wurfwaffen (`sim/throw.rs`): fliegende Granaten und Molotows mit Schatten, Molotow-Feuer am Boden.
//! Streuung nur aus Hashes, nie aus dem Welt-Zufall.
use berlin_engine::{Body, LightSource};
use berlin_sim::events::Event;
use berlin_sim::math::hash01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Flamme bzw. Feuerball: leuchtet selbst (gelb → orange → rot)
    Fire,
    /// Rauch: dunkelgrau, quillt auf, vom Umgebungslicht abgedunkelt
    Smoke,
    /// Trümmer: dunkle Splitter, fliegen und bremsen ab
    Debris,
    /// Flammenzunge (Molotow, Docht): schmal, steigt auf, satt orange
    Lick,
}
#[derive(Debug, Clone, Copy)]
struct Puff {
    kind: Kind,
    at: [f32; 2],
    v: [f32; 2],
    size: f32,
    grow: f32,
    life: f32,
    max: f32,
    rot: f32,
}
#[derive(Debug, Clone, Copy)]
struct Blast {
    at: [f32; 2],
    age: f32,
    r: f32,
}
#[derive(Debug, Clone, Copy)]
struct Scorch {
    at: [f32; 2],
    r: [f32; 2],
    angle: f32,
    age: f32,
}
const PUFFS_MAX: usize = 700;
/// Druckwellenring und Lichtblitz (s)
const BLAST_S: f32 = 0.45;
/// so lange bleibt der Brandfleck (s), so lange glimmt ein ausgebranntes Wrack
const SCORCH_KEEP: f32 = 120.;
const SMOLDER_S: f64 = 30.;

#[derive(Debug, Default)]
pub struct FireFx {
    puffs: Vec<Puff>,
    blasts: Vec<Blast>,
    scorch: std::collections::VecDeque<Scorch>,
    /// brennende bzw. glimmende Wracks dieses Schritts (Lage, Flammen-Stärke) für Licht und Ausstoß
    burning: Vec<([f32; 2], f32)>,
    /// Ausstoß-Rest je Wrack (Bruchteile tragen über Schritte)
    emit: std::collections::HashMap<u32, f32>,
    seq: u32,
    /// Wurfkörper dieses Schritts (Lage, Höhe, Drehung, Granate?, Sekunden seit dem Wurf)
    thrown: Vec<([f32; 2], f32, f32, bool, f32)>,
    /// Molotow-Feuer dieses Schritts (Mitte, Radius, Alter) für den Glutteppich
    flames: Vec<([f32; 2], f32, f32)>,
}
/// Ausstoß-Schlüssel der Molotow-Feuer und brennenden Flaschen (getrennt von den Fahrzeugnummern)
const FLAME_KEY: u32 = 1 << 31;
const BOTTLE_KEY: u32 = 3 << 30;

impl FireFx {
    fn h(&mut self, k: f64) -> f32 {
        self.seq = self.seq.wrapping_add(1);
        hash01(self.seq as f64 * 12.9898 + k * 78.233) as f32
    }
    fn push(&mut self, p: Puff) {
        if self.puffs.len() < PUFFS_MAX {
            self.puffs.push(p);
        }
    }
    /// Ereignisse: Explosion (Feuerball, Ring, Trümmer, Rauch, Brandfleck), Entzünden (Stichflamme).
    pub fn ingest(&mut self, events: &[Event]) {
        for e in events {
            match *e {
                Event::Explosion { x, y, strength, .. } => {
                    let at = [x as f32, y as f32];
                    let k = 0.7 + 0.6 * strength as f32;
                    self.blasts.push(Blast {
                        at,
                        age: 0.,
                        r: 80. * k * 1.4,
                    });
                    // Kern: kurzer heller Blitz
                    let p = Puff {
                        kind: Kind::Fire,
                        at,
                        v: [0., 0.],
                        size: 46. * k,
                        grow: 60.,
                        life: 0.28,
                        max: 0.28,
                        rot: 0.,
                    };
                    self.push(p);
                    // weißer Blitz im Kern (kurz)
                    let p = Puff {
                        kind: Kind::Fire,
                        at,
                        v: [0., 0.],
                        size: 70. * k,
                        grow: 0.,
                        life: 0.1,
                        max: 0.1,
                        rot: 0.,
                    };
                    self.push(p);
                    // Feuerball: viele kleinere Flammen, schnell nach außen, dahinter langsamere große
                    for i in 0..40 {
                        let a = self.h(1.) * std::f32::consts::TAU + i as f32;
                        let fast = i % 3 != 0;
                        let sp = if fast {
                            (160. + self.h(2.) * 220.) * k
                        } else {
                            (50. + self.h(2.) * 80.) * k
                        };
                        let life = if fast { 0.35 } else { 0.6 } + self.h(3.) * 0.4;
                        let size = if fast {
                            (7. + self.h(4.) * 9.) * k
                        } else {
                            (16. + self.h(4.) * 14.) * k
                        };
                        let p = Puff {
                            kind: Kind::Fire,
                            at,
                            v: [a.cos() * sp, a.sin() * sp],
                            size,
                            grow: 26.,
                            life,
                            max: life,
                            rot: a,
                        };
                        self.push(p);
                    }
                    // Trümmer
                    for _ in 0..16 {
                        let a = self.h(5.) * std::f32::consts::TAU;
                        let sp = (180. + self.h(6.) * 260.) * k;
                        let life = 0.8 + self.h(7.) * 0.7;
                        let p = Puff {
                            kind: Kind::Debris,
                            at,
                            v: [a.cos() * sp, a.sin() * sp],
                            size: 1.6 + self.h(8.) * 2.4,
                            grow: 0.,
                            life,
                            max: life,
                            rot: a,
                        };
                        self.push(p);
                    }
                    // Rauchsäule
                    for _ in 0..12 {
                        let a = self.h(9.) * std::f32::consts::TAU;
                        let sp = 20. + self.h(10.) * 45.;
                        let life = 4.5 + self.h(11.) * 3.;
                        let p = Puff {
                            kind: Kind::Smoke,
                            at,
                            v: [a.cos() * sp, a.sin() * sp],
                            size: (22. + self.h(12.) * 14.) * k,
                            grow: 9.,
                            life,
                            max: life,
                            rot: 0.,
                        };
                        self.push(p);
                    }
                    let (rx, ry, ang) = (34. * k, 26. * k, self.h(13.) * std::f32::consts::TAU);
                    self.scorch.push_back(Scorch {
                        at,
                        r: [rx, ry],
                        angle: ang,
                        age: 0.,
                    });
                    while self.scorch.len() > 40 {
                        self.scorch.pop_front();
                    }
                }
                Event::Shatter { x, y, r } => {
                    // Glas und Benzin spritzen auf, Brandfleck bleibt
                    for _ in 0..10 {
                        let a = self.h(30.) * std::f32::consts::TAU;
                        let sp = 40. + self.h(31.) * 60.;
                        let p = Puff {
                            kind: Kind::Fire,
                            at: [x as f32, y as f32],
                            v: [a.cos() * sp, a.sin() * sp],
                            size: 4. + self.h(32.) * 5.,
                            grow: 14.,
                            life: 0.3,
                            max: 0.3,
                            rot: a,
                        };
                        self.push(p);
                    }
                    let a = self.h(33.) * std::f32::consts::PI;
                    self.scorch.push_back(Scorch {
                        at: [x as f32, y as f32],
                        r: [r as f32 * 0.95, r as f32 * 0.8],
                        angle: a,
                        age: 0.,
                    });
                }
                Event::CarFire { x, y, .. } => {
                    // Stichflamme beim Entzünden
                    for _ in 0..6 {
                        let a = self.h(14.) * std::f32::consts::TAU;
                        let sp = 30. + self.h(15.) * 40.;
                        let p = Puff {
                            kind: Kind::Fire,
                            at: [x as f32, y as f32],
                            v: [a.cos() * sp, a.sin() * sp],
                            size: 8. + self.h(16.) * 6.,
                            grow: 18.,
                            life: 0.35,
                            max: 0.35,
                            rot: a,
                        };
                        self.push(p);
                    }
                }
                _ => {}
            }
        }
    }
    /// Brennende Wracks: Flammen am Motorraum und Rauch; ausgebrannte glimmen noch `SMOLDER_S`.
    pub fn tick(&mut self, w: &berlin_sim::world::World, dt: f32) {
        self.burning.clear();
        let (foci, nf) = w.foci();
        let mut live = Vec::new();
        for c in &w.cars {
            let burning = c.burn.is_some() && !c.exploded;
            let smolder = c.exploded && c.wreck_t < SMOLDER_S;
            if !(burning || smolder) || berlin_sim::coop::min_dist(&foci[..nf], c.x, c.y) > 2600. {
                continue;
            }
            live.push(c.id);
            // Flammen sitzen vorn (Motorraum), beim Glimmen in der Mitte
            let (s, co) = c.angle.sin_cos();
            let off = if burning { c.hw * 0.45 } else { 0. };
            let at = [(c.x + co * off) as f32, (c.y + s * off) as f32];
            // Brennstärke wächst mit der Zeit bis zur Explosion
            let heat = if burning {
                let left = c.burn.unwrap_or(0.) as f32;
                (1.2 - left / 5.).clamp(0.35, 1.)
            } else {
                0.25 * (1. - (c.wreck_t / SMOLDER_S) as f32)
            };
            self.burning.push((at, heat));
            let rate = if burning { 26. * heat + 8. } else { 3. };
            let acc = self.emit.entry(c.id).or_insert(0.);
            *acc += rate * dt;
            let n = acc.floor() as usize;
            *acc -= n as f32;
            for _ in 0..n {
                let spread = c.hh as f32 * 0.8;
                let (jx, jy) = (self.h(20.) - 0.5, self.h(21.) - 0.5);
                let base = [at[0] + jx * spread, at[1] + jy * spread];
                let fire = burning && self.h(22.) < 0.72;
                if fire {
                    let life = 0.3 + self.h(23.) * 0.35;
                    let p = Puff {
                        kind: Kind::Fire,
                        at: base,
                        v: [jx * 20., -12. - self.h(24.) * 18.],
                        size: (5. + self.h(25.) * 6.) * (0.7 + 0.5 * heat),
                        grow: 10.,
                        life,
                        max: life,
                        rot: 0.,
                    };
                    self.push(p);
                } else {
                    let life = 2. + self.h(26.) * 2.;
                    let p = Puff {
                        kind: Kind::Smoke,
                        at: base,
                        v: [8. + jx * 12., -10. - self.h(27.) * 10.],
                        size: 7. + self.h(28.) * 6.,
                        grow: 7.,
                        life,
                        max: life,
                        rot: 0.,
                    };
                    self.push(p);
                }
            }
        }
        // Molotow-Feuer: Flammen über die ganze Fläche, etwas Rauch
        self.flames.clear();
        for f in &w.flames {
            if berlin_sim::coop::min_dist(&foci[..nf], f.x, f.y) > 2600. {
                continue;
            }
            let key = FLAME_KEY | f.id;
            live.push(key);
            let r = f.r() as f32;
            let heat = (r / berlin_sim::throw::FLAME_R as f32).clamp(0.2, 1.);
            let at = [f.x as f32, f.y as f32];
            self.burning.push((at, heat * 0.9));
            self.flames.push((at, r, f.t as f32));
            let acc = self.emit.entry(key).or_insert(0.);
            *acc += (30. + r * 2.2) * dt;
            let n = acc.floor() as usize;
            *acc -= n as f32;
            for _ in 0..n {
                let a = self.h(40.) * std::f32::consts::TAU;
                let d = self.h(41.).sqrt() * r;
                let base = [at[0] + a.cos() * d, at[1] + a.sin() * d];
                if self.h(42.) < 0.88 {
                    let life = 0.3 + self.h(43.) * 0.35;
                    let p = Puff {
                        kind: Kind::Lick,
                        at: base,
                        v: [(self.h(44.) - 0.5) * 8., -16. - self.h(45.) * 18.],
                        size: 2.6 + self.h(46.) * 3.4,
                        grow: 9.,
                        life,
                        max: life,
                        rot: 0.,
                    };
                    self.push(p);
                } else {
                    let life = 1.5 + self.h(47.) * 1.5;
                    let p = Puff {
                        kind: Kind::Smoke,
                        at: base,
                        v: [6., -9. - self.h(48.) * 8.],
                        size: 5. + self.h(49.) * 4.,
                        grow: 6.,
                        life,
                        max: life,
                        rot: 0.,
                    };
                    self.push(p);
                }
            }
        }
        // Wurfkörper; die Molotow-Flasche brennt am Docht
        self.thrown.clear();
        for g in &w.thrown {
            let grenade = g.kind == berlin_sim::throw::Kind::Grenade;
            let at = [g.x as f32, g.y as f32];
            self.thrown
                .push((at, g.z as f32, g.spin as f32, grenade, g.t as f32));
            if !grenade {
                let key = BOTTLE_KEY | g.id;
                live.push(key);
                let lift = Self::lift(g.z as f32);
                let acc = self.emit.entry(key).or_insert(0.);
                *acc += 40. * dt;
                let n = acc.floor() as usize;
                *acc -= n as f32;
                for _ in 0..n {
                    let p = Puff {
                        kind: Kind::Lick,
                        at: [at[0], at[1] - lift],
                        v: [(self.h(50.) - 0.5) * 10., -8.],
                        size: 2. + self.h(51.) * 1.5,
                        grow: 6.,
                        life: 0.18,
                        max: 0.18,
                        rot: 0.,
                    };
                    self.push(p);
                }
            }
        }
        self.emit.retain(|id, _| live.contains(id));
    }
    /// Bildversatz eines Wurfkörpers nach oben (Draufsicht mit leichter Schräge)
    fn lift(z: f32) -> f32 {
        z * 0.55
    }
    pub fn step(&mut self, dt: f32) {
        for p in &mut self.puffs {
            p.life -= dt;
            p.at[0] += p.v[0] * dt;
            p.at[1] += p.v[1] * dt;
            // Luftwiderstand: Feuerball und Trümmer bremsen schnell, Rauch treibt
            let drag = match p.kind {
                Kind::Fire => 4.,
                Kind::Lick => 1.5,
                Kind::Debris => 2.5,
                Kind::Smoke => 0.6,
            };
            let k = (1. - drag * dt).max(0.);
            p.v = [p.v[0] * k, p.v[1] * k];
            p.size += p.grow * dt;
            p.rot += dt * 6.;
        }
        self.puffs.retain(|p| p.life > 0.);
        for b in &mut self.blasts {
            b.age += dt;
        }
        self.blasts.retain(|b| b.age < BLAST_S);
        for s in &mut self.scorch {
            s.age += dt;
        }
        while self.scorch.front().is_some_and(|s| s.age > SCORCH_KEEP) {
            self.scorch.pop_front();
        }
    }
    /// Durchscheinend (Effekt-Durchgang): Rauch vom Umgebungslicht abgedunkelt (`lit`), Feuer leuchtet selbst.
    pub fn effects(&self, out: &mut Vec<Body>, lit: impl Fn([f32; 3]) -> [f32; 3]) {
        for p in &self.puffs {
            let age = 1. - p.life / p.max;
            match p.kind {
                Kind::Smoke => {
                    let a = (std::f32::consts::PI * age).sin() * 0.5;
                    let c = lit([0.16, 0.15, 0.15]);
                    out.push(Body {
                        center: p.at,
                        half: [p.size, p.size],
                        angle: 0.,
                        shape: 3.,
                        depth: 0.585,
                        color: [c[0], c[1], c[2], a],
                    });
                }
                Kind::Fire => {
                    // heiß (gelbweiß) → orange → dunkelrot, verlischt
                    // heiß gelbweiß, dann orange; verblasst am Ende rasch (kein stehender roter Fleck)
                    let c = if age < 0.35 {
                        [1., 0.95, 0.72]
                    } else {
                        let t = ((age - 0.35) / 0.65).clamp(0., 1.);
                        [1., 0.72 - 0.3 * t, 0.28 - 0.18 * t]
                    };
                    let a = (1. - age).powf(1.6) * 0.92;
                    out.push(Body {
                        center: p.at,
                        half: [p.size, p.size],
                        angle: 0.,
                        shape: 3.,
                        depth: 0.588,
                        color: [c[0], c[1], c[2], a],
                    });
                }
                Kind::Lick => {
                    // gelb am Fuß, dann sattes Orange, zuletzt Rot; schmal und hochgezogen
                    let c = if age < 0.25 {
                        [1., 0.84, 0.4]
                    } else {
                        let t = ((age - 0.25) / 0.75).clamp(0., 1.);
                        [1., 0.6 - 0.32 * t, 0.16 - 0.1 * t]
                    };
                    let a = (1. - age).powf(1.3) * 0.85;
                    out.push(Body {
                        center: p.at,
                        half: [p.size * 0.65, p.size * 1.35],
                        angle: 0.,
                        shape: 3.,
                        depth: 0.5885,
                        color: [c[0], c[1], c[2], a],
                    });
                }
                Kind::Debris => {
                    let c = lit([0.12, 0.11, 0.1]);
                    out.push(Body {
                        center: p.at,
                        half: [p.size * 1.6, p.size],
                        angle: p.rot,
                        shape: 4.,
                        depth: 0.587,
                        color: [c[0], c[1], c[2], (p.life / p.max).min(1.) * 0.95],
                    });
                }
            }
        }
        // Molotow: glühender Teppich unter den Flammen, flackert
        for (i, &(at, r, t)) in self.flames.iter().enumerate() {
            let fl = 0.85 + 0.15 * (t * 17. + i as f32 * 2.1).sin() * (t * 5.3).cos();
            let a = (t / 0.25).min(1.) * 0.42 * fl;
            out.push(Body {
                center: at,
                half: [r * 1.05, r * 0.9],
                angle: 0.,
                shape: 3.,
                depth: 0.5895,
                color: [1., 0.55, 0.16, a],
            });
            out.push(Body {
                center: at,
                half: [r * 0.6, r * 0.5],
                angle: 0.,
                shape: 3.,
                depth: 0.5893,
                color: [1., 0.86, 0.45, a * 0.8],
            });
        }
        // Druckwelle: heller Ring, der auseinanderläuft
        for b in &self.blasts {
            let k = b.age / BLAST_S;
            let r = 8. + b.r * k.sqrt();
            out.push(Body {
                center: b.at,
                half: [r, r],
                angle: 0.,
                shape: 2.,
                depth: 0.586,
                color: [1., 0.92, 0.75, 0.55 * (1. - k)],
            });
        }
    }
    /// Am Boden: Brandflecken (unter allem Bewegten); Wurfkörper mit Schatten.
    pub fn bodies(&self, out: &mut Vec<Body>) {
        for &(at, z, spin, grenade, t) in &self.thrown {
            // Schatten am Boden, kleiner und blasser je höher
            let k = 1. / (1. + z / 40.);
            out.push(Body {
                center: at,
                half: [3.4 * k, 2.6 * k],
                angle: 0.,
                shape: 1.,
                depth: 0.831,
                color: [0., 0., 0., 0.35 * k],
            });
            let big = 1. + z / 90.;
            let c = [at[0], at[1] - Self::lift(z)];
            let depth = if z > berlin_sim::throw::OVER_CAR as f32 {
                0.6
            } else {
                0.75
            };
            if grenade {
                out.push(Body {
                    center: c,
                    half: [2.6 * big, 2.1 * big],
                    angle: spin,
                    shape: 1.,
                    depth,
                    color: [0.24, 0.27, 0.16, 1.],
                });
                // Zünder blinkt in der letzten Sekunde immer schneller
                let left = berlin_sim::throw::FUSE_S as f32 - t;
                let on = left < 1. && (t * (6. + 14. * (1. - left))).fract() < 0.5;
                out.push(Body {
                    center: [c[0] + spin.cos() * 2.2 * big, c[1] + spin.sin() * 2.2 * big],
                    half: [0.9 * big, 0.9 * big],
                    angle: 0.,
                    shape: 1.,
                    depth: depth - 0.0002,
                    color: if on {
                        [1., 0.25, 0.15, 1.]
                    } else {
                        [0.62, 0.6, 0.55, 1.]
                    },
                });
            } else {
                out.push(Body {
                    center: c,
                    half: [4.2 * big, 1.6 * big],
                    angle: spin,
                    shape: 0.,
                    depth,
                    color: [0.2, 0.36, 0.2, 1.],
                });
                out.push(Body {
                    center: [c[0] + spin.cos() * 4.2 * big, c[1] + spin.sin() * 4.2 * big],
                    half: [1.4 * big, 0.9 * big],
                    angle: spin,
                    shape: 0.,
                    depth: depth - 0.0002,
                    color: [0.85, 0.82, 0.72, 1.],
                });
            }
        }
        for s in &self.scorch {
            let fade = 1. - (s.age / SCORCH_KEEP).powi(3);
            out.push(Body {
                center: s.at,
                half: s.r,
                angle: s.angle,
                shape: 1.,
                depth: 0.832,
                color: [0.05, 0.045, 0.04, 0.7 * fade],
            });
        }
    }
    /// Licht: Feuer flackert orange, der Knall blitzt weit (nachts sichtbar).
    pub fn lights(&self, out: &mut Vec<LightSource>, dark: f32, time: f32) {
        if dark <= 0. {
            return;
        }
        for (i, (at, heat)) in self.burning.iter().enumerate() {
            let flick = 0.8 + 0.2 * (time * 23. + i as f32 * 1.7).sin() * (time * 7.3).cos();
            out.push(LightSource {
                center: *at,
                radius: 70. + 90. * heat,
                angle: 0.,
                color: [1., 0.55, 0.2],
                intensity: (0.5 + 0.5 * heat) * flick * dark,
                cone: 0.,
                pad: 0.,
            });
        }
        for b in &self.blasts {
            let k = 1. - b.age / BLAST_S;
            out.push(LightSource {
                center: b.at,
                radius: 180. + b.r * 2.,
                angle: 0.,
                color: [1., 0.8, 0.5],
                intensity: k * dark,
                cone: 0.,
                pad: 0.,
            });
        }
    }
    #[cfg(test)]
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.puffs.len(), self.blasts.len(), self.scorch.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explosion_makes_fireball_ring_debris_smoke_and_a_scorch_mark() {
        let mut fx = FireFx::default();
        fx.ingest(&[Event::Explosion {
            x: 100.,
            y: 50.,
            car: Some(1),
            strength: 0.6,
        }]);
        let (p, b, s) = fx.counts();
        assert!(p > 40 && b == 1 && s == 1);
        let mut out = Vec::new();
        fx.effects(&mut out, |c| c);
        assert!(out.iter().any(|b| b.shape == 2.), "Druckwellenring");
        // nach einer halben Sekunde: Ring weg, Rauch noch da; nach 10 s nur noch der Brandfleck
        for _ in 0..30 {
            fx.step(1. / 60.);
        }
        assert_eq!(fx.counts().1, 0);
        assert!(fx.counts().0 > 5);
        for _ in 0..600 {
            fx.step(1. / 60.);
        }
        assert_eq!(fx.counts(), (0, 0, 1));
        let mut ground = Vec::new();
        fx.bodies(&mut ground);
        assert_eq!(ground.len(), 1);
    }

    #[test]
    fn molotov_shatter_sprays_fire_and_leaves_a_scorch_mark() {
        let mut fx = FireFx::default();
        fx.ingest(&[Event::Shatter {
            x: 10.,
            y: 20.,
            r: 34.,
        }]);
        assert!(fx.puffs.iter().filter(|p| p.kind == Kind::Fire).count() >= 8);
        assert_eq!(fx.scorch.len(), 1);
        let mut ground = Vec::new();
        fx.bodies(&mut ground);
        assert_eq!(ground.len(), 1, "nur der Brandfleck, kein Wurfkörper");
    }

    #[test]
    fn same_events_same_picture() {
        let ev = [Event::Explosion {
            x: 0.,
            y: 0.,
            car: Some(3),
            strength: 1.,
        }];
        let run = || {
            let mut fx = FireFx::default();
            fx.ingest(&ev);
            fx.step(0.1);
            let mut out = Vec::new();
            fx.effects(&mut out, |c| c);
            out.iter().map(|b| b.center[0] + b.center[1]).sum::<f32>()
        };
        assert_eq!(run(), run(), "keine Zufallsquelle außer Hashes");
    }
}
