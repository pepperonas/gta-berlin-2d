//! Spielbare Welt: verbindet die Simulation (`berlin-sim`) mit Fenster, Tasten und Zeichnen der Engine.
use anyhow::Result;
use berlin_engine::{Body, Game, KeyCode, Keys, LightSource, Lighting};
use berlin_sim::city::{City, ThreadedSource};
use berlin_sim::daylight::{format_clock, light_at};
use berlin_sim::lamps::LampCache;
use berlin_sim::mission::{Outcome, State};
use berlin_sim::pedestrians::PedState;
use berlin_sim::roadgraph::{Light, signal_state};
use berlin_sim::save::{FileStorage, Storage, read_save, write_save};
use berlin_sim::world::{DT, Input, World};
use glam::Vec2;
use std::path::Path;

pub struct Play {
    pub world: World,
    storage: Option<FileStorage>,
    saved_for: u32,
    lamps: LampCache,
    audio: Option<berlin_audio::output::Audio>,
    listener: crate::sound::Listener,
    /// beim ersten geladenen Schritt ins eigene Auto setzen (`--im-auto`)
    pub auto_enter: bool,
    pub bigmap: crate::bigmap::BigMap,
    /// HUD-Breite des letzten Bildes (Basiseinheiten), für die Kartenbedienung im Simulationsschritt
    hud_width: f32,
    pub screen: Screen,
    menu: crate::menu::Menu,
    root: std::path::PathBuf,
    seed: u32,
    /// Stick-Stellung des letzten Schritts (Menüauswahl per Stick als Flanke)
    stick_prev: f32,
    quit: bool,
    /// Statistik: dieses Spiel, insgesamt, Stand des gespeicherten Spiels (für „Fortsetzen“)
    pub stats: berlin_sim::stats::Stats,
    pub stats_total: berlin_sim::stats::Stats,
    stats_saved: berlin_sim::stats::Stats,
    tracker: berlin_sim::stats::Tracker,
    stats_written: f64,
    result_menu: Option<crate::menu::Menu>,
    trigger_was: bool,
    /// Aufnahme-Option `--kampf-demo`: schießt mit der Pistole auf den nächsten Passanten
    pub demo_combat: bool,
    /// zuletzt mit der Maus gezielt (sonst Controller); Zeiger im HUD für das Fadenkreuz
    mouse_aim: bool,
    cursor: Option<Vec2>,
    /// Kurzlebige Effekte (Mündungsfeuer, Leuchtspuren, Blut)
    pub fx: crate::effects::Effects,
}

/// Statistikdatei neben dem Spielstand: `{"total": …, "saved": …}`.
fn stats_path(st: &FileStorage) -> std::path::PathBuf {
    st.path.with_file_name("stats.json")
}
fn read_stats(st: &FileStorage) -> (berlin_sim::stats::Stats, berlin_sim::stats::Stats) {
    use berlin_sim::stats::Stats;
    let v: serde_json::Value = std::fs::read(stats_path(st))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    (Stats::from_json(&v["total"]), Stats::from_json(&v["saved"]))
}

/// Bildschirm (game.js): Titel mit laufender Stadt dahinter, Spiel, Pause, Steuerungstafel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Title,
    Playing,
    Paused,
    /// Steuerung bzw. Statistik; merkt sich, ob „Zurück“ zum Titel führt
    Controls(bool),
    Stats(bool),
}

/// Wie das Programm startet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    Title,
    New,
    Continue,
}

fn fresh_world(root: &Path, seed: u32) -> Result<World> {
    let city = City::open(root, Box::new(ThreadedSource::new(root)?))?;
    Ok(World::new(
        city,
        seed,
        berlin_sim::world::TRAFFIC_CARS,
        berlin_sim::world::TRAFFIC_PEDS,
    ))
}

/// Licht der Engine aus dem Tageslicht der Spieluhr.
pub fn lighting_at(minutes: f64) -> Lighting {
    lighting_of(&light_at(minutes))
}
/// Tageslicht der Welt mit Wetter (Wolken nehmen Schatten, Regen/Nebel machen den Tag grau, Schnee hellt auf).
pub fn world_light(w: &World) -> berlin_sim::daylight::Light {
    berlin_sim::weather::weather_light(&light_at(w.clock), &w.sky.p, w.weather.snow)
}
pub fn lighting_of(l: &berlin_sim::daylight::Light) -> Lighting {
    let (az, el) = (l.azimuth as f32, l.elevation as f32);
    Lighting {
        sun: [az.sin() * el.cos(), -az.cos() * el.cos(), el.sin()],
        shadow: [l.sun.dx as f32, l.sun.dy as f32],
        shadow_len: l.sun.len as f32,
        shadow_strength: l.sun.strength as f32,
        ambient: l.ambient.map(|c| c as f32),
        dark: l.dark as f32,
    }
}

impl Play {
    pub fn new(
        root: &Path,
        seed: u32,
        save: Option<FileStorage>,
        sound: bool,
        start: Start,
    ) -> Result<Self> {
        let mut world = fresh_world(root, seed)?;
        let saved = save.as_ref().and_then(|st| read_save(st as &dyn Storage));
        let screen = match (start, saved) {
            (Start::Title, _) => Screen::Title,
            (Start::Continue, Some(data)) => {
                eprintln!("Spielstand geladen");
                world.apply_save(data);
                Screen::Playing
            }
            _ => Screen::Playing,
        };
        let has_save = save
            .as_ref()
            .is_some_and(|st| read_save(st as &dyn Storage).is_some());
        let (stats_total, stats_saved) = save.as_ref().map(read_stats).unwrap_or_default();
        let stats = if start == Start::Continue && screen == Screen::Playing && has_save {
            stats_saved.clone()
        } else {
            Default::default()
        };
        let bigmap = match berlin_map_loader::overview::Overview::read(root) {
            Ok(ov) => crate::bigmap::BigMap::new(ov),
            Err(e) => {
                eprintln!("Stadtplan nicht verfügbar: {e:#}");
                crate::bigmap::BigMap::default()
            }
        };
        Ok(Self {
            bigmap,
            hud_width: 1280.,
            world,
            storage: save,
            saved_for: 0,
            lamps: LampCache::default(),
            audio: if sound {
                match berlin_audio::output::Audio::start() {
                    Ok(a) => {
                        eprintln!("Ton: {} ({} Hz)", a.device, a.sample_rate);
                        Some(a)
                    }
                    Err(e) => {
                        eprintln!("Ton aus: {e:#}");
                        None
                    }
                }
            } else {
                None
            },
            listener: Default::default(),
            auto_enter: false,
            screen,
            menu: crate::menu::title_menu(has_save),
            root: root.to_path_buf(),
            seed,
            stick_prev: 0.,
            quit: false,
            stats,
            stats_total,
            stats_saved,
            tracker: Default::default(),
            stats_written: 0.,
            result_menu: None,
            trigger_was: false,
            demo_combat: false,
            mouse_aim: false,
            cursor: None,
            fx: Default::default(),
        })
    }
    fn save(&mut self) -> bool {
        let Some(st) = self.storage.as_mut() else {
            return false;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.);
        match write_save(st, &self.world.make_save(now)) {
            Ok(()) => {
                eprintln!("Spielstand gespeichert: {}", st.path.display());
                self.stats_saved = self.stats.clone();
                self.write_stats();
                true
            }
            Err(e) => {
                eprintln!("Speichern fehlgeschlagen: {e:#}");
                false
            }
        }
    }
    /// Statistik sichern (insgesamt + Stand des gespeicherten Spiels), atomar ersetzt.
    pub fn write_stats(&mut self) {
        let Some(st) = &self.storage else {
            return;
        };
        let path = stats_path(st);
        let v = serde_json::json!({ "total": self.stats_total.to_json(), "saved": self.stats_saved.to_json() });
        let tmp = path.with_extension("json.tmp");
        let ok = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(&tmp, v.to_string()))
            .and_then(|_| std::fs::rename(&tmp, &path));
        if let Err(e) = ok {
            eprintln!("Statistik nicht gespeichert: {e}");
        }
        self.stats_written = self.world.time;
    }
    fn has_save(&self) -> bool {
        self.storage
            .as_ref()
            .is_some_and(|st| read_save(st as &dyn Storage).is_some())
    }
    /// Neues Spiel oder gespeichertes fortsetzen: frische Welt (eigene Stadt-Kacheln), Klang zurücksetzen.
    pub fn start(&mut self, resume: bool) {
        match fresh_world(&self.root, self.seed) {
            Ok(mut w) => {
                if resume
                    && let Some(data) = self
                        .storage
                        .as_ref()
                        .and_then(|st| read_save(st as &dyn Storage))
                {
                    w.apply_save(data);
                    w.notice = Some(berlin_sim::world::Notice {
                        text: "Spielstand geladen".into(),
                        t: 2.,
                    });
                }
                w.force_weather = self.world.force_weather;
                self.world = w;
                self.stats = if resume {
                    self.stats_saved.clone()
                } else {
                    Default::default()
                };
                self.tracker = Default::default();
                self.result_menu = None;
                self.listener = Default::default();
                self.lamps = LampCache::default();
                self.saved_for = self.world.completed as u32;
                self.bigmap.open = false;
                self.screen = Screen::Playing;
            }
            Err(e) => eprintln!("Neue Welt nicht ladbar: {e:#}"),
        }
    }
    fn free_play(&mut self) {
        self.world.mission.reset();
        for c in &mut self.world.cars {
            c.cargo = false;
        }
        self.result_menu = None;
    }
    pub fn pause(&mut self) {
        self.screen = Screen::Paused;
        self.menu = crate::menu::pause_menu();
    }
    fn ui_sound(&self) {
        if let Some(a) = &self.audio {
            a.play(berlin_audio::synth::Sfx::Ui);
        }
    }
    /// Menübildschirme; `true` = der Schritt ist damit erledigt.
    fn step_screens(&mut self, keys: &Keys, dt: f64) -> bool {
        use crate::menu::{Action, Pick};
        let mk = crate::menu::MenuKeys::from(keys, self.stick_prev);
        self.stick_prev = keys.pad.ly;
        // solange Kacheln fehlen, steht die Welt ohnehin still; weiterladen auch in den Menüs
        if self.world.loading
            && matches!(
                self.screen,
                Screen::Paused | Screen::Controls(false) | Screen::Stats(false)
            )
        {
            self.world.update(&Input::default(), dt);
        }
        match self.screen {
            Screen::Playing => {
                let pause = keys.pressed.contains(&KeyCode::Escape)
                    || keys.pressed.contains(&KeyCode::KeyP)
                    || keys.pad_pressed.menu;
                if pause && self.bigmap.open && !keys.pad_pressed.menu {
                    self.bigmap.open = false;
                    return false;
                }
                if pause && !self.world.loading {
                    self.pause();
                    self.ui_sound();
                    if let Some(a) = &self.audio {
                        a.apply(&berlin_audio::synth::Frame::default());
                    }
                    return true;
                }
                // Ergebnis eines Auftrags: Welt steht, Auswahl wie game.js resultMenu
                let state = self.world.mission.state;
                if matches!(state, State::Success | State::Failed) {
                    let menu = self
                        .result_menu
                        .get_or_insert_with(|| crate::menu::result_menu(state == State::Success));
                    let y = if state == State::Success { 400. } else { 330. };
                    let mp = menu.mouse(self.hud_width / 2., y, &keys.mouse);
                    let pick = mp.or(menu.input(mk));
                    let success = state == State::Success;
                    match pick {
                        Some(Pick::Choose(Action::Retry)) => {
                            self.world.restart_mission();
                            self.result_menu = None;
                        }
                        // Auftrag zurücksetzen, der Spieler bleibt, wo er ist (Zurück nur nach Erfolg)
                        Some(Pick::Choose(Action::Next | Action::Free)) => self.free_play(),
                        Some(Pick::Back) if success => self.free_play(),
                        _ => {}
                    }
                    if pick.is_some() {
                        self.ui_sound();
                    }
                    if let Some(a) = &self.audio {
                        a.apply(&berlin_audio::synth::Frame::default());
                    }
                    return true;
                }
                self.result_menu = None;
                false
            }
            Screen::Title => {
                if !self.world.loading {
                    let mp = self.menu.mouse(self.hud_width / 2., 350., &keys.mouse);
                    match mp.or(self.menu.input(mk)) {
                        Some(Pick::Choose(Action::Continue)) => {
                            self.ui_sound();
                            self.start(true)
                        }
                        Some(Pick::Choose(Action::New)) => {
                            self.ui_sound();
                            self.start(false)
                        }
                        Some(Pick::Choose(Action::Controls)) => {
                            self.screen = Screen::Controls(true)
                        }
                        Some(Pick::Choose(Action::Stats)) => self.screen = Screen::Stats(true),
                        Some(Pick::Choose(Action::Quit)) => {
                            self.write_stats();
                            self.quit = true
                        }
                        Some(_) => self.ui_sound(),
                        None => {}
                    }
                }
                // die Stadt hinter dem Titel lebt weiter (ohne Spieler-Eingaben)
                if self.screen == Screen::Title {
                    self.world.update(&Input::default(), dt);
                    let frame = self.listener.frame(&mut self.world, dt);
                    if let Some(a) = &self.audio {
                        a.apply(&frame);
                    }
                }
                true
            }
            Screen::Paused => {
                let mp = self.menu.mouse(self.hud_width / 2., 280., &keys.mouse);
                match mp.or(self.menu.input(mk)) {
                    Some(Pick::Back | Pick::Choose(Action::Resume)) => {
                        self.screen = Screen::Playing
                    }
                    Some(Pick::Choose(Action::Save)) => {
                        let ok = self.save();
                        self.world.notice = Some(berlin_sim::world::Notice {
                            text: (if ok {
                                "Spiel gespeichert"
                            } else {
                                "Speichern fehlgeschlagen"
                            })
                            .into(),
                            t: 2.,
                        });
                        self.screen = Screen::Playing;
                    }
                    Some(Pick::Choose(Action::Restart)) => {
                        self.world.restart_mission();
                        self.world.notice = Some(berlin_sim::world::Notice {
                            text: "Mission neu gestartet".into(),
                            t: 2.,
                        });
                        self.screen = Screen::Playing;
                    }
                    Some(Pick::Choose(Action::Controls)) => self.screen = Screen::Controls(false),
                    Some(Pick::Choose(Action::Stats)) => self.screen = Screen::Stats(false),
                    Some(Pick::Choose(Action::Title)) => {
                        self.write_stats();
                        self.screen = Screen::Title;
                        self.menu = crate::menu::title_menu(self.has_save());
                    }
                    _ => {}
                }
                if mk.up || mk.down || mk.confirm || mk.back || mp.is_some() {
                    self.ui_sound();
                }
                true
            }
            Screen::Controls(from_title) | Screen::Stats(from_title) => {
                let click = keys.mouse.left_pressed || keys.mouse.right_pressed;
                if mk.back || mk.confirm || click {
                    self.ui_sound();
                    self.screen = if from_title {
                        Screen::Title
                    } else {
                        Screen::Paused
                    };
                }
                if from_title {
                    self.world.update(&Input::default(), dt);
                }
                true
            }
        }
    }
}

/// Radiale Totzone (input.js radialDeadzone): kleine Ausschläge zählen nicht, darüber linear auf 0…1.
pub fn radial_deadzone(x: f32, y: f32, dz: f32) -> (f32, f32) {
    let m = x.hypot(y);
    if m < dz {
        return (0., 0.);
    }
    let k = ((m - dz) / (1. - dz)).min(1.) / m;
    (x * k, y * k)
}

/// Tasten und Gamepad → abstrakte Eingabe (Belegung wie `input.js`).
pub fn input_from(keys: &Keys, driving: bool) -> Input {
    let held = |a: KeyCode, b: KeyCode| keys.held.contains(&a) || keys.held.contains(&b);
    let pressed = |k: KeyCode| keys.pressed.contains(&k);
    let axis = |pos: bool, neg: bool| (pos as i32 - neg as i32) as f64;
    let (p, pe) = (&keys.pad, &keys.pad_pressed);
    let (plx, ply) = radial_deadzone(p.lx, p.ly, 0.22);
    let kx = axis(
        held(KeyCode::KeyD, KeyCode::ArrowRight),
        held(KeyCode::KeyA, KeyCode::ArrowLeft),
    );
    let ky = axis(
        held(KeyCode::KeyS, KeyCode::ArrowDown),
        held(KeyCode::KeyW, KeyCode::ArrowUp),
    );
    // Stick überstimmt die Tasten nur, wenn er ausgelenkt ist
    let lx = if plx != 0. { plx as f64 } else { kx };
    let ly = if ply != 0. { ply as f64 } else { ky };
    let up = held(KeyCode::KeyW, KeyCode::ArrowUp);
    let down = held(KeyCode::KeyS, KeyCode::ArrowDown);
    Input {
        move_x: if driving { 0. } else { lx },
        move_y: if driving { 0. } else { ly },
        sprint: held(KeyCode::ShiftLeft, KeyCode::ShiftRight) || (!driving && p.a),
        walk_slow: held(KeyCode::AltLeft, KeyCode::AltRight),
        throttle: if driving {
            f64::from(u8::from(up)).max(p.rt as f64)
        } else {
            0.
        },
        brake: if driving {
            f64::from(u8::from(down)).max(p.lt as f64)
        } else {
            0.
        },
        steer: if driving { lx.clamp(-1., 1.) } else { 0. },
        handbrake: driving && (keys.held.contains(&KeyCode::Space) || p.rb || p.b),
        horn: keys.held.contains(&KeyCode::KeyH) || p.x,
        enter_exit: pressed(KeyCode::KeyF) || pe.y,
        action: pressed(KeyCode::KeyE) || pe.a,
        action_held: keys.held.contains(&KeyCode::KeyE) || p.a,
        esp_toggle: pressed(KeyCode::KeyX),
        abs_toggle: pressed(KeyCode::KeyY) || pressed(KeyCode::KeyZ),
        combat: combat_input(keys, driving),
    }
}

/// Kampf zu Fuß (input.js): Strg/RT feuert, V/B tritt, R/X lädt nach, Q/RB nächste, LB vorige Waffe, 1–6 direkt,
/// rechter Stick zielt. Die Flanke des Triggers ergänzt `Play::step` (der Stand des letzten Schritts liegt dort).
pub fn combat_input(keys: &Keys, driving: bool) -> berlin_sim::combat::CombatInput {
    if driving {
        return Default::default();
    }
    let pressed = |k: KeyCode| keys.pressed.contains(&k);
    let (p, pe) = (&keys.pad, &keys.pad_pressed);
    let ctrl =
        keys.held.contains(&KeyCode::ControlLeft) || keys.held.contains(&KeyCode::ControlRight);
    let (rx, ry) = radial_deadzone(p.rx, p.ry, 0.22);
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ];
    berlin_sim::combat::CombatInput {
        fire: ctrl || p.rt > 0.5,
        fire_pressed: pressed(KeyCode::ControlLeft) || pressed(KeyCode::ControlRight),
        kick: pressed(KeyCode::KeyV) || pe.b,
        reload: pressed(KeyCode::KeyR) || pe.x,
        weapon_next: pressed(KeyCode::KeyQ) || pe.rb,
        weapon_prev: pe.lb,
        weapon_slot: digits
            .iter()
            .position(|&d| pressed(d))
            .map_or(0, |i| i as u8 + 1),
        aim_x: rx as f64,
        aim_y: ry as f64,
        aim_world: None,
    }
}

/// `--kampf-demo`: Pistole ziehen, auf den nächsten lebenden Passanten zielen und jeden zweiten Schritt abdrücken.
fn demo_combat_input(w: &World, input: &mut Input) {
    let (x, y) = (w.player.x, w.player.y);
    let target = w
        .peds
        .iter()
        .filter(|p| p.state != PedState::Dead && p.level.lvl == w.player.level.lvl)
        .min_by(|a, b| {
            (a.x - x)
                .hypot(a.y - y)
                .total_cmp(&(b.x - x).hypot(b.y - y))
        });
    input.combat.weapon_slot = if w.player.combat.weapon == 3 { 0 } else { 4 };
    if let Some(t) = target.filter(|t| (t.x - x).hypot(t.y - y) < 400.) {
        let a = (t.y - y).atan2(t.x - x);
        input.combat.aim_x = a.cos();
        input.combat.aim_y = a.sin();
        let fire = (w.time * 60.).round() as i64 % 12 == 0;
        input.combat.fire = fire;
        input.combat.fire_pressed = fire;
    }
}

/// Waffe in der Hand der Spielfigur (vor dem Körper in Zielrichtung) und Schlagbewegung.
fn weapon_bodies(
    c: &berlin_sim::combat::Combat,
    at: (f32, f32),
    a: f32,
    depth: f32,
    out: &mut Vec<Body>,
) {
    use berlin_sim::combat::AttackKind;
    let wp = c.weapon();
    let (fx, fy) = (a.cos(), a.sin());
    // Ausholen: beim Schlag schwingt die Waffe ein Stück herum, beim Tritt schiebt sich ein Fuß nach vorn
    let swing = c
        .attack
        .filter(|t| t.kind == AttackKind::Swing)
        .map_or(0., |t| (t.t / 0.22) as f32);
    let kick = c
        .attack
        .filter(|t| t.kind == AttackKind::Kick)
        .map_or(0., |t| (t.t / 0.28) as f32);
    if kick > 0. {
        out.push(Body {
            center: [at.0 + fx * (6. + 6. * kick), at.1 + fy * (6. + 6. * kick)],
            half: [3.5, 2.2],
            angle: a,
            shape: 0.,
            depth: depth + 0.0001,
            color: [0.12, 0.12, 0.14, 1.],
        });
    }
    let (len, wid, col) = match wp.id {
        "bat" => (11., 1.6, [0.55, 0.38, 0.2, 1.]),
        "knife" => (5., 0.9, [0.8, 0.82, 0.85, 1.]),
        "pistol" => (5., 1.4, [0.1, 0.1, 0.11, 1.]),
        "smg" => (8., 1.8, [0.1, 0.1, 0.11, 1.]),
        "shotgun" => (11., 1.6, [0.25, 0.18, 0.12, 1.]),
        _ => (0., 0., [0.; 4]),
    };
    if len == 0. {
        // Fäuste: beim Schlag schnellt eine Hand vor
        if swing > 0. {
            out.push(Body {
                center: [at.0 + fx * (6. + 6. * swing), at.1 + fy * (6. + 6. * swing)],
                half: [2.4, 2.4],
                angle: 0.,
                shape: 1.,
                depth: depth - 0.0003,
                color: rgba(0xe0ac69, 1.),
            });
        }
        return;
    }
    let wa = a + swing * 1.2 - 0.6 * swing.signum();
    let (wx, wy) = (wa.cos(), wa.sin());
    let side = (-fy * 3.5, fx * 3.5);
    out.push(Body {
        center: [
            at.0 + side.0 + wx * (5. + len / 2.),
            at.1 + side.1 + wy * (5. + len / 2.),
        ],
        half: [len / 2., wid],
        angle: wa,
        shape: 0.,
        depth: depth - 0.0003,
        color: col,
    });
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
        if self.step_screens(keys, dt) {
            return;
        }
        if keys.pressed.contains(&KeyCode::F5) {
            self.save();
        }
        self.bigmap.toggle(keys);
        if self.bigmap.open {
            let v = self.bigmap.view(self.hud_width);
            self.bigmap.control(keys, dt as f32, v);
        }
        if keys.pressed.contains(&KeyCode::KeyM)
            && let Some(a) = &self.audio
        {
            let muted = a.toggle_mute();
            self.world.notice = Some(berlin_sim::world::Notice {
                text: (if muted { "Ton aus" } else { "Ton an" }).into(),
                t: 1.5,
            });
        }
        let w2 = &mut self.world;
        if keys.pressed.contains(&KeyCode::KeyN) {
            // Wetter durchschalten: Tagesverlauf → klar → … → Schneesturm → Tagesverlauf
            let kinds = berlin_sim::weather::KINDS;
            let next = match w2.force_weather {
                None => Some(kinds[0]),
                Some(k) => kinds
                    .iter()
                    .position(|x| *x == k)
                    .and_then(|i| kinds.get(i + 1))
                    .copied(),
            };
            w2.force_weather = next;
            let text = next
                .map(|k| format!("Wetter: {}", berlin_sim::weather::label(k)))
                .unwrap_or_else(|| "Wetter: Tagesverlauf".into());
            w2.notice = Some(berlin_sim::world::Notice { text, t: 1.8 });
        }
        if keys.pressed.contains(&KeyCode::KeyT) {
            w2.clock = (w2.clock + 60.) % 1440.;
            w2.notice = Some(berlin_sim::world::Notice {
                text: format!("Uhr {}", format_clock(w2.clock)),
                t: 1.5,
            });
        }
        // offene Karte: die Welt läuft weiter, der Spieler bekommt keine Eingaben
        let mut input = if self.bigmap.open {
            Input::default()
        } else {
            input_from(keys, w2.player.in_car.is_some())
        };
        if self.demo_combat && !w2.loading && w2.player.in_car.is_none() {
            demo_combat_input(w2, &mut input);
        }
        // Maus oder Controller zielt: wer zuletzt bewegt wurde
        let m = keys.mouse;
        if m.moved || m.left_pressed || m.right_pressed {
            self.mouse_aim = true;
        }
        if keys.pad.rx.hypot(keys.pad.ry) > 0.35 || keys.pad.rt > 0.5 {
            self.mouse_aim = false;
        }
        self.cursor = m.hud;
        if self.mouse_aim && !self.bigmap.open && w2.player.in_car.is_none() {
            input.combat.aim_world = m.world.map(|p| (p.x as f64, p.y as f64));
            input.combat.fire |= m.left;
            input.combat.fire_pressed |= m.left_pressed;
            input.combat.kick |= m.right_pressed;
        }
        // Trigger als Taste: Flanke gegenüber dem letzten Schritt
        let trigger = keys.pad.rt > 0.5;
        input.combat.fire_pressed |= trigger && !self.trigger_was && input.combat.fire;
        self.trigger_was = trigger;
        if self.auto_enter && !w2.loading && w2.player.in_car.is_none() {
            self.auto_enter = false;
            if let Some((x, y)) = w2
                .player_car_id
                .and_then(|id| w2.car(id))
                .map(|c| (c.x, c.y))
            {
                (w2.player.x, w2.player.y) = (x + 15., y);
                input.enter_exit = true;
            }
        }
        w2.update(&input, dt);
        self.fx.ingest(&self.world.events);
        self.fx.step(dt as f32);
        berlin_sim::stats::track_step(
            &mut [&mut self.stats, &mut self.stats_total],
            &mut self.tracker,
            &self.world,
            dt,
        );
        if self.world.time - self.stats_written > 60. {
            self.write_stats();
        }
        // Klang-Frame immer berechnen: der Motorzustand speist auch Drehzahlmesser und Gang im HUD
        let frame = self.listener.frame(&mut self.world, dt);
        if let Some(audio) = &self.audio {
            audio.apply(&frame);
        }
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
        if matches!(
            self.screen,
            Screen::Title | Screen::Controls(true) | Screen::Stats(true)
        ) {
            // langsame Kreisfahrt über dem Kiez (main.js demo-Kamera, kleiner Radius: geladene Kacheln)
            let t = self.world.time;
            let (x, y) = (c.x + (t * 0.05).cos() * 900., c.y + (t * 0.07).sin() * 600.);
            return (Vec2::new(x as f32, y as f32), 0.8);
        }
        (Vec2::new(c.x as f32, c.y as f32), c.zoom as f32)
    }
    fn quit(&self) -> bool {
        self.quit
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
                // liegt in Sturzrichtung: Körper lang, Kopf voraus
                let f = p.fall as f32;
                out.push(Body {
                    center: [x, y],
                    half: [8.5, 4.5],
                    angle: f,
                    shape: 1.,
                    depth: depth + 0.001,
                    color: shade(rgba(p.shirt, 1.), 0.8),
                });
                out.push(Body {
                    center: [x + f.cos() * 9., y + f.sin() * 9.],
                    half: [3., 3.],
                    angle: 0.,
                    shape: 1.,
                    depth: depth + 0.0008,
                    color: rgba(p.skin, 1.),
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
            // Faustschlag eines Kämpfers
            if p.punch > 0. {
                let r = 6. + 5. * (p.punch / 0.22) as f32;
                out.push(Body {
                    center: [x + a.cos() * r, y + a.sin() * r],
                    half: [2.2, 2.2],
                    angle: 0.,
                    shape: 1.,
                    depth: depth - 0.0003,
                    color: rgba(p.skin, 1.),
                });
            }
        }
        if w.player.in_car.is_none() && w.player.combat.dead {
            // K. o.: liegt
            let (x, y, f) = (
                w.player.x as f32,
                w.player.y as f32,
                w.player.combat.fall as f32,
            );
            out.push(Body {
                center: [x, y],
                half: [9., 5.],
                angle: f,
                shape: 1.,
                depth: 0.617,
                color: rgba(0x2b2f3a, 1.),
            });
            out.push(Body {
                center: [x + f.cos() * 10., y + f.sin() * 10.],
                half: [3.2, 3.2],
                angle: 0.,
                shape: 1.,
                depth: 0.6168,
                color: rgba(0xe0ac69, 1.),
            });
        } else if w.player.in_car.is_none() {
            let (x, y, a) = (w.player.x as f32, w.player.y as f32, w.player.angle as f32);
            let depth0 = if w.player.level.lvl >= 1 {
                0.548
            } else {
                0.617
            };
            weapon_bodies(&w.player.combat, (x, y), a, depth0, out);
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
        self.fx.bodies(out);
        crate::weatherfx::bodies(w, out);
    }
    fn take_overview(&mut self) -> Option<berlin_map_loader::overview::OverlayMesh> {
        self.bigmap.mesh.take()
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
    fn hud(
        &mut self,
        camera: &berlin_engine::camera::Camera,
        viewport: Vec2,
        out: &mut berlin_engine::hud::Hud,
    ) {
        let engine = self.world.player_car().map(|_| self.listener.engine());
        crate::weatherfx::overlay(&self.world, out);
        self.hud_width = out.width;
        match self.screen {
            Screen::Title => {
                crate::menu::draw_title(out, &self.menu, self.world.loading);
                return;
            }
            Screen::Controls(_) => {
                crate::menu::draw_controls(out);
                return;
            }
            Screen::Stats(from_title) => {
                crate::menu::draw_stats(out, &self.stats, &self.stats_total, !from_title);
                return;
            }
            _ => {}
        }
        if self.bigmap.open {
            self.bigmap.draw(&self.world, out);
            return;
        }
        let warn = self.world.road_warning();
        crate::hud::draw(&self.world, engine.as_ref(), warn, camera, viewport, out);
        let p = &self.world.player;
        if self.screen == Screen::Playing
            && self.mouse_aim
            && p.in_car.is_none()
            && !p.combat.dead
            && let Some(c) = self.cursor
        {
            crate::hud::crosshair(out, c, p.combat.weapon().melee);
        }
        if let Some(m) = &self.result_menu {
            let y = if self.world.mission.state == State::Success {
                400.
            } else {
                330.
            };
            crate::menu::draw_menu(out, m, out.width / 2., y);
        }
        if self.screen == Screen::Paused {
            crate::menu::draw_pause(
                out,
                &self.menu,
                self.world.completed as u32,
                self.world.best_time,
            );
        }
    }
    fn lighting(&self) -> Option<Lighting> {
        let mut l = lighting_of(&world_light(&self.world));
        // Blitze hellen alles kurz auf
        let flash = berlin_sim::weather::flash_total(
            self.world.seed,
            self.world.time,
            self.world.sky.p.thunder,
        ) as f32;
        if flash > 0. {
            l.ambient = l.ambient.map(|a| (a + flash * 0.8).min(1.6));
            l.dark *= 1. - flash.min(1.);
        }
        Some(l)
    }
    fn lights(&mut self, out: &mut Vec<LightSource>) {
        let l = world_light(&self.world);
        let k = l.dark as f32;
        if k <= 0. {
            return;
        }
        self.fx.lights(out, k);
        let rgb = |c: [u8; 3]| c.map(|v| v as f32 / 255.);
        let push =
            |out: &mut Vec<LightSource>, x: f64, y: f64, radius: f32, color: [f32; 3], a: f32| {
                out.push(LightSource {
                    center: [x as f32, y as f32],
                    radius,
                    angle: 0.,
                    color,
                    intensity: a.min(1.),
                    cone: 0.,
                    pad: 0.,
                });
            };
        let (cx, cy, zoom) = (
            self.world.camera.x,
            self.world.camera.y,
            self.world.camera.zoom,
        );
        let view = 2200. / zoom.max(0.5);
        if l.lamps_on {
            for lp in self.lamps.near(&mut self.world.city, cx, cy, view + 250.) {
                push(
                    out,
                    lp.x + lp.nx * 18.,
                    lp.y + lp.ny * 18.,
                    if lp.main { 150. } else { 125. },
                    rgb(lp.rgb),
                    if lp.gas { 0.55 } else { 0.7 } * k,
                );
            }
        }
        let w = &self.world;
        let (cx, cy) = (w.camera.x, w.camera.y);
        let view = 2200. / w.camera.zoom.max(0.5);
        let near =
            |x: f64, y: f64| (x - cx).abs() < view + 250. && (y - cy).abs() < view * 0.7 + 250.;
        // Ampeln: farbiger Schein an der Haltelinie jeder Zufahrt
        for &v in &w.city.signals {
            let Some(nd) = w.city.nodes.get(&v) else {
                continue;
            };
            if !near(nd.x, nd.y) {
                continue;
            }
            for e in nd
                .edges
                .iter()
                .filter_map(|id| w.city.edges.get(id))
                .filter(|e| e.cls <= 8)
            {
                let p = &e.pts;
                let (a, b) = if e.a == v {
                    (p[1], p[0])
                } else {
                    (p[p.len() - 2], p[p.len() - 1])
                };
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let len = dx.hypot(dy).max(1e-6);
                let (ux, uy) = (dx / len, dy / len);
                let heading = uy.atan2(ux);
                let (x, y) = (
                    nd.x - ux * nd.trim.max(e.w / 2.) - uy * e.w / 2.,
                    nd.y - uy * nd.trim.max(e.w / 2.) + ux * e.w / 2.,
                );
                let color = match signal_state(&w.city, v, heading, w.time) {
                    Light::Red => [1., 0.24, 0.18],
                    Light::Yellow => [1., 0.75, 0.2],
                    Light::Green => [0.3, 1., 0.5],
                };
                push(out, x, y, 34., color, 0.8 * k);
            }
        }
        // Fahrzeuge mit Fahrer: Scheinwerferkegel, Standlicht, Rück- und Bremslicht
        for c in w
            .cars
            .iter()
            .filter(|c| !c.wrecked && c.driver.is_some() && near(c.x, c.y))
        {
            let (sa, ca) = c.angle.sin_cos();
            let (fx, fy, bx, by) = (
                c.x + ca * c.hw,
                c.y + sa * c.hw,
                c.x - ca * c.hw,
                c.y - sa * c.hw,
            );
            out.push(LightSource {
                center: [(fx - ca * 4.) as f32, (fy - sa * 4.) as f32],
                radius: 230.,
                angle: c.angle as f32,
                color: [1., 0.925, 0.77],
                intensity: 0.85 * k,
                cone: 1.,
                pad: 0.,
            });
            push(out, fx, fy, 34., [1., 0.94, 0.82], 0.6 * k);
            let braking = c.controls.brake > 0.1;
            push(
                out,
                bx,
                by,
                if braking { 46. } else { 26. },
                [1., 0.2, 0.14],
                if braking { 0.9 } else { 0.45 } * k,
            );
        }
        if w.player.in_car.is_none() {
            push(out, w.player.x, w.player.y, 70., [1., 0.82, 0.67], 0.35 * k);
        }
        let pv = berlin_sim::mission::PlayerView {
            x: w.player.x,
            y: w.player.y,
            in_car: w.player.in_car,
        };
        if let (_, Some((tx, ty))) = w.mission.objective(&w.city.places, pv, &w.cars) {
            push(out, tx, ty, 90., [1., 0.8, 0.25], 0.7 * k);
        }
    }
    fn ready(&self) -> bool {
        !self.world.loading
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use berlin_engine::pad::Pad;
    use std::collections::HashSet;
    #[test]
    fn gamepad_and_keys_merge() {
        assert_eq!(radial_deadzone(0.1, 0.1, 0.22), (0., 0.));
        let (x, y) = radial_deadzone(1., 0., 0.22);
        assert!((x - 1.).abs() < 1e-6 && y == 0.);
        let held: HashSet<KeyCode> = [KeyCode::KeyW].into();
        let none = HashSet::new();
        let pad = Pad {
            connected: true,
            lx: -0.8,
            rt: 0.4,
            lt: 0.2,
            ..Default::default()
        };
        let edges = Pad {
            y: true,
            ..Default::default()
        };
        let keys = Keys {
            held: &held,
            pressed: &none,
            pad,
            pad_pressed: edges,
            mouse: Default::default(),
        };
        let i = input_from(&keys, true);
        assert_eq!(i.throttle, 1., "Taste W und Trigger: das Stärkere zählt");
        assert!((i.brake - 0.2).abs() < 1e-6 && i.steer < -0.5 && i.enter_exit);
        let walk = input_from(
            &Keys {
                held: &none,
                pressed: &none,
                pad: Pad {
                    ly: -1.,
                    a: true,
                    ..Default::default()
                },
                pad_pressed: Pad::default(),
                mouse: Default::default(),
            },
            false,
        );
        assert!(walk.move_y < -0.9 && walk.sprint && walk.throttle == 0.);
    }
    #[test]
    fn lighting_follows_the_clock() {
        let noon = lighting_at(13. * 60.);
        // Schatten zeigt von der Sonne weg: Sonnenvektor (x, y) und Schattenrichtung sind entgegengesetzt
        let dot = noon.sun[0] * noon.shadow[0] + noon.sun[1] * noon.shadow[1];
        assert!(dot < 0. && noon.sun[2] > 0.8 && noon.dark == 0. && noon.shadow_strength == 1.);
        let night = lighting_at(23. * 60.);
        assert!(night.dark > 0.9 && night.shadow_strength == 0. && night.ambient[0] < 0.5);
        let eve = lighting_at(19. * 60.);
        assert!(eve.shadow[0] > 0.5 && eve.shadow_len > noon.shadow_len);
    }
    #[test]
    fn screens_title_pause_save_and_continue() {
        let dir = std::env::temp_dir().join(format!("gta-berlin-screens-{}", std::process::id()));
        let path = dir.join("save.json");
        let _ = std::fs::remove_file(&path);
        let root = berlin_map_loader::default_data_root();
        let mut p =
            Play::new(&root, 4, Some(FileStorage::new(&path)), false, Start::Title).unwrap();
        let none = HashSet::new();
        let mut press = |p: &mut Play, k: Option<KeyCode>| {
            let pressed: HashSet<KeyCode> = k.into_iter().collect();
            p.step(
                &Keys {
                    held: &none,
                    pressed: &pressed,
                    pad: Pad::default(),
                    pad_pressed: Pad::default(),
                    mouse: Default::default(),
                },
                DT,
            );
        };
        let wait = |p: &mut Play, press: &mut dyn FnMut(&mut Play, Option<KeyCode>)| {
            let t0 = std::time::Instant::now();
            while p.world.loading && t0.elapsed().as_secs() < 30 {
                press(p, None);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(!p.world.loading, "Welt lädt nicht");
        };
        wait(&mut p, &mut press);
        assert_eq!(p.screen, Screen::Title);
        // ohne Spielstand: „Fortsetzen“ aus, Auswahl auf „Neues Spiel“
        assert!(!p.menu.items[0].enabled);
        let t = p.world.time;
        press(&mut p, None);
        assert!(p.world.time > t, "die Stadt hinter dem Titel läuft");
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(p.screen, Screen::Playing);
        wait(&mut p, &mut press);
        // gescheiterter Auftrag: Welt steht, Menü „Erneut versuchen / Frei weiterspielen“
        p.world.mission.state = State::Failed;
        p.world.mission.result = Some(Outcome::Failed {
            reason: "Test".into(),
        });
        let (t, px) = (p.world.time, p.world.player.x);
        press(&mut p, None);
        assert_eq!(p.world.time, t);
        assert_eq!(p.result_menu.as_ref().map(|m| m.items.len()), Some(2));
        press(&mut p, Some(KeyCode::Escape));
        // Esc pausiert hier (wie game.js: Pause hat Vorrang), Zurück ins Ergebnis
        assert_eq!(p.screen, Screen::Paused);
        press(&mut p, Some(KeyCode::Escape));
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(
            p.world.mission.state,
            State::Available,
            "frei weiterspielen setzt zurück"
        );
        assert_eq!(p.world.player.x, px, "der Spieler bleibt, wo er ist");
        assert!(p.result_menu.is_none());
        // Esc pausiert, die Welt steht
        press(&mut p, Some(KeyCode::Escape));
        assert_eq!(p.screen, Screen::Paused);
        let t = p.world.time;
        press(&mut p, None);
        assert_eq!(p.world.time, t);
        // Spiel speichern
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(p.screen, Screen::Playing);
        assert!(path.exists(), "Spielstand geschrieben");
        // zum Hauptmenü: jetzt mit „Fortsetzen“ vorn
        press(&mut p, Some(KeyCode::KeyP));
        for _ in 0..5 {
            press(&mut p, Some(KeyCode::ArrowDown));
        }
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(p.screen, Screen::Title);
        assert!(p.menu.items[0].enabled && p.menu.index == 0);
        // Steuerung und zurück
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(p.screen, Screen::Controls(true));
        press(&mut p, Some(KeyCode::Escape));
        assert_eq!(p.screen, Screen::Title);
        // Statistik: Spielzeit des Spiels ist gezählt und liegt in der Datei
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(p.screen, Screen::Stats(true));
        assert!(p.stats.get("timePlayed") > 0. && p.stats_total.get("timePlayed") > 0.);
        let file = std::fs::read_to_string(dir.join("stats.json")).expect("stats.json");
        assert!(file.contains("timePlayed"));
        press(&mut p, Some(KeyCode::Escape));
        assert_eq!(p.screen, Screen::Title);
        // Beenden
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::Enter));
        assert!(p.quit());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
