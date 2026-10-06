//! Klang je Simulationsschritt: aus der Welt die Parameter für den Synthesizer (wie `main.js` in der Browserfassung).
use berlin_audio::sampler::EngineFrame;
use berlin_audio::synth::{Frame, Sfx, Vehicle};
use berlin_sim::ambience::ambience_at;
use berlin_sim::car::Car;
use berlin_sim::enginesound::{
    EngineSound, Profile, SoundInput, SoundOut, config as sound_config, profile_for, select_voices,
    spatial,
};
use berlin_sim::events::Event;
use berlin_sim::soundscape::{
    EngineState, Voices, footstep_kind, step_engine, steps_between, tire_state,
};
use berlin_sim::world::World;
use std::collections::HashMap;

/// Hörweite für Ereignisklänge (px): weiter weg verklingt ein Unfall oder eine Hupe.
const EVENT_HEAR: f64 = 1400.;

/// Eingaben des Motorsound-Debug-Panels: ersetzen Drehzahl, Gas, Gang und Profil des eigenen Motors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineOverride {
    pub rpm: f64,
    pub throttle: f64,
    pub gear: usize,
    /// Index in `enginesound::config().all_profiles()`
    pub profile: usize,
}

/// Was der eigene Sample-Motor zuletzt gespielt hat (für das Debug-Panel).
#[derive(Debug, Clone, Default)]
pub struct EngineView {
    pub profile: String,
    pub out: SoundOut,
    pub voices: usize,
}

#[derive(Default)]
pub struct Listener {
    voices: Voices,
    engine: EngineState,
    engine_car: Option<u32>,
    /// Motoren aus Aufnahmen: eigener und fremde Zustände
    sample_own: Option<EngineSound>,
    sample_npc: HashMap<u32, (EngineSound, f64)>,
    /// Koop: Motor aus Aufnahmen für das Auto von Spieler 2 (eigene Stimme, nicht aus dem Verkehr)
    sample_p2: Option<(u32, EngineSound)>,
    /// brennende Wracks: wann das Knistern zuletzt neu angesetzt wurde (Spielzeit)
    crackle: HashMap<u32, f64>,
    pub engine_override: Option<EngineOverride>,
    pub reference: Option<std::sync::Arc<[f32]>>,
    pub engine_view: EngineView,
    prev_step: f64,
    thunder_t: Option<f64>,
    clock: Option<f64>,
    rail: crate::railaudio::RailAudio,
}

/// Eingaben für den Sample-Motor eines Autos: Drehzahl und Gang aus der Fahrphysik, sonst virtuelles Getriebe.
fn sound_input(c: &Car) -> SoundInput {
    let v = berlin_sim::car::vphys_vehicle(c);
    let phys = c.phys.as_deref();
    SoundInput {
        rpm: phys.map(|p| p.rpm),
        // Fahrphysik zählt ab 0, der Motorsound ab 1
        gear: phys.map(|p| p.gear + 1),
        limiter: v.map(|v| v.engine.n_max).filter(|n| *n > 0.),
        throttle: if c.wrecked { 0. } else { c.controls.throttle },
        speed: c.speed() / 10.,
        slip: phys.map_or(if c.spin > 0. { 0.6 } else { 0. }, |p| {
            p.slip[0].abs().max(p.slip[1].abs()).min(1.)
        }),
    }
}

impl Listener {
    /// Motorzustand des eigenen Fahrzeugs (für Protokolle).
    pub fn engine(&self) -> EngineState {
        self.engine
    }
    pub fn frame(&mut self, w: &mut World, dt: f64) -> Frame {
        let mut f = Frame::default();
        let (cx, cy) = (w.camera.x, w.camera.y);
        // Koop: was einer der beiden Spieler hört (allein genau der Abstand zur Kamera)
        let (foci, nf) = w.foci();
        let dist = |x: f64, y: f64| berlin_sim::coop::min_dist(&foci[..nf], x, y);
        let near = |x: f64, y: f64| (1. - dist(x, y) / EVENT_HEAR).clamp(0., 1.) as f32;
        for e in &w.events {
            let s = match *e {
                Event::Crash { x, y, strength, .. } => {
                    Some((Sfx::Crash(strength as f32), near(x, y)))
                }
                Event::Knock { x, y, .. } => Some((Sfx::Knock(1.), near(x, y))),
                Event::Curb { x, y, .. } => Some((Sfx::Knock(0.5), near(x, y))),
                Event::Horn { x, y, npc } => {
                    Some((Sfx::Horn(if npc { 0.6 } else { 1. }), near(x, y)))
                }
                Event::Door { x, y } => Some((Sfx::Door, near(x, y))),
                Event::Aquaplane { x, y, .. } => Some((Sfx::Splash(1.), near(x, y))),
                Event::TramBell { x, y } => Some((Sfx::TramBell(1.), near(x, y))),
                Event::Board { hop, .. } | Event::Alight { hop, .. } => {
                    Some((if hop { Sfx::Hit } else { Sfx::Door }, 1.))
                }
                Event::TrainTake { .. } => Some((Sfx::Door, 1.)),
                Event::StationEnter { .. }
                | Event::StationExit { .. }
                | Event::StationTransfer { .. } => Some((Sfx::Door, 1.)),
                Event::DoorsOpen { .. } => Some((Sfx::GongOpen, 1.)),
                Event::DoorsClose => Some((Sfx::GongClose, 1.)),
                Event::Tip { .. } => Some((Sfx::Pickup, 1.)),
                Event::TrainBlocked => Some((Sfx::TramBell(1.), 1.)),
                Event::BusStop { .. }
                | Event::BusBoard { .. }
                | Event::RideEnd { .. }
                | Event::TurnAround { .. } => None,
                Event::Carjack { x, y, .. } => Some((Sfx::Carjack, near(x, y))),
                Event::PassengersFell { x, y, .. } => Some((Sfx::Hit, near(x, y))),
                Event::BikeDown { x, y, .. } => Some((Sfx::Hit, near(x, y))),
                Event::Bump { .. } => Some((Sfx::Hit, 1.)),
                Event::Hit { x, y, .. } => Some((Sfx::Hit, near(x, y))),
                Event::Ui => Some((Sfx::Ui, 1.)),
                Event::Tick => Some((Sfx::Tick, 1.)),
                Event::Pickup => Some((Sfx::Pickup, 1.)),
                Event::MissionStart => Some((Sfx::MissionStart, 1.)),
                Event::MissionSuccess => Some((Sfx::MissionSuccess, 1.)),
                Event::MissionFail => Some((Sfx::MissionFail, 1.)),
                Event::Shot { x, y, weapon, .. } => {
                    let kind = gun_kind(weapon);
                    // Schüsse hört man weiter als einen Unfall
                    let k = (1. - dist(x, y) / (EVENT_HEAR * 2.)).clamp(0., 1.) as f32;
                    Some((Sfx::Gun(kind, 1.), k))
                }
                Event::Swing { x, y, hit, .. } => Some((
                    if hit { Sfx::Punch(1.) } else { Sfx::Swing(1.) },
                    near(x, y),
                )),
                Event::Thud { x, y } => Some((Sfx::Thud(1.), near(x, y))),
                Event::Impact { x, y, .. } => Some((Sfx::Impact(1.), near(x, y))),
                Event::PlayerHurt { .. } => Some((Sfx::Punch(1.), 1.)),
                Event::Wasted { .. } => Some((Sfx::MissionFail, 1.)),
                Event::Respawn { .. } => Some((Sfx::Pickup, 1.)),
                Event::Reload { weapon } => Some((Sfx::Reload(gun_kind(weapon)), 1.)),
                Event::Reloaded { weapon } => Some((Sfx::Reloaded(gun_kind(weapon)), 1.)),
                Event::WeaponSwitch { .. } => Some((Sfx::WeaponSwitch, 1.)),
                // Absprung und Landung: Schritte auf dem Untergrund (unten, braucht die Stadt)
                Event::Jump { .. } | Event::Land { .. } => None,
                // Knall: weit hörbar (wie ein Schuss), nah mit voller Wucht
                Event::Explosion { x, y, strength, .. } => {
                    let k = (1. - dist(x, y) / (EVENT_HEAR * 3.)).clamp(0., 1.) as f32;
                    Some((
                        Sfx::Explosion((0.7 + 0.3 * strength as f32) * k.powf(0.7)),
                        1.,
                    ))
                }
                Event::CarFire { x, y, .. } => Some((Sfx::FireCrackle(1.), near(x, y))),
                Event::Wreck { .. }
                | Event::Notice(_)
                | Event::Blood { .. }
                | Event::Kill { .. }
                | Event::WeaponHit { .. }
                | Event::PickupBody { .. } => None,
            };
            if let Some((sfx, gain)) = s
                && gain > 0.02
            {
                f.sfx.push(match sfx {
                    Sfx::Crash(k) => Sfx::Crash(k * gain),
                    Sfx::Horn(k) => Sfx::Horn(k * gain),
                    Sfx::Knock(k) => Sfx::Knock(k * gain),
                    Sfx::Gun(w, k) => Sfx::Gun(w, k * gain),
                    Sfx::Swing(k) => Sfx::Swing(k * gain),
                    Sfx::Punch(k) => Sfx::Punch(k * gain),
                    Sfx::Thud(k) => Sfx::Thud(k * gain),
                    Sfx::Impact(k) => Sfx::Impact(k * gain),
                    Sfx::FireCrackle(k) => Sfx::FireCrackle(k * gain),
                    s => s,
                });
            }
        }
        // brennende Wracks knistern, solange sie brennen (alle 2,2 s neu angesetzt, nach Entfernung leiser)
        let mut live = Vec::new();
        for c in &w.cars {
            if c.burn.is_none() || c.exploded {
                continue;
            }
            live.push(c.id);
            let g = near(c.x, c.y);
            let last = self.crackle.entry(c.id).or_insert(w.time);
            if w.time - *last >= 2.2 && g > 0.02 {
                *last = w.time;
                f.sfx.push(Sfx::FireCrackle(g));
            }
        }
        self.crackle.retain(|id, _| live.contains(id));
        // eigenes Fahrzeug
        let (wet, snow) = (w.weather.wet, w.weather.snow);
        let hops: Vec<(f64, f64, f32)> = w
            .events
            .iter()
            .filter_map(|e| match *e {
                Event::Jump { x, y } => Some((x, y, 0.7)),
                Event::Land { x, y } => Some((x, y, 1.4)),
                _ => None,
            })
            .collect();
        for (x, y, gain) in hops {
            let lvl = w.player.level.lvl;
            let kind = footstep_kind(w.city.surface_at(x, y, Some(lvl)), wet, snow);
            f.sfx.push(Sfx::Footstep(kind, gain));
        }
        let pc = w.player_car().cloned();
        let mix = &sound_config().mix;
        f.engine_mix = mix.engine as f32;
        f.reference = self.reference.clone();
        let mut own: Option<EngineFrame> = None;
        if let Some(c) = &pc {
            let fresh = self.engine_car != Some(c.id);
            if fresh {
                self.engine = EngineState::default();
                self.engine_car = Some(c.id);
                self.sample_own = None;
            }
            step_engine(&mut self.engine, c, dt);
            let ground = w.city.surface_at(c.x, c.y, Some(c.lvl()));
            let tires = tire_state(ground, wet, snow, c);
            let open = c.kind_info().bike || c.kind_info().moto;
            // Sportwagen mit Profil: Motor aus Aufnahmen statt Synthese (Reifen und Wind bleiben)
            let profile = if self.engine_override.is_none() {
                profile_for(c)
            } else {
                None
            };
            if let Some(p) = profile {
                let st = self
                    .sample_own
                    .get_or_insert_with(|| EngineSound::new(c.id));
                let mut out = st.step(p, sound_config().bank(&p.bank), &sound_input(c), dt, false);
                // Anlassen beim Einsteigen in ein stehendes Auto
                if fresh
                    && c.speed() < 10.
                    && let Some(s) = EngineSound::start_shot(sound_config().bank(&p.bank), p)
                {
                    out.shots.push(s);
                }
                self.engine_view = EngineView {
                    profile: p.name.clone(),
                    out: out.clone(),
                    voices: 0,
                };
                own = Some(EngineFrame {
                    id: c.id,
                    player: true,
                    out,
                    gain: if open { 1. } else { 0.85 },
                    pan: 0.,
                    rate: 1.,
                    // im geschlossenen Auto dämpft die Karosserie die Höhen
                    lowpass: if open { 16000. } else { 6500. },
                });
            }
            f.vehicle = Vehicle {
                active: true,
                in_car: !open,
                rain: w.sky.p.rain as f32,
                engine: (!c.kind_info().bike && own.is_none()).then_some(self.engine),
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
        // Debug-Panel: eigener Motor mit fest eingestellten Werten, auch zu Fuß
        if let Some(o) = self.engine_override {
            let all = sound_config().all_profiles();
            let p: &Profile = all[o.profile.min(all.len() - 1)];
            let st = self.sample_own.get_or_insert_with(|| EngineSound::new(1));
            let inp = SoundInput {
                rpm: Some(o.rpm),
                gear: Some(o.gear.max(1)),
                limiter: Some(p.begrenzer),
                throttle: o.throttle,
                speed: 20.,
                slip: 0.,
            };
            let out = st.step(p, sound_config().bank(&p.bank), &inp, dt, false);
            self.engine_view = EngineView {
                profile: p.name.clone(),
                out: out.clone(),
                voices: 0,
            };
            own = Some(EngineFrame {
                id: u32::MAX,
                player: true,
                out,
                gain: 1.,
                pan: 0.,
                rate: 1.,
                lowpass: 16000.,
            });
        }
        let (lvx, lvy) = pc.as_ref().map(|c| (c.vx, c.vy)).unwrap_or((0., 0.));
        // fremde Sportwagen mit Profil: Sample-Stimmen (die lautesten, das Spielerauto hat Vorrang)
        let own_id = pc.as_ref().map(|c| c.id);
        // Koop: das Auto von Spieler 2 klingt wie ein eigenes (nah, volle Stimme), seitlich nach seiner Lage
        let p2_car =
            w.p2.as_ref()
                .and_then(|s| s.player.in_car)
                .and_then(|id| w.car(id))
                .filter(|c| !c.wrecked && !c.kind_info().bike)
                .cloned();
        let p2_id = p2_car.as_ref().map(|c| c.id);
        let p2_pan = p2_car
            .as_ref()
            .map_or(0., |c| ((c.x - w.player.x) / 300.).clamp(-1., 1.) * 0.6);
        let mut p2_voice = None;
        match p2_car.as_ref().and_then(|c| profile_for(c).map(|p| (c, p))) {
            Some((c, p)) => {
                if self.sample_p2.as_ref().is_none_or(|(id, _)| *id != c.id) {
                    self.sample_p2 = Some((c.id, EngineSound::new(c.id)));
                }
                let st = &mut self.sample_p2.as_mut().expect("eben gesetzt").1;
                let out = st.step(p, sound_config().bank(&p.bank), &sound_input(c), dt, false);
                let open = c.kind_info().moto;
                f.engines.push(EngineFrame {
                    id: c.id,
                    player: false,
                    out,
                    gain: if open { 0.85 } else { 0.7 },
                    pan: p2_pan as f32,
                    rate: 1.,
                    lowpass: if open { 16000. } else { 9000. },
                });
            }
            None => {
                self.sample_p2 = None;
                if let Some(c) = &p2_car {
                    p2_voice = self
                        .voices
                        .voice(w, c, (c.x, c.y, c.vx, c.vy), 500.)
                        .map(|v| berlin_sim::soundscape::CarVoice { pan: p2_pan, ..v });
                }
            }
        }
        let mut cands = Vec::new();
        for c in &w.cars {
            if Some(c.id) == own_id
                || (p2_id.is_some() && Some(c.id) == p2_id)
                || c.wrecked
                || c.driver.is_none()
            {
                continue;
            }
            let (dx, dy) = (c.x - cx, c.y - cy);
            let d = dx.hypot(dy);
            if d >= mix.hoerweite_px {
                continue;
            }
            let Some(p) = profile_for(c) else {
                continue;
            };
            let dd = d.max(1.);
            let closing = -((c.vx - lvx) * dx / dd + (c.vy - lvy) * dy / dd);
            let sp = spatial(mix, d, dx, closing);
            cands.push((sp.gain, (c.id, p, sp, sound_input(c))));
        }
        let picked = select_voices(
            None,
            cands,
            mix.stimmen
                .saturating_sub(usize::from(own.is_some()) + f.engines.len()),
        );
        let mut sampled: Vec<u32> = Vec::new();
        for (id, p, sp, inp) in picked {
            let (st, at) = self
                .sample_npc
                .entry(id)
                .or_insert_with(|| (EngineSound::new(id), w.time));
            let step = (w.time - *at).clamp(1. / 120., 0.25);
            *at = w.time;
            let out = st.step(p, sound_config().bank(&p.bank), &inp, step, sp.lod);
            sampled.push(id);
            f.engines.push(EngineFrame {
                id,
                player: false,
                out,
                gain: sp.gain as f32,
                pan: sp.pan as f32,
                rate: sp.rate as f32,
                lowpass: sp.lowpass as f32,
            });
        }
        if self.sample_npc.len() > 64 {
            let live: Vec<u32> = w.cars.iter().map(|c| c.id).collect();
            self.sample_npc.retain(|id, _| live.contains(id));
        }
        self.engine_view.voices = f.engines.len() + usize::from(own.is_some());
        if let Some(o) = own {
            f.engines.insert(0, o);
        }
        f.voices = p2_voice
            .into_iter()
            .chain(
                self.voices
                    .near(w, (cx, cy, lvx, lvy), 12, 500., own_id)
                    .into_iter()
                    .filter(|v| p2_id.is_none() || Some(v.id) != p2_id)
                    .filter(|v| w.car(v.id).is_none_or(|c| profile_for(c).is_none())),
            )
            .take(4)
            .collect();
        // Donner: Einschläge seit dem letzten Bild (aus Seed und Zeit, wie der Blitz im Bild)
        let t0 = self.thunder_t.unwrap_or(w.time);
        for (loud, near) in
            berlin_sim::weather::thunder_between(w.seed, t0, w.time, w.sky.p.thunder)
        {
            f.sfx.push(Sfx::Thunder(loud as f32, near));
        }
        self.thunder_t = Some(w.time);
        // Kirchenglocke zur vollen Stunde, wenn eine Kirche in Hörweite steht
        let prev = self.clock.replace(w.clock).unwrap_or(w.clock);
        if prev != w.clock {
            use berlin_sim::ambience::{BELLS_HEAR, bell_strikes};
            let n = bell_strikes(prev, w.clock, true);
            if n > 0 && w.city.church_near(cx, cy, BELLS_HEAR) {
                f.sfx.push(Sfx::Bells(n, 1.));
            }
        }
        f.rail = self.rail.step(w, dt, &mut f.sfx);
        f.ambience = ambience_at(w);
        f
    }
}

/// Waffe → Klangnummer (Schuss und Nachladen): 0 Pistole, 1 MP, 2 Schrotflinte.
fn gun_kind(weapon: &str) -> u8 {
    match weapon {
        "pistol" => 0,
        "smg" => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use berlin_sim::city::{City, DiskSource};
    use berlin_sim::world::{DT, Input};

    /// Koop: das Auto von Spieler 2 hat eine eigene Motorstimme, auch wenn es weit von Spieler 1 entfernt ist.
    #[test]
    fn second_players_car_has_its_own_voice() {
        let root = berlin_map_loader::default_data_root();
        let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).unwrap();
        let mut w = World::new(city, 5, 22, 55);
        let mut l = Listener::default();
        assert!(w.join_p2());
        let pc = w.player_car_id.unwrap();
        let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
        {
            let s = w.p2.as_mut().unwrap();
            (s.player.x, s.player.y) = (x + 15., y);
        }
        let go = Input {
            enter_exit: true,
            ..Default::default()
        };
        w.update_coop(&Input::default(), &go, DT);
        assert_eq!(w.p2.as_ref().unwrap().player.in_car, Some(pc));
        // Spieler 1 geht weit weg: die Stimme bleibt, weil sie Spieler 2 gehört
        w.player.x -= 3000.;
        let gas = Input {
            throttle: 1.,
            ..Default::default()
        };
        let mut heard = false;
        for _ in 0..30 {
            w.update_coop(&Input::default(), &gas, DT);
            let f = l.frame(&mut w, DT);
            heard |= f.voices.first().is_some_and(|v| v.id == pc && v.gain > 0.3)
                || f.engines.iter().any(|e| e.id == pc);
        }
        assert!(heard, "Motor von Spieler 2 hörbar");
    }

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

    #[test]
    fn sports_cars_play_samples_instead_of_synthesis() {
        let root = berlin_map_loader::default_data_root();
        let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).unwrap();
        let mut w = World::new(city, 5, 22, 55);
        let mut l = Listener::default();
        let pc = w.player_car_id.unwrap();
        let v = berlin_sim::vehdata::game_vehicle("supercar_awd").unwrap();
        let (x, y) = {
            let c = w.cars.iter_mut().find(|c| c.id == pc).unwrap();
            c.model = Some(v.id.as_str());
            (c.x, c.y)
        };
        (w.player.x, w.player.y) = (x + 15., y);
        w.update(
            &Input {
                enter_exit: true,
                ..Default::default()
            },
            DT,
        );
        let f = l.frame(&mut w, DT);
        assert!(f.vehicle.active && f.vehicle.engine.is_none() && f.vehicle.tires.is_some());
        let own = &f.engines[0];
        assert!(own.player && own.id == pc);
        assert!(own.out.layers.iter().any(|x| x.gain > 0.1));
        // beim Einsteigen in den stehenden Wagen: Anlassen
        let bank = sound_config().bank("v10");
        assert!(
            own.out
                .shots
                .iter()
                .any(|s| bank.shots[s.0].kind == berlin_sim::enginesound::ShotKind::Start)
        );
        assert!(f.engine_mix > 0.);
        assert!(f.engines.len() <= sound_config().mix.stimmen);
        assert_eq!(l.engine_view.profile, "supercar");
        // Gas: die Drehzahl der Fahrphysik kommt im Klang an
        for _ in 0..90 {
            w.update(
                &Input {
                    throttle: 1.,
                    ..Default::default()
                },
                DT,
            );
            l.frame(&mut w, DT);
        }
        // gemessen wird bei stehendem Gang (direkt nach dem Schalten zieht der Klang der Drehzahl kurz nach)
        let gear = |w: &World| w.car(pc).and_then(|c| c.phys.as_ref().map(|p| p.gear));
        let mut steady = 0;
        let mut last = gear(&w);
        for _ in 0..240 {
            if steady >= 30 {
                break;
            }
            w.update(
                &Input {
                    throttle: 1.,
                    ..Default::default()
                },
                DT,
            );
            l.frame(&mut w, DT);
            let g = gear(&w);
            steady = if g == last { steady + 1 } else { 0 };
            last = g;
        }
        let phys = w
            .car(pc)
            .and_then(|c| c.phys.as_ref().map(|p| p.rpm))
            .unwrap();
        assert!(
            (l.engine_view.out.rpm - phys).abs() < phys * 0.2,
            "{} vs {phys}",
            l.engine_view.out.rpm
        );
    }

    #[test]
    fn thunder_and_rain_reach_the_mix() {
        let root = berlin_map_loader::default_data_root();
        let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).unwrap();
        let mut w = World::new(city, 5, 4, 4);
        w.force_weather = Some("thunder");
        let mut l = Listener::default();
        let mut thunder = 0;
        for _ in 0..(90. / DT) as usize {
            w.update(&Input::default(), DT);
            let f = l.frame(&mut w, DT);
            thunder += f
                .sfx
                .iter()
                .filter(|s| matches!(s, Sfx::Thunder(..)))
                .count();
            assert!(f.ambience.rain > 0.5);
        }
        assert!(
            (1..=40).contains(&thunder),
            "90 s Gewitter = {thunder} Donner"
        );
    }
}
