//! Spielbare Welt: verbindet die Simulation (`berlin-sim`) mit Fenster, Tasten und Zeichnen der Engine.
use anyhow::Result;
use berlin_engine::{Body, Game, KeyCode, Keys};
use berlin_sim::city::{City, ThreadedSource};
use berlin_sim::mission::{Outcome, State};
use berlin_sim::pedestrians::PedState;
use berlin_sim::save::{FileStorage, Storage, read_save, write_save};
use berlin_sim::world::{DT, Input, World};
use glam::Vec2;
use std::path::Path;

pub struct Play {
    pub world: World,
    storage: Option<FileStorage>,
    saved_for: u32,
}

impl Play {
    pub fn new(root: &Path, seed: u32, save: Option<FileStorage>) -> Result<Self> {
        let city = City::open(root, Box::new(ThreadedSource::new(root)?))?;
        let mut world = World::new(
            city,
            seed,
            berlin_sim::world::TRAFFIC_CARS,
            berlin_sim::world::TRAFFIC_PEDS,
        );
        if let Some(st) = &save
            && let Some(data) = read_save(st as &dyn Storage)
        {
            eprintln!("Spielstand geladen: {}", st.path.display());
            world.apply_save(data);
        }
        Ok(Self {
            world,
            storage: save,
            saved_for: 0,
        })
    }
    pub fn set_storage(&mut self, st: FileStorage) {
        self.storage = Some(st);
    }
    fn save(&mut self) {
        if let Some(st) = self.storage.as_mut() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as f64)
                .unwrap_or(0.);
            match write_save(st, &self.world.make_save(now)) {
                Ok(()) => eprintln!("Spielstand gespeichert: {}", st.path.display()),
                Err(e) => eprintln!("Speichern fehlgeschlagen: {e:#}"),
            }
        }
    }
}

/// Tasten → abstrakte Eingabe (Belegung wie `input.js`).
pub fn input_from(keys: &Keys, driving: bool) -> Input {
    let held = |a: KeyCode, b: KeyCode| keys.held.contains(&a) || keys.held.contains(&b);
    let pressed = |k: KeyCode| keys.pressed.contains(&k);
    let axis = |pos: bool, neg: bool| (pos as i32 - neg as i32) as f64;
    let lx = axis(
        held(KeyCode::KeyD, KeyCode::ArrowRight),
        held(KeyCode::KeyA, KeyCode::ArrowLeft),
    );
    let ly = axis(
        held(KeyCode::KeyS, KeyCode::ArrowDown),
        held(KeyCode::KeyW, KeyCode::ArrowUp),
    );
    let up = held(KeyCode::KeyW, KeyCode::ArrowUp);
    let down = held(KeyCode::KeyS, KeyCode::ArrowDown);
    Input {
        move_x: if driving { 0. } else { lx },
        move_y: if driving { 0. } else { ly },
        sprint: held(KeyCode::ShiftLeft, KeyCode::ShiftRight),
        walk_slow: held(KeyCode::AltLeft, KeyCode::AltRight),
        throttle: if driving && up { 1. } else { 0. },
        brake: if driving && down { 1. } else { 0. },
        steer: if driving { lx } else { 0. },
        handbrake: driving && keys.held.contains(&KeyCode::Space),
        horn: keys.held.contains(&KeyCode::KeyH),
        enter_exit: pressed(KeyCode::KeyF),
        action: pressed(KeyCode::KeyE),
        action_held: keys.held.contains(&KeyCode::KeyE),
        esp_toggle: pressed(KeyCode::KeyX),
        abs_toggle: pressed(KeyCode::KeyY) || pressed(KeyCode::KeyZ),
    }
}

fn rgba(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 255) as f32 / 255.,
        ((rgb >> 8) & 255) as f32 / 255.,
        (rgb & 255) as f32 / 255.,
        a,
    ]
}
fn shade(c: [f32; 4], k: f32) -> [f32; 4] {
    [
        (c[0] * k).min(1.),
        (c[1] * k).min(1.),
        (c[2] * k).min(1.),
        c[3],
    ]
}

impl Game for Play {
    fn step_seconds(&self) -> f64 {
        DT
    }
    fn step(&mut self, keys: &Keys, dt: f64) {
        if keys.pressed.contains(&KeyCode::F5) {
            self.save();
        }
        let w2 = &mut self.world;
        // Ergebnis bestätigen: neuer Auftrag
        if matches!(w2.mission.state, State::Success | State::Failed)
            && keys.pressed.contains(&KeyCode::KeyE)
        {
            w2.restart_mission();
            return;
        }
        let input = input_from(keys, w2.player.in_car.is_some());
        w2.update(&input, dt);
        // nach einem erledigten Auftrag automatisch speichern (wie die JS-Fassung)
        if self.world.mission.state == State::Success
            && self.saved_for != self.world.completed as u32
        {
            self.saved_for = self.world.completed as u32;
            self.save();
        }
    }
    fn camera(&self) -> (Vec2, f32) {
        let c = self.world.camera;
        (Vec2::new(c.x as f32, c.y as f32), c.zoom as f32)
    }
    fn bodies(&self, out: &mut Vec<Body>) {
        let w = &self.world;
        let (cx, cy) = (w.camera.x, w.camera.y);
        let near = |x: f64, y: f64| (x - cx).abs() < 2600. && (y - cy).abs() < 1800.;
        // Missionsziel als Ring am Boden
        let pv = berlin_sim::mission::PlayerView {
            x: w.player.x,
            y: w.player.y,
            in_car: w.player.in_car,
        };
        let (_, target) = w.mission.objective(&w.city.places, pv, &w.cars);
        if let Some((tx, ty)) = target {
            let pulse = 1. + 0.08 * (w.time * 4.).sin() as f32;
            out.push(Body {
                center: [tx as f32, ty as f32],
                half: [46. * pulse, 46. * pulse],
                angle: 0.,
                shape: 2.,
                depth: 0.84,
                color: [1., 0.82, 0.18, 0.9],
            });
        }
        for c in w.cars.iter().filter(|c| near(c.x, c.y)) {
            let depth = if c.lvl() >= 1 { 0.55 } else { 0.62 };
            let mut color = rgba(c.color, 1.);
            if c.wrecked {
                color = shade(color, 0.35);
            }
            let (x, y, a) = (c.x as f32, c.y as f32, c.angle as f32);
            let (hw, hh) = (c.hw as f32, c.hh as f32);
            let (fx, fy) = (a.cos(), a.sin());
            // Schatten, Karosserie, Dach, Frontscheibe
            out.push(Body {
                center: [x + 2., y + 3.],
                half: [hw + 1., hh + 1.],
                angle: a,
                shape: 0.,
                depth: depth + 0.0004,
                color: [0., 0., 0., 0.28],
            });
            out.push(Body {
                center: [x, y],
                half: [hw, hh],
                angle: a,
                shape: 0.,
                depth,
                color,
            });
            if c.kind_info().bike || c.kind_info().moto {
                continue;
            }
            let roof = shade(color, 1.18);
            out.push(Body {
                center: [x - fx * hw * 0.1, y - fy * hw * 0.1],
                half: [hw * 0.45, hh * 0.78],
                angle: a,
                shape: 0.,
                depth: depth - 0.0002,
                color: roof,
            });
            out.push(Body {
                center: [x + fx * hw * 0.42, y + fy * hw * 0.42],
                half: [hw * 0.12, hh * 0.8],
                angle: a,
                shape: 0.,
                depth: depth - 0.0003,
                color: [0.16, 0.2, 0.24, 1.],
            });
            // Bremslichter
            if c.controls.brake > 0. && c.speed() > 2.
                || c.driver.is_some() && c.forward_speed() < -2.
            {
                for side in [-1f32, 1.] {
                    let (rx, ry) = (-fy, fx);
                    out.push(Body {
                        center: [
                            x - fx * hw * 0.92 + rx * hh * 0.7 * side,
                            y - fy * hw * 0.92 + ry * hh * 0.7 * side,
                        ],
                        half: [2., 2.],
                        angle: a,
                        shape: 1.,
                        depth: depth - 0.0004,
                        color: [1., 0.15, 0.1, 1.],
                    });
                }
            }
            if c.cargo {
                out.push(Body {
                    center: [x - fx * hw * 0.55, y - fy * hw * 0.55],
                    half: [hw * 0.22, hh * 0.6],
                    angle: a,
                    shape: 0.,
                    depth: depth - 0.0005,
                    color: rgba(0x8d6e4b, 1.),
                });
            }
        }
        for p in w.peds.iter().filter(|p| near(p.x, p.y)) {
            let depth = if p.level.lvl >= 1 { 0.549 } else { 0.618 };
            let (x, y, a) = (p.x as f32, p.y as f32, p.facing as f32);
            if p.state == PedState::Dead {
                out.push(Body {
                    center: [x, y],
                    half: [9., 4.],
                    angle: a,
                    shape: 1.,
                    depth: depth + 0.001,
                    color: [0.45, 0.05, 0.05, 0.9],
                });
                continue;
            }
            out.push(Body {
                center: [x + 1.5, y + 2.],
                half: [6., 6.],
                angle: 0.,
                shape: 1.,
                depth: depth + 0.0003,
                color: [0., 0., 0., 0.25],
            });
            out.push(Body {
                center: [x, y],
                half: [4.5, 6.5],
                angle: a,
                shape: 1.,
                depth,
                color: rgba(p.shirt, 1.),
            });
            out.push(Body {
                center: [x, y],
                half: [3., 3.],
                angle: 0.,
                shape: 1.,
                depth: depth - 0.0002,
                color: rgba(p.skin, 1.),
            });
        }
        if w.player.in_car.is_none() {
            let (x, y, a) = (w.player.x as f32, w.player.y as f32, w.player.angle as f32);
            let depth = if w.player.level.lvl >= 1 {
                0.548
            } else {
                0.617
            };
            out.push(Body {
                center: [x, y],
                half: [11., 11.],
                angle: 0.,
                shape: 2.,
                depth: depth + 0.0005,
                color: [0.25, 0.85, 1., 0.9],
            });
            out.push(Body {
                center: [x, y],
                half: [5., 7.],
                angle: a,
                shape: 1.,
                depth,
                color: rgba(0x2b2f3a, 1.),
            });
            out.push(Body {
                center: [x, y],
                half: [3.2, 3.2],
                angle: 0.,
                shape: 1.,
                depth: depth - 0.0002,
                color: rgba(0xe0ac69, 1.),
            });
        }
    }
    fn status(&self) -> String {
        let w = &self.world;
        if w.loading {
            return "Berlin wird geladen …".into();
        }
        let pv = berlin_sim::mission::PlayerView {
            x: w.player.x,
            y: w.player.y,
            in_car: w.player.in_car,
        };
        let (text, _) = w.mission.objective(&w.city.places, pv, &w.cars);
        let mut parts = vec![format!("{} €", w.money as i64)];
        match &w.mission.state {
            State::ToPickup | State::ToDropoff => {
                parts.push(format!("{} · {:.0} s", text, w.mission.timer))
            }
            State::Briefing => parts.push(format!(
                "{} (E: annehmen)",
                berlin_sim::mission::BRIEFING[1].trim_matches('„')
            )),
            State::Success => {
                if let Some(Outcome::Success { reward, .. }) = &w.mission.result {
                    parts.push(format!("Geschafft! +{reward} € (E: neuer Auftrag)"));
                }
            }
            State::Failed => {
                if let Some(Outcome::Failed { reason }) = &w.mission.result {
                    parts.push(format!("{reason} (E: neuer Versuch)"));
                }
            }
            State::Available => parts.push(text.into()),
        }
        if let Some(p) = w.mission.prompt {
            parts.push(p.into());
        }
        if let Some(c) = w.player_car() {
            parts.push(format!(
                "{:.0} km/h · {}",
                c.speed() * 0.36,
                berlin_sim::carmodels::vehicle_name(c.model_name())
            ));
        }
        if let Some(n) = &w.notice {
            parts.push(n.text.clone());
        }
        parts.join(" · ")
    }
    fn ready(&self) -> bool {
        !self.world.loading
    }
}
