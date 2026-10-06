//! Kurzlebige Effekte: Reifenwolken, Bremsspuren, Blut am Boden und die Waffeneffekte aus `gunfx.rs`
//! (Mündungsfeuer, Pulverdampf, Geschossbahnen, Hülsen, Einschläge). Rein darstellend: Streuung der Tropfen aus Hashes, nie aus dem Welt-Zufall.
use berlin_engine::{Body, LightSource};
use berlin_sim::events::Event;
use berlin_sim::math::hash01;

/// so lange bleiben Blutflecken liegen (s), höchstens so viele
const BLOOD_KEEP: f32 = 90.;
const BLOOD_MAX: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Splat {
    at: [f32; 2],
    r: [f32; 2],
    angle: f32,
    age: f32,
}

/// Art eines Reifenpartikels (worldfx.js tireEffect)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kick {
    Smoke,
    Dust,
    Spray,
    Snow,
}
impl Kick {
    /// Farbe ohne Blaustich: Reifenqualm ist warmweißes Grau, Gischt fast farbloser Sprühnebel (vorher hellblau
    /// bzw. bläulich – las sich wie ein Leuchteffekt am Rad).
    pub fn color(self) -> [f32; 3] {
        match self {
            Kick::Dust => [0.678, 0.604, 0.475],
            Kick::Smoke => [0.86, 0.85, 0.83],
            Kick::Spray => [0.82, 0.83, 0.84],
            Kick::Snow => [0.937, 0.951, 0.96],
        }
    }
    /// Höchste Deckkraft: Gischt ist dünn, Qualm dichter, Staub dazwischen.
    fn alpha(self) -> f32 {
        match self {
            Kick::Smoke => 0.24,
            Kick::Spray => 0.3,
            Kick::Dust => 0.45,
            Kick::Snow => 0.5,
        }
    }
}
/// Welche Wolke ein Reifen aufwirbelt: Schnee, Gischt bei Nässe oder im Wasser, Staub auf Gras und Gehweg, auf
/// trockenem Asphalt nur beim Driften Qualm.
pub fn tire_effect(
    ground: berlin_sim::city::Ground,
    wet: f64,
    snow: f64,
    hard: bool,
) -> Option<Kick> {
    use berlin_sim::city::Ground as G;
    if snow > 0.2 {
        Some(Kick::Snow)
    } else if wet > 0.2 || ground == G::Water {
        Some(Kick::Spray)
    } else if matches!(ground, G::Grass | G::Sidewalk) {
        Some(Kick::Dust)
    } else {
        hard.then_some(Kick::Smoke)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct Particle {
    kind: Kick,
    at: [f32; 2],
    v: [f32; 2],
    size: f32,
    life: f32,
    max: f32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct Skid {
    a: [f32; 2],
    b: [f32; 2],
    life: f32,
}
const PARTICLES_MAX: usize = 360;
const SKIDS_MAX: usize = 600;
const SKID_S: f32 = 8.;

#[derive(Debug, Default)]
pub struct Effects {
    particles: Vec<Particle>,
    skids: std::collections::VecDeque<Skid>,
    /// Ausstoß je Auto (Bruchteile tragen über Schritte), letzter Reifenpunkt für Bremsspuren
    emit: std::collections::HashMap<u32, f32>,
    last_skid: std::collections::HashMap<(u32, i8), [f32; 2]>,
    gun: crate::gunfx::GunFx,
    /// brennende Wracks und Explosionen
    fire: crate::firefx::FireFx,
    splats: std::collections::VecDeque<Splat>,
    seq: u32,
    /// Ring am Boden, wohin ein Klick die Figur schickt (main.js clickFx)
    ring: Option<([f32; 2], f32)>,
}
/// so lange sieht man den Klickring (s)
const RING_S: f32 = 0.45;

impl Effects {
    /// Ereignisse eines Schritts übernehmen.
    pub fn ingest(&mut self, events: &[Event]) {
        self.gun.ingest(events);
        self.fire.ingest(events);
        for e in events {
            match e {
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
    /// Reifen (worldfx.js update, render.js Bremsspuren): Wolken je nach Untergrund und Wetter, Qualm aus
    /// beschädigten Autos, Bremsspuren beim Rutschen, Vollbremsen oder mit Handbremse.
    pub fn tires(&mut self, w: &mut berlin_sim::world::World, dt: f32) {
        let dt = dt.clamp(0., 0.1);
        let (cx, cy) = (w.camera.x, w.camera.y);
        let (wet, snow) = (w.weather.wet, w.weather.snow);
        let mut live = std::collections::HashSet::new();
        for c in &w.cars {
            if (c.x - cx).abs() > 1600. || (c.y - cy).abs() > 1100. || c.lvl() > 0 {
                continue;
            }
            let speed = c.speed();
            let (co, si) = (c.angle.cos() as f32, c.angle.sin() as f32);
            let (x, y, hw, hh) = (c.x as f32, c.y as f32, c.hw as f32, c.hh as f32);
            // Rauch aus dem Motorraum eines beschädigten oder zerstörten Autos
            if c.health < 35. || c.wrecked {
                self.seq = self.seq.wrapping_add(1);
                let h = |m: f64| hash01(self.seq as f64 * 7.13 + m) as f32;
                if h(1.) < if c.wrecked { 0.35 } else { 0.15 } {
                    self.add(Particle {
                        kind: Kick::Smoke,
                        at: [x + co * hw * 0.7, y + si * hw * 0.7],
                        v: [(h(2.) - 0.5) * 10., -18.],
                        size: 4.,
                        life: 1.4,
                        max: 1.4,
                    });
                }
            }
            if speed < 30. {
                continue;
            }
            let hard = c.skid > 0.2
                || (c.controls.handbrake && speed > 60.)
                || (c.controls.brake > 0.8 && speed > 150. && c.driver.is_some());
            // Bremsspuren an den Hinterrädern
            for side in [-1i8, 1] {
                let sd = side as f32;
                let p = [
                    x - co * (hw - 6.) - si * sd * (hh - 3.),
                    y - si * (hw - 6.) + co * sd * (hh - 3.),
                ];
                if hard && snow < 0.2 {
                    if let Some(l) = self.last_skid.get(&(c.id, side))
                        && (l[0] - p[0]).hypot(l[1] - p[1]) < 30.
                    {
                        self.skids.push_back(Skid {
                            a: *l,
                            b: p,
                            life: SKID_S,
                        });
                    }
                    self.last_skid.insert((c.id, side), p);
                } else {
                    self.last_skid.remove(&(c.id, side));
                }
            }
            // ohne Durchdrehen/Blockieren: Nässe sprüht erst ab etwa 40 km/h, Schnee stäubt immer
            if !hard && snow <= 0.2 && (wet <= 0.2 || speed < 110.) {
                continue;
            }
            let ground = w.city.surface_at(c.x, c.y, Some(c.lvl()));
            let Some(kind) = tire_effect(ground, wet, snow, hard) else {
                continue;
            };
            live.insert(c.id);
            let next =
                self.emit.get(&c.id).copied().unwrap_or(0.) + dt * if hard { 26. } else { 8. };
            self.emit.insert(c.id, next.fract());
            let (vx, vy) = (c.vx as f32, c.vy as f32);
            for _ in 0..next as usize {
                for sd in [-1f32, 1.] {
                    // Streuung je Partikel aus einem Hash (nie aus dem Simulationszufall): viele dünne, leicht
                    // verschiedene Wölkchen verschmelzen zu einer Wolke statt als einzelne Ballen zu stehen
                    self.seq = self.seq.wrapping_add(1);
                    let seq = self.seq;
                    let h = |m: f64| hash01(seq as f64 * 3.71 + m) as f32 - 0.5;
                    let smoke = kind == Kick::Smoke;
                    let spread = if smoke { 14. } else { 6. };
                    self.add(Particle {
                        kind,
                        at: [
                            x - co * hw * 0.65 - si * sd * hh + h(1.) * 3.,
                            y - si * hw * 0.65 + co * sd * hh + h(2.) * 3.,
                        ],
                        // Gischt fliegt als Fahne hinter dem Rad her, Qualm und Staub quellen seitlich auf
                        v: if kind == Kick::Spray {
                            [
                                vx * 0.3 - si * sd * 4. + h(3.) * spread,
                                vy * 0.3 + co * sd * 4. + h(4.) * spread,
                            ]
                        } else {
                            [
                                vx * 0.12 - si * sd * 9. + h(3.) * spread,
                                vy * 0.12 + co * sd * 9. + h(4.) * spread,
                            ]
                        },
                        size: if smoke { 4. + h(5.) * 2. } else { 3.5 },
                        life: if smoke { 1.5 + h(6.) * 0.6 } else { 0.6 },
                        max: if smoke { 1.5 + h(6.) * 0.6 } else { 0.6 },
                    });
                }
            }
        }
        self.emit.retain(|id, _| live.contains(id));
        for q in &mut self.particles {
            q.life -= dt;
            q.at[0] += q.v[0] * dt;
            q.at[1] += q.v[1] * dt;
            // Luftwiderstand: die Wolke bleibt hinter dem Auto zurück und quillt auf
            let drag = (1. - dt * if q.kind == Kick::Smoke { 1.6 } else { 0.8 }).max(0.);
            q.v = [q.v[0] * drag, q.v[1] * drag];
            q.size += dt * if q.kind == Kick::Smoke { 17. } else { 7. };
        }
        self.particles.retain(|q| q.life > 0.);
        for k in &mut self.skids {
            k.life -= dt;
        }
        while self.skids.front().is_some_and(|k| k.life <= 0.) || self.skids.len() > SKIDS_MAX {
            self.skids.pop_front();
        }
    }
    fn add(&mut self, p: Particle) {
        if self.particles.len() >= PARTICLES_MAX {
            self.particles.remove(0);
        }
        self.particles.push(p);
    }
    pub fn click_ring(&mut self, at: (f64, f64)) {
        self.ring = Some(([at.0 as f32, at.1 as f32], RING_S));
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
        if let Some((_, t)) = self.ring.as_mut() {
            *t -= dt;
            if *t <= 0. {
                self.ring = None;
            }
        }
        self.gun.step(dt);
        self.fire.step(dt);
        for s in &mut self.splats {
            s.age += dt;
        }
        while self.splats.front().is_some_and(|s| s.age > BLOOD_KEEP) {
            self.splats.pop_front();
        }
    }
    /// Spuren am Boden unter allem Bewegten: Bremsspuren, Blut, Klickring.
    /// Durchscheinende Effekte für den eigenen Durchgang nach dem Licht (`Game::effects`): Reifenwolken, Leuchtspuren,
    /// Mündungsfeuer, Einschlagwölkchen. Sie schreiben keine Tiefe, verdecken also nichts – vorher lagen sie mit
    /// Tiefe 0,59 vor dem Auto und ließen dessen Silhouette unter jeder Reifenwolke hellblau aufleuchten.
    /// `ambient`: Umgebungslicht (Qualm und Staub werden nachts dunkel; Glut und Mündungsfeuer leuchten selbst).
    pub fn effects(&self, out: &mut Vec<Body>, ambient: [f32; 3]) {
        // Umgebungslicht halb entsättigt: nachts dunkel wie die Straße, aber ohne den bläulichen Mondton (blauer
        // Qualm las sich als Leuchteffekt am Rad)
        let lum = ambient[0] * 0.2126 + ambient[1] * 0.7152 + ambient[2] * 0.0722;
        let amb = ambient.map(|a| (a + lum) * 0.5);
        let lit = |c: [f32; 3]| [c[0] * amb[0], c[1] * amb[1], c[2] * amb[2]];
        // Reifenwolken: weicher Fleck, der aufquillt und vergeht
        for q in &self.particles {
            let age = 1. - q.life / q.max;
            let a = (std::f32::consts::PI * age).sin() * q.kind.alpha();
            let c = lit(q.kind.color());
            // Gischt: längliche Fahne in Flugrichtung, sonst runde Wolke
            let (half, angle) = if q.kind == Kick::Spray {
                ([q.size * 1.7, q.size * 0.6], q.v[1].atan2(q.v[0]))
            } else {
                ([q.size, q.size], 0.)
            };
            out.push(Body {
                center: q.at,
                half,
                angle,
                shape: 3.,
                depth: 0.59,
                color: [c[0], c[1], c[2], a],
            });
        }
        self.fire.effects(out, lit);
        self.gun.effects(out, lit);
    }
    /// Brennende und glimmende Wracks der Welt (jeden Schritt).
    pub fn fires(&mut self, w: &berlin_sim::world::World, dt: f32) {
        self.fire.tick(w, dt);
    }
    pub fn bodies(&self, out: &mut Vec<Body>) {
        // Bremsspuren: dunkle Gummistriche, die nach 8 s verblassen
        for (i, k) in self.skids.iter().enumerate() {
            let (dx, dy) = (k.b[0] - k.a[0], k.b[1] - k.a[1]);
            let len = dx.hypot(dy);
            if len < 0.3 {
                continue;
            }
            out.push(Body {
                center: [(k.a[0] + k.b[0]) / 2., (k.a[1] + k.b[1]) / 2.],
                half: [len / 2. + 0.6, 1.3],
                angle: dy.atan2(dx),
                shape: 4.,
                depth: 0.8395 + i as f32 * 1e-8,
                color: [0.06, 0.06, 0.065, 0.32 * (k.life / SKID_S).min(1.)],
            });
        }
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
        self.fire.bodies(out);
        self.gun.bodies(out);
        if let Some((at, t)) = self.ring {
            let k = t / RING_S;
            let r = 6. + (1. - k) * 8.;
            out.push(Body {
                center: at,
                half: [r, r * 0.75],
                angle: 0.,
                shape: 2.,
                depth: 0.82,
                color: [1., 0.83, 0.24, 0.85 * k],
            });
        }
    }
    /// Mündungsfeuer erhellt nachts kurz die Umgebung.
    pub fn lights(&self, out: &mut Vec<LightSource>, dark: f32, time: f32) {
        self.gun.lights(out, dark);
        self.fire.lights(out, dark, time);
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
        assert_eq!(out.len(), 7 + 1, "Blut und Einschussloch am Boden");
        let mut glow = Vec::new();
        fx.effects(&mut glow, [1.; 3]);
        assert!(
            glow.len() >= 2 + 4,
            "Bahnen, Feuer, Dampf, Staub im Effekt-Durchgang"
        );
        // Blutstropfen liegen in Schlagrichtung
        assert!(fx.splats.iter().all(|s| s.at[0] > 50.));
        fx.step(2.);
        assert_eq!(fx.blood_count(), 7, "das Blut bleibt");
        let mut glow = Vec::new();
        fx.effects(&mut glow, [1.; 3]);
        assert!(glow.is_empty(), "Bahnen, Feuer, Dampf und Staub sind weg");
        fx.step(BLOOD_KEEP);
        assert_eq!(fx.blood_count(), 0);
    }
}

#[cfg(test)]
mod tire_tests {
    use super::*;
    use berlin_sim::city::Ground as G;

    #[test]
    fn tire_clouds_draw_in_the_effect_pass_without_blue_tint() {
        let mut fx = Effects::default();
        for kind in [Kick::Smoke, Kick::Spray, Kick::Dust, Kick::Snow] {
            fx.add(Particle {
                kind,
                at: [0., 0.],
                v: [30., 0.],
                size: 4.,
                life: 0.5,
                max: 1.,
            });
            // kein Blaustich: Blau höchstens knapp über Rot (vorher Gischt 0,90 gegen 0,75)
            let c = kind.color();
            assert!(c[2] - c[0] < 0.05, "{kind:?} bläulich: {c:?}");
        }
        // Bodenspuren-Liste: keine Wolken (sie lagen dort vor dem Auto und lösten dessen Silhouette aus)
        let mut ground = Vec::new();
        fx.bodies(&mut ground);
        assert!(
            ground.iter().all(|b| b.shape != 3.),
            "Wolke in der Bodenspuren-Liste"
        );
        let mut day = Vec::new();
        fx.effects(&mut day, [1.; 3]);
        assert_eq!(day.iter().filter(|b| b.shape == 3.).count(), 4);
        // Gischt als Fahne in Flugrichtung
        let spray = day
            .iter()
            .find(|b| b.half[0] > b.half[1] * 2.)
            .expect("Gischtfahne");
        assert!(spray.angle.abs() < 1e-6);
        // nachts dunkler, und der bläuliche Mondton färbt nicht voll durch
        let mut night = Vec::new();
        fx.effects(&mut night, [0.2, 0.25, 0.4]);
        let (d, n) = (day[0].color, night[0].color);
        assert!(n[0] < d[0] * 0.5, "nachts dunkler");
        assert!(n[2] / n[0] < 0.4 / 0.2 * 0.8, "Mondton gedämpft: {n:?}");
    }

    #[test]
    fn tire_effect_by_ground_and_weather() {
        assert_eq!(
            tire_effect(G::Road, 0., 0., false),
            None,
            "trocken, ohne Drift: nichts"
        );
        assert_eq!(tire_effect(G::Road, 0., 0., true), Some(Kick::Smoke));
        assert_eq!(tire_effect(G::Grass, 0., 0., false), Some(Kick::Dust));
        assert_eq!(tire_effect(G::Road, 0.5, 0., false), Some(Kick::Spray));
        assert_eq!(tire_effect(G::Road, 0.5, 0.5, true), Some(Kick::Snow));
    }
}
