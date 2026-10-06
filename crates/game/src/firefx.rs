//! Feuer, Explosionen und Rauch (Darstellung zu `sim/fire.rs` und `sim/throw.rs`) aus gerenderten Flipbooks.
//!
//! Die Bilder stammen aus den CC0-Flipbooks von Unity Labs Paris (in Houdini simuliert, `engine/vfx.rs`,
//! `tools/gfx/build_vfx.py`): Explosionen spielen eine Folge vom Feuerball bis zur abziehenden Rauchwolke ab,
//! brennende Wracks und Molotow-Feuer zeigen mehrere Flammenzungen mit versetzter Phase über einem additiven
//! Glutkern, Rauch quillt als Schwaden-Flipbook auf. Prozedural bleiben nur Funken, Trümmer, Lichtblitz,
//! Brandfleck und die Wurfkörper. Streuung nur aus Hashes, nie aus dem Welt-Zufall.
use berlin_engine::vfx;
use berlin_engine::{Body, LightSource};
use berlin_sim::events::Event;
use berlin_sim::math::hash01;
use std::f32::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Funke (Glas, Benzin, Stichflamme): kleiner heller Fleck
    Fire,
    /// Rauchschwade (Flipbook): quillt auf, treibt, vom Umgebungslicht abgedunkelt
    Smoke,
    /// Trümmer: dunkle Splitter, fliegen und bremsen ab
    Debris,
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
    /// Drehung; beim Rauch zugleich Phase im Flipbook
    rot: f32,
    /// Grauwert des Rauchs (dunkler über brennendem Öl)
    shade: f32,
}
/// Einmal abgespieltes Flipbook (Explosion, Stichflamme).
#[derive(Debug, Clone, Copy)]
struct Flip {
    seq: u32,
    at: [f32; 2],
    half: f32,
    /// negativ = wartet noch (Nachdetonation)
    age: f32,
    dur: f32,
    rot: f32,
    /// steigt auf (px/s nach Norden auf dem Bildschirm)
    rise: f32,
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
/// Brennendes oder glimmendes Wrack dieses Schritts.
#[derive(Debug, Clone, Copy)]
struct Burn {
    id: u32,
    at: [f32; 2],
    angle: f32,
    hw: f32,
    hh: f32,
    /// 0…1: wächst bis zur Explosion; beim Glimmen klein
    heat: f32,
    smolder: bool,
}
/// Molotow-Feuer dieses Schritts.
#[derive(Debug, Clone, Copy)]
struct Patch {
    id: u32,
    at: [f32; 2],
    r: f32,
    t: f32,
}
/// Wurfkörper dieses Schritts.
#[derive(Debug, Clone, Copy)]
struct Toss {
    at: [f32; 2],
    z: f32,
    spin: f32,
    grenade: bool,
    t: f32,
}
const PUFFS_MAX: usize = 600;
const FLIPS_MAX: usize = 48;
/// Lichtblitz der Explosion (s)
const BLAST_S: f32 = 0.45;
/// so lange bleibt der Brandfleck (s), so lange glimmt ein ausgebranntes Wrack
const SCORCH_KEEP: f32 = 120.;
const SMOLDER_S: f64 = 30.;
/// Bilder je Sekunde der Schleifen
const FLAME_FPS: f32 = 24.;
const FIREBALL_FPS: f32 = 20.;
const SMOKE_FPS: f32 = 14.;
/// Explosion: Dauer der ganzen Folge (s), Halbmaß des Bildes je Stärke (px; der Feuerball füllt etwa 60 %)
const EXPLOSION_S: f32 = 2.6;
const EXPLOSION_HALF: f32 = 118.;
/// Ausstoß-Schlüssel der Molotow-Feuer (getrennt von den Fahrzeugnummern)
const FLAME_KEY: u32 = 1 << 31;

/// Nummern der Folgen im Atlas (einmal nachgeschlagen).
#[derive(Debug, Clone, Copy)]
struct Seqs {
    explosion: [u32; 2],
    blast: u32,
    fireball: u32,
    flame: u32,
    flame_small: u32,
    smoke: u32,
}
fn seqs() -> Seqs {
    let s = |n: &str| vfx::seq(n).unwrap_or_else(|| panic!("VFX-Folge {n} fehlt"));
    Seqs {
        explosion: [s("explosion_a"), s("explosion_b")],
        blast: s("blast"),
        fireball: s("fireball"),
        flame: s("flame"),
        flame_small: s("flame_small"),
        smoke: s("smoke"),
    }
}
fn hk(a: u32, b: f32) -> f32 {
    hash01(a as f64 * 0.618 + b as f64 * 7.13) as f32
}

#[derive(Debug)]
pub struct FireFx {
    s: Seqs,
    puffs: Vec<Puff>,
    flips: Vec<Flip>,
    blasts: Vec<Blast>,
    scorch: std::collections::VecDeque<Scorch>,
    burning: Vec<Burn>,
    patches: Vec<Patch>,
    thrown: Vec<Toss>,
    /// Ausstoß-Rest je Brandherd (Bruchteile tragen über Schritte)
    emit: std::collections::HashMap<u32, f32>,
    seq: u32,
    /// Darstellungszeit (Schleifen-Phase)
    time: f32,
}
impl Default for FireFx {
    fn default() -> Self {
        Self {
            s: seqs(),
            puffs: Vec::new(),
            flips: Vec::new(),
            blasts: Vec::new(),
            scorch: Default::default(),
            burning: Vec::new(),
            patches: Vec::new(),
            thrown: Vec::new(),
            emit: Default::default(),
            seq: 0,
            time: 0.,
        }
    }
}

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
    fn flip(&mut self, f: Flip) {
        if self.flips.len() < FLIPS_MAX {
            self.flips.push(f);
        }
    }
    fn sparks(&mut self, at: [f32; 2], n: usize, speed: f32, size: f32) {
        for _ in 0..n {
            let a = self.h(30.) * TAU;
            let sp = speed * (0.5 + self.h(31.));
            let life = 0.25 + self.h(32.) * 0.2;
            let p = Puff {
                kind: Kind::Fire,
                at,
                v: [a.cos() * sp, a.sin() * sp],
                size: size * (0.6 + 0.6 * self.h(33.)),
                grow: 4.,
                life,
                max: life,
                rot: a,
                shade: 1.,
            };
            self.push(p);
        }
    }
    fn smoke(&mut self, at: [f32; 2], v: [f32; 2], size: f32, life: f32, shade: f32) {
        let p = Puff {
            kind: Kind::Smoke,
            at,
            v,
            size,
            grow: size * 0.35,
            life,
            max: life,
            rot: self.h(60.) * TAU,
            shade,
        };
        self.push(p);
    }
    fn scorch_mark(&mut self, at: [f32; 2], r: [f32; 2]) {
        let angle = self.h(13.) * TAU;
        self.scorch.push_back(Scorch {
            at,
            r,
            angle,
            age: 0.,
        });
        while self.scorch.len() > 40 {
            self.scorch.pop_front();
        }
    }
    /// Ereignisse: Explosion (Flipbook, Nachdetonationen, Lichtblitz, Trümmer, Rauch, Brandfleck), Molotow zerschellt,
    /// Wrack fängt Feuer (Stichflamme).
    pub fn ingest(&mut self, events: &[Event]) {
        for e in events {
            match *e {
                Event::Explosion {
                    x,
                    y,
                    strength,
                    car,
                } => {
                    let at = [x as f32, y as f32];
                    let k = 0.7 + 0.6 * strength as f32;
                    let rot = self.h(14.) * TAU;
                    if let Some(id) = car {
                        // Fahrzeug: große Folge (zwei Varianten nach Nummer), dazu zwei kleinere Nachdetonationen
                        let seq = self.s.explosion[(id % 2) as usize];
                        self.flip(Flip {
                            seq,
                            at,
                            half: EXPLOSION_HALF * k,
                            age: 0.,
                            dur: EXPLOSION_S,
                            rot,
                            rise: 9.,
                        });
                        for (i, d) in [(0usize, 0.12f32), (1, 0.27)] {
                            let a = self.h(15. + i as f64) * TAU;
                            let off = 26. * k;
                            let blast = self.s.blast;
                            self.flip(Flip {
                                seq: blast,
                                at: [at[0] + a.cos() * off, at[1] + a.sin() * off],
                                half: 52. * k,
                                age: -d,
                                dur: 1.5,
                                rot: a,
                                rise: 6.,
                            });
                        }
                    } else {
                        // Handgranate: kompakter Blitz, dann Rauch
                        let blast = self.s.blast;
                        self.flip(Flip {
                            seq: blast,
                            at,
                            half: 92. * k,
                            age: 0.,
                            dur: 1.9,
                            rot,
                            rise: 5.,
                        });
                    }
                    self.blasts.push(Blast {
                        at,
                        age: 0.,
                        r: 80. * k * 1.4,
                    });
                    self.sparks(at, 18, 260. * k, 2.2);
                    // Trümmer
                    for _ in 0..16 {
                        let a = self.h(5.) * TAU;
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
                            shade: 1.,
                        };
                        self.push(p);
                    }
                    // Rauch, der nach der Folge weiter abzieht
                    for i in 0..6 {
                        let a = self.h(9.) * TAU;
                        let sp = 10. + self.h(10.) * 18.;
                        let life = 5. + self.h(11.) * 3. + i as f32 * 0.3;
                        let size = (34. + self.h(12.) * 18.) * k;
                        self.smoke(at, [a.cos() * sp + 5., a.sin() * sp - 9.], size, life, 0.42);
                    }
                    self.scorch_mark(at, [34. * k, 26. * k]);
                }
                Event::Shatter { x, y, r } => {
                    // Glas und Benzin spritzen auf, Brandfleck bleibt; die Flammen wachsen aus dem Feuer selbst
                    let at = [x as f32, y as f32];
                    self.sparks(at, 12, 80., 2.6);
                    self.scorch_mark(at, [r as f32 * 0.95, r as f32 * 0.8]);
                }
                Event::CarFire { x, y, .. } => {
                    // Stichflamme beim Entzünden
                    let at = [x as f32, y as f32];
                    let rot = self.h(16.) * TAU;
                    let blast = self.s.blast;
                    self.flip(Flip {
                        seq: blast,
                        at,
                        half: 30.,
                        age: 0.,
                        dur: 0.9,
                        rot,
                        rise: 8.,
                    });
                    self.sparks(at, 6, 60., 2.);
                }
                _ => {}
            }
        }
    }
    /// Brandherde der Welt (jeden Schritt): brennende und glimmende Wracks, Molotow-Feuer, Wurfkörper. Rauch wird
    /// hier ausgestoßen, Flammen zeichnet `effects` direkt aus diesem Stand.
    pub fn tick(&mut self, w: &berlin_sim::world::World, dt: f32) {
        self.burning.clear();
        self.patches.clear();
        self.thrown.clear();
        let (foci, nf) = w.foci();
        let near = |x: f64, y: f64| berlin_sim::coop::min_dist(&foci[..nf], x, y) < 2600.;
        let mut live = Vec::new();
        for c in &w.cars {
            let burning = c.burn.is_some() && !c.exploded;
            let smolder = c.exploded && c.wreck_t < SMOLDER_S;
            if !(burning || smolder) || !near(c.x, c.y) {
                continue;
            }
            live.push(c.id);
            // Brennstärke wächst mit der Zeit bis zur Explosion
            let heat = if burning {
                let left = c.burn.unwrap_or(0.) as f32;
                (1.2 - left / 5.).clamp(0.35, 1.)
            } else {
                0.3 * (1. - (c.wreck_t / SMOLDER_S) as f32)
            };
            let b = Burn {
                id: c.id,
                at: [c.x as f32, c.y as f32],
                angle: c.angle as f32,
                hw: c.hw as f32,
                hh: c.hh as f32,
                heat,
                smolder: !burning,
            };
            self.burning.push(b);
            // Rauchfahne: dichter, schwarzer Ölqualm über dem Brand, heller beim Glimmen
            let rate = if burning { 4. + 6. * heat } else { 1.2 };
            let acc = self.emit.entry(c.id).or_insert(0.);
            *acc += rate * dt;
            let n = acc.floor() as usize;
            *acc -= n as f32;
            for _ in 0..n {
                let (jx, jy) = (self.h(20.) - 0.5, self.h(21.) - 0.5);
                let spread = b.hh * 0.8;
                let at = [b.at[0] + jx * spread, b.at[1] + jy * spread];
                let life = 3. + self.h(26.) * 2.5;
                let size = if burning { 12. + 10. * heat } else { 9. } * (0.8 + 0.4 * self.h(28.));
                let shade = if burning { 0.3 } else { 0.5 };
                let up = -12. - self.h(27.) * 12.;
                self.smoke(at, [7. + jx * 10., up], size, life, shade);
            }
        }
        for f in &w.flames {
            if !near(f.x, f.y) {
                continue;
            }
            let key = FLAME_KEY | f.id;
            live.push(key);
            let p = Patch {
                id: f.id,
                at: [f.x as f32, f.y as f32],
                r: f.r() as f32,
                t: f.t as f32,
            };
            self.patches.push(p);
            let acc = self.emit.entry(key).or_insert(0.);
            *acc += (1.5 + p.r * 0.06) * dt;
            let n = acc.floor() as usize;
            *acc -= n as f32;
            for _ in 0..n {
                let a = self.h(40.) * TAU;
                let d = self.h(41.).sqrt() * p.r;
                let at = [p.at[0] + a.cos() * d, p.at[1] + a.sin() * d];
                let life = 2.5 + self.h(42.) * 2.;
                let size = 10. + self.h(43.) * 8.;
                let up = -11. - self.h(44.) * 8.;
                self.smoke(at, [6., up], size, life, 0.38);
            }
        }
        for g in &w.thrown {
            self.thrown.push(Toss {
                at: [g.x as f32, g.y as f32],
                z: g.z as f32,
                spin: g.spin as f32,
                grenade: g.kind == berlin_sim::throw::Kind::Grenade,
                t: g.t as f32,
            });
        }
        self.emit.retain(|id, _| live.contains(id));
    }
    pub fn step(&mut self, dt: f32) {
        self.time += dt;
        for p in &mut self.puffs {
            p.life -= dt;
            p.at[0] += p.v[0] * dt;
            p.at[1] += p.v[1] * dt;
            // Luftwiderstand: Funken und Trümmer bremsen schnell, Rauch treibt
            let drag = match p.kind {
                Kind::Fire => 4.,
                Kind::Debris => 2.5,
                Kind::Smoke => 0.5,
            };
            let k = (1. - drag * dt).max(0.);
            p.v = [p.v[0] * k, p.v[1] * k];
            p.size += p.grow * dt;
            if p.kind != Kind::Smoke {
                p.rot += dt * 6.;
            }
        }
        self.puffs.retain(|p| p.life > 0.);
        for f in &mut self.flips {
            f.age += dt;
            if f.age > 0. {
                f.at[1] -= f.rise * dt;
            }
        }
        self.flips.retain(|f| f.age < f.dur);
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

    /// Eine Flammenzunge (Flipbook, steht mit dem Fuß auf `base` und wächst auf dem Bildschirm nach oben).
    fn flame(&self, out: &mut Vec<Body>, base: [f32; 2], w: f32, small: bool, phase: f32, a: f32) {
        let seq = if small {
            self.s.flame_small
        } else {
            self.s.flame
        };
        let fps = FLAME_FPS * (0.85 + 0.3 * (phase * 7.3).fract());
        // der Flammenfuß liegt im Bild bei etwa 80 % der Höhe
        out.push(Body {
            center: [base[0], base[1] - w * 1.55],
            half: [w, w * 2.],
            angle: 0.,
            shape: vfx::shape(seq, phase * 64. + self.time * fps),
            depth: 0.587,
            color: [1., 1., 1., a],
        });
    }
    /// Additiver Glutkern (Schleife).
    fn glow(&self, out: &mut Vec<Body>, at: [f32; 2], half: f32, tint: [f32; 3], phase: f32) {
        out.push(Body {
            center: at,
            half: [half, half],
            angle: phase * TAU,
            shape: vfx::shape(self.s.fireball, phase * 64. + self.time * FIREBALL_FPS),
            depth: 0.5885,
            color: [tint[0], tint[1], tint[2], 1.],
        });
    }

    /// Durchscheinend (Effekt-Durchgang, vormultipliziert): Rauch hinten, darüber Glut, Flammen, Explosionen,
    /// Funken und Trümmer. `lit` dunkelt nicht leuchtende Teile mit dem Umgebungslicht ab.
    pub fn effects(&self, out: &mut Vec<Body>, lit: impl Fn([f32; 3]) -> [f32; 3]) {
        // Rauch
        for p in self.puffs.iter().filter(|p| p.kind == Kind::Smoke) {
            let age = 1. - p.life / p.max;
            let a = (PI * age.powf(0.6)).sin() * 0.8;
            let c = lit([p.shade; 3]);
            out.push(Body {
                center: p.at,
                half: [p.size * 1.35, p.size * 1.35],
                angle: p.rot,
                shape: vfx::shape(self.s.smoke, p.rot * 10.18 + (p.max - p.life) * SMOKE_FPS),
                depth: 0.589,
                color: [c[0], c[1], c[2], a],
            });
        }
        // Brennende und glimmende Wracks
        for b in &self.burning {
            let (s, co) = b.angle.sin_cos();
            let along =
                |d: f32, side: f32| [b.at[0] + co * d - s * side, b.at[1] + s * d + co * side];
            let ph = hk(b.id, 1.);
            if b.smolder {
                self.glow(
                    out,
                    b.at,
                    b.hh * 1.1,
                    [0.5 * b.heat, 0.2 * b.heat, 0.06 * b.heat],
                    ph,
                );
                for i in 0..2 {
                    let hi = hk(b.id, 10. + i as f32);
                    let p = along((hi - 0.5) * b.hw, (hk(b.id, 20. + i as f32) - 0.5) * b.hh);
                    self.flame(out, p, 4. + 3. * b.heat, true, hi, (b.heat * 3.).min(0.9));
                }
                continue;
            }
            // Glutkern am Motorraum, der mit der Hitze über das ganze Auto wandert
            let front = b.hw * 0.45 * (1. - b.heat * 0.6);
            self.glow(
                out,
                along(front, 0.),
                b.hh * (1.2 + 1.2 * b.heat),
                [0.12, 0.05, 0.015],
                ph,
            );
            let n = 2 + (b.heat * 4.).round() as usize;
            for i in 0..n {
                let hi = hk(b.id, 30. + i as f32);
                // vorn zuerst, mit der Hitze bis ans Heck
                let u = i as f32 / n as f32;
                let d = b.hw * (0.55 - u * (0.4 + 0.9 * b.heat));
                let side = (hk(b.id, 40. + i as f32) - 0.5) * b.hh * 1.1;
                let w = b.hh * (0.5 + 0.5 * b.heat) * (0.75 + 0.5 * hi);
                self.flame(out, along(d, side), w, i % 3 == 2, hi, 0.95);
            }
        }
        // Molotow-Feuer: Glutteppich und Flammenzungen über die ganze Fläche
        for p in &self.patches {
            let rise = (p.t / 0.25).min(1.);
            let fl = 0.85 + 0.15 * (p.t * 17. + p.id as f32 * 2.1).sin();
            out.push(Body {
                center: p.at,
                half: [p.r * 1.05, p.r * 0.9],
                angle: 0.,
                shape: 3.,
                depth: 0.5895,
                color: [1., 0.45, 0.12, 0.18 * rise * fl],
            });
            for i in 0..14 {
                let hi = hk(p.id, 50. + i as f32);
                let a = hk(p.id, 60. + i as f32) * TAU;
                let d = hk(p.id, 70. + i as f32).sqrt() * p.r * 0.85;
                let base = [p.at[0] + a.cos() * d, p.at[1] + a.sin() * d];
                // innen hoch, zum Rand niedriger
                let w = p.r * (0.16 + 0.12 * hi) * (1.15 - d / p.r * 0.6);
                self.flame(out, base, w, i % 3 == 1, hi, 0.95 * rise);
            }
        }
        // Brennende Molotow-Flasche im Flug: kleine Flamme am Docht
        for t in self.thrown.iter().filter(|t| !t.grenade) {
            let lift = lift(t.z);
            let base = [
                t.at[0] + t.spin.cos() * 4.,
                t.at[1] - lift + t.spin.sin() * 4.,
            ];
            self.flame(out, base, 3.2, true, (t.spin * 0.1).fract(), 0.95);
        }
        // Explosionen
        for f in self.flips.iter().filter(|f| f.age >= 0.) {
            let n = vfx::frames(f.seq) as f32;
            let u = (f.age / f.dur).clamp(0., 1.);
            // schneller Anfang, ruhiges Abziehen
            let frame = (n - 1.) * u.powf(0.8);
            let c = lit([0.92; 3]);
            out.push(Body {
                center: f.at,
                half: [f.half, f.half],
                angle: f.rot,
                shape: vfx::shape(f.seq, frame.min(n - 1.001)),
                depth: 0.586,
                color: [c[0], c[1], c[2], 1.],
            });
        }
        // Funken und Trümmer
        for p in &self.puffs {
            let age = 1. - p.life / p.max;
            match p.kind {
                Kind::Fire => {
                    let c = if age < 0.35 {
                        [1., 0.95, 0.72]
                    } else {
                        let t = ((age - 0.35) / 0.65).clamp(0., 1.);
                        [1., 0.72 - 0.3 * t, 0.28 - 0.18 * t]
                    };
                    out.push(Body {
                        center: p.at,
                        half: [p.size, p.size],
                        angle: 0.,
                        shape: 3.,
                        depth: 0.5855,
                        color: [c[0], c[1], c[2], (1. - age).powf(1.6) * 0.95],
                    });
                }
                Kind::Debris => {
                    let c = lit([0.12, 0.11, 0.1]);
                    out.push(Body {
                        center: p.at,
                        half: [p.size * 1.6, p.size],
                        angle: p.rot,
                        shape: 4.,
                        depth: 0.5855,
                        color: [c[0], c[1], c[2], (p.life / p.max).min(1.) * 0.95],
                    });
                }
                Kind::Smoke => {}
            }
        }
    }
    /// Am Boden: Brandflecken (unter allem Bewegten); Wurfkörper mit Schatten.
    pub fn bodies(&self, out: &mut Vec<Body>) {
        for t in &self.thrown {
            let (at, z, spin) = (t.at, t.z, t.spin);
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
            let c = [at[0], at[1] - lift(z)];
            let depth = if z > berlin_sim::throw::OVER_CAR as f32 {
                0.6
            } else {
                0.75
            };
            if t.grenade {
                out.push(Body {
                    center: c,
                    half: [2.6 * big, 2.1 * big],
                    angle: spin,
                    shape: 1.,
                    depth,
                    color: [0.24, 0.27, 0.16, 1.],
                });
                // Zünder blinkt in der letzten Sekunde immer schneller
                let left = berlin_sim::throw::FUSE_S as f32 - t.t;
                let on = left < 1. && (t.t * (6. + 14. * (1. - left))).fract() < 0.5;
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
        let flick = |i: usize| 0.8 + 0.2 * (time * 23. + i as f32 * 1.7).sin() * (time * 7.3).cos();
        for (i, b) in self.burning.iter().enumerate() {
            let heat = if b.smolder { b.heat } else { b.heat.max(0.4) };
            out.push(LightSource {
                center: b.at,
                radius: 70. + 100. * heat,
                angle: 0.,
                color: [1., 0.55, 0.2],
                intensity: (0.5 + 0.5 * heat) * flick(i) * dark,
                cone: 0.,
                pad: 0.,
            });
        }
        for (i, p) in self.patches.iter().enumerate() {
            out.push(LightSource {
                center: p.at,
                radius: 60. + p.r * 2.,
                angle: 0.,
                color: [1., 0.55, 0.2],
                intensity: 0.8 * flick(i + 7) * dark * (p.r / berlin_sim::throw::FLAME_R as f32),
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
        // der Feuerball leuchtet noch, wenn der Blitz vorbei ist
        for f in self.flips.iter().filter(|f| f.age >= 0.) {
            let k = (1. - f.age / (f.dur * 0.45)).max(0.);
            if k > 0. {
                out.push(LightSource {
                    center: f.at,
                    radius: f.half * 1.6,
                    angle: 0.,
                    color: [1., 0.6, 0.25],
                    intensity: 0.8 * k * dark,
                    cone: 0.,
                    pad: 0.,
                });
            }
        }
    }
    #[cfg(test)]
    fn counts(&self) -> (usize, usize, usize, usize) {
        (
            self.puffs.len(),
            self.flips.len(),
            self.blasts.len(),
            self.scorch.len(),
        )
    }
}
/// Bildversatz eines Wurfkörpers nach oben (Draufsicht mit leichter Schräge)
fn lift(z: f32) -> f32 {
    z * 0.55
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flipbooks(out: &[Body]) -> usize {
        out.iter().filter(|b| b.shape < 0.).count()
    }

    #[test]
    fn car_explosion_plays_a_flipbook_with_afterblasts_ring_debris_smoke_and_scorch() {
        let mut fx = FireFx::default();
        fx.ingest(&[Event::Explosion {
            x: 100.,
            y: 50.,
            car: Some(1),
            strength: 0.6,
        }]);
        let (p, f, b, s) = fx.counts();
        assert!(p > 20 && f == 3 && b == 1 && s == 1, "{:?}", fx.counts());
        let mut out = Vec::new();
        fx.effects(&mut out, |c| c);
        assert!(
            !out.iter().any(|b| b.shape == 2.),
            "kein Ring mehr: das Flipbook zeigt die Wucht"
        );
        // sofort sichtbar nur die Hauptexplosion (+ Rauch); die Nachdetonationen warten noch
        let main = vfx::seq("explosion_b").unwrap();
        assert!(
            out.iter()
                .any(|b| b.shape < 0. && b.shape > vfx::shape(main, 0.) - 1.)
        );
        // die Folge läuft vorwärts: nach einer Sekunde ein späteres Bild
        let frame_of = |fx: &FireFx| {
            let mut o = Vec::new();
            fx.effects(&mut o, |c| c);
            o.iter()
                .filter(|b| b.shape < 0. && b.half[0] > 100.)
                .map(|b| -b.shape)
                .next()
                .unwrap()
        };
        let f0 = frame_of(&fx);
        for _ in 0..60 {
            fx.step(1. / 60.);
        }
        assert!(frame_of(&fx) > f0 + 5.);
        // nach 12 s nur noch der Brandfleck
        for _ in 0..660 {
            fx.step(1. / 60.);
        }
        assert_eq!(fx.counts(), (0, 0, 0, 1));
        let mut ground = Vec::new();
        fx.bodies(&mut ground);
        assert_eq!(ground.len(), 1);
    }

    #[test]
    fn grenade_explosion_uses_the_compact_blast() {
        let mut fx = FireFx::default();
        fx.ingest(&[Event::Explosion {
            x: 0.,
            y: 0.,
            car: None,
            strength: 0.6,
        }]);
        assert_eq!(fx.counts().1, 1);
        let mut out = Vec::new();
        fx.effects(&mut out, |c| c);
        let blast = vfx::seq("blast").unwrap();
        assert!(
            out.iter()
                .any(|b| b.shape <= vfx::shape(blast, 0.) && b.shape > vfx::shape(blast + 1, 0.))
        );
    }

    #[test]
    fn molotov_shatter_sprays_sparks_and_leaves_a_scorch_mark() {
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
    fn fires_draw_flames_and_glow_and_their_phases_differ() {
        let mut fx = FireFx::default();
        fx.burning.push(Burn {
            id: 7,
            at: [0., 0.],
            angle: 0.,
            hw: 22.,
            hh: 9.,
            heat: 1.,
            smolder: false,
        });
        fx.patches.push(Patch {
            id: 3,
            at: [100., 0.],
            r: 34.,
            t: 2.,
        });
        let mut out = Vec::new();
        fx.effects(&mut out, |c| c);
        // Wrack: Glut + 6 Flammen, Molotow: 14 Flammen
        assert_eq!(flipbooks(&out), 1 + 6 + 14);
        let mut shapes: Vec<i64> = out
            .iter()
            .filter(|b| b.shape < 0.)
            .map(|b| (b.shape * 10.) as i64)
            .collect();
        shapes.sort();
        shapes.dedup();
        assert!(shapes.len() >= 12, "Flammen nicht im Gleichschritt");
        // Flammen stehen mit dem Fuß im Brandherd (tiefster Fuß: Molotow-Rand, 0,85 r unter der Mitte) und wachsen
        // auf dem Bildschirm nach oben: die Bildmitte liegt mindestens 1,5 Breiten über dem Fuß
        let lowest = 0.85 * 34.;
        assert!(
            out.iter()
                .filter(|b| b.shape < 0. && b.half[1] > b.half[0] * 1.5)
                .all(|b| b.center[1] <= lowest - 1.5 * b.half[0])
        );
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
            out.iter()
                .map(|b| b.center[0] + b.center[1] + b.shape)
                .sum::<f32>()
        };
        assert_eq!(run(), run(), "keine Zufallsquelle außer Hashes");
    }
}
