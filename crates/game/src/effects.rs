//! Kurzlebige Kampfeffekte (render.js aus den Ereignissen `shot/impact/blood/kill`): Mündungsfeuer, Leuchtspuren,
//! Einschläge und Blut am Boden. Rein darstellend: Streuung der Tropfen aus Hashes, nie aus dem Welt-Zufall.
use berlin_engine::{Body, LightSource};
use berlin_sim::events::Event;
use berlin_sim::math::hash01;

/// so lange bleiben Blutflecken liegen (s), höchstens so viele
const BLOOD_KEEP: f32 = 90.;
const BLOOD_MAX: usize = 300;
const TRACER_S: f32 = 0.09;
const FLASH_S: f32 = 0.06;
const PUFF_S: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Tracer {
    a: [f32; 2],
    b: [f32; 2],
    t: f32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct Flash {
    at: [f32; 2],
    angle: f32,
    t: f32,
    big: bool,
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct Puff {
    at: [f32; 2],
    t: f32,
    metal: bool,
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct Splat {
    at: [f32; 2],
    r: [f32; 2],
    angle: f32,
    age: f32,
}

#[derive(Debug, Default)]
pub struct Effects {
    tracers: Vec<Tracer>,
    flashes: Vec<Flash>,
    puffs: Vec<Puff>,
    splats: std::collections::VecDeque<Splat>,
    seq: u32,
}

impl Effects {
    /// Ereignisse eines Schritts übernehmen.
    pub fn ingest(&mut self, events: &[Event]) {
        for e in events {
            match e {
                Event::Shot {
                    x,
                    y,
                    a,
                    weapon,
                    traces,
                } => {
                    let from = [*x as f32, *y as f32];
                    self.flashes.push(Flash {
                        at: from,
                        angle: *a as f32,
                        t: FLASH_S,
                        big: *weapon == "shotgun",
                    });
                    for &(tx, ty) in traces {
                        self.tracers.push(Tracer {
                            a: from,
                            b: [tx as f32, ty as f32],
                            t: TRACER_S,
                        });
                    }
                }
                Event::Impact { x, y, metal } => self.puffs.push(Puff {
                    at: [*x as f32, *y as f32],
                    t: PUFF_S,
                    metal: *metal,
                }),
                Event::Blood { x, y, a, n } => {
                    // Tropfen fächern in Schlagrichtung auf, Größe und Abstand aus Hashes
                    for k in 0..*n {
                        self.seq = self.seq.wrapping_add(1);
                        let h = |m: f64| hash01(self.seq as f64 * 12.9898 + m) as f32;
                        let ang = *a as f32 + (h(1.) - 0.5) * 1.2;
                        let d = 4. + h(2.) * 16. * (k as f32 + 1.) / *n as f32;
                        let r = 1.5 + h(3.) * 3.5;
                        self.add_splat(
                            [*x as f32 + ang.cos() * d, *y as f32 + ang.sin() * d],
                            [r * (1. + h(4.)), r],
                            ang,
                        );
                    }
                }
                Event::Kill { x, y, .. } => {
                    self.seq = self.seq.wrapping_add(1);
                    let h = hash01(self.seq as f64 * 3.7) as f32;
                    self.add_splat(
                        [*x as f32, *y as f32],
                        [13. + h * 5., 10. + h * 3.],
                        h * std::f32::consts::TAU,
                    );
                }
                _ => {}
            }
        }
    }
    fn add_splat(&mut self, at: [f32; 2], r: [f32; 2], angle: f32) {
        self.splats.push_back(Splat {
            at,
            r,
            angle,
            age: 0.,
        });
        while self.splats.len() > BLOOD_MAX {
            self.splats.pop_front();
        }
    }
    pub fn step(&mut self, dt: f32) {
        for t in &mut self.tracers {
            t.t -= dt;
        }
        self.tracers.retain(|t| t.t > 0.);
        for f in &mut self.flashes {
            f.t -= dt;
        }
        self.flashes.retain(|f| f.t > 0.);
        for p in &mut self.puffs {
            p.t -= dt;
        }
        self.puffs.retain(|p| p.t > 0.);
        for s in &mut self.splats {
            s.age += dt;
        }
        while self.splats.front().is_some_and(|s| s.age > BLOOD_KEEP) {
            self.splats.pop_front();
        }
    }
    /// Blut am Boden (vor den Figuren), Leuchtspuren, Mündungsfeuer und Staub über allem Bewegten.
    pub fn bodies(&self, out: &mut Vec<Body>) {
        for s in &self.splats {
            // trocknet nach: wird dunkler und verblasst gegen Ende
            let k = (s.age / BLOOD_KEEP).min(1.);
            let fade = if k > 0.8 { 1. - (k - 0.8) / 0.2 } else { 1. };
            out.push(Body {
                center: s.at,
                half: s.r,
                angle: s.angle,
                shape: 1.,
                depth: 0.83,
                color: [0.42 - 0.18 * k, 0.03, 0.03, 0.85 * fade],
            });
        }
        for t in &self.tracers {
            let (dx, dy) = (t.b[0] - t.a[0], t.b[1] - t.a[1]);
            let len = dx.hypot(dy);
            if len < 1. {
                continue;
            }
            let k = t.t / TRACER_S;
            out.push(Body {
                center: [(t.a[0] + t.b[0]) / 2., (t.a[1] + t.b[1]) / 2.],
                half: [len / 2., 0.7],
                angle: dy.atan2(dx),
                shape: 0.,
                depth: 0.3,
                color: [1., 0.92, 0.6, 0.75 * k],
            });
        }
        for f in &self.flashes {
            let k = f.t / FLASH_S;
            let s = if f.big { 1.6 } else { 1. };
            out.push(Body {
                center: [
                    f.at[0] + f.angle.cos() * 4. * s,
                    f.at[1] + f.angle.sin() * 4. * s,
                ],
                half: [7. * s, 3.5 * s],
                angle: f.angle,
                shape: 1.,
                depth: 0.29,
                color: [1., 0.85, 0.35, 0.9 * k],
            });
        }
        for p in &self.puffs {
            let k = p.t / PUFF_S;
            let r = 2. + (1. - k) * 5.;
            out.push(Body {
                center: p.at,
                half: [r, r],
                angle: 0.,
                shape: 1.,
                depth: 0.31,
                color: if p.metal {
                    [1., 0.8, 0.4, 0.8 * k]
                } else {
                    [0.75, 0.72, 0.68, 0.6 * k]
                },
            });
        }
    }
    /// Mündungsfeuer erhellt nachts kurz die Umgebung.
    pub fn lights(&self, out: &mut Vec<LightSource>, dark: f32) {
        for f in &self.flashes {
            out.push(LightSource {
                center: f.at,
                radius: if f.big { 160. } else { 110. },
                angle: 0.,
                color: [1., 0.8, 0.45],
                intensity: (f.t / FLASH_S * dark).min(1.),
                cone: 0.,
                pad: 0.,
            });
        }
    }
    #[cfg(test)]
    pub fn blood_count(&self) -> usize {
        self.splats.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effects_appear_and_fade() {
        let mut fx = Effects::default();
        fx.ingest(&[
            Event::Shot {
                x: 0.,
                y: 0.,
                a: 0.,
                weapon: "shotgun",
                traces: vec![(100., 0.), (90., 10.)],
            },
            Event::Blood {
                x: 50.,
                y: 0.,
                a: 0.,
                n: 7,
            },
            Event::Impact {
                x: 100.,
                y: 0.,
                metal: false,
            },
        ]);
        let mut out = Vec::new();
        fx.bodies(&mut out);
        assert_eq!(out.len(), 7 + 2 + 1 + 1, "Blut, Spuren, Feuer, Staub");
        // Blutstropfen liegen in Schlagrichtung
        assert!(fx.splats.iter().all(|s| s.at[0] > 50.));
        fx.step(0.3);
        let mut out = Vec::new();
        fx.bodies(&mut out);
        assert_eq!(
            out.len(),
            7,
            "Spuren, Feuer und Staub sind weg, das Blut bleibt"
        );
        fx.step(BLOOD_KEEP);
        assert_eq!(fx.blood_count(), 0);
    }
}
