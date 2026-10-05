//! Waffeneffekte (Darstellung): Mündungsfeuer, Pulverdampf, Geschossbahnen, Hülsen, Einschläge.
//!
//! - **Mündungsfeuer:** heißer Kern, Flammenzunge nach vorn und 2–4 seitliche Strahlen, je Schuss anders gedreht
//!   und skaliert, nur ein bis zwei Bilder lang (35–60 ms); die Schrotflinte deutlich größer.
//! - **Pulverdampf:** graue Schwaden vor der Mündung, die aufquellen, verwehen und vergehen (nachts dunkel).
//! - **Geschossbahn:** ein kurzer heller Streifen, der mit Geschossgeschwindigkeit (~350 m/s) die Bahn entlangfliegt.
//! - **Hülsen:** Messing fliegt nach rechts aus, springt einmal auf und bleibt liegen; die Schrotflinte wirft ihre
//!   rote Hülse erst beim Repetieren aus (`PUMP_S` nach dem Schuss).
//! - **Einschläge:** an Wänden Staub und Splitter zurück zum Schützen und ein Einschussloch, das liegen bleibt; auf
//!   Blech ein Funkenregen. Die Richtung kommt aus der Geschossbahn desselben Schritts.
//!
//! Streuung nur aus Hashes eines Zählers, nie aus dem Welt-Zufall.
use berlin_engine::{Body, LightSource};
use berlin_sim::events::Event;
use berlin_sim::math::hash01;
use std::collections::VecDeque;

/// Geschossgeschwindigkeit im Bild (px/s, 10 px = 1 m)
pub const BULLET_SPEED: f32 = 3500.;
/// Länge des Streifens (px)
const STREAK_LEN: f32 = 34.;
/// so lange nach dem Schuss wirft die Schrotflinte die Hülse aus (Repetieren)
pub const PUMP_S: f32 = 0.42;
/// Schwerkraft der Hülsen (px/s², von oben gesehen nur für die Höhe)
const CASING_G: f32 = 420.;
/// Hülsen und Einschusslöcher: so lange liegen sie (s), höchstens so viele
pub const DEBRIS_KEEP: f32 = 30.;
const CASINGS_MAX: usize = 160;
const HOLES_MAX: usize = 200;
const MOTES_MAX: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Flash {
    at: [f32; 2],
    angle: f32,
    t: f32,
    max: f32,
    len: f32,
    wid: f32,
    /// Seitenstrahlen: Winkel zur Schussrichtung, Länge
    spikes: [(f32, f32); 4],
    n: usize,
    big: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoteKind {
    Smoke,
    Dust,
    Chip,
    Spark,
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct Mote {
    at: [f32; 2],
    v: [f32; 2],
    life: f32,
    max: f32,
    size: f32,
    grow: f32,
    kind: MoteKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Streak {
    a: [f32; 2],
    dir: [f32; 2],
    len: f32,
    /// zurückgelegter Weg (px)
    s: f32,
    width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Casing {
    at: [f32; 2],
    v: [f32; 2],
    z: f32,
    vz: f32,
    rot: f32,
    spin: f32,
    age: f32,
    shell: bool,
    bounced: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Hole {
    at: [f32; 2],
    age: f32,
    r: f32,
}

#[derive(Debug, Default)]
pub struct GunFx {
    flashes: Vec<Flash>,
    motes: Vec<Mote>,
    streaks: Vec<Streak>,
    casings: VecDeque<Casing>,
    /// noch auszuwerfende Hülsen (Schrotflinte): Verzögerung, Hülse
    pending: Vec<(f32, Casing)>,
    holes: VecDeque<Hole>,
    seq: u32,
}

fn norm(v: [f32; 2]) -> [f32; 2] {
    let l = v[0].hypot(v[1]).max(1e-6);
    [v[0] / l, v[1] / l]
}
fn rot(v: [f32; 2], a: f32) -> [f32; 2] {
    let (s, c) = a.sin_cos();
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

impl GunFx {
    /// Zufallszahl 0…1 aus dem fortlaufenden Zähler (reine Darstellung).
    fn r(&mut self) -> f32 {
        self.seq = self.seq.wrapping_add(1);
        hash01(self.seq as f64 * 12.9898 + 0.31) as f32
    }
    fn mote(&mut self, m: Mote) {
        if self.motes.len() >= MOTES_MAX {
            self.motes.remove(0);
        }
        self.motes.push(m);
    }

    pub fn ingest(&mut self, events: &[Event]) {
        // Geschossbahnen dieses Schritts: Einschläge (die vor dem Schuss-Ereignis kommen) finden so ihre Richtung
        let traces: Vec<([f32; 2], [f32; 2])> = events
            .iter()
            .filter_map(|e| match e {
                Event::Shot { x, y, traces, .. } => Some(
                    traces
                        .iter()
                        .map(|&(tx, ty)| ([*x as f32, *y as f32], [tx as f32, ty as f32]))
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .flatten()
            .collect();
        for e in events {
            match e {
                Event::Shot {
                    x,
                    y,
                    a,
                    weapon,
                    traces,
                } => self.shot([*x as f32, *y as f32], *a as f32, weapon, traces),
                Event::Impact { x, y, metal } => {
                    let at = [*x as f32, *y as f32];
                    let dir = traces
                        .iter()
                        .filter(|(_, b)| (b[0] - at[0]).hypot(b[1] - at[1]) < 3.)
                        .map(|(a, b)| norm([b[0] - a[0], b[1] - a[1]]))
                        .next();
                    self.impact(at, dir, *metal);
                }
                _ => {}
            }
        }
    }

    fn shot(&mut self, at: [f32; 2], angle: f32, weapon: &str, traces: &[(f64, f64)]) {
        let shotgun = weapon == "shotgun";
        let smg = weapon == "smg";
        // Mündungsfeuer: Größe und Strahlen je Schuss gewürfelt
        let (len, wid, max) = if shotgun {
            (14. + 6. * self.r(), 8. + 3. * self.r(), 0.06)
        } else if smg {
            (7. + 3. * self.r(), 4. + 1.5 * self.r(), 0.035)
        } else {
            (9. + 4. * self.r(), 4.5 + 1.5 * self.r(), 0.045)
        };
        let n = 2 + (self.r() * 3.) as usize;
        let mut spikes = [(0., 0.); 4];
        for (k, sp) in spikes.iter_mut().enumerate().take(n) {
            let side = if k % 2 == 0 { 1. } else { -1. };
            *sp = (
                side * (0.45 + 0.75 * self.r()),
                (0.35 + 0.4 * self.r()) * len,
            );
        }
        self.flashes.push(Flash {
            at,
            angle,
            t: max,
            max,
            len,
            wid,
            spikes,
            n,
            big: shotgun,
        });
        let dir = [angle.cos(), angle.sin()];
        // Pulverdampf vor der Mündung, treibt langsam nach vorn
        let puffs = if shotgun {
            5
        } else if smg {
            1
        } else {
            2
        };
        for _ in 0..puffs {
            let sp = 8. + 18. * self.r();
            let off = (self.r() - 0.5) * 0.9;
            let v = rot(dir, off);
            let fwd = 3. + 6. * self.r();
            let r1 = 0.7 + 0.6 * self.r();
            self.mote(Mote {
                at: [at[0] + dir[0] * fwd, at[1] + dir[1] * fwd],
                v: [v[0] * sp, v[1] * sp],
                life: r1,
                max: 1.3,
                size: if shotgun { 4. } else { 2.5 },
                grow: if shotgun { 14. } else { 9. },
                kind: MoteKind::Smoke,
            });
        }
        // Geschossbahnen
        for &(tx, ty) in traces {
            let d = [tx as f32 - at[0], ty as f32 - at[1]];
            let l = d[0].hypot(d[1]);
            if l < 1. {
                continue;
            }
            self.streaks.push(Streak {
                a: at,
                dir: norm(d),
                len: l,
                s: 0.,
                width: if shotgun { 0.35 } else { 0.5 },
            });
        }
        // Hülse: aus dem Auswurf rechts neben der Waffe, nach rechts und etwas nach hinten
        let right = [-dir[1], dir[0]];
        let out = 45. + 35. * self.r();
        let back = -8. + 16. * self.r();
        let c = Casing {
            at: [
                at[0] - dir[0] * 7. + right[0] * 2.,
                at[1] - dir[1] * 7. + right[1] * 2.,
            ],
            v: [
                right[0] * out - dir[0] * back,
                right[1] * out - dir[1] * back,
            ],
            z: 12.,
            vz: 40. + 25. * self.r(),
            rot: angle,
            spin: (self.r() - 0.5) * 40.,
            age: 0.,
            shell: shotgun,
            bounced: false,
        };
        if shotgun {
            self.pending.push((PUMP_S, c));
        } else {
            self.push_casing(c);
        }
    }
    fn push_casing(&mut self, c: Casing) {
        self.casings.push_back(c);
        while self.casings.len() > CASINGS_MAX {
            self.casings.pop_front();
        }
    }

    fn impact(&mut self, at: [f32; 2], dir: Option<[f32; 2]>, metal: bool) {
        // zurück zum Schützen (gegen die Flugrichtung); ohne bekannte Richtung rundum
        let back = dir.map(|d| [-d[0], -d[1]]);
        let spread = if back.is_some() {
            1.3
        } else {
            std::f32::consts::PI
        };
        let base = back.unwrap_or([1., 0.]);
        if metal {
            for _ in 0..7 {
                let v = rot(base, (self.r() - 0.5) * 2. * spread);
                let sp = 90. + 160. * self.r();
                let r1 = 0.1 + 0.15 * self.r();
                self.mote(Mote {
                    at,
                    v: [v[0] * sp, v[1] * sp],
                    life: r1,
                    max: 0.25,
                    size: 0.9,
                    grow: 0.,
                    kind: MoteKind::Spark,
                });
            }
        } else {
            for _ in 0..3 {
                let v = rot(base, (self.r() - 0.5) * spread);
                let sp = 15. + 25. * self.r();
                let r1 = 0.45 + 0.3 * self.r();
                self.mote(Mote {
                    at,
                    v: [v[0] * sp, v[1] * sp],
                    life: r1,
                    max: 0.75,
                    size: 1.8,
                    grow: 8.,
                    kind: MoteKind::Dust,
                });
            }
            for _ in 0..4 {
                let v = rot(base, (self.r() - 0.5) * 2. * spread);
                let sp = 60. + 90. * self.r();
                let r1 = 0.15 + 0.15 * self.r();
                self.mote(Mote {
                    at,
                    v: [v[0] * sp, v[1] * sp],
                    life: r1,
                    max: 0.3,
                    size: 0.6,
                    grow: 0.,
                    kind: MoteKind::Chip,
                });
            }
            let r1 = 0.8 + 0.4 * self.r();
            self.holes.push_back(Hole { at, age: 0., r: r1 });
            while self.holes.len() > HOLES_MAX {
                self.holes.pop_front();
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        for f in &mut self.flashes {
            f.t -= dt;
        }
        self.flashes.retain(|f| f.t > 0.);
        for m in &mut self.motes {
            m.at[0] += m.v[0] * dt;
            m.at[1] += m.v[1] * dt;
            // Luftwiderstand: Rauch bremst schnell, Funken und Splitter weniger
            let drag = match m.kind {
                MoteKind::Smoke | MoteKind::Dust => 2.5,
                MoteKind::Chip => 4.,
                MoteKind::Spark => 3.,
            };
            let k = (-drag * dt).exp();
            m.v = [m.v[0] * k, m.v[1] * k];
            m.size += m.grow * dt;
            m.life -= dt;
        }
        self.motes.retain(|m| m.life > 0.);
        for s in &mut self.streaks {
            s.s += BULLET_SPEED * dt;
        }
        self.streaks.retain(|s| s.s < s.len + STREAK_LEN);
        let mut ready = Vec::new();
        for (t, c) in &mut self.pending {
            *t -= dt;
            if *t <= 0. {
                ready.push(*c);
            }
        }
        self.pending.retain(|(t, _)| *t > 0.);
        for c in ready {
            self.push_casing(c);
        }
        for c in &mut self.casings {
            c.age += dt;
            if c.z > 0. || c.vz > 0. {
                c.at[0] += c.v[0] * dt;
                c.at[1] += c.v[1] * dt;
                c.rot += c.spin * dt;
                c.vz -= CASING_G * dt;
                c.z += c.vz * dt;
                if c.z <= 0. {
                    c.z = 0.;
                    if c.bounced {
                        c.vz = 0.;
                        c.v = [0., 0.];
                    } else {
                        // einmal aufspringen, dann rollen sie aus
                        c.bounced = true;
                        c.vz = -c.vz * 0.3;
                        c.v = [c.v[0] * 0.35, c.v[1] * 0.35];
                        c.spin *= 0.5;
                    }
                }
            }
        }
        while self.casings.front().is_some_and(|c| c.age > DEBRIS_KEEP) {
            self.casings.pop_front();
        }
        for h in &mut self.holes {
            h.age += dt;
        }
        while self.holes.front().is_some_and(|h| h.age > DEBRIS_KEEP) {
            self.holes.pop_front();
        }
    }

    /// Durchscheinendes nach dem Licht: Feuer und Funken leuchten selbst, Dampf, Staub und Splitter werden mit dem
    /// Umgebungslicht dunkel (`lit`).
    pub fn effects(&self, out: &mut Vec<Body>, lit: impl Fn([f32; 3]) -> [f32; 3]) {
        for m in &self.motes {
            let k = (m.life / m.max).clamp(0., 1.);
            let (half, angle, color) = match m.kind {
                MoteKind::Smoke => {
                    let c = lit([0.78, 0.77, 0.74]);
                    ([m.size, m.size], 0., [c[0], c[1], c[2], 0.32 * k])
                }
                MoteKind::Dust => {
                    let c = lit([0.74, 0.68, 0.58]);
                    ([m.size, m.size], 0., [c[0], c[1], c[2], 0.55 * k])
                }
                MoteKind::Chip => {
                    let c = lit([0.35, 0.32, 0.3]);
                    ([m.size, m.size], 0., [c[0], c[1], c[2], 0.9 * k])
                }
                MoteKind::Spark => {
                    // Funke als kurzer Strich in Flugrichtung
                    let sp = m.v[0].hypot(m.v[1]);
                    (
                        [(sp * 0.012).max(0.8), 0.35],
                        m.v[1].atan2(m.v[0]),
                        [1., 0.85 + 0.15 * k, 0.45 + 0.4 * k, k],
                    )
                }
            };
            out.push(Body {
                center: m.at,
                half,
                angle,
                shape: if m.kind == MoteKind::Spark { 0. } else { 3. },
                depth: 0.31,
                color,
            });
        }
        for s in &self.streaks {
            // Kopf und Ende des Streifens auf der Bahn
            let head = s.s.min(s.len);
            let tail = (s.s - STREAK_LEN).max(0.);
            let l = head - tail;
            if l <= 0.5 {
                continue;
            }
            let mid = (head + tail) / 2.;
            let k = if s.s > s.len {
                1. - (s.s - s.len) / STREAK_LEN
            } else {
                1.
            };
            out.push(Body {
                center: [s.a[0] + s.dir[0] * mid, s.a[1] + s.dir[1] * mid],
                half: [l / 2., s.width],
                angle: s.dir[1].atan2(s.dir[0]),
                shape: 3.,
                depth: 0.3,
                color: [1., 0.93, 0.72, 0.85 * k],
            });
        }
        for f in &self.flashes {
            let k = (f.t / f.max).clamp(0., 1.);
            let a = k.sqrt();
            let d = [f.angle.cos(), f.angle.sin()];
            let at = |l: f32, dd: [f32; 2]| [f.at[0] + dd[0] * l, f.at[1] + dd[1] * l];
            // Flammenzunge (orange), heißer Innenteil, weißer Kern an der Mündung
            out.push(Body {
                center: at(f.len * 0.5, d),
                half: [f.len * 0.5, f.wid * 0.5],
                angle: f.angle,
                shape: 3.,
                depth: 0.292,
                color: [1., 0.62, 0.22, 0.9 * a],
            });
            out.push(Body {
                center: at(f.len * 0.32, d),
                half: [f.len * 0.3, f.wid * 0.24],
                angle: f.angle,
                shape: 3.,
                depth: 0.291,
                color: [1., 0.92, 0.6, a],
            });
            for &(off, len) in &f.spikes[..f.n] {
                let sd = rot(d, off);
                out.push(Body {
                    center: at(len * 0.5, sd),
                    half: [len * 0.5, if f.big { 1.1 } else { 0.7 }],
                    angle: f.angle + off,
                    shape: 3.,
                    depth: 0.2915,
                    color: [1., 0.75, 0.35, 0.8 * a],
                });
            }
            let core = if f.big { 3.6 } else { 2.4 };
            out.push(Body {
                center: at(1.2, d),
                half: [core, core * 0.8],
                angle: f.angle,
                shape: 3.,
                depth: 0.29,
                color: [1., 0.98, 0.9, a],
            });
        }
    }

    /// Am Boden: Einschusslöcher und Hülsen (fliegende über den Figuren).
    pub fn bodies(&self, out: &mut Vec<Body>) {
        for h in &self.holes {
            let fade = 1. - ((h.age / DEBRIS_KEEP - 0.8) / 0.2).clamp(0., 1.);
            out.push(Body {
                center: h.at,
                half: [h.r, h.r],
                angle: 0.,
                shape: 1.,
                depth: 0.828,
                color: [0.08, 0.075, 0.07, 0.85 * fade],
            });
        }
        for c in &self.casings {
            let fade = 1. - ((c.age / DEBRIS_KEEP - 0.8) / 0.2).clamp(0., 1.);
            let flying = c.z > 0.;
            // in der Luft: näher an der Kamera, etwas größer
            let s = 1. + c.z * 0.02;
            let (half, color) = if c.shell {
                ([1.25 * s, 0.5 * s], [0.72, 0.12, 0.1, fade])
            } else {
                ([0.6 * s, 0.28 * s], [0.86, 0.68, 0.3, fade])
            };
            out.push(Body {
                center: c.at,
                half,
                angle: c.rot,
                shape: 0.,
                depth: if flying { 0.58 } else { 0.826 },
                color,
            });
            if c.shell {
                // Messingboden der Schrothülse
                let d = [c.rot.cos(), c.rot.sin()];
                out.push(Body {
                    center: [c.at[0] - d[0] * 1.0 * s, c.at[1] - d[1] * 1.0 * s],
                    half: [0.35 * s, 0.55 * s],
                    angle: c.rot,
                    shape: 0.,
                    depth: if flying { 0.5799 } else { 0.8259 },
                    color: [0.86, 0.68, 0.3, fade],
                });
            }
        }
    }

    /// Mündungsfeuer erhellt die Umgebung, Funken kurz ein wenig.
    pub fn lights(&self, out: &mut Vec<LightSource>, dark: f32) {
        for f in &self.flashes {
            out.push(LightSource {
                center: f.at,
                radius: if f.big { 170. } else { 115. },
                angle: 0.,
                color: [1., 0.8, 0.45],
                intensity: (f.t / f.max * dark).min(1.),
                cone: 0.,
                pad: 0.,
            });
        }
    }

    #[cfg(test)]
    pub fn counts(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.flashes.len(),
            self.streaks.len(),
            self.motes.len(),
            self.casings.len() + self.pending.len(),
            self.holes.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(weapon: &'static str, a: f64) -> Event {
        Event::Shot {
            x: 0.,
            y: 0.,
            a,
            weapon,
            traces: vec![(200. * a.cos(), 200. * a.sin())],
        }
    }

    #[test]
    fn muzzle_flash_varies_and_is_gone_within_two_frames() {
        let mut fx = GunFx::default();
        fx.ingest(&[shot("pistol", 0.)]);
        fx.ingest(&[shot("pistol", 0.)]);
        let (f0, f1) = (fx.flashes[0], fx.flashes[1]);
        assert!(
            f0.len != f1.len || f0.n != f1.n || f0.spikes != f1.spikes,
            "jeder Schuss anders"
        );
        let mut out = Vec::new();
        fx.effects(&mut out, |c| c);
        assert!(
            out.len() >= 8,
            "Kern, Zunge, Strahlen, Dampf, Streifen: {}",
            out.len()
        );
        fx.step(1. / 30.);
        assert!(!fx.flashes.is_empty(), "ein Bild später noch zu sehen");
        fx.step(1. / 30.);
        assert!(fx.flashes.is_empty(), "nach zwei Bildern weg");
        // die Schrotflinte feuert größer
        let mut g = GunFx::default();
        g.ingest(&[shot("shotgun", 0.)]);
        assert!(g.flashes[0].len > 13. && g.flashes[0].big);
    }

    #[test]
    fn bullets_fly_along_their_path() {
        let mut fx = GunFx::default();
        fx.ingest(&[shot("pistol", 0.)]);
        fx.step(0.02);
        // nach 20 ms 70 px weit: der Streifen liegt zwischen 36 und 70 px
        let mut out = Vec::new();
        fx.effects(&mut out, |c| c);
        let st = out.iter().find(|b| b.depth == 0.3).expect("Streifen");
        assert!(
            (st.center[0] - 53.).abs() < 2. && st.center[1].abs() < 0.1,
            "{:?}",
            st.center
        );
        fx.step(0.1);
        assert_eq!(fx.counts().1, 0, "angekommen und verblasst");
    }

    #[test]
    fn casings_fly_right_land_and_stay() {
        let mut fx = GunFx::default();
        // nach rechts (+x): rechts ist +y
        fx.ingest(&[shot("pistol", 0.)]);
        for _ in 0..60 {
            fx.step(1. / 60.);
        }
        let c = fx.casings[0];
        assert_eq!(c.z, 0.);
        assert!(c.at[1] > 10., "nach rechts ausgeworfen: {:?}", c.at);
        let mut out = Vec::new();
        fx.bodies(&mut out);
        assert!(out.iter().any(|b| b.depth == 0.826), "liegt am Boden");
        // Schrotflinte: Hülse erst beim Repetieren
        let mut g = GunFx::default();
        g.ingest(&[shot("shotgun", 0.)]);
        assert!(g.casings.is_empty() && g.pending.len() == 1);
        g.step(PUMP_S + 0.01);
        assert_eq!(g.casings.len(), 1);
        assert!(g.casings[0].shell);
        // nach 30 s weg
        fx.step(DEBRIS_KEEP + 1.);
        assert!(fx.casings.is_empty());
    }

    #[test]
    fn impacts_spray_back_toward_the_shooter() {
        let mut fx = GunFx::default();
        // Schuss nach +x, Einschlag in der Wand bei x = 200: Staub fliegt nach −x
        fx.ingest(&[
            Event::Impact {
                x: 200.,
                y: 0.,
                metal: false,
            },
            shot("pistol", 0.),
        ]);
        let dust: Vec<&Mote> = fx
            .motes
            .iter()
            .filter(|m| m.kind == MoteKind::Dust)
            .collect();
        assert_eq!(dust.len(), 3);
        assert!(dust.iter().all(|m| m.v[0] < 0.), "zurück zum Schützen");
        assert_eq!(fx.counts().4, 1, "Einschussloch");
        // Blech: Funken, kein Loch
        let mut m = GunFx::default();
        m.ingest(&[
            Event::Impact {
                x: 200.,
                y: 0.,
                metal: true,
            },
            shot("pistol", 0.),
        ]);
        assert!(m.motes.iter().filter(|q| q.kind == MoteKind::Spark).count() >= 5);
        assert_eq!(m.counts().4, 0);
        // Funken vergehen schnell
        m.step(0.3);
        assert!(m.motes.iter().all(|q| q.kind != MoteKind::Spark));
    }
}
