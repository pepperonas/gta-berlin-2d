//! Klang je Simulationsschritt: aus der Welt die Parameter für den Synthesizer (wie `main.js` in der Browserfassung).
use berlin_audio::synth::{Frame, Sfx, Vehicle};
use berlin_sim::ambience::ambience_at;
use berlin_sim::events::Event;
use berlin_sim::soundscape::{
    EngineState, Voices, footstep_kind, step_engine, steps_between, tire_state,
};
use berlin_sim::world::World;

/// Hörweite für Ereignisklänge (px): weiter weg verklingt ein Unfall oder eine Hupe.
const EVENT_HEAR: f64 = 1400.;

#[derive(Default)]
pub struct Listener {
    voices: Voices,
    engine: EngineState,
    engine_car: Option<u32>,
    prev_step: f64,
}

impl Listener {
    /// Motorzustand des eigenen Fahrzeugs (für Protokolle).
    pub fn engine(&self) -> EngineState {
        self.engine
    }
    pub fn frame(&mut self, w: &mut World, dt: f64) -> Frame {
        let mut f = Frame::default();
        let (cx, cy) = (w.camera.x, w.camera.y);
        let near = |x: f64, y: f64| (1. - (x - cx).hypot(y - cy) / EVENT_HEAR).clamp(0., 1.) as f32;
        for e in &w.events {
            let s = match *e {
                Event::Crash { x, y, strength, .. } => {
                    Some((Sfx::Crash(strength as f32), near(x, y)))
                }
                Event::Knock { x, y, .. } => Some((Sfx::Knock(1.), near(x, y))),
                Event::Horn { x, y, npc } => {
                    Some((Sfx::Horn(if npc { 0.6 } else { 1. }), near(x, y)))
                }
                Event::Door { x, y } => Some((Sfx::Door, near(x, y))),
                Event::Carjack { x, y } => Some((Sfx::Carjack, near(x, y))),
                Event::Bump { .. } => Some((Sfx::Hit, 1.)),
                Event::Hit { x, y, .. } => Some((Sfx::Hit, near(x, y))),
                Event::Ui => Some((Sfx::Ui, 1.)),
                Event::Tick => Some((Sfx::Tick, 1.)),
                Event::Pickup => Some((Sfx::Pickup, 1.)),
                Event::MissionStart => Some((Sfx::MissionStart, 1.)),
                Event::MissionSuccess => Some((Sfx::MissionSuccess, 1.)),
                Event::MissionFail => Some((Sfx::MissionFail, 1.)),
                Event::Wreck { .. } | Event::Notice(_) => None,
            };
            if let Some((sfx, gain)) = s
                && gain > 0.02
            {
                f.sfx.push(match sfx {
                    Sfx::Crash(k) => Sfx::Crash(k * gain),
                    Sfx::Horn(k) => Sfx::Horn(k * gain),
                    Sfx::Knock(k) => Sfx::Knock(k * gain),
                    s => s,
                });
            }
        }
        // eigenes Fahrzeug
        let (wet, snow) = (w.weather.wet, w.weather.snow);
        let pc = w.player_car().cloned();
        if let Some(c) = &pc {
            if self.engine_car != Some(c.id) {
                self.engine = EngineState::default();
                self.engine_car = Some(c.id);
            }
            step_engine(&mut self.engine, c, dt);
            let ground = w.city.surface_at(c.x, c.y, Some(c.lvl()));
            let tires = tire_state(ground, wet, snow, c);
            let open = c.kind_info().bike || c.kind_info().moto;
            f.vehicle = Vehicle {
                active: true,
                in_car: !open,
                rain: 0.,
                engine: (!c.kind_info().bike).then_some(self.engine),
                tires: Some(tires),
            };
            self.prev_step = w.player.step;
        } else {
            self.engine_car = None;
            // Schritte zu Fuß
            let p = &w.player;
            let n = steps_between(self.prev_step, p.step, p.move_speed);
            self.prev_step = p.step;
            if n > 0 {
                let (x, y, lvl) = (p.x, p.y, p.level.lvl);
                let kind = footstep_kind(w.city.surface_at(x, y, Some(lvl)), wet, snow);
                f.sfx.push(Sfx::Footstep(
                    kind,
                    if p.move_speed > 45. { 1.2 } else { 0.8 },
                ));
            }
        }
        let (lvx, lvy) = pc.as_ref().map(|c| (c.vx, c.vy)).unwrap_or((0., 0.));
        f.voices = self
            .voices
            .near(w, (cx, cy, lvx, lvy), 4, 500., pc.as_ref().map(|c| c.id));
        f.ambience = ambience_at(w);
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use berlin_sim::city::{City, DiskSource};
    use berlin_sim::world::{DT, Input};

    #[test]
    fn frames_follow_the_player() {
        let root = berlin_map_loader::default_data_root();
        let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).unwrap();
        let mut w = World::new(city, 5, 22, 55);
        let mut l = Listener::default();
        // zu Fuß: Schritte, keine Fahrzeugstimme
        let mut steps = 0;
        for _ in 0..120 {
            w.update(
                &Input {
                    move_x: 1.,
                    ..Default::default()
                },
                DT,
            );
            let f = l.frame(&mut w, DT);
            assert!(!f.vehicle.active);
            steps += f
                .sfx
                .iter()
                .filter(|s| matches!(s, Sfx::Footstep(..)))
                .count();
        }
        assert!((5..=12).contains(&steps), "2 s joggen = {steps} Schritte");
        // im eigenen Auto: Motor und Reifen, Tür beim Einsteigen
        let pc = w.player_car_id.unwrap();
        let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
        (w.player.x, w.player.y) = (x + 15., y);
        w.update(
            &Input {
                enter_exit: true,
                ..Default::default()
            },
            DT,
        );
        let f = l.frame(&mut w, DT);
        assert!(f.sfx.contains(&Sfx::Door));
        assert!(
            f.vehicle.active && f.vehicle.in_car && f.vehicle.engine.is_some_and(|e| e.cyl == 6)
        );
        assert!(f.ambience.in_car && f.ambience.muffle > 0.6);
        assert!(f.voices.len() <= 4 && f.voices.iter().all(|v| v.id != pc && v.gain >= 0.));
    }
}
