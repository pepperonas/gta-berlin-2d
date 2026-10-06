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

/// Teleport-Rückfrage: Klickpunkt und (sobald geladen) Ziel x, y, Winkel, Ortsname.
pub type Teleport = ((f64, f64), Option<(f64, f64, f64, String)>);

pub struct Play {
    pub world: World,
    /// Koop: welcher Controller Spieler 2 gehört (1 = zweiter Controller, 0 = erster – Spieler 1 spielt dann nur
    /// mit Tastatur und Maus)
    pub p2_pad: u8,
    /// `--koop [METER]`: Spieler 2 beim Start dazuholen, optional so weit östlich (geteiltes Bild)
    pub koop_start: Option<f64>,
    /// Ansichten des laufenden Bildes (von der Engine, `set_views`) und Bildgröße
    views: berlin_engine::split::Views,
    viewport: Vec2,
    rumbler2: crate::rumble::Rumbler,
    rumble_out2: Option<berlin_engine::Rumble>,
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
    /// Klickfläche des Entwickler-Links auf dem Titelbild (vom letzten Zeichnen) und ob der Zeiger darauf liegt
    title_link: Option<[f32; 4]>,
    title_link_hover: bool,
    /// Anzeige-Schalter der Befehlszeile (fps, ebenen, silhouetten)
    debug: crate::console::Debug,
    fps: crate::fps::Meter,
    pub screen: Screen,
    pub menu: crate::menu::Menu,
    root: std::path::PathBuf,
    /// Bar-Feed (Standard `web/data/bars.json` neben den Kacheln, `--bars DATEI`), `None` = aus
    pub bars_file: Option<std::path::PathBuf>,
    /// Live-Abruf des Bar-Feeds (`--bars live|URL`, Befehl `bars URL`); ersetzt die Datei
    pub bars_live: Option<crate::barfeed::Live>,
    seed: u32,
    /// Stick-Stellung des letzten Schritts (Menüauswahl per Stick als Flanke)
    stick_prev: f32,
    stick_prev_x: f32,
    quit: bool,
    /// Statistik: dieses Spiel, insgesamt, Stand des gespeicherten Spiels (für „Fortsetzen“)
    pub stats: berlin_sim::stats::Stats,
    pub stats_total: berlin_sim::stats::Stats,
    stats_saved: berlin_sim::stats::Stats,
    tracker: berlin_sim::stats::Tracker,
    stats_written: f64,
    result_menu: Option<crate::menu::Menu>,
    /// Wegpunkt und Route (Stadtplan-Klick)
    pub nav: crate::nav::Nav,
    /// frei belegbare Steuerung (settings.json `bindings`) und ihre Tafel
    pub bindings: crate::bindings::Bindings,
    bindmenu: crate::bindmenu::BindMenu,
    pub about: crate::about::About,
    /// Kamerazoom über die Belegung (Kamera näher/weiter), Faktor auf den Spielzoom
    zoom_user: f32,
    /// Grafikmodus und Qualität (aktiv) und wie sie in `settings.json` stehen (CLI-Vorgaben werden nicht gespeichert)
    pub graphics: berlin_engine::graphics::GraphicsSettings,
    graphics_saved: berlin_engine::graphics::GraphicsSettings,
    /// Fester Kamerazoom für reproduzierbare Aufnahmen (`--zoom` im Spiel); `None` = Kamera folgt dem Spiel
    pub zoom_fix: Option<f32>,
    /// Controller-Vibration: Regeln und die nächste abzuholende
    rumbler: crate::rumble::Rumbler,
    rumble_out: Option<berlin_engine::Rumble>,
    /// Gas- und Bremsstellung der Tastatur (Rampe, `bindings::pedal_ramp`)
    pedals: [f64; 2],
    /// Bild-Interpolation zwischen den 60-Hz-Schritten
    interp: crate::interp::Interp,
    /// Aufnahme-Option `--kampf-demo`: schießt mit der Pistole auf den nächsten Passanten
    pub demo_combat: bool,
    /// `--drift-demo`: im eigenen Auto Vollgas mit Handbremse und Lenkung (Reifenqualm, Bremsspuren prüfen)
    pub demo_drift: bool,
    demo_drift_started: bool,
    demo_drift_t: f64,
    /// Entwickler-Build (`--dev`, `GTA_DEV=1` oder Debug-Build): Physik-Anzeige mit Live-Reglern (F3)
    pub dev: bool,
    pub physdebug: crate::physdebug::PhysDebug,
    pub enginedebug: crate::enginedebug::EngineDebug,
    /// Aufnahme-Option `--fahrzeugschau`: einmal alle Fahrzeugarten vor die Figur stellen
    pub vehicle_show: bool,
    /// Aufnahme-Option `--bildschirm zugfahrt`: die nächste Straßenbahn übernehmen und anfahren
    pub demo_drive: bool,
    /// Aufnahmen: `--bildschirm bahnhof` (hinunter in den nächsten U-Bahnhof), `tunnelfahrt` (dazu einsteigen)
    pub demo_station: Option<bool>,
    pub people_show: bool,
    /// Laternen und Wegweiser im Bild (in `lights` gesammelt, dort ist die Stadt veränderlich)
    street_lamps: Vec<berlin_sim::lamps::Lamp>,
    street_signs: Vec<berlin_sim::city::Sign>,
    lamps_lit: bool,
    /// Fahrzeug-Atlas schon an die Engine gegeben
    vehicle_atlas_sent: bool,
    /// Schaufensterlicht und Leuchtreklame (nachts)
    neon: crate::neon::Neon,
    pub demo_covered: bool,
    pub demo_neon: bool,
    /// Musterseite aller Schild-Bauarten, tags und nachts (`--bildschirm schilder`)
    pub sign_lab: bool,
    /// Musterseite der Motorräder neben der Kamera (`--bildschirm motorraeder`)
    pub moto_lab: bool,
    /// Musterseite aller Pkw-Modelle und Nutzfahrzeuge als echte, abgestellte Autos (`--bildschirm autos`);
    /// `Some(true)` = schon abgestellt
    pub car_lab: Option<bool>,
    /// zuletzt mit der Maus gezielt (sonst Controller); Zeiger im HUD für das Fadenkreuz
    mouse_aim: bool,
    cursor: Option<Vec2>,
    /// Steuerschema zu Fuß am PC: Diablo (Klick, Standard) oder klassisch (WASD + Maus zielt)
    pub diablo: bool,
    /// letzter Linksklick (Spielzeit, HUD-Punkt) für den Doppelklick
    ctrl_held: bool,
    /// Teleport-Rückfrage: Ziel (Kartenpunkt) und, sobald geladen, die Landestelle mit Namen
    pub teleport: Option<Teleport>,

    /// Waffenrad (Maus rechts, Controller LB), Echtzeit für das Halten, Zeitlupe
    wheel_m: crate::wheel::WheelButton,
    wheel_p: crate::wheel::WheelButton,
    /// Waffenrad von Spieler 2 (Koop, LB an seinem Controller; ohne Zeitlupe – die Welt gehört beiden)
    wheel_p2: crate::wheel::WheelButton,
    /// LB + RB gleichzeitig = Befehlszeile (je Controller) und deren Bildschirmtastatur
    chord: [crate::bindings::ShoulderChord; 2],
    kbd: crate::padkbd::PadKbd,
    /// Teleport-Rückfrage: „Nein“ ausgewählt (Pfeile/Stick wechseln, A bzw. Enter nimmt die Auswahl)
    teleport_no: bool,
    /// Fahrhilfen-Rad (rechter Stick im Fahrzeug) je Spieler und die Hupe mit langem Druck (Sirene)
    assist: [crate::wheel::WheelButton; 2],
    horn: [crate::bindings::HornPress; 2],
    /// Navigation von Spieler 2: gemeinsamer Wegpunkt, eigene Route
    nav2: crate::nav::Nav,
    real_t: f64,
    time_scale: f64,
    /// Kurzlebige Effekte (Mündungsfeuer, Leuchtspuren, Blut)
    pub fx: crate::effects::Effects,
    /// Reifenspuren im Schnee (nur Darstellung)
    pub trails: crate::snowtracks::Trails,
    /// sichtbare Bahnen und Straßenbahngleise um die Kamera (nach jedem Schritt erneuert)
    trains: Vec<berlin_sim::transitlive::Visible>,
    tram_segs: Vec<(berlin_sim::city::Pt, berlin_sim::city::Pt)>,
    /// Befehlszeile (Enter) und ihre Ortsliste aus dem Stadtplan
    pub console: crate::console::Console,
    pub places: Vec<crate::console::Place>,
    /// Teleport der Befehlszeile: ohne Rückfrage springen, sobald das Ziel geladen ist
    teleport_auto: bool,
}

/// Einstellungen neben dem Spielstand (`settings.json`): Steuerschema und Tastenbelegung.
fn settings_path(st: &FileStorage) -> std::path::PathBuf {
    st.path.with_file_name("settings.json")
}
fn read_settings(
    st: &FileStorage,
) -> (
    bool,
    crate::bindings::Bindings,
    berlin_engine::graphics::GraphicsSettings,
) {
    let v = std::fs::read(settings_path(st))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .unwrap_or_default();
    let diablo = v["controls"].as_str().is_none_or(|c| c != "classic");
    (
        diablo,
        crate::bindings::Bindings::from_json(&v["bindings"]),
        graphics_from_json(&v),
    )
}
/// `"grafik"` und `"qualitaet"` aus `settings.json`; Fehlendes oder Unbekanntes = Standard (HD, hoch).
pub fn graphics_from_json(v: &serde_json::Value) -> berlin_engine::graphics::GraphicsSettings {
    use berlin_engine::graphics::{GraphicsMode, GraphicsSettings, Quality};
    let d = GraphicsSettings::default();
    GraphicsSettings {
        mode: v["grafik"]
            .as_str()
            .and_then(GraphicsMode::parse)
            .unwrap_or(d.mode),
        quality: v["qualitaet"]
            .as_str()
            .and_then(Quality::parse)
            .unwrap_or(d.quality),
    }
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
    /// Belegungstafel (aus der Steuerungstafel); merkt sich die Herkunft wie `Controls`
    Bindings(bool),
    /// „Über das Spiel“; merkt sich die Herkunft wie `Controls`
    About(bool),
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
    let mut w = World::new(
        city,
        seed,
        berlin_sim::world::TRAFFIC_CARS,
        berlin_sim::world::TRAFFIC_PEDS,
    );
    match berlin_sim::transit::Transit::read(root) {
        Ok(t) => w.set_transit(t),
        Err(e) => eprintln!("Kein Nahverkehr: {e:#}"),
    }
    Ok(w)
}

/// Bar-Auslastungs-Feed lesen und an die Stadt hängen (Datei wie `npm run bars:fetch` sie schreibt).
pub fn load_bars(world: &mut World, path: &Path) -> Result<usize, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Bar-Feed: {e}"))?;
    attach_feed(world, &json)
}

/// Feed-Dokument (Datei oder Netz) an die Stadt hängen; Zahl der Bars.
pub fn attach_feed(world: &mut World, json: &serde_json::Value) -> Result<usize, String> {
    let (_, _, bars) = berlin_sim::nightlife::parse_bar_feed(json)?;
    Ok(berlin_sim::nightlife::attach_bars(&mut world.city, bars))
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
        windows: l.windows_lit as f32,
        minutes: l.minutes as f32,
        warmth: berlin_sim::daylight::film_mood(l).0 as f32,
        fog: 0.,
        wet: 0.,
    }
}

impl Play {
    fn hud_main(
        &mut self,
        camera: &berlin_engine::camera::Camera,
        viewport: Vec2,
        out: &mut berlin_engine::hud::Hud,
    ) {
        let engine = self.world.player_car().map(|_| self.listener.engine());
        let station = self.world.current_station().cloned();
        if let Some(st) = station.as_ref().filter(|s| !s.open_air) {
            // im U-Bahnhof: der Bahnhof statt der Stadt (kein Wetter, kein Himmel)
            crate::underground::draw_station(&self.world, st, camera, viewport, out);
        } else {
            // Hochbahn, ebenerdig, Einschnitt: der Bahnsteig über der sichtbaren Stadt, das Wetter darüber
            if let Some(st) = &station {
                crate::underground::draw_station(&self.world, st, camera, viewport, out);
            }
            crate::weatherfx::sky_overlay(&self.world, camera, viewport, out);
            crate::weatherfx::storm_overlay(&self.world, camera, viewport, out);
            crate::weatherfx::overlay(&self.world, out);
            crate::underground::draw_tunnels(&mut self.world, camera, viewport, out);
            // Weltmarken je Ansicht; im geteilten Bild bleibt nur, was auf der eigenen Hälfte liegt
            let views = self.views.clone();
            let split = self.world.coop() && views.count == 2;
            for k in 0..if split { 2 } else { 1 } {
                let cam = if split { &views.cams[k] } else { camera };
                let from = out.items.len();
                crate::underground::entrance_letters(&self.world, cam, viewport, out);
                crate::neon::draw(&self.neon, cam, viewport, out);
                crate::hud::ped_health_bars(&self.world, cam, viewport, out);
                if split {
                    let mut i = from;
                    while i < out.items.len() {
                        let c = out.items[i].center;
                        if views.view_at(Vec2::new(c[0], c[1]), viewport) == k {
                            i += 1;
                        } else {
                            out.items.remove(i);
                        }
                    }
                }
            }
        }
        self.hud_width = out.width;
        if self.sign_lab {
            crate::neon::draw_lab(out, self.world.time);
            return;
        }
        match self.screen {
            Screen::Title => {
                self.title_link = Some(crate::menu::draw_title(
                    out,
                    &self.menu,
                    self.world.loading,
                    self.title_link_hover,
                ));
                return;
            }
            Screen::Controls(_) => {
                crate::menu::draw_controls(out, self.diablo, &self.bindings);
                return;
            }
            Screen::Bindings(_) => {
                self.bindmenu.draw(out, &self.bindings);
                return;
            }
            Screen::Stats(from_title) => {
                crate::menu::draw_stats(out, &self.stats, &self.stats_total, !from_title);
                return;
            }
            Screen::About(from_title) => {
                self.about.draw(out, !from_title);
                return;
            }
            _ => {}
        }
        if self.bigmap.open {
            let nav = self.nav.view(self.nav_pos());
            self.bigmap.draw(&self.world, &nav, !self.mouse_aim, out);
            if let Some((_, spot)) = &self.teleport {
                crate::menu::draw_teleport(
                    out,
                    spot.as_ref().map(|s| s.3.as_str()),
                    self.teleport_no,
                );
            }
            return;
        }
        let warn = self.world.road_warning();
        let nav = self.nav.view(self.nav_pos());
        if self.world.coop() {
            self.coop_hud(engine.as_ref(), warn, &nav, camera, viewport, out);
        } else {
            crate::hud::draw(
                &self.world,
                engine.as_ref(),
                warn,
                &nav,
                camera,
                viewport,
                out,
                crate::hud::Parts::ALL,
            );
        }
        self.physdebug.draw(out, &self.world);
        self.enginedebug.draw(out, &self.listener.engine_view);
        let p = &self.world.player;
        let ctrl_aim = !self.diablo || self.ctrl_held;
        if self.screen == Screen::Playing
            && self.mouse_aim
            && ctrl_aim
            && p.in_car.is_none()
            && !p.combat.dead
            && let Some(c) = self.cursor
        {
            crate::hud::crosshair(out, c, p.combat.weapon().melee);
        }
        if let Some((_, spot)) = &self.teleport
            && !self.teleport_auto
        {
            crate::menu::draw_teleport(out, spot.as_ref().map(|s| s.3.as_str()), self.teleport_no);
        }
        if self.screen == Screen::Playing {
            crate::console::draw(out, &self.console, &self.world);
            if self.console.open {
                crate::padkbd::draw(out, &self.kbd);
            }
        }
        if self.assist[0].open && self.screen == Screen::Playing {
            let items = assist_items(&self.world);
            let age = self.real_t - self.assist[0].opened_at;
            crate::wheel::draw_assist(out, &self.assist[0], &items, age);
        }
        for (wh, pad) in [(&self.wheel_m, false), (&self.wheel_p, true)] {
            if wh.open && self.screen == Screen::Playing {
                crate::wheel::draw(
                    out,
                    wh,
                    &self.world.player.combat,
                    self.real_t - wh.opened_at,
                    pad,
                );
            }
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
        let (diablo, bindings, graphics) = save.as_ref().map(read_settings).unwrap_or_default();
        let diablo = save.is_none() || diablo;
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
        let places = crate::console::place_index(&bigmap.labels);
        let mut play = Self {
            console: Default::default(),
            places,
            teleport_auto: false,
            bigmap,
            hud_width: 1280.,
            title_link: None,
            title_link_hover: false,
            debug: Default::default(),
            fps: Default::default(),
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
            menu: crate::menu::title_menu(has_save).with_graphics(graphics.mode),
            root: root.to_path_buf(),
            bars_live: None,
            bars_file: root
                .parent()
                .map(|p| p.join("bars.json"))
                .filter(|p| p.exists()),
            seed,
            stick_prev: 0.,
            stick_prev_x: 0.,
            quit: false,
            stats,
            stats_total,
            stats_saved,
            tracker: Default::default(),
            stats_written: 0.,
            result_menu: None,
            nav: crate::nav::Nav::start(root.to_path_buf()),
            bindings,
            bindmenu: Default::default(),
            about: Default::default(),
            zoom_user: 1.,
            graphics,
            graphics_saved: graphics,
            zoom_fix: None,
            rumbler: Default::default(),
            rumble_out: None,
            p2_pad: 1,
            koop_start: None,
            views: berlin_engine::split::Views::single(Default::default()),
            viewport: Vec2::new(1280., 720.),
            rumbler2: Default::default(),
            rumble_out2: None,
            pedals: [0.; 2],
            interp: Default::default(),
            demo_combat: false,
            demo_drift: false,
            demo_drift_started: false,
            demo_drift_t: 0.,
            dev: cfg!(debug_assertions) || std::env::var_os("GTA_DEV").is_some(),
            physdebug: Default::default(),
            enginedebug: Default::default(),
            vehicle_show: false,
            demo_drive: false,
            demo_station: None,
            people_show: false,
            street_lamps: Vec::new(),
            street_signs: Vec::new(),
            lamps_lit: false,
            vehicle_atlas_sent: false,
            neon: Default::default(),
            demo_covered: false,
            demo_neon: false,
            sign_lab: false,
            moto_lab: false,
            car_lab: None,
            mouse_aim: false,
            cursor: None,
            diablo,
            ctrl_held: false,
            teleport: None,

            wheel_m: Default::default(),
            wheel_p: Default::default(),
            wheel_p2: Default::default(),
            chord: Default::default(),
            kbd: Default::default(),
            teleport_no: false,
            assist: Default::default(),
            horn: Default::default(),
            nav2: Default::default(),
            real_t: 0.,
            time_scale: 1.,
            fx: Default::default(),
            trails: Default::default(),
            trains: Vec::new(),
            tram_segs: Vec::new(),
        };
        match play.reload_bars() {
            Ok(m) if play.bars_file.is_some() => eprintln!("Nachtleben: {m}"),
            Err(e) => eprintln!("Bar-Feed nicht ladbar: {e}"),
            _ => {}
        }
        Ok(play)
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
                if let Err(e) = self.reload_bars() {
                    eprintln!("Bar-Feed nicht ladbar: {e}");
                }
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
    /// Aufnahme: Waffenrad offen in der Bildmitte, die Pistole gezeigt.
    pub fn demo_wheel(&mut self) {
        let c = Vec2::new(self.hud_width / 2., 360.);
        self.wheel_m.press(0., c);
        self.wheel_m
            .tick(1., true, 0, berlin_sim::combat::WEAPONS.len());
        self.wheel_m.mov(c + Vec2::new(60., 30.));
        self.real_t = 10.;
        self.wheel_m.opened_at = 0.;
        // offen halten, ohne dass ein Loslassen der (nicht gedrückten) Taste es schließt
        self.wheel_m.down = false;
    }
    /// Teleport per Klick auf den Stadtplan vormerken (game.js requestTeleport).
    /// Für Aufnahmen: Stadtplan offen, Rückfrage für einen Punkt 2 km nördlich.
    pub fn demo_teleport(&mut self) {
        let (x, y) = (self.world.player.x, self.world.player.y - 20000.);
        self.bigmap.open = true;
        self.screen = Screen::Playing;
        self.request_teleport(x, y);
    }
    /// Startpunkt für Aufnahmen (`--geo` im Spiel): wie `tp` der Befehlszeile ohne Rückfrage springen, sobald das
    /// Ziel geladen ist.
    pub fn start_at(&mut self, x: f64, y: f64) {
        self.teleport = Some(((x, y), None));
        self.teleport_auto = true;
    }
    pub fn request_teleport(&mut self, x: f64, y: f64) {
        use berlin_sim::world::TeleportSpot;
        let notice = |w: &mut World, t: &str| {
            w.notice = Some(berlin_sim::world::Notice {
                text: t.into(),
                t: 2.,
            })
        };
        if matches!(
            self.world.mission.state,
            State::ToPickup | State::ToDropoff | State::Briefing
        ) {
            notice(&mut self.world, "Während eines Auftrags nicht möglich");
            return;
        }
        match self.world.find_teleport_spot(x, y) {
            None => notice(
                &mut self.world,
                "Dort kann man nicht hin (außerhalb von Berlin)",
            ),
            Some(TeleportSpot::Pending) => self.teleport = Some(((x, y), None)),
            Some(TeleportSpot::Spot {
                x: sx,
                y: sy,
                angle,
                name,
            }) => self.teleport = Some(((x, y), Some((sx, sy, angle, name)))),
        }
    }
    /// Rückfrage beantworten.
    fn confirm_teleport(&mut self, yes: bool) {
        self.trails.clear();
        let Some((_, spot)) = self.teleport.take() else {
            return;
        };
        match spot {
            Some((x, y, a, name)) if yes => {
                self.world.teleport_to(x, y, a);
                self.bigmap.open = false;
                self.world.notice = Some(berlin_sim::world::Notice {
                    text: format!("Teleportiert: {name}"),
                    t: 2.5,
                });
                self.stats.bump("teleports");
                self.stats_total.bump("teleports");
            }
            Some(_) if !yes => self.world.city.release("teleport"),
            None if !yes => self.world.city.release("teleport"),
            _ => {
                // Ziel lädt noch: weiter fragen
            }
        }
    }
    /// Quelle des Bar-Feeds wählen: `aus`, `neu` (gleiche Quelle neu laden), `live`/URL (Netz) oder eine Datei.
    pub fn set_bars_source(&mut self, arg: Option<&str>) {
        match arg {
            Some("aus") => {
                self.bars_file = None;
                self.bars_live = None;
            }
            Some("neu") | None => {
                if let Some(l) = &self.bars_live {
                    self.bars_live = Some(crate::barfeed::Live::start(l.url.clone()));
                }
            }
            Some(a) => match crate::barfeed::url_of(a) {
                Some(url) => self.bars_live = Some(crate::barfeed::Live::start(url)),
                None => {
                    self.bars_live = None;
                    self.bars_file = Some(a.into());
                }
            },
        }
    }
    /// Fertige Antwort des Live-Abrufs übernehmen (jeder Schritt; billig, wenn nichts da ist).
    pub fn poll_bars(&mut self) {
        let Some(r) = self.bars_live.as_ref().and_then(|l| l.poll()) else {
            return;
        };
        self.apply_live_bars(r);
    }
    /// Erste Antwort abwarten (Aufnahmen und `--befehl`, damit das Bild den Feed schon zeigt).
    pub fn wait_bars(&mut self, max: std::time::Duration) {
        if let Some(r) = self.bars_live.as_ref().and_then(|l| l.wait(max)) {
            self.apply_live_bars(r);
        }
    }
    fn apply_live_bars(&mut self, r: Result<serde_json::Value, String>) {
        let msg = match r.and_then(|doc| attach_feed(&mut self.world, &doc)) {
            Ok(n) => format!("Nachtleben: {n} Bars live"),
            Err(e) => format!("Bar-Feed: {e} – letzter Stand bleibt"),
        };
        eprintln!("{msg}");
    }
    /// Bar-Feed (neu) an die Stadt hängen; Meldung für Befehlszeile und Konsole.
    pub fn reload_bars(&mut self) -> Result<String, String> {
        if let Some(l) = &self.bars_live {
            return Ok(format!("Bar-Feed wird geladen: {}", l.url));
        }
        let Some(p) = self.bars_file.clone() else {
            self.world.city.bars = None;
            return Ok("Kein Bar-Feed – bars <Datei|live>".into());
        };
        let n = load_bars(&mut self.world, &p)?;
        Ok(format!("{n} Bars aus {}", p.display()))
    }
    /// Befehlszeile bedienen (Welt steht): Tasten und getippter Text, danach die Folgen der Befehle.
    fn step_console(&mut self, keys: &Keys) {
        use crate::console::{Ctx, Key};
        let p = |k: KeyCode| keys.pressed.contains(&k);
        let held = |k: KeyCode| keys.held.contains(&k);
        let shift = held(KeyCode::ShiftLeft) || held(KeyCode::ShiftRight);
        let word = held(KeyCode::ControlLeft)
            || held(KeyCode::ControlRight)
            || held(KeyCode::AltLeft)
            || held(KeyCode::AltRight);
        let mut list: Vec<Key> = keys.typed.chars().map(Key::Char).collect();
        for (k, key) in [
            (KeyCode::Escape, Key::Escape),
            (KeyCode::Tab, Key::Tab),
            (KeyCode::ArrowRight, Key::Right),
            (KeyCode::ArrowLeft, Key::Left),
            (KeyCode::ArrowUp, Key::Up),
            (KeyCode::ArrowDown, Key::Down),
            (
                KeyCode::Backspace,
                if word {
                    Key::DeleteWord
                } else {
                    Key::Backspace
                },
            ),
            (KeyCode::Enter, Key::Enter),
            (KeyCode::NumpadEnter, Key::Enter),
        ] {
            if p(k) {
                list.push(key);
            }
        }
        if self.kbd.on {
            let (pad, e) = if self.kbd.slot == 0 {
                (keys.pad, keys.pad_pressed)
            } else {
                (keys.pad2, keys.pad2_pressed)
            };
            list.extend(self.kbd.step(&pad, &e));
        } else if keys.pad_pressed.b {
            list.push(Key::Escape);
        }
        let now = self.world.time;
        for key in list {
            let mut ctx = Ctx {
                world: &mut self.world,
                places: &self.places,
                actions: Vec::new(),
                debug: self.debug,
            };
            self.console.key(key, &mut ctx, now, shift);
            self.debug = ctx.debug;
            let actions = std::mem::take(&mut ctx.actions);
            self.console_actions(actions, now);
            if !self.console.open {
                break;
            }
        }
        if !self.console.open {
            self.kbd.on = false;
        }
    }
    fn console_actions(&mut self, actions: Vec<crate::console::Action>, now: f64) {
        use crate::console::Action;
        for a in actions {
            match a {
                Action::Teleport { x, y, .. } => {
                    self.teleport = Some(((x, y), None));
                    self.teleport_auto = true;
                }
                Action::Waypoint(None) => self.nav.clear(),
                Action::Waypoint(Some((x, y, _))) => {
                    self.nav.clear();
                    self.nav.toggle((x, y), 0.);
                }
                Action::Stats => self.screen = Screen::Stats(false),
                Action::Cheat(_) => {
                    self.stats.bump("cheats");
                    self.stats_total.bump("cheats");
                }
                Action::Money(m) => self.tracker.set_money(m),
                Action::Bars(arg) => {
                    self.set_bars_source(arg.as_deref());
                    let (msg, ok) = match self.reload_bars() {
                        Ok(m) => (m, true),
                        Err(e) => (e, false),
                    };
                    self.console.log.push((msg, ok, now));
                }
                Action::Graphics(None) => self.toggle_graphics(),
                Action::Graphics(Some(mode)) => {
                    self.set_graphics(berlin_engine::graphics::GraphicsSettings {
                        mode,
                        ..self.graphics
                    })
                }
                Action::Quality(quality) => {
                    self.set_graphics(berlin_engine::graphics::GraphicsSettings {
                        quality,
                        ..self.graphics
                    })
                }
            }
        }
    }
    /// Befehlszeile ausführen, ohne sie zu öffnen (`--befehl`, Aufnahmen): Ergebnis als Meldung.
    pub fn run_command(&mut self, line: &str) -> crate::console::Outcome {
        let mut ctx = crate::console::Ctx {
            world: &mut self.world,
            places: &self.places,
            actions: Vec::new(),
            debug: crate::console::Debug {
                physics: self.physdebug.open,
                engine_sound: self.enginedebug.open,
                ..self.debug
            },
        };
        let r = crate::console::execute(line, &mut ctx);
        self.debug = ctx.debug;
        self.physdebug.open = ctx.debug.physics;
        self.enginedebug.open = ctx.debug.engine_sound;
        // beide Anzeigen teilen sich die Tasten: die zuletzt geöffnete gewinnt
        if self.enginedebug.open && self.physdebug.open {
            if line.contains("motor") || line.contains("sound") || line.contains("f4") {
                self.physdebug.open = false;
            } else {
                self.enginedebug.open = false;
            }
        }
        let actions = std::mem::take(&mut ctx.actions);
        let now = self.world.time;
        self.console_actions(actions, now);
        r
    }
    /// Wo die Navigation den Spieler sieht: das gefahrene Auto bzw. die Figur.
    fn nav_pos(&self) -> (f64, f64) {
        let w = &self.world;
        w.player
            .in_car
            .and_then(|id| w.car(id))
            .map_or((w.player.x, w.player.y), |c| (c.x, c.y))
    }
    /// Route nachführen; Ankunft bzw. „keine Route“ als Meldung.
    fn step_nav(&mut self) {
        use berlin_sim::routing::Mode;
        let mode = if self.world.player.in_car.is_some() {
            Mode::Car
        } else {
            Mode::Foot
        };
        let pos = self.nav_pos();
        let text = |e| match e {
            crate::nav::NavEvent::Arrived => "Wegpunkt erreicht",
            crate::nav::NavEvent::NoRoute => "Keine Route zum Wegpunkt",
        };
        if let Some(e) = self.nav.step(pos, mode, self.world.time) {
            self.world.notice = Some(berlin_sim::world::Notice {
                text: text(e).into(),
                t: 2.,
            });
        }
        // Koop: Spieler 2 folgt demselben Wegpunkt auf eigener Route; wer ankommt, löscht ihn für beide
        if self.world.coop() {
            self.nav2.follow(&self.nav);
            let (pos2, mode2) = self.nav2_pos();
            if let Some(e) = self.nav2.step(pos2, mode2, self.world.time) {
                if matches!(e, crate::nav::NavEvent::Arrived) {
                    self.nav.clear();
                }
                if let Some(s) = self.world.p2.as_mut() {
                    s.notice = Some(berlin_sim::world::Notice {
                        text: text(e).into(),
                        t: 2.,
                    });
                }
            }
        } else if self.nav2.waypoint.is_some() {
            self.nav2.clear();
        }
    }
    /// Lage und Fortbewegung von Spieler 2 für die Route.
    fn nav2_pos(&self) -> ((f64, f64), berlin_sim::routing::Mode) {
        use berlin_sim::routing::Mode;
        let w = &self.world;
        let Some(s) = w.p2.as_ref() else {
            return ((0., 0.), Mode::Foot);
        };
        let car = s.player.in_car.and_then(|id| w.car(id));
        (
            car.map_or((s.player.x, s.player.y), |c| (c.x, c.y)),
            if car.is_some() { Mode::Car } else { Mode::Foot },
        )
    }
    /// `settings.json` schreiben: Steuerschema und (abweichende) Tastenbelegung.
    fn write_settings(&self) {
        if let Some(st) = &self.storage {
            let v = serde_json::json!({
                "controls": if self.diablo { "diablo" } else { "classic" },
                "bindings": self.bindings.to_json(),
                "grafik": self.graphics_saved.mode.key(),
                "qualitaet": self.graphics_saved.quality.key(),
            });
            if let Err(e) = std::fs::write(settings_path(st), v.to_string()) {
                eprintln!("Einstellungen nicht gespeichert: {e}");
            }
        }
    }
    /// Ausschnitte dieses Bildes zum Aussortieren (ohne Koop: die Kamera der Welt).
    fn spots(&self) -> crate::coopview::Spots {
        let c = self.world.camera;
        if self.world.coop() {
            crate::coopview::Spots::from_views(&self.views)
        } else {
            crate::coopview::Spots::one(c.x, c.y, c.zoom)
        }
    }
    /// Koop-HUD: Gemeinsames (Geld, Uhr, Auftrag) übers ganze Bild, je Spieler sein Block in seiner Bildhälfte
    /// (Spieler 2 rechts, gezeichnet auf seinem Platz per Sitztausch), im geteilten Bild an der Trennlinie ein Pfeil
    /// zum anderen Spieler.
    fn coop_hud(
        &mut self,
        engine: Option<&berlin_sim::soundscape::EngineState>,
        warn: Option<&'static str>,
        nav: &crate::nav::NavView,
        camera: &berlin_engine::camera::Camera,
        viewport: Vec2,
        out: &mut berlin_engine::hud::Hud,
    ) {
        use crate::hud::Parts;
        let views = self.views.clone();
        let full = out.width;
        let half = full / 2.;
        let merged = views.count < 2;
        let only = |player: bool| Parts {
            shared: !player,
            player,
            // gemeinsames Bild: ein Zielpfeil übers ganze Bild; geteilt: je Hälfte
            arrow: if player { !merged } else { merged },
            dx: 0.,
        };
        crate::hud::draw(
            &self.world,
            engine,
            warn,
            nav,
            camera,
            viewport,
            out,
            only(false),
        );
        out.width = half;
        crate::hud::draw(
            &self.world,
            engine,
            warn,
            nav,
            camera,
            viewport,
            out,
            only(true),
        );
        let cam2 = views.cams[(views.count == 2) as usize].clone();
        let from = out.items.len();
        let nav2 = self.nav2.view(self.nav2_pos().0);
        self.world.swap_seat();
        let warn2 = self.world.road_warning();
        crate::hud::draw(
            &self.world,
            None,
            warn2,
            &nav2,
            &cam2,
            viewport,
            out,
            Parts {
                shared: false,
                player: true,
                arrow: !merged,
                dx: half,
            },
        );
        let items2 = assist_items(&self.world);
        self.world.swap_seat();
        out.shift_since(from, half);
        out.width = full;
        // Räder von Spieler 2 liegen schon in Bildschirmlage (Mitte seiner Hälfte)
        if self.assist[1].open && self.screen == Screen::Playing {
            let age = self.real_t - self.assist[1].opened_at;
            crate::wheel::draw_assist(out, &self.assist[1], &items2, age);
        }
        if self.wheel_p2.open
            && self.screen == Screen::Playing
            && let Some(s) = self.world.p2.as_ref()
        {
            crate::wheel::draw(
                out,
                &self.wheel_p2,
                &s.player.combat,
                self.real_t - self.wheel_p2.opened_at,
                true,
            );
        }
        // geteilt: Pfeil an der Linie zum anderen Spieler, mit Entfernung
        if views.count == 2 && views.line > 0.05 {
            let (Some(a), Some(b)) = (
                Some((self.world.player.x, self.world.player.y)),
                self.world.p2.as_ref().map(|s| (s.player.x, s.player.y)),
            ) else {
                return;
            };
            let meters = ((b.0 - a.0).hypot(b.1 - a.1) / self.world.city.scale).round();
            let n = views.normal;
            let center = Vec2::new(full / 2., 360.);
            for (k, dir) in [(0usize, n), (1, -n)] {
                // knapp vor der Linie auf der eigenen Seite, Spitze zur Linie; entlang der Linie nach unten
                // versetzt (die Bildmitte gehört dem Zielpfeil)
                let along = Vec2::new(-n.y, n.x);
                let along = if along.y < 0. { -along } else { along };
                let p = center - dir * 46. + along * 100.;
                let col = crate::play::PLAYER_RING[1 - k];
                let ang = dir.y.atan2(dir.x);
                let alpha = views.line;
                out.triangle(p.x, p.y, 15., ang, [0., 0., 0., 0.7 * alpha]);
                out.triangle(p.x, p.y, 12., ang, [col[0], col[1], col[2], alpha]);
                let t = p - dir * 26.;
                out.text(
                    &format!("{meters} m"),
                    t.x,
                    t.y + 5.,
                    14.,
                    [1., 1., 1., alpha],
                    berlin_engine::hud::Align::Center,
                    true,
                );
            }
        }
    }
    /// Aufnahmen (`--bildschirm konsole-pad`): Bildschirmtastatur zur offenen Befehlszeile.
    pub fn open_pad_keyboard(&mut self) {
        self.kbd = crate::padkbd::PadKbd::new(0);
    }
    /// Befehlszeile per LB + RB öffnen (nur im laufenden Spiel, nicht über Karte, Rückfrage oder Ergebnis).
    fn chord_console(&mut self, slot: u8) {
        if self.screen != Screen::Playing
            || self.console.open
            || self.bigmap.open
            || self.world.loading
            || self.teleport.is_some()
            || matches!(self.world.mission.state, State::Success | State::Failed)
        {
            return;
        }
        self.console.open(&self.places);
        self.kbd = crate::padkbd::PadKbd::new(slot);
        self.ui_sound();
    }
    /// Spieler 2 tritt bei (Controller-Platz `slot`).
    pub fn join_coop(&mut self, slot: u8) {
        if self.world.join_p2() {
            self.p2_pad = slot;
            self.ui_sound();
        } else if !self.world.coop() {
            self.world.notice = Some(berlin_sim::world::Notice {
                text: "Kein Platz für Spieler 2".into(),
                t: 2.,
            });
        }
    }
    pub fn leave_coop(&mut self) {
        self.world.leave_p2();
        self.p2_pad = 1;
        self.world.notice = Some(berlin_sim::world::Notice {
            text: "Spieler 2 hat das Spiel verlassen".into(),
            t: 2.,
        });
    }
    /// Eingaben von Spieler 2: nur sein Controller, dieselbe Belegung wie Spieler 1 (ohne Tastatur und Maus).
    fn p2_input(&mut self, keys: &Keys, bind: &crate::bindings::Bindings) -> Input {
        use crate::bindings::Action as B;
        let Some(s) = self.world.p2.as_ref() else {
            return Input::default();
        };
        let (pad, edges) = if self.p2_pad == 0 {
            (keys.pad, keys.pad_pressed)
        } else {
            (keys.pad2, keys.pad2_pressed)
        };
        let none = std::collections::HashSet::new();
        let k2 = Keys {
            held: &none,
            pressed: &none,
            pad,
            pad_pressed: edges,
            pad2: Default::default(),
            pad2_pressed: Default::default(),
            mouse: Default::default(),
            typed: "",
        };
        let drives_train = s
            .player
            .ride
            .as_ref()
            .is_some_and(|r| r.kind == berlin_sim::ride::RideKind::Driver);
        let mut i = input_from(&k2, s.player.in_car.is_some() || drives_train, bind);
        // Waffenrad wie bei Spieler 1 am Controller: LB tippen = vorige Waffe, halten = Rad (in seiner Bildhälfte)
        let alive_foot =
            s.player.in_car.is_none() && !s.player.combat.dead && s.player.ride.is_none();
        let cur = s.player.combat.weapon;
        let n = berlin_sim::combat::WEAPONS.len();
        let t = self.real_t;
        let center = Vec2::new(self.hud_width * 0.75, 360.);
        let in_car = s.player.in_car.is_some();
        let siren_car = s
            .player
            .in_car
            .and_then(|id| self.world.car(id))
            .is_some_and(|c| c.has_siren());
        let wh = &mut self.wheel_p2;
        let was = wh.open;
        let o = wh.pad_step(
            t,
            center,
            bind.pad_pressed(&k2, B::WeaponWheel),
            edges.a,
            edges.b,
            (pad.rx, pad.ry),
            alive_foot,
            cur,
            n,
        );
        if let Some(k) = o.pick {
            i.combat.weapon_slot = k as u8 + 1;
        }
        if was || wh.open {
            // bei offenem Rad kein Schuss, Zielen eingefroren; A und B gehören dem Rad
            i.combat.fire = false;
            i.combat.fire_pressed = false;
            i.combat.kick = false;
            i.combat.reload = false;
            i.combat.aim_x = 0.;
            i.combat.aim_y = 0.;
            i.sprint = false;
            i.action = false;
            i.action_held = false;
            i.jump = false;
        }
        assist_input(
            &mut self.assist[1],
            &mut self.horn[1],
            &mut i,
            t,
            center,
            &k2,
            bind,
            in_car,
            siren_car,
            1. / 60.,
        );
        i
    }
    pub fn pause(&mut self) {
        self.screen = Screen::Paused;
        self.menu = crate::menu::pause_menu()
            .with_graphics(self.graphics.mode)
            .with_coop(self.world.coop());
    }
    /// Grafik wählen (Menü, Taste, Konsole): sofort wirksam, gespeichert, als Meldung bestätigt. `--grafik` setzt
    /// `self.graphics` direkt und schreibt nichts.
    pub fn set_graphics(&mut self, g: berlin_engine::graphics::GraphicsSettings) {
        self.graphics = g;
        self.graphics_saved = g;
        self.menu.set_graphics(g.mode);
        self.write_settings();
        self.world.notice = Some(berlin_sim::world::Notice {
            text: format!("Grafik: {} · Qualität {}", g.mode.label(), g.quality.key()),
            t: 1.6,
        });
    }
    fn toggle_graphics(&mut self) {
        let mut g = self.graphics;
        g.mode = g.mode.toggled();
        self.set_graphics(g);
    }
    fn ui_sound(&self) {
        if let Some(a) = &self.audio {
            a.play(berlin_audio::synth::Sfx::Ui);
        }
    }
    /// Von einer Tafel (Steuerung, Statistik, Über das Spiel) zurück zum Titel bzw. in die Pause.
    fn back_from_page(&mut self, from_title: bool) {
        if from_title {
            self.screen = Screen::Title;
            return;
        }
        // zurück in die Pause: deren Menü (die Tafel ist auch vom Titel aus erreichbar)
        let keep = self
            .menu
            .items
            .iter()
            .any(|i| i.action == crate::menu::Action::Resume);
        self.screen = Screen::Paused;
        if !keep {
            self.menu = crate::menu::pause_menu()
                .with_graphics(self.graphics.mode)
                .with_coop(self.world.coop());
        }
    }
    /// Menübildschirme; `true` = der Schritt ist damit erledigt.
    fn step_screens(&mut self, keys: &Keys, dt: f64) -> bool {
        use crate::menu::{Action, Pick};
        let mk = crate::menu::MenuKeys::from(keys, self.stick_prev);
        let bk = crate::bindmenu::BindKeys::from(keys, (self.stick_prev_x, self.stick_prev));
        self.stick_prev = keys.pad.ly;
        self.stick_prev_x = keys.pad.lx;
        // solange Kacheln fehlen, steht die Welt ohnehin still; weiterladen auch in den Menüs
        if self.world.loading
            && (matches!(
                self.screen,
                Screen::Paused
                    | Screen::Controls(false)
                    | Screen::Stats(false)
                    | Screen::Bindings(false)
                    | Screen::About(false)
            ) || (self.screen == Screen::Playing
                && (self.teleport.is_some() || self.console.open)))
        {
            self.world.update(&Input::default(), dt);
        }
        match self.screen {
            Screen::Playing if self.console.open => {
                self.step_console(keys);
                true
            }
            Screen::Playing if self.teleport.is_some() => {
                // Teleport-Rückfrage: Welt steht; Ziel nachladen, bis es da ist
                if let Some(((x, y), None)) = self.teleport.clone() {
                    match self.world.find_teleport_spot(x, y) {
                        Some(berlin_sim::world::TeleportSpot::Spot {
                            x: sx,
                            y: sy,
                            angle,
                            name,
                        }) => self.teleport = Some(((x, y), Some((sx, sy, angle, name)))),
                        Some(berlin_sim::world::TeleportSpot::Pending) => {
                            self.world.city.pump();
                        }
                        None => {
                            self.teleport = None;
                            self.world.city.release("teleport");
                        }
                    }
                }
                if self.teleport_auto {
                    // Befehlszeile: ohne Rückfrage, sobald das Ziel geladen ist
                    if self.teleport.as_ref().is_some_and(|t| t.1.is_some()) {
                        self.teleport_auto = false;
                        self.confirm_teleport(true);
                    } else if self.teleport.is_none() {
                        self.teleport_auto = false;
                    }
                    return true;
                }
                let (yes, no) = crate::menu::teleport_buttons(self.hud_width);
                let m = keys.mouse;
                let hit = |r: [f32; 4]| {
                    m.left_pressed
                        && m.hud.is_some_and(|p| {
                            p.x >= r[0] && p.x <= r[0] + r[2] && p.y >= r[1] && p.y <= r[1] + r[3]
                        })
                };
                if bk.left || bk.right {
                    self.teleport_no = bk.right;
                    self.ui_sound();
                }
                if hit(yes) || (mk.confirm && !self.teleport_no) {
                    self.ui_sound();
                    self.teleport_no = false;
                    self.confirm_teleport(true);
                } else if mk.back || hit(no) || (mk.confirm && self.teleport_no) {
                    self.ui_sound();
                    self.teleport_no = false;
                    self.confirm_teleport(false);
                }
                true
            }
            Screen::Playing => {
                use crate::bindings::Action as B;
                // Start auf dem zweiten Controller: beitreten bzw. (als Spieler 2) Pause
                let start2 = self
                    .bindings
                    .pad_of(B::Pause)
                    .is_some_and(|b| b.pressed(&keys.pad2_pressed));
                if start2 && !self.world.coop() && !self.world.loading {
                    self.join_coop(1);
                    return false;
                }
                let pause = self.bindings.pressed(keys, B::Pause)
                    || (start2 && self.world.coop() && self.p2_pad == 1);
                if pause && self.bigmap.open && !self.bindings.pad_pressed(keys, B::Pause) {
                    self.bigmap.open = false;
                    return false;
                }
                let enter = self.bindings.pressed(keys, B::Console);
                if enter
                    && !self.bigmap.open
                    && !self.world.loading
                    && !matches!(self.world.mission.state, State::Success | State::Failed)
                {
                    self.console.open(&self.places);
                    // mit der Tastatur geöffnet: keine Bildschirmtastatur
                    self.kbd = Default::default();
                    self.ui_sound();
                    return true;
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
                // Entwickler-Link unten links: Zeiger darüber hebt ihn hervor, Klick öffnet den Browser
                self.title_link_hover = self
                    .title_link
                    .zip(keys.mouse.hud)
                    .is_some_and(|(r, p)| crate::menu::in_rect(r, p));
                if self.title_link_hover && keys.mouse.left_pressed {
                    crate::menu::open_url(crate::menu::CREDIT_URL);
                }
                if !self.world.loading {
                    let mp = self.menu.mouse(
                        self.hud_width / 2.,
                        crate::menu::TITLE_MENU_Y,
                        &keys.mouse,
                    );
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
                        Some(Pick::Choose(Action::Graphics)) => self.toggle_graphics(),
                        Some(Pick::Choose(Action::About)) => {
                            self.ui_sound();
                            self.about = Default::default();
                            self.screen = Screen::About(true)
                        }
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
                    Some(Pick::Choose(Action::Coop)) => {
                        if self.world.coop() {
                            self.leave_coop();
                        } else {
                            // zweiter Controller, sonst der erste (Spieler 1 bleibt bei Tastatur und Maus)
                            let slot = if keys.pad2.connected { 1 } else { 0 };
                            if slot == 0 && !keys.pad.connected {
                                self.world.notice = Some(berlin_sim::world::Notice {
                                    text: "Spieler 2 braucht einen Controller".into(),
                                    t: 2.5,
                                });
                            } else {
                                self.join_coop(slot);
                            }
                        }
                        self.screen = Screen::Playing;
                    }
                    Some(Pick::Choose(Action::Controls)) => self.screen = Screen::Controls(false),
                    Some(Pick::Choose(Action::Stats)) => self.screen = Screen::Stats(false),
                    Some(Pick::Choose(Action::Graphics)) => self.toggle_graphics(),
                    Some(Pick::Choose(Action::About)) => {
                        self.about = Default::default();
                        self.screen = Screen::About(false)
                    }
                    Some(Pick::Choose(Action::Title)) => {
                        self.write_stats();
                        self.screen = Screen::Title;
                        self.menu = crate::menu::title_menu(self.has_save())
                            .with_graphics(self.graphics.mode);
                    }
                    _ => {}
                }
                if mk.up || mk.down || mk.confirm || mk.back || mp.is_some() {
                    self.ui_sound();
                }
                true
            }
            Screen::Controls(_)
                if keys.pressed.contains(&KeyCode::ArrowLeft)
                    || keys.pressed.contains(&KeyCode::ArrowRight)
                    || keys.pad_pressed.left
                    || keys.pad_pressed.right =>
            {
                self.diablo = !self.diablo;
                self.ui_sound();
                self.write_settings();
                true
            }
            Screen::Controls(from_title) if mk.confirm || keys.mouse.left_pressed => {
                // Enter/A bzw. Klick: Belegung ändern
                self.ui_sound();
                self.bindmenu = Default::default();
                self.screen = Screen::Bindings(from_title);
                if from_title {
                    self.world.update(&Input::default(), dt);
                }
                true
            }
            Screen::Bindings(from_title) => {
                let out = self
                    .bindmenu
                    .step(bk, &mut self.bindings, self.real_t, self.hud_width);
                self.real_t += dt;
                match out {
                    crate::bindmenu::Out::Changed => {
                        self.ui_sound();
                        self.write_settings();
                    }
                    crate::bindmenu::Out::Back => {
                        self.ui_sound();
                        self.screen = Screen::Controls(from_title);
                    }
                    crate::bindmenu::Out::Stay => {
                        if bk.up || bk.down || bk.left || bk.right {
                            self.ui_sound();
                        }
                    }
                }
                if from_title {
                    self.world.update(&Input::default(), dt);
                }
                true
            }
            Screen::About(from_title) => {
                let w = self.hud_width;
                let k = crate::about::AboutKeys::from(keys, |p| crate::about::tab_at(w, p));
                match self.about.step(k, dt as f32) {
                    crate::about::Out::Back => {
                        self.ui_sound();
                        self.back_from_page(from_title);
                    }
                    crate::about::Out::Switched => self.ui_sound(),
                    crate::about::Out::Stay => {}
                }
                if from_title {
                    self.world.update(&Input::default(), dt);
                }
                true
            }
            Screen::Controls(from_title) | Screen::Stats(from_title) => {
                let click = keys.mouse.left_pressed || keys.mouse.right_pressed;
                if mk.back || mk.confirm || click {
                    self.ui_sound();
                    self.back_from_page(from_title);
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

/// Gas bzw. Bremse aus der Belegung: Taste = voll, Trigger über die Kennlinie (Totzone, progressiv), andere
/// Controller-Taste = voll. Die stärkere Quelle zählt.
fn pedal(
    b: &crate::bindings::Bindings,
    keys: &Keys,
    a: crate::bindings::Action,
    gamma: f32,
) -> f64 {
    use crate::bindings::{PadButton, trigger_curve};
    let key: f32 = if b.key_held(keys, a) { 1. } else { 0. };
    let pad = match b.pad_of(a) {
        Some(PadButton::LT | PadButton::RT) => trigger_curve(b.pad_value(keys, a), gamma),
        Some(_) => b.pad_value(keys, a),
        None => 0.,
    };
    f64::from(key.max(pad))
}

/// Tasten und Gamepad → abstrakte Eingabe über die frei belegbare Steuerung (`bindings.rs`).
pub fn input_from(keys: &Keys, driving: bool, b: &crate::bindings::Bindings) -> Input {
    use crate::bindings::{Action as A, BRAKE_GAMMA, THROTTLE_GAMMA, steer_curve};
    let axis = |pos: bool, neg: bool| (pos as i32 - neg as i32) as f64;
    let p = &keys.pad;
    let (plx, ply) = radial_deadzone(p.lx, p.ly, 0.18);
    let kx = axis(b.key_held(keys, A::Right), b.key_held(keys, A::Left));
    let ky = axis(b.key_held(keys, A::Down), b.key_held(keys, A::Up));
    // Stick überstimmt die Tasten nur, wenn er ausgelenkt ist
    let lx = if plx != 0. { plx as f64 } else { kx };
    let ly = if ply != 0. { ply as f64 } else { ky };
    // Lenken: Stick über die Lenk-Kennlinie (feine Mitte), sonst die Tasten
    let stick = steer_curve(p.lx, b.steer_sens);
    let steer = if stick != 0. { stick as f64 } else { kx };
    Input {
        move_x: if driving { 0. } else { lx },
        move_y: if driving { 0. } else { ly },
        // zu Fuß und auf dem Fahrrad (Ausdauer)
        sprint: b.held(keys, A::Sprint),
        walk_slow: !driving && b.held(keys, A::Slow),
        throttle: if driving {
            pedal(b, keys, A::Throttle, THROTTLE_GAMMA)
        } else {
            0.
        },
        brake: if driving {
            pedal(b, keys, A::Brake, BRAKE_GAMMA)
        } else {
            0.
        },
        steer: if driving { steer.clamp(-1., 1.) } else { 0. },
        handbrake: driving && b.held(keys, A::Handbrake),
        horn: driving && b.held(keys, A::Horn),
        enter_exit: b.pressed(keys, A::EnterExit),
        action: b.pressed(keys, A::Use),
        action_held: b.held(keys, A::Use),
        esp_toggle: driving && b.pressed(keys, A::Esp),
        abs_toggle: driving && b.pressed(keys, A::Abs),
        ride: !driving && b.pressed(keys, A::Ride),
        jump: !driving && b.pressed(keys, A::Jump),
        combat: combat_input(keys, driving, b),
        ..Default::default()
    }
}

/// Kampf zu Fuß über die Belegung (Standard: Strg/RT feuert, V/B tritt, R/X lädt nach, Q/RB nächste Waffe; das
/// Waffenrad liegt in `Play::step`), 1–6 wählen fest, der rechte Stick zielt.
pub fn combat_input(
    keys: &Keys,
    driving: bool,
    b: &crate::bindings::Bindings,
) -> berlin_sim::combat::CombatInput {
    use crate::bindings::Action as A;
    if driving {
        return Default::default();
    }
    let pressed = |k: KeyCode| keys.pressed.contains(&k);
    let p = &keys.pad;
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
        fire: b.held(keys, A::Fire),
        fire_pressed: b.pressed(keys, A::Fire),
        kick: b.pressed(keys, A::Kick),
        reload: b.pressed(keys, A::Reload),
        weapon_next: b.pressed(keys, A::NextWeapon),
        // Waffenrad-Taste: tippen = vorige Waffe, halten = Waffenrad (Play::step)
        weapon_prev: false,
        weapon_slot: digits
            .iter()
            .position(|&d| pressed(d))
            .map_or(0, |i| i as u8 + 1),
        aim_x: rx as f64,
        aim_y: ry as f64,
        aim_world: None,
    }
}

/// Zweirad: Schräglage (rad, positiv = nach rechts), Nickwinkel (Wheelie positiv) und ob es gestürzt liegt.
fn two_wheel_pose(c: &berlin_sim::car::Car) -> (f32, f32, bool) {
    let lying = c
        .phys
        .as_ref()
        .is_some_and(|s| s.fallen.is_some() && c.driver.is_none());
    let (lean, pitch) = c
        .dyn_state
        .as_ref()
        .filter(|_| c.driver.is_some())
        .map_or((0., 0.), |d| (d.lean, d.wheelie - d.stoppie));
    (lean as f32, pitch as f32, lying)
}

/// Nicken und Wanken des gefahrenen Autos (vehicles.js bodyShift): Bremsen taucht vorn ein, Kurven drücken nach
/// außen. Längs/quer in Welteinheiten; Karosserie und Silhouette nutzen denselben Wert.
fn body_shift(c: &berlin_sim::car::Car) -> (f32, f32) {
    let Some(d) = c
        .dyn_state
        .as_ref()
        .filter(|_| !c.wrecked && c.role == berlin_sim::car::Role::Player)
    else {
        return (0., 0.);
    };
    // Fahrphysik mit Fahrzeugdaten: Nick- und Wankwinkel je g aus dem Fahrwerk (Grad), rund 0,8 px je Grad; die
    // Federung über Bodenwellen (Kopfstein) rüttelt längs mit (1 cm Federweg ≈ 1 px)
    if let (Some(s), Some(v)) = (
        c.phys.as_ref(),
        berlin_sim::vehdata::game_vehicle(c.model_name()),
    ) {
        const PX_PER_DEG: f64 = 0.8;
        let g = berlin_sim::vehdata::G;
        let pitch = -d.ax / g * v.chassis.pitch * PX_PER_DEG + (s.susp[0] - s.susp[1]) * 100.;
        // beim Kippen hebt sich die Innenseite: der Aufbau wandert sichtbar nach außen
        let roll = -d.ay / g * v.chassis.roll * PX_PER_DEG - d.ay.signum() * s.tip * 5.;
        return (pitch.clamp(-6., 6.) as f32, roll.clamp(-6., 6.) as f32);
    }
    let k = berlin_sim::carmodels::spec_of(c.model_name()).h * 0.35;
    (
        (-d.ax * k).clamp(-6., 6.) as f32,
        (-d.ay * k).clamp(-6., 6.) as f32,
    )
}

/// Silhouette eines Fahrzeugs (`scene.wgsl silhouette_fs`): Pkw und Nutzfahrzeuge in der Form ihres Fahrzeugbilds,
/// Zweiräder als Ellipse. `color` = Linienfarbe, Alpha = Stärke.
fn vehicle_silhouette(c: &berlin_sim::car::Car, depth: f32, color: [f32; 4]) -> Body {
    let (x, y, a) = (c.x as f32, c.y as f32, c.angle as f32);
    if c.kind_info().bike || c.kind_info().moto {
        return Body {
            center: [x, y],
            half: [c.hw as f32, c.hh as f32 * 0.8],
            angle: a,
            shape: 1.,
            depth,
            color,
        };
    }
    let (sx, sy) = body_shift(c);
    let (fx, fy) = (a.cos(), a.sin());
    let (rx, ry) = (-fy, fx);
    Body {
        center: [x + fx * sx + rx * sy, y + fy * sx + ry * sy],
        half: [
            c.hw as f32 + crate::carart::PAD,
            c.hh as f32 + crate::carart::PAD,
        ],
        angle: a,
        shape: 16. + crate::carart::model_index(c.model_name()) as f32,
        depth,
        color,
    }
}

/// Silhouetten (Linienfarbe + Stärke): die eigene Figur bzw. das eigene Auto warmweiß und deutlich, alle anderen
/// neutral und zurückhaltend – man soll sich selbst sofort finden, ohne dass die Straße voller Umrisse ist.
pub const SIL_OWN: [f32; 4] = [1., 0.93, 0.8, 0.95];
pub const SIL_OTHER: [f32; 4] = [0.9, 0.92, 0.95, 0.32];

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

/// Rad bzw. E-Roller von oben: zwei Räder, Rahmen bzw. Trittbrett mit Lenker, darauf der Fahrer im Trikot
/// (beim Rad mit Tretbewegung). Liegende Räder kippen zur Seite, ohne Fahrer.
/// Aufnahmen: Spielfigur direkt nördlich eines Hauses abstellen; Fassade und Dach ragen dort in der Schrägansicht über
/// sie (Silhouette sichtbar). Gesucht wird im Raster um die Figur nach einem Hausrand, dessen Nordseite frei ist.
fn demo_cover(w: &mut World) {
    let (px, py) = (w.player.x, w.player.y);
    for r in (40..600).step_by(20) {
        for k in 0..24 {
            let a = k as f64 / 24. * std::f64::consts::TAU;
            let (x, y) = (px + a.cos() * r as f64, py + a.sin() * r as f64);
            if w.city.in_building(x, y).is_none() {
                continue;
            }
            // nach Norden bis zum Hausrand, dann ein Stück weiter
            let mut ny = y;
            while w.city.in_building(x, ny).is_some() && ny > y - 400. {
                ny -= 2.;
            }
            let (tx, ty) = (x, ny - 14.);
            if w.city.in_building(tx, ty).is_none() && w.city.in_building(tx, ty - 10.).is_none() {
                (w.player.x, w.player.y) = (tx, ty);
                w.player.angle = -std::f64::consts::FRAC_PI_2;
                // im Auto (`--im-auto`): das Auto quer unter den Hausrand, halb verdeckt
                if let Some(id) = w.player.in_car
                    && let Some(c) = w.cars.iter_mut().find(|c| c.id == id)
                {
                    (c.x, c.y, c.angle) = (tx, ty - 6., 0.);
                    (c.vx, c.vy, c.ang_vel) = (0., 0., 0.);
                }
                w.camera.x = tx;
                w.camera.y = ty;
                return;
            }
        }
    }
}

/// Aufnahmen: in den nächsten U-Bahnhof hinunter, mit `ride` danach in den nächsten haltenden Zug; `true` = fertig.
/// Musterseite der Autos: alle Pkw-Modelle (Spalten nach Atlasreihenfolge) und die Nutzfahrzeuge in Reihen um die
/// Figur, als abgestellte Autos ohne Fahrer; der übrige Verkehr in der Nähe wird entfernt.
fn car_lab_park(w: &mut berlin_sim::world::World) {
    use berlin_sim::car::{Car, Role};
    // freie Fläche (kein Haus, kein Hindernis) von 560 × 330 px in der Nähe suchen und die Figur dorthin stellen
    let (sx, sy) = (w.player.x, w.player.y);
    let lvl = w.player.level.lvl;
    let mut spot = (sx, sy);
    'search: for ring in 0..24 {
        let r = ring as f64 * 120.;
        let n = (ring * 8).max(1);
        for k in 0..n {
            let a = k as f64 / n as f64 * std::f64::consts::TAU;
            let (cx, cy) = (sx + a.cos() * r, sy + a.sin() * r);
            let mut free = true;
            'cells: for gx in -14..=14 {
                for gy in -8..=8 {
                    if !berlin_sim::footpath::foot_free(
                        w,
                        cx + gx as f64 * 20.,
                        cy + gy as f64 * 20.,
                        lvl,
                    ) {
                        free = false;
                        break 'cells;
                    }
                }
            }
            if free {
                spot = (cx, cy);
                break 'search;
            }
        }
    }
    (w.player.x, w.player.y) = spot;
    let (px, py) = spot;
    w.cars.retain(|c| (c.x - px).hypot(c.y - py) > 900.);
    w.peds.retain(|p| (p.x - px).hypot(p.y - py) > 600.);
    let models: Vec<&'static str> = berlin_sim::carmodels::CAR_MODELS
        .iter()
        .map(|(m, _)| *m)
        .collect();
    let cols = 8;
    for (i, m) in models.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        let (x, y) = (px + (c as f64 - 3.5) * 62., py - 130. + r as f64 * 30.);
        let id = 900_000 + i as u32;
        let mut car = Car::new(id, x, y, 0., 0, Role::Parked, "car");
        car.set_model(m);
        car.controls.handbrake = true;
        w.cars.push(car);
    }
    for (i, k) in [
        "truck",
        "delivery",
        "garbage",
        "police",
        "ambulance",
        "bus",
        "motorcycle",
        "scooter",
    ]
    .iter()
    .enumerate()
    {
        let ki = berlin_sim::carmodels::kind(k);
        // Zweiräder neben den letzten Pkw der Reihe (zum Größenvergleich)
        let (x, y) = if i < 6 {
            (px - 260. + i as f64 * 104., py + 115.)
        } else {
            (px + 60. + (i - 6) as f64 * 40., py + 45.)
        };
        let mut car = Car::new(900_100 + i as u32, x, y, 0., ki.colors[0], Role::Parked, k);
        car.controls.handbrake = true;
        if ki.moto {
            // mit Fahrer, wie im Verkehr (ohne KI: es steht trotzdem)
            car.driver = Some(berlin_sim::car::Driver::Npc);
        }
        w.cars.push(car);
    }
}

/// Musterseite: jede Bauart (Spalten) aufrecht und eingelenkt, nach rechts in Schräglage, im Wheelie und gestürzt
/// (Zeilen), um (cx, cy) herum.
fn moto_lab_bodies(cx: f32, cy: f32, out: &mut Vec<Body>) {
    // vergrößert, damit die Teile im Bild zu erkennen sind
    const LAB_SCALE: f32 = 2.;
    use crate::motoart::{Pose, Style, bodies};
    let styles = [
        (Style::Sport, 0xc8102e, 10.35),
        (Style::Naked, 0x1d4ed8, 10.5),
        (Style::Cruiser, 0x15171b, 12.),
        (Style::Scooter, 0x2fa84f, 9.),
    ];
    let rows: [(f32, f32, f32, bool, bool); 4] = [
        (0., 0., 0.35, false, false),
        (0.75, 0., 0.1, false, true),
        (0., 0.35, 0., false, false),
        (0.6, 0., 0., true, false),
    ];
    for (i, &(st, paint, hw)) in styles.iter().enumerate() {
        for (j, &(lean, pitch, steer, lying, braking)) in rows.iter().enumerate() {
            let (jacket, helmet) = crate::motoart::rider_colors(i as u32 * 7 + j as u32);
            bodies(
                &Pose {
                    x: cx + (i as f32 - 1.5) * 40. * LAB_SCALE,
                    y: cy + (j as f32 - 1.5) * 26. * LAB_SCALE,
                    angle: 0.,
                    hw: hw * LAB_SCALE,
                    lean,
                    pitch,
                    steer,
                    lying,
                    braking,
                    rider: !lying,
                    paint,
                    jacket,
                    helmet,
                    depth: 0.5,
                },
                st,
                out,
            );
        }
    }
}

/// Sprung: die Figur (und die Waffe in der Hand) kommt der Kamera näher und wird größer, ihr Schatten (der erste
/// Körper der Figur) bleibt am Boden, rückt ab und wird blasser.
fn lift_bodies(
    out: &mut [Body],
    weapon: std::ops::Range<usize>,
    fig0: usize,
    (x, y): (f32, f32),
    z: f32,
) {
    let s = 1. + z * 0.045;
    let grow = |b: &mut Body| {
        b.center = [x + (b.center[0] - x) * s, y + (b.center[1] - y) * s];
        b.half = [b.half[0] * s, b.half[1] * s];
    };
    for b in &mut out[weapon] {
        grow(b);
    }
    if let Some((shadow, body)) = out[fig0..].split_first_mut() {
        shadow.center[0] += z * 0.6;
        shadow.center[1] += z * 0.6;
        shadow.color[3] *= 1. / (1. + z * 0.15);
        for b in body {
            grow(b);
        }
    }
}

pub fn demo_station_step(w: &mut World, ride: bool) -> bool {
    if w.player.ride.is_some() {
        return true;
    }
    if let Some(st) = w
        .player
        .inside
        .as_ref()
        .and_then(|i| w.station_by_id(&i.id))
        .cloned()
    {
        if !ride {
            return true;
        }
        // nächster fahrender Zug unter Tage: direkt als Fahrgast hinein (Aufnahmen sollen nicht minutenlang warten)
        let refs: Vec<berlin_sim::ride::Ref> = match w.transit.as_ref() {
            Some(tr) => w
                .transit_state
                .tracked
                .iter()
                .filter(|(pid, _)| tr.patterns[**pid].mode.rail())
                .flat_map(|(&pid, s)| {
                    s.veh.iter().map(move |v| berlin_sim::ride::Ref::Veh {
                        pid,
                        key: v.key.clone(),
                    })
                })
                .collect(),
            None => Vec::new(),
        };
        for r in refs {
            let Some(vs) = w.vehicle_state(&r) else {
                continue;
            };
            let c = vs.cars[2.min(vs.cars.len() - 1)];
            if !vs.underground || vs.dwelling || (c.x - st.x).hypot(c.y - st.y) > 20000. {
                continue;
            }
            let Some(tr) = w.transit.as_ref() else { break };
            let p = &tr.patterns[vs.pid];
            let i = vs.stop.saturating_sub(1);
            let (sx, sy, _) = berlin_sim::transit::point_on_shape(tr.shape_of(p), p.stops[i]);
            w.player.ride = Some(berlin_sim::ride::Ride {
                kind: berlin_sim::ride::RideKind::Passenger,
                r,
                mode: vs.mode,
                car: 2,
                last_stop: berlin_sim::ride::LastStop {
                    x: sx,
                    y: sy,
                    name: p.stop_names.get(i).cloned().unwrap_or_default(),
                    i,
                    pid: vs.pid,
                },
                since: w.time,
                line: p.name.clone(),
                dest: p.stop_names.last().cloned().unwrap_or_default(),
                speed: vs.speed,
                underground: true,
            });
            w.player.inside = None;
            return true;
        }
        if let Some(t) = w.trains_at(&st).into_iter().find(|t| t.dwelling) {
            let c = t.cars[2.min(t.cars.len() - 1)];
            (w.player.x, w.player.y) =
                st.to_world(c.0, t.dir as f64 * (berlin_sim::station::HALF - 6.));
            w.board_at_platform();
        }
        return false;
    }
    let (x, y) = (w.player.x, w.player.y);
    let Some(st) = w.stations_near(x, y, 1500.).into_iter().min_by(|a, b| {
        (a.x - x)
            .hypot(a.y - y)
            .total_cmp(&(b.x - x).hypot(b.y - y))
    }) else {
        return false;
    };
    let ex = st.exits[0].clone();
    (w.player.x, w.player.y) = (ex.x, ex.y);
    w.st_tick = 0;
    w.refresh_stations();
    w.update_station_presence(&Input {
        enter_exit: true,
        ..Default::default()
    });
    false
}

/// Aufnahmen: Figur an den Führerstand der nächsten Straßenbahn stellen und übernehmen.
fn demo_take_tram(w: &mut World) {
    use berlin_sim::ride::{Near, Ref};
    use berlin_sim::transit::Mode;
    let refs: Vec<Ref> = match w.transit.as_ref() {
        Some(tr) => w
            .transit_state
            .tracked
            .iter()
            .filter(|(pid, _)| tr.patterns[**pid].mode == Mode::Tram)
            .flat_map(|(&pid, s)| {
                s.veh.iter().map(move |v| Ref::Veh {
                    pid,
                    key: v.key.clone(),
                })
            })
            .collect(),
        None => return,
    };
    for r in refs {
        let Some(st) = w.vehicle_state(&r) else {
            continue;
        };
        let f = st.cars[0];
        if (f.x - w.camera.x).hypot(f.y - w.camera.y) > 900. {
            continue;
        }
        let (fx, fy) = (
            f.x + f.angle.cos() * (f.l / 2. - 4.),
            f.y + f.angle.sin() * (f.l / 2. - 4.),
        );
        (w.player.x, w.player.y) = (fx, fy);
        let near = Near {
            r,
            mode: Mode::Tram,
            dist: 0.,
            car: 0,
            front: 4.,
        };
        if w.take_train(&near) {
            return;
        }
    }
}

/// Bahnen (railart.js drawTrainCar): Wagenkasten, Dach mit Geräten, Zierlinie, Führerstand mit Scheinwerfern,
/// Schlusslichter, Stromabnehmer der Straßenbahn; dazu die Straßenbahngleise in der Fahrbahn.
fn rail_bodies(
    trains: &[berlin_sim::transitlive::Visible],
    tram_segs: &[(berlin_sim::city::Pt, berlin_sim::city::Pt)],
    w: &World,
    out: &mut Vec<Body>,
) {
    use berlin_sim::transit::Mode;
    // Gleise: zwei Schienen im Abstand der Normalspur
    for &(a, b) in tram_segs {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy);
        if l < 1. {
            continue;
        }
        let (nx, ny) = (-dy / l, dx / l);
        for off in [-7.2, 7.2] {
            out.push(Body {
                center: [
                    ((a.0 + b.0) / 2. + nx * off) as f32,
                    ((a.1 + b.1) / 2. + ny * off) as f32,
                ],
                half: [l as f32 / 2. + 0.5, 0.75],
                angle: dy.atan2(dx) as f32,
                shape: 4.,
                depth: 0.8492,
                color: [0.42, 0.42, 0.44, 0.9],
            });
        }
    }
    let sun = crate::play::world_light(w).sun;
    let (sdx, sdy) = (sun.dx as f32 * 6., sun.dy as f32 * 6.);
    for t in trains {
        let (side, roof, line) = match t.mode {
            Mode::Tram => (0xf2c230, 0xe8e4d8, 0x3a3a3a),
            Mode::SBahn => (0x9b2b25, 0x5b5f66, 0xd9a441),
            _ => (0xf0c419, 0x6a6457, 0x3a3a3a),
        };
        for (c, &lvl) in t.cars.iter().zip(&t.lvl) {
            let depth = if lvl >= 1 { 0.547 } else { 0.6205 };
            let (x, y, a) = (c.x as f32, c.y as f32, c.angle as f32);
            let (l, wd) = (c.l as f32, c.w as f32);
            let (fx, fy) = (a.cos(), a.sin());
            let push =
                |out: &mut Vec<Body>, cx: f32, cy: f32, hx: f32, hy: f32, d: f32, col: [f32; 4]| {
                    out.push(Body {
                        center: [cx, cy],
                        half: [hx, hy],
                        angle: a,
                        shape: 4.,
                        depth: d,
                        color: col,
                    })
                };
            push(
                out,
                x + sdx,
                y + sdy,
                l / 2.,
                wd / 2.,
                depth + 0.0006,
                [0., 0., 0., 0.28],
            );
            push(out, x, y, l / 2., wd / 2., depth, rgba(side, 1.));
            push(
                out,
                x,
                y,
                l / 2. - 2.,
                wd / 2. - 3.,
                depth - 0.0001,
                rgba(roof, 1.),
            );
            let mut u = -l / 2. + 16.;
            while u < l / 2. - 10. {
                push(
                    out,
                    x + fx * u,
                    y + fy * u,
                    4.,
                    wd / 2. - 5.,
                    depth - 0.0002,
                    shade(rgba(roof, 1.), 0.85),
                );
                u += 22.;
            }
            for k in [-1f32, 1.] {
                let (ox, oy) = (-fy * (wd / 2. - 0.6) * k, fx * (wd / 2. - 0.6) * k);
                push(
                    out,
                    x + ox,
                    y + oy,
                    l / 2.,
                    0.6,
                    depth - 0.0003,
                    rgba(line, 1.),
                );
            }
            if t.mode == Mode::Tram {
                push(out, x, y, 6., 0.5, depth - 0.0004, rgba(0x333333, 1.));
            }
            if c.first {
                let (hx, hy) = (x + fx * (l / 2. - 3.5), y + fy * (l / 2. - 3.5));
                push(
                    out,
                    hx,
                    hy,
                    1.5,
                    wd / 2. - 3.,
                    depth - 0.0004,
                    rgba(0x26303c, 1.),
                );
                let glow = if t.lit { 1. } else { 0.7 };
                for k in [-1f32, 1.] {
                    let (ox, oy) = (-fy * (wd / 2. - 3.5) * k, fx * (wd / 2. - 3.5) * k);
                    push(
                        out,
                        x + fx * (l / 2. - 0.8) + ox,
                        y + fy * (l / 2. - 0.8) + oy,
                        0.8,
                        1.5,
                        depth - 0.0005,
                        rgba(0xfff6c8, glow),
                    );
                }
            }
            if c.last {
                for k in [-1f32, 1.] {
                    let (ox, oy) = (-fy * (wd / 2. - 3.5) * k, fx * (wd / 2. - 3.5) * k);
                    push(
                        out,
                        x - fx * (l / 2. - 0.8) + ox,
                        y - fy * (l / 2. - 0.8) + oy,
                        0.8,
                        1.5,
                        depth - 0.0005,
                        rgba(0x8a1c1c, 1.),
                    );
                }
            }
        }
    }
}

/// Stadtleben am Boden: abgestellte E-Roller, Tauben und Enten (fliegende Tauben über den Dächern).
fn life_bodies(w: &World, near: &dyn Fn(f64, f64) -> bool, out: &mut Vec<Body>) {
    use berlin_sim::animals::{Kind, State};
    const SCOOTER: [u32; 4] = [0x1abc9c, 0xe84393, 0x55efc4, 0x6c5ce7];
    for sc in w.scooters.values().flatten().filter(|s| near(s.x, s.y)) {
        let (x, y, a) = (sc.x as f32, sc.y as f32, sc.angle as f32);
        let (fx, fy) = (a.cos(), a.sin());
        out.push(Body {
            center: [x + 0.5, y + 1.],
            half: [7.5, if sc.lying { 3. } else { 2. }],
            angle: a,
            shape: 0.,
            depth: 0.6205,
            color: [0., 0., 0., 0.22],
        });
        out.push(Body {
            center: [x - fx, y - fy],
            half: [6., 1.5],
            angle: a,
            shape: 0.,
            depth: 0.6203,
            color: rgba(0x2d3436, 1.),
        });
        out.push(Body {
            center: [x + fx * 5.5, y + fy * 5.5],
            half: [1.5, 1.2],
            angle: a,
            shape: 0.,
            depth: 0.6202,
            color: rgba(SCOOTER[(sc.seed.rem_euclid(4)) as usize], 1.),
        });
        // Lenkstange: steht quer, liegt längs
        let (lx, ly) = if sc.lying {
            (x + fx * 6.2 - fy * 4.5, y + fy * 6.2 + fx * 4.5)
        } else {
            (x + fx * 6.2, y + fy * 6.2)
        };
        out.push(Body {
            center: [lx, ly],
            half: [
                if sc.lying { 0.7 } else { 4.5 },
                if sc.lying { 4.5 } else { 0.7 },
            ],
            angle: a + std::f32::consts::FRAC_PI_2 * if sc.lying { 0. } else { 1. },
            shape: 0.,
            depth: 0.6201,
            color: rgba(0x111111, 1.),
        });
    }
    for b in w.animals.iter().filter(|b| near(b.x, b.y)) {
        let fly = matches!(b.state, State::Fly | State::Land);
        let z = b.z as f32;
        let (x, y, a) = (b.x as f32, b.y as f32 - z * 0.6, b.facing as f32);
        let (fx, fy) = (a.cos(), a.sin());
        let depth = if fly { 0.06 } else { 0.6195 };
        // Schatten am Boden, im Flug nach Höhe versetzt
        out.push(Body {
            center: [b.x as f32 + z * 0.35 + 1., b.y as f32 + z * 0.5 + 1.5],
            half: [4., 2.5],
            angle: a,
            shape: 1.,
            depth: 0.6199,
            color: [0., 0., 0., if fly { 0.18 } else { 0.2 }],
        });
        if b.kind == Kind::Duck {
            let drake = b.seed % 2 == 0;
            out.push(Body {
                center: [x, y],
                half: [5., 3.],
                angle: a,
                shape: 1.,
                depth,
                color: rgba(if drake { 0x8d8f86 } else { 0x8a6d4e }, 1.),
            });
            out.push(Body {
                center: [x + fx * 4.5, y + fy * 4.5],
                half: [2., 2.],
                angle: 0.,
                shape: 1.,
                depth: depth - 0.0001,
                color: rgba(if drake { 0x1f6f43 } else { 0x7a5b3c }, 1.),
            });
            out.push(Body {
                center: [x + fx * 6.6, y + fy * 6.6],
                half: [1.3, 0.8],
                angle: a,
                shape: 1.,
                depth: depth - 0.0002,
                color: rgba(0xe1a32a, 1.),
            });
            continue;
        }
        let r = berlin_sim::math::hash01(b.seed as f64);
        let g = if r < 0.15 {
            0xf2f2f2
        } else if r < 0.3 {
            0x6d5a4c
        } else {
            0x7d8491
        };
        if fly {
            let wing = (b.flap as f32 * 26.).sin() * 5. + 2.;
            out.push(Body {
                center: [x - fx, y - fy],
                half: [1.6, wing.abs() + 3.],
                angle: a,
                shape: 1.,
                depth: depth + 0.0001,
                color: shade(rgba(g, 1.), 0.85),
            });
        }
        out.push(Body {
            center: [x, y],
            half: [3.6, 2.2],
            angle: a,
            shape: 1.,
            depth,
            color: rgba(g, 1.),
        });
        out.push(Body {
            center: [x + fx * 3., y + fy * 3.],
            half: [1.5, 1.5],
            angle: 0.,
            shape: 1.,
            depth: depth - 0.0001,
            color: rgba(0x5d6d7e, 1.),
        });
    }
}

/// Haltung am Lebensplatz: Sitzen auf der Bank, Liegen auf der Decke, Flasche, Zigarette, Gitarre.
/// `true` = vollständig gezeichnet (sonst die normale Figur, Zubehör schon dazu gelegt).
fn hang_bodies(
    p: &berlin_sim::pedestrians::Ped,
    h: &berlin_sim::life::Hang,
    out: &mut Vec<Body>,
) -> bool {
    use berlin_sim::life::Act;
    let (x, y, a) = (p.x as f32, p.y as f32, p.facing as f32);
    let (fx, fy) = (a.cos(), a.sin());
    let depth = 0.618;
    match h.act {
        Act::Lie => {
            // Decke einmal je Gruppe (die erste Person), Leute strahlenförmig darauf
            if h.key.ends_with(":0") {
                let hue = berlin_sim::math::hash01(h.gx + h.gy);
                let col =
                    [0xc0392b, 0x2980b9, 0xf1c40f, 0x16a085, 0x8e44ad][(hue * 5.) as usize % 5];
                out.push(Body {
                    center: [h.gx as f32, h.gy as f32],
                    half: [17., 13.],
                    angle: (hue * 3.) as f32,
                    shape: 0.,
                    depth: 0.6208,
                    color: rgba(col, 0.92),
                });
            }
            out.push(Body {
                center: [x - fx * 4., y - fy * 4.],
                half: [8.5, 4.5],
                angle: a,
                shape: 1.,
                depth,
                color: rgba(p.shirt, 1.),
            });
            out.push(Body {
                center: [x - fx * 13., y - fy * 13.],
                half: [3., 3.],
                angle: 0.,
                shape: 1.,
                depth: depth - 0.0002,
                color: rgba(p.skin, 1.),
            });
            true
        }
        Act::Sit if h.bench => {
            if h.key.ends_with(":0") {
                let ba = (h.face - std::f64::consts::FRAC_PI_2) as f32;
                out.push(Body {
                    center: [h.gx as f32, h.gy as f32],
                    half: [14., 3.5],
                    angle: ba,
                    shape: 0.,
                    depth: 0.6207,
                    color: rgba(0x7a5230, 1.),
                });
                out.push(Body {
                    center: [h.gx as f32 - fx * 4., h.gy as f32 - fy * 4.],
                    half: [14., 1.2],
                    angle: ba,
                    shape: 0.,
                    depth: 0.6206,
                    color: rgba(0x4b3420, 1.),
                });
            }
            out.push(Body {
                center: [x + fx * 2.5, y + fy * 2.5],
                half: [3.5, 4.],
                angle: a,
                shape: 0.,
                depth: depth + 0.0002,
                color: rgba(0x2c3e50, 1.),
            });
            false
        }
        Act::Drink | Act::Smoke | Act::Music => {
            let (r, col, half) = match h.act {
                Act::Drink => (6., 0x2e7d32, [1.3, 1.3]),
                Act::Smoke => (6.5, 0xff8a3d, [0.9, 0.9]),
                _ => (6., 0x9c6b3a, [4.5, 2.5]),
            };
            // rechte Hand vor dem Körper
            let (sx, sy) = (-fy, fx);
            out.push(Body {
                center: [x + fx * r + sx * 2.5, y + fy * r + sy * 2.5],
                half,
                angle: a + if h.act == Act::Music { 0.6 } else { 0. },
                shape: 1.,
                depth: depth - 0.0003,
                color: rgba(col, 1.),
            });
            false
        }
        _ => false,
    }
}

fn bike_bodies(b: &berlin_sim::bikes::Bike, _t: f64, out: &mut Vec<Body>) {
    use berlin_sim::bikes::{Kind, State};
    let depth = if b.level.lvl >= 1 { 0.551 } else { 0.619 };
    let lying = b.state == State::Lying;
    let a = b.angle as f32 + if lying { 0.9 } else { 0. };
    let (x, y) = (b.x as f32, b.y as f32);
    let (fx, fy) = (a.cos(), a.sin());
    let scooter = b.kind == Kind::Scooter;
    let half = if scooter { 6. } else { 8. };
    let dark = [0.12, 0.13, 0.15, 1.];
    out.push(Body {
        center: [x + 1., y + 2.],
        half: [half + 1., 3.],
        angle: a,
        shape: 0.,
        depth: depth + 0.0004,
        color: [0., 0., 0., 0.22],
    });
    for k in [-1f32, 1.] {
        out.push(Body {
            center: [x + fx * half * 0.8 * k, y + fy * half * 0.8 * k],
            half: [if scooter { 1.8 } else { 2.6 }, 1.],
            angle: a,
            shape: 1.,
            depth: depth + 0.0002,
            color: dark,
        });
    }
    out.push(Body {
        center: [x, y],
        half: [half * 0.7, if scooter { 1.6 } else { 0.9 }],
        angle: a,
        shape: 0.,
        depth: depth + 0.0001,
        color: if scooter {
            [0.25, 0.27, 0.3, 1.]
        } else {
            rgba(0x1e272e, 1.)
        },
    });
    // Lenker quer
    out.push(Body {
        center: [x + fx * half * 0.7, y + fy * half * 0.7],
        half: [0.7, 3.],
        angle: a,
        shape: 0.,
        depth,
        color: dark,
    });
    if lying || b.state != State::Ride {
        return;
    }
    let shirt = rgba(b.shirt(), 1.);
    let lean = if scooter { 0.25 } else { -0.1 };
    out.push(Body {
        center: [x + fx * half * lean, y + fy * half * lean],
        half: [3.4, 5.],
        angle: a,
        shape: 1.,
        depth: depth - 0.0002,
        color: shirt,
    });
    out.push(Body {
        center: [x + fx * half * (lean + 0.12), y + fy * half * (lean + 0.12)],
        half: [2.7, 2.7],
        angle: 0.,
        shape: 1.,
        depth: depth - 0.0003,
        color: shade(shirt, 0.6),
    });
    if !scooter {
        // Knie bewegen sich mit der Kurbel
        let ph = b.pedal as f32;
        let (rx, ry) = (-fy, fx);
        for (k, side) in [(0f32, 1f32), (std::f32::consts::PI, -1.)] {
            let along = (ph + k).sin() * 2.;
            out.push(Body {
                center: [
                    x + fx * (half * 0.15 + along) + rx * side * 3.,
                    y + fy * (half * 0.15 + along) + ry * side * 3.,
                ],
                half: [1.6, 1.2],
                angle: a,
                shape: 1.,
                depth: depth - 0.0001,
                color: rgba(0x2b2f3a, 1.),
            });
        }
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

impl Play {
    /// Schilder im Bild für `hud()` vormerken (auch am Tag), die nächsten zuerst – die Obergrenze soll das Bild
    /// treffen, nicht den Rand.
    fn collect_signs(&mut self, neon_k: f32) {
        self.neon.signs.clear();
        self.neon.alpha = neon_k;
        if self.world.in_tunnel_station() {
            return;
        }
        let sp = self.spots();
        let t = self.world.time;
        let s = self.world.city.scale;
        let city = &mut self.world.city;
        let mut pois = sp.gather(
            |cx, cy, z| city.pois_near(cx, cy, 2200. / z.max(0.5) + 250.),
            |q| (q.x, q.y),
        );
        pois.sort_by(|a, b| {
            let da = sp.dist(a.x, a.y);
            let db = sp.dist(b.x, b.y);
            da.total_cmp(&db)
        });
        for q in pois {
            if self.neon.signs.len() >= crate::neon::MAX_SIGNS {
                break;
            }
            let Some(spec) = crate::neon::sign_spec(&q) else {
                continue;
            };
            let Some((gx, gy)) = self.neon.glow_point(&mut self.world.city, &q) else {
                continue;
            };
            let (dx, dy) = (q.x - gx, q.y - gy);
            let d = dx.hypot(dy).max(1.);
            let (x, y) = (gx + dx / d * 1.6 * s, gy + dy / d * 1.6 * s);
            // zwei Einträge für dasselbe Lokal: nur ein Schild
            if self
                .neon
                .signs
                .iter()
                .any(|o| (o.x - x).hypot(o.y - y) < 3. * s)
            {
                continue;
            }
            let on = !spec.flicker || crate::neon::neon_on(&q, t);
            self.neon
                .signs
                .push(crate::neon::Sign { x, y, spec, on, t });
        }
    }
}

impl Play {
    fn step_keys(&mut self, keys: &Keys, dt: f64) {
        self.interp.record(&self.world);

        if self.step_screens(keys, dt) {
            return;
        }
        use crate::bindings::Action as Bind;
        let bind = self.bindings.clone();
        if bind.pressed(keys, Bind::Save) {
            self.save();
        }
        self.bigmap.toggle(keys, bind.pressed(keys, Bind::Map));
        if self.bigmap.open {
            let v = self.bigmap.view(self.hud_width);
            match self.bigmap.control(keys, dt as f32, v) {
                Some(crate::bigmap::MapClick::Teleport(at)) if self.teleport.is_none() => {
                    self.request_teleport(at.x as f64, at.y as f64);
                }
                Some(crate::bigmap::MapClick::Waypoint(at)) => {
                    // ein Klick nahe am Wegpunkt (14 HUD-Einheiten) entfernt ihn
                    let near = 14. / v.f as f64;
                    let set = self.nav.toggle((at.x as f64, at.y as f64), near);
                    self.world.notice = Some(berlin_sim::world::Notice {
                        text: if !set {
                            "Wegpunkt entfernt".into()
                        } else if self.nav.ready() {
                            "Wegpunkt gesetzt".into()
                        } else {
                            "Wegpunkt gesetzt – Route wird vorbereitet …".into()
                        },
                        t: 1.6,
                    });
                    self.ui_sound();
                }
                _ => {}
            }
        }
        // Physik-Anzeige mit Live-Reglern (nur Entwickler-Build)
        if self.dev && bind.pressed(keys, Bind::DebugToggle) {
            self.physdebug.open = !self.physdebug.open;
            self.enginedebug.open &= !self.physdebug.open;
        }
        // Motorsound-Anzeige (nur Entwickler-Build; per Konsole `motorsound` in jedem Build)
        if self.dev && bind.pressed(keys, Bind::EngineDebug) {
            self.enginedebug.open = !self.enginedebug.open;
            self.physdebug.open &= !self.enginedebug.open;
        }
        if self.enginedebug.open {
            if bind.pressed(keys, Bind::DebugPrev) {
                self.enginedebug.select(-1);
            }
            if bind.pressed(keys, Bind::DebugNext) {
                self.enginedebug.select(1);
            }
            let view = &self.listener.engine_view;
            if bind.pressed(keys, Bind::DebugLess) {
                self.enginedebug.adjust(-1., view);
            }
            if bind.pressed(keys, Bind::DebugMore) {
                self.enginedebug.adjust(1., view);
            }
            if bind.pressed(keys, Bind::EngineAb) {
                self.enginedebug.toggle_ab(view);
            }
        }
        self.enginedebug.sync(&mut self.listener);
        if self.physdebug.open {
            if bind.pressed(keys, Bind::DebugPrev) {
                self.physdebug.select(&self.world, -1);
            }
            if bind.pressed(keys, Bind::DebugNext) {
                self.physdebug.select(&self.world, 1);
            }
            if bind.pressed(keys, Bind::DebugLess) {
                self.physdebug.adjust(&mut self.world, -1.);
            }
            if bind.pressed(keys, Bind::DebugMore) {
                self.physdebug.adjust(&mut self.world, 1.);
            }
            if bind.pressed(keys, Bind::DebugExport) {
                let json = self.physdebug.export(&self.world);
                println!("Physik-Änderungen: {json}");
                self.console
                    .log
                    .push((format!("Physik: {json}"), true, self.world.time));
                self.world.notice = Some(berlin_sim::world::Notice {
                    text: "Physik-Änderungen in der Konsole".into(),
                    t: 1.6,
                });
            }
        }
        if bind.pressed(keys, Bind::GraphicsMode) {
            self.toggle_graphics();
        }
        if bind.pressed(keys, Bind::Mute)
            && let Some(a) = &self.audio
        {
            let muted = a.toggle_mute();
            self.world.notice = Some(berlin_sim::world::Notice {
                text: (if muted { "Ton aus" } else { "Ton an" }).into(),
                t: 1.5,
            });
        }
        if self.bars_live.is_some() {
            self.poll_bars();
        }
        let w2 = &mut self.world;
        if bind.pressed(keys, Bind::Weather) {
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
        if bind.pressed(keys, Bind::Clock) {
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
            // im Auto oder am Führerstand einer Bahn: Gas/Bremse statt Laufen
            let drives_train = w2
                .player
                .ride
                .as_ref()
                .is_some_and(|r| r.kind == berlin_sim::ride::RideKind::Driver);
            input_from(keys, w2.player.in_car.is_some() || drives_train, &bind)
        };
        // Tastatur: Gas und Bremse mit kurzer Rampe (analoge Trigger bleiben, wie sie sind)
        {
            use crate::bindings::{Action as A, pedal_ramp};
            for (k, (a, v)) in [
                (A::Throttle, &mut input.throttle),
                (A::Brake, &mut input.brake),
            ]
            .into_iter()
            .enumerate()
            {
                if bind.pad_value(keys, a) > 0.02 {
                    self.pedals[k] = *v;
                } else {
                    self.pedals[k] = pedal_ramp(self.pedals[k], *v, dt);
                    *v = self.pedals[k];
                }
            }
        }
        if self.vehicle_show && !w2.loading {
            self.vehicle_show = false;
            w2.vehicle_show();
        }
        if self.demo_neon && !w2.loading {
            self.demo_neon = false;
            let (px, py) = (w2.player.x, w2.player.y);
            let pois: Vec<_> = w2
                .city
                .pois_near(px, py, 20000.)
                .into_iter()
                .filter(|q| crate::neon::sign_text(q).is_some())
                .collect();
            // die Stelle mit den meisten Schildern in 30 m, bei Gleichstand die nächste
            let dense = |q: &berlin_sim::city::Poi| {
                pois.iter()
                    .filter(|o| (o.x - q.x).hypot(o.y - q.y) < 300.)
                    .count()
            };
            let spot = pois.iter().max_by(|a, b| {
                dense(a).cmp(&dense(b)).then_with(|| {
                    (b.x - px)
                        .hypot(b.y - py)
                        .total_cmp(&(a.x - px).hypot(a.y - py))
                })
            });
            if let Some(q) = spot.cloned()
                && let Some((gx, gy)) = self.neon.glow_point(&mut w2.city, &q)
            {
                let (x, y) = (gx + (gx - q.x) * 0.6, gy + (gy - q.y) * 0.6);
                (w2.player.x, w2.player.y) = (x, y);
                (w2.camera.x, w2.camera.y) = (x, y);
            }
        }
        // nach `--im-auto` (Einsteigen weiter unten im selben Schritt): erst im Auto verdecken
        if self.demo_covered && !w2.loading && !self.auto_enter {
            self.demo_covered = false;
            demo_cover(w2);
        }
        if self.people_show && !w2.loading {
            self.people_show = false;
            w2.people_show();
        }
        if let Some(ride) = self.demo_station
            && !w2.loading
            && demo_station_step(w2, ride)
        {
            self.demo_station = None;
        }
        if self.demo_drive && !w2.loading {
            if w2.player_train.is_none() {
                demo_take_tram(w2);
            } else {
                input.throttle = 1.;
            }
        }
        if self.demo_drift && !w2.loading && w2.player.in_car.is_some() {
            // Anlauf geschenkt (55 km/h in Fahrtrichtung), kurz Handbremse mit Lenkung (leitet den Drift ein), dann
            // Halbgas mit leichtem Lenken – den Winkel hält die Drift-Schicht
            let t = w2.time;
            if !self.demo_drift_started && w2.demo_launch(170.) {
                self.demo_drift_started = true;
                self.demo_drift_t = t;
            }
            let t = t - self.demo_drift_t;
            input.steer = if t < 0.8 {
                0.
            } else if t < 1.1 {
                0.7
            } else {
                0.25
            };
            input.handbrake = (0.8..1.1).contains(&t);
            input.throttle = if t < 0.8 {
                1.
            } else if t < 1.1 {
                0.3
            } else {
                0.65
            };
        }
        if self.car_lab == Some(false) && !w2.loading {
            self.car_lab = Some(true);
            car_lab_park(w2);
            self.zoom_user = 0.9;
        }
        if self.demo_combat && !w2.loading && w2.player.in_car.is_none() {
            demo_combat_input(w2, &mut input);
            // nah heran, damit Mündungsfeuer, Hülsen und Einschläge im Bild zu erkennen sind
            self.zoom_user = 1.8;
        }
        // Maus oder Controller zielt: wer zuletzt bewegt wurde
        let m = keys.mouse;
        if m.moved || m.left_pressed || m.right_pressed {
            self.mouse_aim = true;
        }
        if keys.pad.rx.hypot(keys.pad.ry) > 0.35 || bind.pad_value(keys, Bind::Fire) > 0.5 {
            self.mouse_aim = false;
        }
        self.cursor = m.hud;
        self.ctrl_held =
            keys.held.contains(&KeyCode::ControlLeft) || keys.held.contains(&KeyCode::ControlRight);
        let on_foot = !self.bigmap.open && w2.player.in_car.is_none();
        let world_pt = m.world.map(|p| (p.x as f64, p.y as f64));
        // Maus am PC: links läuft (nie schießen), rechts schießt bzw. schlägt immer zum Zeiger, beide Tasten
        // zusammen öffnen das Waffenrad – solange das Rad gedrückt oder offen ist, weder laufen noch feuern
        let combo = self.wheel_m.down || self.wheel_m.open || (m.left && m.right);
        if on_foot && !combo && (m.right || m.right_pressed) {
            input.combat.aim_world = world_pt;
            input.combat.fire |= m.right;
            input.combat.fire_pressed |= m.right_pressed;
        }
        if self.diablo && on_foot && !combo {
            // Diablo: Klick läuft hin (steigt nie ein, greift nie an, `click_attack` bleibt aus)
            let ctrl = keys.held.contains(&KeyCode::ControlLeft)
                || keys.held.contains(&KeyCode::ControlRight);
            input.click_world = world_pt;
            input.click_held = m.left;
            input.click_pressed = m.left_pressed;
            if ctrl {
                // Strg allein zielt mit der Maus (Fadenkreuz), feuern mit Klick oder Strg-Taste
                input.combat.aim_world = world_pt;
            }
        } else if self.mouse_aim && on_foot && !self.diablo {
            // klassisch: die Maus zielt, gefeuert wird mit rechts (oben) oder Strg
            input.combat.aim_world = world_pt;
        }
        // Kamera näher/weiter (gehalten, sanft; Mausrad zoomt in der Engine zusätzlich)
        let zoom = bind.held(keys, Bind::ZoomIn) as i32 - bind.held(keys, Bind::ZoomOut) as i32;
        if zoom != 0 && !self.bigmap.open {
            self.zoom_user =
                (self.zoom_user * (zoom as f32 * 1.4 * dt as f32).exp()).clamp(0.6, 1.8);
        }
        if let Some(m) = self.koop_start
            && !w2.loading
        {
            if !w2.coop() {
                w2.join_p2();
                self.p2_pad = 1;
            }
            if m <= 0. {
                self.koop_start = None;
            } else {
                // Spieler 2 weiter östlich auf einen freien Platz (Kacheln dort laden wie beim Teleport)
                let (x, y) = (w2.player.x + m * w2.city.scale, w2.player.y);
                match w2.find_teleport_spot(x, y) {
                    Some(berlin_sim::world::TeleportSpot::Spot { x: sx, y: sy, .. }) => {
                        if let Some(s) = w2.p2.as_mut() {
                            (s.player.x, s.player.y) = (sx, sy);
                            (s.camera.x, s.camera.y) = (sx, sy);
                            s.player.level_init = false;
                        }
                        w2.city.release("teleport");
                        self.koop_start = None;
                    }
                    Some(berlin_sim::world::TeleportSpot::Pending) => {
                        w2.city.pump();
                    }
                    None => {
                        w2.city.release("teleport");
                        self.koop_start = None;
                    }
                }
            }
        }
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
        // Waffenrad: rechte Maustaste bzw. LB (tippen oder halten)
        self.real_t += dt;
        let alive_foot = on_foot && !w2.player.combat.dead;
        let (t, cur) = (self.real_t, w2.player.combat.weapon);
        let n = berlin_sim::combat::WEAPONS.len();
        let hud_center = Vec2::new(self.hud_width / 2., 360.);
        // Waffenrad: beide Maustasten gemeinsam (die zweite Taste löst es aus)
        if m.left
            && m.right
            && (m.left_pressed || m.right_pressed)
            && !self.wheel_m.down
            && alive_foot
        {
            self.wheel_m.press(t, m.hud.unwrap_or(hud_center));
        }
        if let Some(p) = m.hud {
            self.wheel_m.mov(p);
        }
        if self.wheel_m.tick(t, alive_foot, cur, n).opened {
            // ganz im Bild halten
            let r = crate::wheel::RADIUS + 24.;
            let c = self.wheel_m.center;
            self.wheel_m.place(Vec2::new(
                c.x.clamp(r, (self.hud_width - r).max(r)),
                c.y.clamp(r, 720. - r - 30.),
            ));
        }
        let mut outcomes = Vec::new();
        // erst wenn beide Tasten los sind: Auswahl unter dem Zeiger (ein kurzes Doppeltippen tut nichts)
        if self.wheel_m.down && !m.left && !m.right {
            let o = self.wheel_m.release(t);
            outcomes.push(o);
        }
        if self.wheel_m.open
            && m.left_pressed
            && let Some(i) = self.wheel_m.hover
        {
            outcomes.push(self.wheel_m.choose(i));
        }
        // Controller: rechten Stick drücken öffnet bzw. nimmt, A nimmt, B bricht ab
        let pad_was = self.wheel_p.open;
        let o = self.wheel_p.pad_step(
            t,
            hud_center,
            bind.pad_pressed(keys, Bind::WeaponWheel),
            keys.pad_pressed.a,
            keys.pad_pressed.b,
            (keys.pad.rx, keys.pad.ry),
            alive_foot,
            cur,
            n,
        );
        outcomes.push(o);
        if pad_was || self.wheel_p.open {
            // das Rad bedient A und B selbst
            input.sprint = false;
            input.action = false;
            input.action_held = false;
            input.jump = false;
            input.combat.reload = false;
        }
        // Fahrhilfen-Rad und Hupe/Sirene im Fahrzeug
        let siren_car = w2.player_car().is_some_and(|c| c.has_siren());
        let in_car = !self.bigmap.open && w2.player.in_car.is_some();
        assist_input(
            &mut self.assist[0],
            &mut self.horn[0],
            &mut input,
            t,
            hud_center,
            keys,
            &bind,
            in_car,
            siren_car,
            dt,
        );
        let open = self.wheel_m.open || self.wheel_p.open;
        if open {
            // Zifferntaste wählt und schließt
            let digits = [
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
                KeyCode::Digit5,
                KeyCode::Digit6,
            ];
            if let Some(i) = digits.iter().position(|d| keys.pressed.contains(d)) {
                outcomes.push(self.wheel_m.choose(i));
                outcomes.push(self.wheel_p.choose(i));
            }
            // bei offenem Rad kein Schuss, kein Klick, Ziel eingefroren
            input.combat.fire = false;
            input.combat.fire_pressed = false;
            input.combat.kick = false;
            input.combat.aim_world = None;
            input.combat.aim_x = 0.;
            input.combat.aim_y = 0.;
            input.combat.weapon_slot = 0;
            input.click_world = None;
            input.click_pressed = false;
            input.click_held = false;
        }
        if let Some(i) = outcomes.iter().find_map(|o| o.pick) {
            input.combat.weapon_slot = i as u8 + 1;
        }
        self.time_scale = crate::wheel::ease_time_scale(
            self.time_scale,
            self.wheel_m.open || self.wheel_p.open,
            dt,
        );
        if w2.coop() {
            let input2 = if self.bigmap.open {
                Input::default()
            } else {
                self.p2_input(keys, &bind)
            };
            self.world
                .update_coop(&input, &input2, dt * self.time_scale);
        } else {
            w2.update(&input, dt * self.time_scale);
        }
        if input.click_pressed
            && let (Some(at), Some(berlin_sim::world::Click::Walk { .. })) =
                (input.click_world, &self.world.player.click)
        {
            self.fx.click_ring(at);
        }
        self.fx.ingest(&self.world.events);
        self.step_nav();
        // Controller-Vibration aus Stößen und dem Schlupf des eigenen Autos
        if self.bindings.rumble {
            let w = &self.world;
            let own = w.player.in_car;
            let drive = own.and_then(|id| w.car(id)).and_then(|c| {
                c.dyn_state.as_ref().map(|d| crate::rumble::Drive {
                    spin: d.spin_f.max(d.spin_r),
                    lock: d.lock_r,
                    assist: d.esp > 0.05,
                })
            });
            if let Some(r) =
                self.rumbler
                    .step(&w.events, (w.player.x, w.player.y), own, drive, w.time)
            {
                self.rumble_out = Some(r);
            }
            if let Some(s) = w.p2.as_ref() {
                let own2 = s.player.in_car;
                let drive2 = own2.and_then(|id| w.car(id)).and_then(|c| {
                    c.dyn_state.as_ref().map(|d| crate::rumble::Drive {
                        spin: d.spin_f.max(d.spin_r),
                        lock: d.lock_r,
                        assist: d.esp > 0.05,
                    })
                });
                if let Some(r) =
                    self.rumbler2
                        .step(&w.events, (s.player.x, s.player.y), own2, drive2, w.time)
                {
                    self.rumble_out2 = Some(r);
                }
            }
        }
        self.fx.step(dt as f32);
        self.fx.tires(&mut self.world, dt as f32);
        self.trails
            .record(&self.world.cars, self.world.time, self.world.weather.snow);
        // alle Ausschnitte umfassen (Koop: die Bahnen gibt es ohnehin nur um die Spieler)
        let mut view =
            berlin_sim::collision::Rect::around(self.world.camera.x, self.world.camera.y, 2600.);
        if self.world.coop() {
            for &(x, y, _) in self.spots().all() {
                let r = berlin_sim::collision::Rect::around(x, y, 2600.);
                let (x0, y0) = (view.x.min(r.x), view.y.min(r.y));
                let (x1, y1) = (
                    (view.x + view.w).max(r.x + r.w),
                    (view.y + view.h).max(r.y + r.h),
                );
                view = berlin_sim::collision::Rect::new(x0, y0, x1 - x0, y1 - y0);
            }
        }
        self.trains = self.world.transit_visible(view);
        // auf einem Bahnsteig unter freiem Himmel zeichnet der Bahnsteig seine Züge selbst (an seinen Gleisen)
        if let Some(st) = self.world.current_station().filter(|s| s.open_air) {
            let reach = st.hl + st.l;
            self.trains.retain(|t| {
                !st.lines.contains(&t.line)
                    || t.cars.iter().all(|c| {
                        let (u, v) = st.to_local(c.x, c.y);
                        u.abs() > reach || v.abs() > berlin_sim::station::WALL + 60.
                    })
            });
        }
        self.tram_segs = self.world.tram_track_segments(view);
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
}

/// Fahrhilfen-Rad (rechten Stick drücken im Fahrzeug: ESP, ABS, Sirene) und Hupe mit langem Druck (Sirene) für
/// einen Spieler; schreibt die Schalter in `input`. Bei offenem Rad gehören A und B dem Rad.
#[allow(clippy::too_many_arguments)]
fn assist_input(
    wheel: &mut crate::wheel::WheelButton,
    horn: &mut crate::bindings::HornPress,
    input: &mut Input,
    t: f64,
    center: Vec2,
    keys: &Keys,
    bind: &crate::bindings::Bindings,
    in_car: bool,
    siren_car: bool,
    dt: f64,
) {
    use crate::bindings::Action as B;
    let was = wheel.open;
    let o = wheel.pad_step(
        t,
        center,
        in_car && bind.pad_pressed(keys, B::AssistWheel),
        keys.pad_pressed.a,
        keys.pad_pressed.b,
        (keys.pad.rx, keys.pad.ry),
        in_car,
        0,
        ASSIST_ITEMS,
    );
    match o.pick {
        Some(0) => input.esp_toggle = true,
        Some(1) => input.abs_toggle = true,
        Some(2) => input.siren_toggle = siren_car,
        _ => {}
    }
    if was || wheel.open {
        input.handbrake = false;
        input.action = false;
        input.action_held = false;
    }
    let (h, toggle) = horn.step(input.horn, siren_car, dt);
    input.horn = h;
    input.siren_toggle |= toggle;
}
/// Einträge des Fahrhilfen-Rads
pub const ASSIST_ITEMS: usize = 3;
/// Einträge des Fahrhilfen-Rads mit Zustand für das gefahrene Auto des aktuellen Sitzes.
fn assist_items(w: &World) -> Vec<crate::wheel::AssistItem> {
    let car = w.player_car();
    let siren = car.is_some_and(|c| c.has_siren());
    vec![
        crate::wheel::AssistItem {
            name: "ESP",
            state: berlin_sim::world::esp_label(w.esp, w.esp_full).into(),
            enabled: true,
        },
        crate::wheel::AssistItem {
            name: "ABS",
            state: if w.abs { "AN" } else { "AUS" }.into(),
            enabled: true,
        },
        crate::wheel::AssistItem {
            name: "Sirene",
            state: if !siren {
                "–".into()
            } else if car.is_some_and(|c| c.siren) {
                "AN".into()
            } else {
                "AUS".into()
            },
            enabled: siren,
        },
    ]
}

/// Fahrzeug, in das die Figur einsteigen würde (wie `World::try_enter_car`: das nächste heile in Reichweite, das
/// kein Spieler fährt).
pub fn enter_target<'a>(
    w: &'a World,
    p: &berlin_sim::world::Player,
) -> Option<&'a berlin_sim::car::Car> {
    if p.in_car.is_some() || p.ride.is_some() || p.inside.is_some() || p.combat.dead {
        return None;
    }
    w.cars
        .iter()
        .filter(|c| !c.wrecked && c.driver != Some(berlin_sim::car::Driver::Player))
        .map(|c| (c, (c.x - p.x).hypot(c.y - p.y)))
        .filter(|(_, d)| *d < berlin_sim::world::ENTER_DIST)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, _)| c)
}
/// Dezenter Schimmer unter dem Fahrzeug, in das man einsteigen kann (pulsiert leicht, Farbe des Spielers).
fn enter_hint(
    w: &World,
    p: &berlin_sim::world::Player,
    ring: [f32; 4],
    ambient: [f32; 3],
    out: &mut Vec<Body>,
) {
    let Some(c) = enter_target(w, p) else {
        return;
    };
    let pulse = 0.5 + 0.5 * (w.time * 4.).sin() as f32;
    let depth = if c.lvl() >= 1 { 0.55 } else { 0.62 };
    out.push(Body {
        center: [c.x as f32, c.y as f32],
        half: [c.hw as f32 + 5., c.hh as f32 + 5.],
        angle: c.angle as f32,
        shape: 0.,
        // hinter dem Auto, über seinem Schatten: nur der Rand ist zu sehen
        depth: depth + 0.0003,
        color: [
            ring[0] * ambient[0],
            ring[1] * ambient[1],
            ring[2] * ambient[2],
            0.22 + 0.16 * pulse,
        ],
    });
}

/// Bodenring der Spielfiguren: Spieler 1 Cyan, Spieler 2 Orange (dieselben Farben wie die Trennlinie).
pub const PLAYER_RING: [[f32; 4]; 2] = [[0.25, 0.85, 1., 0.9], [1., 0.6, 0.15, 0.9]];

/// Spielfigur zu Fuß (oder bewusstlos liegend) mit Bodenring in der Farbe des Spielers.
fn player_figure(
    p: &berlin_sim::world::Player,
    look: &crate::figure::Look,
    ring: [f32; 4],
    time: f64,
    out: &mut Vec<Body>,
) {
    if p.in_car.is_none() && p.combat.dead {
        // K. o.: liegt
        let (x, y, f) = (p.x as f32, p.y as f32, p.combat.fall as f32);
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
    } else if p.in_car.is_none() && p.ride.is_none() {
        let (x, y, a) = (p.x as f32, p.y as f32, p.angle as f32);
        let depth0 = if p.level.lvl >= 1 { 0.548 } else { 0.617 };
        let lift = p.z as f32;
        let weapon0 = out.len();
        weapon_bodies(&p.combat, (x, y), a, depth0, out);
        let weapon1 = out.len();
        let depth = if p.level.lvl >= 1 { 0.548 } else { 0.617 };
        out.push(Body {
            center: [x, y],
            half: [11., 11.],
            angle: 0.,
            shape: 2.,
            depth: depth + 0.0005,
            color: ring,
        });
        let pl = p;
        let who = crate::figure::Who {
            x: pl.x,
            y: pl.y,
            facing: pl.angle,
            step: pl.step,
            amp: (pl.move_speed / 40.).clamp(0., 1.) as f32,
            run: ((pl.move_speed - 95.) / 30.).clamp(0., 1.) as f32,
            skin: 0xf2d0b1,
        };
        let fig0 = out.len();
        crate::figure::person_bodies(&who, look, depth, time, out);
        if lift > 0. {
            lift_bodies(out, weapon0..weapon1, fig0, (x, y), lift);
        }
    }
}

impl Game for Play {
    fn step_seconds(&self) -> f64 {
        DT
    }
    fn step(&mut self, keys: &Keys, dt: f64) {
        // LB + RB gleichzeitig: Befehlszeile mit Bildschirmtastatur; einzelne Drücke kommen leicht verzögert durch
        let (mut pe, mut pe2) = (keys.pad_pressed, keys.pad2_pressed);
        let (c1, lb1, rb1) = self.chord[0].step(keys.pad.lb, keys.pad.rb, pe.lb, pe.rb, dt);
        (pe.lb, pe.rb) = (lb1, rb1);
        let (c2, lb2, rb2) = self.chord[1].step(keys.pad2.lb, keys.pad2.rb, pe2.lb, pe2.rb, dt);
        (pe2.lb, pe2.rb) = (lb2, rb2);
        let filtered = Keys {
            held: keys.held,
            pressed: keys.pressed,
            pad: keys.pad,
            pad_pressed: pe,
            pad2: keys.pad2,
            pad2_pressed: pe2,
            mouse: keys.mouse,
            typed: keys.typed,
        };
        let keys = &filtered;
        if c1 || c2 {
            // physischer Controller → Platz in den Tasten, die `step_keys` sieht (Spieler 2 auf dem ersten: Platz 2)
            let phys = if c1 { 0 } else { 1 };
            let mapped = self.world.coop() && self.p2_pad == 0;
            self.chord_console(if mapped { 1 - phys } else { phys });
        }
        if self.world.coop() && self.p2_pad == 0 {
            // Spieler 2 hat den ersten Controller: Spieler 1 sieht ihn nicht
            let k1 = Keys {
                held: keys.held,
                pressed: keys.pressed,
                pad: Default::default(),
                pad_pressed: Default::default(),
                pad2: keys.pad,
                pad2_pressed: keys.pad_pressed,
                mouse: keys.mouse,
                typed: keys.typed,
            };
            self.p2_pad = 1;
            self.step_keys(&k1, dt);
            if self.world.coop() {
                self.p2_pad = 0;
            }
        } else {
            self.step_keys(keys, dt);
        }
    }
    fn rumble2(&mut self) -> Option<berlin_engine::Rumble> {
        self.rumble_out2.take()
    }
    fn interpolate(&mut self, alpha: f64) {
        self.interp.apply(&mut self.world, alpha);
    }
    fn end_frame(&mut self) {
        self.interp.restore(&mut self.world);
    }
    fn camera(&self) -> (Vec2, f32) {
        let c = self.world.camera;
        if matches!(
            self.screen,
            Screen::Title
                | Screen::Controls(true)
                | Screen::Stats(true)
                | Screen::Bindings(true)
                | Screen::About(true)
        ) {
            // langsame Kreisfahrt über dem Kiez (main.js demo-Kamera, kleiner Radius: geladene Kacheln)
            let t = self.world.time;
            let (x, y) = (c.x + (t * 0.05).cos() * 900., c.y + (t * 0.07).sin() * 600.);
            return (Vec2::new(x as f32, y as f32), 0.8);
        }
        (
            Vec2::new(c.x as f32, c.y as f32),
            self.zoom_fix.unwrap_or(c.zoom as f32 * self.zoom_user),
        )
    }
    fn quit(&self) -> bool {
        self.quit
    }
    fn camera2(&self) -> Option<(Vec2, f32)> {
        if !matches!(self.screen, Screen::Playing | Screen::Paused) {
            return None;
        }
        let c = self.world.p2.as_ref()?.camera;
        Some((
            Vec2::new(c.x as f32, c.y as f32),
            self.zoom_fix.unwrap_or(c.zoom as f32 * self.zoom_user),
        ))
    }
    fn set_views(&mut self, views: &berlin_engine::split::Views, viewport: Vec2) {
        self.views = views.clone();
        self.viewport = viewport;
    }
    fn bodies(&self, out: &mut Vec<Body>) {
        let w = &self.world;
        let sp = self.spots();
        let near = |x: f64, y: f64| sp.near(x, y, |_| (2600., 1800.));
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
            // Schatten, Karosserie, Dach, Frontscheibe (Motorräder werfen ihren eigenen, schmalen Schatten)
            if !c.kind_info().moto {
                out.push(Body {
                    center: [x + 2., y + 3.],
                    half: [hw + 1., hh + 1.],
                    angle: a,
                    shape: 0.,
                    depth: depth + 0.0004,
                    color: [0., 0., 0., 0.28],
                });
            }
            if c.kind_info().moto {
                // Motorrad/Roller aus Teilen (motoart.rs): Schräglage, Wheelie/Stoppie und Sturz aus der Fahrphysik;
                // Verkehr ohne Fahrphysik legt sich nach seiner Gierrate in die Kurve (φ = atan(v·ω/g))
                let (mut lean, pitch, lying) = two_wheel_pose(c);
                if c.dyn_state.is_none() && c.driver.is_some() {
                    lean = ((c.speed() / 10. * c.ang_vel / berlin_sim::vehdata::G).atan() as f32)
                        .clamp(-0.8, 0.8);
                }
                let steer = c
                    .phys
                    .as_ref()
                    .map(|s| -s.delta)
                    .unwrap_or(c.controls.steer * 0.3) as f32;
                let (jacket, helmet) = crate::motoart::rider_colors(c.id);
                crate::motoart::bodies(
                    &crate::motoart::Pose {
                        x,
                        y,
                        angle: a,
                        hw,
                        lean,
                        pitch,
                        steer,
                        lying,
                        braking: c.controls.brake > 0.1 && c.speed() > 2.,
                        rider: c.driver.is_some() && !c.wrecked,
                        paint: if c.wrecked { 0x3b332d } else { c.color },
                        jacket,
                        helmet,
                        depth,
                    },
                    crate::motoart::style_of(c.model_name()),
                    out,
                );
                continue;
            }
            if c.kind_info().bike {
                // Schräglage, Wheelie/Stoppie und Sturz aus der Fahrphysik: von oben wird das Rad in Schräglage
                // schmaler und wandert zur Kurveninnenseite (der Fahrer weiter als der Rahmen), im Wheelie wirkt es
                // kürzer; gestürzt liegt es flach
                let (lean, pitch, lying) = two_wheel_pose(c);
                let (rx, ry) = (-fy, fx);
                let (sl, cl) = (lean.sin(), lean.cos());
                let len = hw * pitch.abs().cos();
                let back = (hw - len) * pitch.signum();
                let (bx, by) = (x - fx * back + rx * sl * 2., y - fy * back + ry * sl * 2.);
                out.push(Body {
                    center: [bx, by],
                    half: if lying {
                        [hw, hh * 2.2]
                    } else {
                        [len, hh * (0.55 + 0.45 * cl)]
                    },
                    angle: a,
                    shape: 0.,
                    depth,
                    color,
                });
                // Zweirad: Fahrer mit Helm (geparkt ohne Fahrer)
                if c.driver.is_some() && !c.wrecked {
                    let (ox, oy) = (rx * sl * 6., ry * sl * 6.);
                    out.push(Body {
                        center: [bx - fx * len * 0.15 + ox, by - fy * len * 0.15 + oy],
                        half: [4., 5.5 * (0.7 + 0.3 * cl)],
                        angle: a,
                        shape: 1.,
                        depth: depth - 0.0002,
                        color: rgba(0x2b2f3a, 1.),
                    });
                    out.push(Body {
                        center: [
                            bx - fx * len * 0.05 + ox * 1.4,
                            by - fy * len * 0.05 + oy * 1.4,
                        ],
                        half: [3.2, 3.2],
                        angle: 0.,
                        shape: 1.,
                        depth: depth - 0.0003,
                        color: shade(color, 0.8),
                    });
                }
                continue;
            }
            // Pkw und Nutzfahrzeuge: Bild aus dem Fahrzeug-Atlas (vehicles.js drawCarBody)
            let model = c.model_name();
            let tint = if c.wrecked {
                0x3b332d
            } else if model == "taxi" {
                0xf1e9c8
            } else {
                c.color
            };
            // Räder unter der Karosserie, an den Ecken sichtbar; die vorderen lenken mit
            let steer = c
                .dyn_state
                .as_ref()
                .map_or(c.controls.steer * 0.45, |d| d.delta) as f32;
            // Gespann: Anhänger hinter dem Zugfahrzeug, am Gelenk um den Knickwinkel gedreht
            if let Some((tx, ty, ta, thw, thh)) = berlin_sim::car::trailer_pose(c) {
                let (tx, ty, ta) = (tx as f32, ty as f32, ta as f32);
                let (tfx, tfy) = (ta.cos(), ta.sin());
                let (thw, thh) = (thw as f32, thh as f32);
                out.push(Body {
                    center: [tx + 2., ty + 3.],
                    half: [thw + 1., thh + 1.],
                    angle: ta,
                    shape: 0.,
                    depth: depth + 0.0004,
                    color: [0., 0., 0., 0.28],
                });
                out.push(Body {
                    center: [tx, ty],
                    half: [thw, thh],
                    angle: ta,
                    shape: 0.,
                    depth: depth + 0.00005,
                    color: shade(rgba(tint, 1.), 0.85),
                });
                // Dachfläche (Plane bzw. Busdach) etwas heller, mit Längsfugen
                out.push(Body {
                    center: [tx, ty],
                    half: [thw - 2., thh - 2.5],
                    angle: ta,
                    shape: 0.,
                    depth: depth + 0.00004,
                    color: shade(rgba(tint, 1.), 1.08),
                });
                for k in [-0.33f32, 0.33] {
                    out.push(Body {
                        center: [tx + tfx * thw * k, ty + tfy * thw * k],
                        half: [0.6, thh - 3.],
                        angle: ta,
                        shape: 0.,
                        depth: depth + 0.00003,
                        color: shade(rgba(tint, 1.), 0.7),
                    });
                }
            }
            let model = crate::carart::sprite_model(model);
            // Räder: Pkw an Radstand und Spur aus den Fahrzeugdaten, Nutzfahrzeuge wie bisher
            let (fa, ra, wy, tl, tw) = if crate::carart::SPECIAL.contains(&model) {
                let wx = hw - 8.;
                (wx, 1. - wx, hh - 1.2, 4.2, 2.05)
            } else {
                crate::carart::wheels(model, hw, hh)
            };
            let (rx, ry) = (-fy, fx);
            for (along, side, front) in [
                (fa, -wy, true),
                (fa, wy, true),
                (ra, -wy, false),
                (ra, wy, false),
            ] {
                out.push(Body {
                    center: [x + fx * along + rx * side, y + fy * along + ry * side],
                    half: [tl, tw],
                    angle: a + if front { steer } else { 0. },
                    shape: 0.,
                    depth: depth + 0.0002,
                    color: [0.067, 0.075, 0.09, 1.],
                });
            }
            let (sx, sy) = body_shift(c);
            out.push(Body {
                center: [x + fx * sx + rx * sy, y + fy * sx + ry * sy],
                half: [hw + crate::carart::PAD, hh + crate::carart::PAD],
                angle: a,
                shape: 16. + crate::carart::model_index(model) as f32,
                depth: depth - 0.0001,
                color: rgba(tint, 1.),
            });
            if c.kind == "garbage" && c.work {
                // Rundumleuchte und zwei Müllwerker mit Tonne am Heck
                let t = w.time as f32;
                out.push(Body {
                    center: [x + fx * (hw - 8.), y + fy * (hw - 8.)],
                    half: [3., 3.],
                    angle: 0.,
                    shape: 1.,
                    depth: depth - 0.0006,
                    color: [1., 0.59, 0.08, 0.55 + 0.4 * (t * 4.).sin()],
                });
                let (rx, ry) = (-fy, fx);
                for side in [-1f32, 1.] {
                    let bob = (t * 6. + side).sin() * 1.2;
                    let (px, py) = (
                        x - fx * (hw + 5. + bob) + rx * side * (hh - 3.),
                        y - fy * (hw + 5. + bob) + ry * side * (hh - 3.),
                    );
                    out.push(Body {
                        center: [px - fx * 6., py - fy * 6.],
                        half: [3., 2.5],
                        angle: a,
                        shape: 0.,
                        depth: depth - 0.0002,
                        color: rgba(0x1b5e20, 1.),
                    });
                    out.push(Body {
                        center: [px, py],
                        half: [3.4, 3.4],
                        angle: 0.,
                        shape: 1.,
                        depth: depth - 0.0003,
                        color: rgba(0xf07d00, 1.),
                    });
                }
            }
            // Warnblinker (Paketwagen in zweiter Reihe): alle vier Ecken, 1,5 Hz
            if c.hazard && (w.time * 3.).floor() as i64 % 2 == 0 {
                let (rx, ry) = (-fy, fx);
                for (along, side) in [(1f32, 1f32), (1., -1.), (-1., 1.), (-1., -1.)] {
                    out.push(Body {
                        center: [
                            x + fx * hw * 0.95 * along + rx * hh * 0.85 * side,
                            y + fy * hw * 0.95 * along + ry * hh * 0.85 * side,
                        ],
                        half: [2.2, 2.2],
                        angle: 0.,
                        shape: 1.,
                        depth: depth - 0.0006,
                        color: [1., 0.66, 0.1, 1.],
                    });
                }
            }
            // Blinker (Abbiegen, aus dem Spurgraph) und Rückfahrlicht
            let blink = c.ai.as_ref().map_or(0, |ai| ai.blink);
            if blink != 0 && !c.hazard && (w.time * 3.).floor() as i64 % 2 == 0 {
                let (rx, ry) = (-fy, fx);
                let side = blink.signum() as f32;
                for along in [1f32, -1.] {
                    out.push(Body {
                        center: [
                            x + fx * hw * 0.93 * along + rx * hh * 0.82 * side,
                            y + fy * hw * 0.93 * along + ry * hh * 0.82 * side,
                        ],
                        half: [1.6, 1.],
                        angle: a,
                        shape: 0.,
                        depth: depth - 0.0006,
                        color: [1., 0.64, 0.1, 1.],
                    });
                }
            }
            if c.driver.is_some() && c.forward_speed() < -5. && !c.wrecked {
                out.push(Body {
                    center: [x - fx * hw * 0.95, y - fy * hw * 0.95],
                    half: [1., 1.7],
                    angle: a,
                    shape: 0.,
                    depth: depth - 0.0006,
                    color: [0.96, 0.97, 1., 1.],
                });
            }
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
            // Blaulicht: Balken auf dem Dach, links/rechts im Wechsel (8 Takte/s), leuchtender Hof
            if (c.siren || c.blue) && !c.wrecked {
                let ph = (w.time * 8.).floor() as i64 % 2;
                let bx = if c.kind == "ambulance" { hw - 16. } else { -2. };
                let (rx, ry) = (-fy, fx);
                for side in [-1f32, 1.] {
                    let on = (side < 0.) == (ph == 0);
                    let (px, py) = (x + fx * bx + rx * side * 2.5, y + fy * bx + ry * side * 2.5);
                    out.push(Body {
                        center: [px, py],
                        half: [2.5, 2.],
                        angle: a,
                        shape: 0.,
                        depth: depth - 0.0006,
                        color: if on {
                            [0.29, 0.64, 1., 1.]
                        } else {
                            [0.07, 0.23, 0.48, 1.]
                        },
                    });
                    if on {
                        out.push(Body {
                            center: [px, py],
                            half: [9., 9.],
                            angle: 0.,
                            shape: 1.,
                            depth: depth - 0.0007,
                            color: [0.31, 0.63, 1., 0.35],
                        });
                    }
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
        // Radfahrer und E-Roller (fahrend mit Fahrer, liegend ohne)
        for b in w.bikes.iter().filter(|b| near(b.x, b.y)) {
            bike_bodies(b, w.time, out);
        }
        life_bodies(w, &near, out);
        rail_bodies(&self.trains, &self.tram_segs, w, out);
        crate::underground::entrance_bodies(w, out);
        for p in w.peds.iter().filter(|p| near(p.x, p.y)) {
            let depth = if p.level.lvl >= 1 { 0.549 } else { 0.618 };
            let (x, y, a) = (p.x as f32, p.y as f32, p.facing as f32);
            if p.state == PedState::Hang
                && let Some(h) = &p.hang
                && hang_bodies(p, h, out)
            {
                continue;
            }
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
            crate::figure::person_bodies(
                &crate::figure::Who::of(p),
                &crate::figure::look_of(p),
                depth,
                w.time,
                out,
            );
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
        player_figure(
            &w.player,
            &crate::figure::player_look(),
            PLAYER_RING[0],
            w.time,
            out,
        );
        if let Some(s) = w.p2.as_ref() {
            player_figure(
                &s.player,
                &crate::figure::player2_look(),
                PLAYER_RING[1],
                w.time,
                out,
            );
        }
        if self.moto_lab {
            moto_lab_bodies(self.world.camera.x as f32, self.world.camera.y as f32, out);
        }
        crate::streetfurn::lamp_bodies(&self.street_lamps, self.lamps_lit, out);
        crate::streetfurn::sign_bodies(&self.street_signs, out);
        self.fx.bodies(out);
        crate::weatherfx::ground_bodies(w, &self.trails, out);
        crate::weatherfx::bodies(w, out);
        crate::weatherfx::spray_bodies(w, out);
    }
    /// Umriss der Spielfigur bzw. des eigenen Fahrzeugs, wo Dach, Baumkrone oder Viadukt sie verdecken
    /// (`occlusion.js` + `render.js drawCovered`). Etwas näher als die eigenen Teile, damit der Umriss nur unter
    /// Verdeckendem erscheint.
    fn effects(&self, out: &mut Vec<Body>) {
        // Die Lichtkarte wirkt nur bei Dunkelheit; dann bekommen Qualm und Staub das Umgebungslicht selbst
        let l = self.lighting().unwrap_or_default();
        let ambient = if l.dark > 0. { l.ambient } else { [1.; 3] };
        self.fx.effects(out, ambient);
        // Einsteigen möglich: das Fahrzeug, in das F bzw. Y führt, schimmert dezent (je Spieler zu Fuß); durchscheinend,
        // daher hier und nicht in `bodies` (sonst verdeckte es den Schatten und hellte die Silhouette auf)
        if matches!(self.screen, Screen::Playing) {
            let w = &self.world;
            enter_hint(w, &w.player, PLAYER_RING[0], ambient, out);
            if let Some(s) = w.p2.as_ref() {
                enter_hint(w, &s.player, PLAYER_RING[1], ambient, out);
            }
        }
    }
    fn silhouettes(&self, out: &mut Vec<Body>) {
        if !self.debug.silhouettes {
            return;
        }
        let w = &self.world;
        if !matches!(self.screen, Screen::Playing | Screen::Paused) || w.in_tunnel_station() {
            return;
        }
        // render.js drawCovered: alle Bewegten, knapp vor ihren eigenen Teilen (sie verdecken sich nicht selbst)
        let sp = self.spots();
        let near = |x: f64, y: f64| {
            sp.near(x, y, |z| {
                let view = 2200. / z.max(0.5);
                (view, view * 0.7)
            })
        };
        let own = w.player_car().map(|c| c.id);
        for c in w
            .cars
            .iter()
            .filter(|c| near(c.x, c.y) && Some(c.id) != own)
        {
            let depth = if c.lvl() >= 1 { 0.547 } else { 0.617 };
            out.push(vehicle_silhouette(c, depth, SIL_OTHER));
        }
        // Radfahrer und Bahnwagen
        for b in w.bikes.iter().filter(|b| near(b.x, b.y)) {
            let half = if b.kind == berlin_sim::bikes::Kind::Scooter {
                6.
            } else {
                8.
            };
            out.push(Body {
                center: [b.x as f32, b.y as f32],
                half: [half, 3.5],
                angle: b.angle as f32,
                shape: 1.,
                depth: if b.level.lvl >= 1 { 0.5506 } else { 0.6186 },
                color: SIL_OTHER,
            });
        }
        for t in &self.trains {
            for (c, &lvl) in t.cars.iter().zip(&t.lvl) {
                if !near(c.x, c.y) {
                    continue;
                }
                out.push(Body {
                    center: [c.x as f32, c.y as f32],
                    half: [c.l as f32 / 2., c.w as f32 / 2.],
                    angle: c.angle as f32,
                    shape: 0.,
                    depth: if lvl >= 1 { 0.545 } else { 0.62 },
                    color: SIL_OTHER,
                });
            }
        }
        for p in w.peds.iter().filter(|p| near(p.x, p.y)) {
            if matches!(
                p.state,
                berlin_sim::pedestrians::PedState::Dead | berlin_sim::pedestrians::PedState::Hang
            ) {
                continue;
            }
            let depth = if p.level.lvl >= 1 { 0.5485 } else { 0.6175 };
            out.push(Body {
                center: [p.x as f32, p.y as f32],
                half: [4.5, 6.5],
                angle: p.facing as f32,
                shape: 1.,
                depth,
                color: SIL_OTHER,
            });
        }
        if let Some(c) = w.player_car() {
            let depth = if c.lvl() >= 1 { 0.547 } else { 0.617 };
            out.push(vehicle_silhouette(c, depth - 0.0001, SIL_OWN));
        } else if w.player.ride.is_none() && !w.player.combat.dead {
            let (x, y) = (w.player.x as f32, w.player.y as f32);
            let depth = if w.player.level.lvl >= 1 {
                0.5475
            } else {
                0.6162
            };
            // Körper (Schultern quer zur Blickrichtung) und ein feiner Ortungsring
            out.push(Body {
                center: [x, y],
                half: [5., 7.5],
                angle: w.player.angle as f32,
                shape: 1.,
                depth: depth - 0.0001,
                color: SIL_OWN,
            });
            out.push(Body {
                center: [x, y],
                half: [13., 13.],
                angle: 0.,
                shape: 2.,
                depth,
                color: SIL_OWN,
            });
        }
    }
    fn take_vehicle_atlas(&mut self) -> Option<(Vec<u8>, u32, u32)> {
        if self.vehicle_atlas_sent {
            return None;
        }
        self.vehicle_atlas_sent = true;
        let t = std::time::Instant::now();
        let a = crate::carart::atlas();
        eprintln!(
            "Fahrzeugbilder: {} × {} px in {:.0} ms",
            a.1,
            a.2,
            t.elapsed().as_secs_f64() * 1000.
        );
        Some(a)
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
    fn rumble(&mut self) -> Option<berlin_engine::Rumble> {
        self.rumble_out.take()
    }
    fn frame_stats(&mut self, stats: berlin_engine::FrameStats) {
        self.fps.push(stats.dt, stats.work_ms);
    }
    fn hud(
        &mut self,
        camera: &berlin_engine::camera::Camera,
        viewport: Vec2,
        out: &mut berlin_engine::hud::Hud,
    ) {
        if self.debug.levels {
            crate::levelview::draw(&mut self.world, camera, viewport, out);
        }
        self.hud_main(camera, viewport, out);
        if self.debug.fps {
            let line = self.fps.line();
            let cx = out.width / 2.;
            let w = out.text_width(&line, 14.) + 24.;
            out.rect(cx - w / 2., 8., w, 26., [0., 0., 0., 0.7], 6.);
            out.text(
                &line,
                cx,
                27.,
                14.,
                self.fps.color(),
                berlin_engine::hud::Align::Center,
                false,
            );
        }
    }
    fn graphics(&self) -> berlin_engine::graphics::GraphicsSettings {
        self.graphics
    }
    fn lighting(&self) -> Option<Lighting> {
        let mut l = lighting_of(&world_light(&self.world));
        l.fog = self.world.sky.p.fog as f32;
        l.wet = self.world.weather.wet.clamp(0., 1.) as f32;
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
        {
            // Laternen und Wegweiser im Bild für bodies()/hud() vormerken
            let sp = self.spots();
            let w = &mut self.world;
            let lamps = &mut self.lamps;
            self.street_lamps = if w.in_tunnel_station() {
                Vec::new()
            } else {
                sp.gather(
                    |cx, cy, z| lamps.near(&mut w.city, cx, cy, 1500. / z.max(0.5)),
                    |lp| (lp.x, lp.y),
                )
            };
            self.street_signs = sp.gather(
                |cx, cy, z| w.city.signs_near(cx, cy, 1500. / z.max(0.5)),
                |sg| (sg.x, sg.y),
            );
            self.lamps_lit = l.lamps_on;
        }
        // Schilder am Eingang: tags matt, ab der Dämmerung leuchtend
        self.collect_signs(((l.dark as f32 - 0.25) * 3.).clamp(0., 1.));
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
        let sp = self.spots();
        if l.lamps_on {
            // Laternenköpfe selbst leuchten (render.js lightOccluders malt sie in die Lichtkarte)
            for lp in &self.street_lamps {
                let (hx, hy) = crate::streetfurn::lamp_head(lp);
                push(out, hx as f64, hy as f64, 9., rgb(lp.rgb), 1.);
            }
            let (lamps, city) = (&mut self.lamps, &mut self.world.city);
            for lp in sp.gather(
                |cx, cy, z| lamps.near(city, cx, cy, 2200. / z.max(0.5) + 250.),
                |lp| (lp.x, lp.y),
            ) {
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
        // Läden, Lokale und Bahnhöfe: warmer Schein aus dem Schaufenster auf den Gehweg; Schilder leuchten
        if !self.world.in_tunnel_station() {
            let city = &mut self.world.city;
            let mut pois = sp.gather(
                |cx, cy, z| city.pois_near(cx, cy, 2200. / z.max(0.5) + 250.),
                |q| (q.x, q.y),
            );
            pois.retain(|q| crate::neon::SHOP_GLOW.contains(&q.cat));
            for q in pois {
                if let Some((gx, gy)) = self.neon.glow_point(&mut self.world.city, &q) {
                    push(out, gx, gy, 70., [1., 0.82, 0.59], 0.45 * k);
                }
            }
            let neon_k = self.neon.alpha;
            for s in &self.neon.signs {
                if s.on && neon_k > 0. {
                    let r = if matches!(s.spec.style, crate::neon::Style::Box) {
                        80.
                    } else {
                        60.
                    };
                    push(out, s.x, s.y + 6., r, s.spec.color, 0.6 * neon_k);
                }
            }
        }
        let w = &self.world;
        let near = |x: f64, y: f64| {
            sp.near(x, y, |z| {
                let view = 2200. / z.max(0.5);
                (view + 250., view * 0.7 + 250.)
            })
        };
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
            if c.siren || c.blue {
                // Blaulicht leuchtet die Umgebung im Takt an
                let on = (w.time * 8.).floor() as i64 % 2 == 0;
                push(
                    out,
                    c.x,
                    c.y,
                    140.,
                    [0.3, 0.55, 1.],
                    if on { 0.9 } else { 0.35 } * k,
                );
            }
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
            pad2: Default::default(),
            pad2_pressed: Default::default(),
            mouse: Default::default(),
            typed: "",
        };
        let b = crate::bindings::Bindings::default();
        let i = input_from(&keys, true, &b);
        assert_eq!(i.throttle, 1., "Taste W und Trigger: das Stärkere zählt");
        // Bremse über die progressive Kennlinie: 20 % Trigger bremsen nur leicht
        let lt = crate::bindings::trigger_curve(0.2, crate::bindings::BRAKE_GAMMA) as f64;
        assert!((i.brake - lt).abs() < 1e-6 && i.brake < 0.06, "{}", i.brake);
        assert!(i.steer < -0.5 && i.enter_exit);
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
                pad2: Default::default(),
                pad2_pressed: Default::default(),
                mouse: Default::default(),
                typed: "",
            },
            false,
            &b,
        );
        assert!(walk.move_y < -0.9 && walk.sprint && walk.throttle == 0.);
    }
    /// Controller-Belegung nach dem Notizblatt: zu Fuß A sprinten, X springen, Y einsteigen, B nachladen; im Auto
    /// B Handbremse, Y aussteigen, X hupen; der rechte Stick öffnet zu Fuß das Waffenrad, im Auto das Fahrhilfen-Rad.
    #[test]
    fn controller_layout_follows_the_note() {
        use crate::bindings::{Action as A, Bindings, PadButton as P};
        let b = Bindings::default();
        assert_eq!(b.pad_of(A::Sprint), Some(P::A));
        assert_eq!(b.pad_of(A::Jump), Some(P::X));
        assert_eq!(b.pad_of(A::EnterExit), Some(P::Y));
        assert_eq!(b.pad_of(A::Reload), Some(P::B));
        assert_eq!(b.pad_of(A::WeaponWheel), Some(P::RS));
        assert_eq!(b.pad_of(A::Handbrake), Some(P::B));
        assert_eq!(b.pad_of(A::Horn), Some(P::X));
        assert_eq!(b.pad_of(A::AssistWheel), Some(P::RS));
        assert!(
            b.conflicts().is_empty(),
            "Standard ohne Doppelbelegung: {:?}",
            b.conflicts()
        );
        let none = HashSet::new();
        let keys = |edges: Pad| Keys {
            held: &none,
            pressed: &none,
            pad: Pad {
                connected: true,
                b: edges.b,
                x: edges.x,
                ..Default::default()
            },
            pad_pressed: edges,
            pad2: Default::default(),
            pad2_pressed: Default::default(),
            mouse: Default::default(),
            typed: "",
        };
        let x = Pad {
            x: true,
            ..Default::default()
        };
        let bb = Pad {
            b: true,
            ..Default::default()
        };
        let foot_x = input_from(&keys(x), false, &b);
        assert!(foot_x.jump && !foot_x.combat.reload);
        let foot_b = input_from(&keys(bb), false, &b);
        assert!(foot_b.combat.reload && !foot_b.combat.kick);
        let car_b = input_from(&keys(bb), true, &b);
        assert!(car_b.handbrake);
        let car_x = input_from(&keys(x), true, &b);
        assert!(car_x.horn && !car_x.jump);
    }
    /// LB + RB gleichzeitig öffnen die Befehlszeile mit Bildschirmtastatur; A tippt, B leert bzw. schließt. Ein
    /// einzelner LB-Druck wechselt weiterhin die Waffe (leicht verzögert).
    #[test]
    fn shoulders_open_the_console_with_a_pad_keyboard() {
        let dir = std::env::temp_dir().join(format!("gta-berlin-chord-{}", std::process::id()));
        let root = berlin_map_loader::default_data_root();
        let mut p = Play::new(
            &root,
            4,
            Some(FileStorage::new(dir.join("s.json"))),
            false,
            Start::New,
        )
        .unwrap();
        let none = HashSet::new();
        let step = |p: &mut Play, held: Pad, edges: Pad| {
            p.step(
                &Keys {
                    held: &none,
                    pressed: &none,
                    pad: Pad {
                        connected: true,
                        ..held
                    },
                    pad_pressed: edges,
                    pad2: Default::default(),
                    pad2_pressed: Default::default(),
                    mouse: Default::default(),
                    typed: "",
                },
                DT,
            );
        };
        for _ in 0..5 {
            step(&mut p, Pad::default(), Pad::default());
        }
        // einzelnes LB: nächste Waffe, keine Befehlszeile
        let w0 = p.world.player.combat.weapon;
        let lb = Pad {
            lb: true,
            ..Default::default()
        };
        step(&mut p, lb, lb);
        for _ in 0..10 {
            step(&mut p, Pad::default(), Pad::default());
        }
        assert!(!p.console.open);
        assert_ne!(p.world.player.combat.weapon, w0, "LB wechselt die Waffe");
        // beide gleichzeitig
        let both = Pad {
            lb: true,
            rb: true,
            ..Default::default()
        };
        step(&mut p, both, both);
        assert!(
            p.console.open && p.kbd.on,
            "Befehlszeile mit Bildschirmtastatur"
        );
        let a = Pad {
            a: true,
            ..Default::default()
        };
        step(&mut p, Pad::default(), a);
        assert_eq!(p.console.text, "q");
        let b = Pad {
            b: true,
            ..Default::default()
        };
        step(&mut p, Pad::default(), b);
        assert!(
            p.console.open && p.console.text.is_empty(),
            "B leert zuerst"
        );
        step(&mut p, Pad::default(), b);
        assert!(!p.console.open && !p.kbd.on, "B schließt");
    }
    /// Einsteigen möglich: das nächste freie Fahrzeug schimmert in der Farbe des Spielers, sonst nichts.
    #[test]
    fn enterable_car_is_highlighted() {
        let dir = std::env::temp_dir().join(format!("gta-berlin-hint-{}", std::process::id()));
        let root = berlin_map_loader::default_data_root();
        let mut p = Play::new(
            &root,
            4,
            Some(FileStorage::new(dir.join("s.json"))),
            false,
            Start::New,
        )
        .unwrap();
        let pc = p.world.player_car_id.unwrap();
        let (x, y) = p.world.car(pc).map(|c| (c.x, c.y)).unwrap();
        let halo = |p: &Play| {
            let mut out = Vec::new();
            p.effects(&mut out);
            out.iter()
                .filter(|b| {
                    b.shape == 0.
                        && (b.center[0] - x as f32).abs() < 1.
                        && (b.center[1] - y as f32).abs() < 1.
                })
                .count()
        };
        (p.world.player.x, p.world.player.y) = (x + 300., y);
        assert_eq!(halo(&p), 0, "zu weit weg");
        (p.world.player.x, p.world.player.y) = (x + 20., y);
        assert_eq!(
            enter_target(&p.world, &p.world.player).map(|c| c.id),
            Some(pc)
        );
        assert_eq!(halo(&p), 1, "in Reichweite: Schimmer");
    }
    /// Maus am PC: links läuft nur (auch auf eine Person), rechts schießt zum Zeiger, beide Tasten öffnen das Rad;
    /// Koop: Start auf dem zweiten Controller holt Spieler 2 dazu, sein Stick bewegt nur ihn; mit nur einem
    /// Controller übernimmt Spieler 2 den ersten, Spieler 1 spielt mit Tastatur.
    #[test]
    fn second_controller_joins_and_moves_player_two() {
        let dir = std::env::temp_dir().join(format!("gta-berlin-coop-{}", std::process::id()));
        let root = berlin_map_loader::default_data_root();
        let mut p = Play::new(
            &root,
            4,
            Some(FileStorage::new(dir.join("s.json"))),
            false,
            Start::New,
        )
        .unwrap();
        let none = HashSet::new();
        let step = |p: &mut Play, pad: Pad, pad2: Pad, pad2_pressed: Pad| {
            p.step(
                &Keys {
                    held: &none,
                    pressed: &none,
                    pad,
                    pad_pressed: Pad::default(),
                    pad2,
                    pad2_pressed,
                    mouse: Default::default(),
                    typed: "",
                },
                DT,
            );
        };
        let idle = Pad {
            connected: true,
            ..Default::default()
        };
        for _ in 0..5 {
            step(&mut p, idle, idle, Pad::default());
        }
        assert!(!p.world.coop());
        let start = Pad {
            menu: true,
            ..Default::default()
        };
        step(&mut p, idle, idle, start);
        assert!(p.world.coop(), "Start auf Controller 2 tritt bei");
        assert_eq!(p.screen, Screen::Playing, "kein Pausenmenü beim Beitreten");
        let x1 = p.world.player.x;
        let x2 = p.world.p2.as_ref().unwrap().player.x;
        let right = Pad {
            connected: true,
            lx: 1.,
            ..Default::default()
        };
        for _ in 0..40 {
            step(&mut p, idle, right, Pad::default());
        }
        assert!(
            p.world.p2.as_ref().unwrap().player.x > x2 + 5.,
            "Spieler 2 läuft"
        );
        assert!((p.world.player.x - x1).abs() < 1e-9, "Spieler 1 steht");
        // Start auf Controller 2 im Spiel: Pause, Spieler 2 verlässt über das Menü
        step(&mut p, idle, idle, start);
        assert_eq!(p.screen, Screen::Paused);
        let k = p
            .menu
            .items
            .iter()
            .position(|i| i.action == crate::menu::Action::Coop)
            .unwrap();
        assert_eq!(p.menu.items[k].label, "Spieler 2 verlassen");
        p.menu.index = k;
        let confirm = Pad {
            connected: true,
            a: true,
            ..Default::default()
        };
        p.step(
            &Keys {
                held: &none,
                pressed: &none,
                pad: idle,
                pad_pressed: confirm,
                pad2: idle,
                pad2_pressed: Pad::default(),
                mouse: Default::default(),
                typed: "",
            },
            DT,
        );
        assert!(!p.world.coop(), "Spieler 2 hat verlassen");
        // nur ein Controller: Spieler 2 übernimmt ihn, Spieler 1 bleibt an der Tastatur
        p.join_coop(0);
        assert!(p.world.coop());
        let x1 = p.world.player.x;
        let x2 = p.world.p2.as_ref().unwrap().player.x;
        for _ in 0..40 {
            step(&mut p, right, Pad::default(), Pad::default());
        }
        assert!(
            p.world.p2.as_ref().unwrap().player.x > x2 + 5.,
            "Controller 1 lenkt Spieler 2"
        );
        assert!(
            (p.world.player.x - x1).abs() < 1e-9,
            "Spieler 1 sieht den Controller nicht"
        );
    }

    /// im Auto steigt keine Maustaste aus.
    #[test]
    fn mouse_left_walks_right_shoots_both_open_the_wheel() {
        use berlin_engine::Mouse;
        use berlin_sim::events::Event;
        let dir = std::env::temp_dir().join(format!("gta-berlin-mouse-{}", std::process::id()));
        let root = berlin_map_loader::default_data_root();
        let mut p = Play::new(
            &root,
            4,
            Some(FileStorage::new(dir.join("s.json"))),
            false,
            Start::New,
        )
        .unwrap();
        let none = HashSet::new();
        let step = |p: &mut Play, mouse: Mouse| {
            p.step(
                &Keys {
                    held: &none,
                    pressed: &none,
                    pad: Pad::default(),
                    pad_pressed: Pad::default(),
                    pad2: Default::default(),
                    pad2_pressed: Default::default(),
                    mouse,
                    typed: "",
                },
                DT,
            );
            p.world
                .events
                .iter()
                .filter(|e| matches!(e, Event::Shot { .. }))
                .count()
        };
        let t0 = std::time::Instant::now();
        while p.world.loading && t0.elapsed().as_secs() < 30 {
            step(&mut p, Mouse::default());
        }
        assert_eq!(p.screen, Screen::Playing);
        p.world.mission.state = berlin_sim::mission::State::Available;
        // Pistole, volles Magazin, eine Person 60 px vor der Figur
        let pistol = berlin_sim::combat::WEAPONS
            .iter()
            .position(|w| w.id == "pistol")
            .unwrap();
        p.world.player.combat.weapon = pistol;
        p.world.player.combat.mag[pistol] = 12;
        p.world.player.combat.cool = 0.;
        let (x, y) = (p.world.player.x + 60., p.world.player.y);
        if let Some(q) = p.world.peds.first_mut() {
            (q.x, q.y) = (x, y);
        }
        let at = Some(glam::Vec2::new(x as f32, y as f32));
        let hud = Some(glam::Vec2::new(640., 360.));
        // links auf die Person: hinlaufen, kein Schuss
        let shots = step(
            &mut p,
            Mouse {
                world: at,
                hud,
                left: true,
                left_pressed: true,
                ..Default::default()
            },
        );
        assert_eq!(shots, 0, "links schießt nie");
        assert!(
            matches!(
                p.world.player.click,
                Some(berlin_sim::world::Click::Walk { .. })
            ),
            "links läuft nur hin: {:?}",
            p.world.player.click
        );
        step(
            &mut p,
            Mouse {
                world: at,
                hud,
                ..Default::default()
            },
        );
        p.world.player.click = None;
        // rechts: Schuss zum Zeiger
        let shots = step(
            &mut p,
            Mouse {
                world: at,
                hud,
                right: true,
                right_pressed: true,
                ..Default::default()
            },
        );
        assert_eq!(shots, 1, "rechts schießt");
        for _ in 0..30 {
            step(
                &mut p,
                Mouse {
                    world: at,
                    hud,
                    ..Default::default()
                },
            );
        }
        // beide Tasten: Waffenrad, kein Schuss, nach der Haltezeit offen
        let mut shots = step(
            &mut p,
            Mouse {
                world: at,
                hud,
                right: true,
                right_pressed: true,
                left: true,
                left_pressed: true,
                ..Default::default()
            },
        );
        for _ in 0..30 {
            shots += step(
                &mut p,
                Mouse {
                    world: at,
                    hud,
                    right: true,
                    left: true,
                    ..Default::default()
                },
            );
        }
        assert_eq!(shots, 0, "beide Tasten schießen nicht");
        assert!(p.wheel_m.open, "beide Tasten halten öffnet das Waffenrad");
        step(
            &mut p,
            Mouse {
                world: at,
                hud,
                ..Default::default()
            },
        );
        assert!(!p.wheel_m.open && !p.wheel_m.down, "loslassen schließt");
        // im Auto: keine Maustaste steigt aus (einzeln, zusammen, getippt, gehalten, doppelt)
        let pc = p.world.player_car_id.expect("Spielerauto");
        let (cx, cy) = p.world.car(pc).map(|c| (c.x, c.y)).unwrap();
        (p.world.player.x, p.world.player.y) = (cx + 15., cy);
        p.world.update(
            &Input {
                enter_exit: true,
                ..Default::default()
            },
            DT,
        );
        assert_eq!(p.world.player.in_car, Some(pc), "per Taste eingestiegen");
        let car_at = Some(glam::Vec2::new(cx as f32, cy as f32));
        let presses = [
            (true, false),
            (false, true),
            (true, true),
            (true, false),
            (true, false),
            (false, true),
            (false, true),
        ];
        for (l, r) in presses {
            for k in 0..20 {
                step(
                    &mut p,
                    Mouse {
                        world: car_at,
                        hud,
                        left: l && k < 12,
                        right: r && k < 12,
                        left_pressed: l && k == 0,
                        right_pressed: r && k == 0,
                        ..Default::default()
                    },
                );
            }
        }
        assert_eq!(
            p.world.player.in_car,
            Some(pc),
            "per Maus nicht ausgestiegen"
        );
        let _ = std::fs::remove_dir_all(&dir);
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
    fn graphics_settings_from_json() {
        use berlin_engine::graphics::{GraphicsMode, GraphicsSettings, Quality};
        let g = |v| graphics_from_json(&v);
        assert_eq!(g(serde_json::json!({})), GraphicsSettings::default());
        assert_eq!(
            g(serde_json::json!({"grafik": "pixel", "qualitaet": "mittel"})),
            GraphicsSettings {
                mode: GraphicsMode::Pixel,
                quality: Quality::Mittel
            }
        );
        // Unsinn: Standard je Feld
        assert_eq!(
            g(serde_json::json!({"grafik": 3, "qualitaet": "ultra"})),
            GraphicsSettings::default()
        );
    }

    #[test]
    fn graphics_switch_by_menu_key_and_console_is_saved() {
        use berlin_engine::graphics::{GraphicsMode, Quality};
        let dir = std::env::temp_dir().join(format!("gta-berlin-grafik-{}", std::process::id()));
        let path = dir.join("save.json");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let root = berlin_map_loader::default_data_root();
        let mut p =
            Play::new(&root, 4, Some(FileStorage::new(&path)), false, Start::Title).unwrap();
        assert_eq!(p.graphics.mode, GraphicsMode::Hd, "HD ist Standard");
        let none = HashSet::new();
        let press = |p: &mut Play, k: Option<KeyCode>| {
            let pressed: HashSet<KeyCode> = k.into_iter().collect();
            p.step(
                &Keys {
                    held: &none,
                    pressed: &pressed,
                    pad: Pad::default(),
                    pad_pressed: Pad::default(),
                    pad2: Default::default(),
                    pad2_pressed: Default::default(),
                    mouse: Default::default(),
                    typed: "",
                },
                DT,
            );
        };
        // Titelmenü: Neues Spiel → Steuerung → Grafik
        let g = p
            .menu
            .items
            .iter()
            .position(|i| i.action == crate::menu::Action::Graphics)
            .expect("Grafik im Titelmenü");
        assert_eq!(p.menu.items[g].label, "Grafik: HD");
        while p.menu.index != g {
            press(&mut p, Some(KeyCode::ArrowDown));
        }
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(p.graphics.mode, GraphicsMode::Pixel);
        assert_eq!(p.menu.items[g].label, "Grafik: Pixel");
        let stored = || read_settings(&FileStorage::new(&path)).2;
        assert_eq!(stored().mode, GraphicsMode::Pixel, "in settings.json");
        // Konsole
        assert!(p.run_command("grafik hd").ok);
        assert!(p.run_command("qualitaet mittel").ok);
        assert!(!p.run_command("qualitaet ultra").ok);
        assert_eq!(
            (p.graphics.mode, p.graphics.quality),
            (GraphicsMode::Hd, Quality::Mittel)
        );
        assert_eq!(stored().quality, Quality::Mittel);
        // Taste (F8) im Spiel
        p.screen = Screen::Playing;
        press(&mut p, Some(KeyCode::F8));
        assert_eq!(p.graphics.mode, GraphicsMode::Pixel);
        assert_eq!(stored().mode, GraphicsMode::Pixel);
        // ein neuer Start liest die Einstellung
        let q = Play::new(&root, 4, Some(FileStorage::new(&path)), false, Start::Title).unwrap();
        assert_eq!(q.graphics.mode, GraphicsMode::Pixel);
        assert_eq!(q.menu.items[g].label, "Grafik: Pixel");
        let _ = std::fs::remove_dir_all(&dir);
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
                    pad2: Default::default(),
                    pad2_pressed: Default::default(),
                    mouse: Default::default(),
                    typed: "",
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
        // „Über das Spiel“ aus der Pause und zurück in die Pause (davor: Spieler 2, Steuerung, Grafik, Statistik)
        for _ in 0..7 {
            press(&mut p, Some(KeyCode::ArrowDown));
        }
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(p.screen, Screen::About(false));
        press(&mut p, Some(KeyCode::ArrowRight));
        assert_eq!(p.about.tab, crate::about::Tab::Licenses);
        press(&mut p, Some(KeyCode::Escape));
        assert_eq!(p.screen, Screen::Paused);
        press(&mut p, Some(KeyCode::ArrowDown));
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
        // Statistik (unter dem Grafik-Eintrag): Spielzeit des Spiels ist gezählt und liegt in der Datei
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(p.screen, Screen::Stats(true));
        assert!(p.stats.get("timePlayed") > 0. && p.stats_total.get("timePlayed") > 0.);
        let file = std::fs::read_to_string(dir.join("stats.json")).expect("stats.json");
        assert!(file.contains("timePlayed"));
        press(&mut p, Some(KeyCode::Escape));
        assert_eq!(p.screen, Screen::Title);
        // Über das Spiel vom Titel aus, zurück zum Titel
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::Enter));
        assert_eq!(p.screen, Screen::About(true));
        press(&mut p, Some(KeyCode::Escape));
        assert_eq!(p.screen, Screen::Title);
        // Beenden
        press(&mut p, Some(KeyCode::ArrowDown));
        press(&mut p, Some(KeyCode::Enter));
        assert!(p.quit());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn real_bar_feed_lands_in_berlin() {
        use berlin_sim::city::{City, DiskSource};
        let root = berlin_map_loader::default_data_root();
        let Some(path) = root
            .parent()
            .map(|p| p.join("bars.json"))
            .filter(|p| p.exists())
        else {
            return; // Feed fehlt (gitignored, npm run bars:fetch)
        };
        let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).unwrap();
        let mut w = World::new(city, 3, 4, 4);
        let n = load_bars(&mut w, &path).unwrap();
        assert!(n > 0);
        let b = w.city.bars.as_ref().unwrap();
        let inside = b
            .list
            .iter()
            .filter(|b| matches!((b.x, b.y), (Some(x), Some(y)) if w.city.inside_border(x, y)))
            .count();
        assert!(inside * 10 >= n * 9, "{inside} von {n} Bars in Berlin");
        assert!(
            b.list.iter().filter(|b| b.week.is_some()).count() * 2 >= n,
            "Wochenprofile"
        );
    }
}
