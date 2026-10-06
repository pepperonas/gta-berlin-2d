mod atlas;
pub mod camera;
mod facade;
mod gputime;
pub mod graphics;
pub mod hud;
mod lightpass;
mod materials;
pub use materials::MANIFEST as MATERIAL_MANIFEST;
/// Herkunft und Lizenz der HD-Schrift (`data/gfx/font/manifest.json`), für die Danksagungen im Spiel.
pub const FONT_MANIFEST: &str = include_str!("../../../data/gfx/font/manifest.json");
pub mod pad;
mod palette;
mod renderer;
mod scenepass;
pub mod split;
pub mod vehatlas;
pub mod vfx;
use anyhow::Result;
use berlin_map_loader::{
    format::Index,
    stream::{Focus, Status, Streamer},
};
use camera::Camera;
use glam::Vec2;
use std::path::PathBuf;
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::{Window, WindowId},
};

/// Bewegtes Objekt für die Instanz-Pipeline: Mittelpunkt und halbe Ausdehnung in Kartenpixeln, Drehung (rad),
/// Form (0 = abgerundetes Rechteck, 1 = Ellipse, 2 = Ring, 3 = weicher Fleck, 4/5 = Rechteck/Ellipse mit harter Kante), Tiefe (kleiner = weiter vorn), Farbe sRGB + Deckkraft.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Body {
    pub center: [f32; 2],
    pub half: [f32; 2],
    pub angle: f32,
    pub shape: f32,
    pub depth: f32,
    pub color: [f32; 4],
}

/// Licht eines Bildes (aus dem Tageslicht der Spieluhr): Richtung zur Sonne für die Schattierung von Dächern
/// und Fassaden, Schattenrichtung/-länge je Höhe/-stärke, Umgebungslicht (sRGB-Faktoren) und Dunkelheit 0…1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    pub sun: [f32; 3],
    pub shadow: [f32; 2],
    pub shadow_len: f32,
    pub shadow_strength: f32,
    pub ambient: [f32; 3],
    pub dark: f32,
    /// Anteil brennender Fenster (Tagesgang, `daylight.js windowsLit`) und Spieluhr in Minuten
    pub windows: f32,
    pub minutes: f32,
    /// Wärme der Farbabstimmung (`visualstyle.js filmMood`)
    pub warmth: f32,
    /// Nebel 0…1,7 (`weather.fog`): dämpft das Fensterlicht (render.js `fogK`)
    pub fog: f32,
    /// Nässe der Straßen 0…1 (`weather.wet`): dunklere, glänzende Bodenmaterialien
    pub wet: f32,
}
impl Default for Lighting {
    /// 13 Uhr: Sonne im Süden, kurze Schatten nach Norden, volles Tageslicht.
    fn default() -> Self {
        let el = 58_f32.to_radians();
        Self {
            sun: [0., el.cos(), el.sin()],
            shadow: [0., -1.],
            shadow_len: 1. / el.tan(),
            shadow_strength: 1.,
            ambient: [1.; 3],
            dark: 0.,
            windows: 0.,
            minutes: 780.,
            warmth: 0.018,
            fog: 0.,
            wet: 0.,
        }
    }
}

/// Lichtquelle für die Lichtkarte: Laterne, Scheinwerferkegel (`cone` = 1, zeigt in Richtung `angle`), Ampel,
/// Blaulicht. Farbe sRGB 0…1, `intensity` wie die Deckkraft beim additiven Zeichnen der JS-Fassung.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightSource {
    pub center: [f32; 2],
    pub radius: f32,
    pub angle: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub cone: f32,
    pub pad: f32,
}

/// Maus: Lage im Bild (Pixel), auf dem Boden (Kartenpixel) und im HUD (Basiseinheiten, 720 Zeilen); Tasten
/// gehalten und als Flanke bis zum nächsten Simulationsschritt, Mausrad (Rasten) und ob sie sich bewegt hat.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Mouse {
    pub screen: Option<Vec2>,
    pub world: Option<Vec2>,
    pub hud: Option<Vec2>,
    pub left: bool,
    pub right: bool,
    pub left_pressed: bool,
    pub right_pressed: bool,
    pub left_released: bool,
    pub wheel: f32,
    pub moved: bool,
}

/// Gehaltene und in diesem Schritt neu gedrückte Tasten.
pub struct Keys<'a> {
    pub held: &'a HashSet<KeyCode>,
    pub pressed: &'a HashSet<KeyCode>,
    /// Gamepad: gehaltener Zustand und Tastenflanken seit dem letzten Schritt
    pub pad: pad::Pad,
    pub pad_pressed: pad::Pad,
    /// zweiter Controller (Spieler 2 im Koop)
    pub pad2: pad::Pad,
    pub pad2_pressed: pad::Pad,
    pub mouse: Mouse,
    /// Getippter Text seit dem letzten Schritt (Tastaturbelegung beachtet, Wiederholungen inklusive)
    pub typed: &'a str,
}

/// Zeiten des vorigen Bildes für eine Bildratenanzeige: Abstand zum Bild davor und Arbeitszeit (Simulation,
/// Aufbau der Zeichenlisten und Zeichnen bis zur Abgabe an die GPU).
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    pub dt: f32,
    pub work_ms: f32,
}

/// Vibration des Controllers: starker (tiefer) und schwacher (heller) Motor 0…1, Dauer in ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rumble {
    pub strong: f32,
    pub weak: f32,
    pub ms: u32,
}

/// Spiel, das die Engine mit festem Schritt antreibt (die Simulation selbst kennt weder Fenster noch GPU).
pub trait Game {
    /// Neue Vibration für den Controller (einmal je Bild abgeholt; `None` = nichts Neues).
    fn rumble(&mut self) -> Option<Rumble> {
        None
    }
    /// Vibration für den Controller von Spieler 2.
    fn rumble2(&mut self) -> Option<Rumble> {
        None
    }
    /// Zeiten des vorigen Bildes (jedes Bild einmal, vor `hud`).
    fn frame_stats(&mut self, _stats: FrameStats) {}
    /// Fester Simulationsschritt in Sekunden.
    fn step_seconds(&self) -> f64;
    /// Vor dem Bild: zwischen vorigem und aktuellem Simulationsstand zeichnen (`alpha` = Anteil des angebrochenen
    /// Schritts, 0…1). Nach dem Bild folgt `end_frame`.
    fn interpolate(&mut self, _alpha: f64) {}
    /// Nach dem Bild (nach `hud`).
    fn end_frame(&mut self) {}
    fn step(&mut self, keys: &Keys, dt: f64);
    /// Kameraziel (Kartenpixel) und Zoom.
    fn camera(&self) -> (Vec2, f32);
    /// Kameraziel und Zoom von Spieler 2 (Koop); `None` = ein Spieler, ein Bild.
    fn camera2(&self) -> Option<(Vec2, f32)> {
        None
    }
    /// Ansichten dieses Bildes (gemeinsam oder geteilt, `split.rs`), vor `bodies`, `lights` und `hud`.
    fn set_views(&mut self, _views: &split::Views, _viewport: Vec2) {}
    fn bodies(&self, out: &mut Vec<Body>);
    /// Fahrzeugbilder (RGBA8, Breite, Höhe), einmal abgeholt.
    fn take_vehicle_atlas(&mut self) -> Option<(Vec<u8>, u32, u32)> {
        None
    }
    /// Umrisse, die nur dort erscheinen, wo etwas Näheres davor liegt (Spielfigur unter Dach oder Baumkrone).
    fn silhouettes(&self, _out: &mut Vec<Body>) {}
    /// Durchscheinende Effekte (Qualm, Gischt, Leuchtspuren …): nach Licht und Silhouetten gezeichnet, ohne Tiefe
    /// zu schreiben – sie verdecken nichts. Das Licht wenden sie selbst an (Farbe bereits abgedunkelt).
    fn effects(&self, _out: &mut Vec<Body>) {}
    /// Zeile für den Fenstertitel.
    fn status(&self) -> String;
    /// Ist die Spielwelt geladen (für Smoke-Tests)?
    fn ready(&self) -> bool;
    /// Grafikmodus und Qualitätsstufe (einmal je Bild abgefragt; der Renderer baut nur bei Änderung neu).
    fn graphics(&self) -> graphics::GraphicsSettings {
        graphics::GraphicsSettings::default()
    }
    /// Licht des Bildes (Tageszeit); `None` = Licht aus den Optionen.
    fn lighting(&self) -> Option<Lighting> {
        None
    }
    /// Lichtquellen im Bild (nur nachts sichtbar).
    fn lights(&mut self, _out: &mut Vec<LightSource>) {}
    /// Anzeigen über dem Bild; `camera` + `viewport` rechnen Weltpunkte in Bildschirm-Pixel um.
    fn hud(&mut self, _camera: &camera::Camera, _viewport: Vec2, _out: &mut hud::Hud) {}
    /// Will das Spiel beendet werden (Menü „Beenden“)?
    fn quit(&self) -> bool {
        false
    }
    /// Stadtplan für die große Karte; wird einmal abgeholt und hochgeladen.
    fn take_overview(&mut self) -> Option<berlin_map_loader::overview::OverlayMesh> {
        None
    }
}

pub use winit::keyboard::KeyCode;

pub struct Options {
    pub fps: u32,
    pub smoke_frames: Option<u32>,
    pub data_root: PathBuf,
    pub position: Option<Vec2>,
    pub lighting: Lighting,
    pub capture: Option<PathBuf>,
    pub zoom: f32,
    /// Bildzeiten messen (CPU-Arbeit, GPU per Zeitstempel) und beim Ende als JSON schreiben (`--messung`)
    pub metrics: Option<PathBuf>,
    /// Fenstergröße in Bildpunkten (`--fenster BxH`, reproduzierbare Aufnahmen unabhängig vom Bildschirm)
    pub window: Option<(u32, u32)>,
}

pub fn run(options: Options) -> Result<()> {
    run_with(options, None)
}

pub fn run_with(options: Options, game: Option<Box<dyn Game>>) -> Result<()> {
    let Options {
        fps,
        smoke_frames,
        data_root,
        position,
        lighting,
        capture,
        zoom,
        metrics,
        window,
    } = options;
    let index = Arc::new(Index::read(&data_root)?);
    let streamer = Streamer::new(data_root, index.clone())?;
    let camera = Camera {
        position: position.unwrap_or_else(|| index.places.player_spawn.point()),
        zoom,
        ..Camera::default()
    };
    eprintln!("{} · {} Kacheln", index.meta.attribution, index.tiles.len());
    anyhow::ensure!(fps == 60 || fps == 120, "FPS muss 60 oder 120 sein");
    let mut app = App {
        renderer: None,
        done: false,
        capture,
        views: split::Views::single(camera.clone()),
        camera,
        index,
        streamer,
        status: Status::default(),
        lighting,
        lights: Vec::new(),
        smoke_started: Instant::now(),
        keys: HashSet::new(),
        pressed: HashSet::new(),
        typed: String::new(),
        pads: pad::Gamepads::new(),
        game,
        accumulator: 0.,
        bodies: Vec::new(),
        zoom_factor: 1.,
        interval: Duration::from_secs_f64(1.0 / fps as f64),
        next: Instant::now(),
        last: Instant::now(),
        work_ms: 0.,
        frames: 0,
        smoke_frames,
        error: None,
        focused: true,
        mouse: Mouse::default(),
        metrics,
        cpu_samples: Vec::new(),
        window_size: window,
    };
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    Ok(())
}
struct App {
    renderer: Option<renderer::Renderer>,
    done: bool,
    camera: Camera,
    /// Ansichten des letzten Bildes (Splitscreen: zwei; sonst eine = `camera`)
    views: split::Views,
    keys: HashSet<KeyCode>,
    pressed: HashSet<KeyCode>,
    typed: String,
    pads: pad::Gamepads,
    game: Option<Box<dyn Game>>,
    accumulator: f64,
    bodies: Vec<Body>,
    zoom_factor: f32,
    interval: Duration,
    next: Instant,
    last: Instant,
    /// Arbeitszeit des vorigen Bildes (ms)
    work_ms: f32,
    frames: u32,
    smoke_frames: Option<u32>,
    error: Option<anyhow::Error>,
    focused: bool,
    index: Arc<Index>,
    streamer: Streamer,
    status: Status,
    lighting: Lighting,
    lights: Vec<LightSource>,
    smoke_started: Instant,
    capture: Option<PathBuf>,
    mouse: Mouse,
    metrics: Option<PathBuf>,
    /// CPU-Arbeit je Bild (ms, ohne Warten auf das Swapchain-Bild) nach der Aufwärmphase
    cpu_samples: Vec<f32>,
    window_size: Option<(u32, u32)>,
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let result = (|| {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("GTA Berlin · Rust · Berlin wird geladen …")
                        .with_inner_size::<winit::dpi::Size>(match self.window_size {
                            Some((w, h)) => winit::dpi::PhysicalSize::new(w, h).into(),
                            None => winit::dpi::LogicalSize::new(1280, 720).into(),
                        }),
                )?,
            );
            pollster::block_on(renderer::Renderer::new(
                window,
                self.index.meta.scale,
                self.lighting,
                self.metrics.is_some(),
                self.window_size,
            ))
        })();
        match result {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                self.next = Instant::now();
                self.last = self.next;
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.done {
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        if renderer.window.id() != id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => renderer.resize(size),
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                self.keys.clear();
                self.pressed.clear();
                self.last = Instant::now();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && let Some(t) = &event.text
                {
                    self.typed.extend(t.chars().filter(|c| !c.is_control()));
                }
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        if !event.repeat {
                            self.pressed.insert(key);
                        }
                        self.keys.insert(key);
                        match key {
                            // mit Spiel gehört Esc dem Spiel (Pause/Zurück), sonst beendet es den Betrachter
                            KeyCode::Escape if self.game.is_none() => event_loop.exit(),
                            KeyCode::Digit1 if self.game.is_none() => self.camera.zoom = 0.72,
                            KeyCode::Digit2 if self.game.is_none() => self.camera.zoom = 1.0,
                            KeyCode::Digit3 if self.game.is_none() => self.camera.zoom = 2.0,
                            _ => {}
                        }
                    } else {
                        self.keys.remove(&key);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse.screen = Some(Vec2::new(position.x as f32, position.y as f32));
                self.mouse.moved = true;
            }
            WindowEvent::CursorLeft { .. } => self.mouse.screen = None,
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => {
                        self.mouse.left_pressed |= down && !self.mouse.left;
                        self.mouse.left_released |= !down && self.mouse.left;
                        self.mouse.left = down;
                    }
                    MouseButton::Right => {
                        self.mouse.right_pressed |= down && !self.mouse.right;
                        self.mouse.right = down;
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.15,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.0015,
                };
                self.mouse.wheel += match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.,
                };
                if self.game.is_some() {
                    self.zoom_factor = (self.zoom_factor * amount.exp()).clamp(0.5, 2.);
                } else {
                    self.camera.zoom_by(amount.exp());
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
                self.last = now;
                let pressed = |a, b| {
                    if self.keys.contains(&a) || self.keys.contains(&b) {
                        1.0
                    } else {
                        0.0
                    }
                };
                let movement = Vec2::new(
                    pressed(KeyCode::KeyD, KeyCode::ArrowRight)
                        - pressed(KeyCode::KeyA, KeyCode::ArrowLeft),
                    pressed(KeyCode::KeyS, KeyCode::ArrowDown)
                        - pressed(KeyCode::KeyW, KeyCode::ArrowUp),
                );
                self.pads.poll();
                if let Some(game) = self.game.as_mut() {
                    // fester Simulationsschritt; kurze Tastendrücke gelten bis zum nächsten Schritt
                    let step = game.step_seconds();
                    // Smoke-Test und Aufnahme: genau ein Schritt je Bild – die Welt hängt dann nur an der Bildzahl,
                    // nicht an der Uhr, und Aufnahmen lassen sich Bild für Bild vergleichen
                    let advance = if self.smoke_frames.is_some() {
                        step
                    } else {
                        dt as f64
                    };
                    self.accumulator = (self.accumulator + advance).min(step * 5.);
                    let viewport = renderer.viewport();
                    // Smoke-Test und Aufnahme: keine Eingaben (Tasten, die zufällig ins Fenster gehen, verfälschten
                    // sonst das Bild)
                    let quiet = self.smoke_frames.is_some();
                    if quiet {
                        self.keys.clear();
                        self.pressed.clear();
                        self.typed.clear();
                        self.mouse = Mouse::default();
                        self.pads.state = pad::Pad::default();
                        self.pads.edges = pad::Pad::default();
                        self.pads.state2 = pad::Pad::default();
                        self.pads.edges2 = pad::Pad::default();
                    }
                    let mut mouse = self.mouse;
                    mouse.world = mouse
                        .screen
                        .map(|p| self.camera.screen_to_ground(p, viewport));
                    mouse.hud = mouse.screen.map(|p| p / (viewport.y / 720.).max(0.25));
                    let mut stepped = false;
                    while self.accumulator >= step {
                        stepped = true;
                        game.step(
                            &Keys {
                                held: &self.keys,
                                pressed: &self.pressed,
                                pad: self.pads.state,
                                pad_pressed: self.pads.edges,
                                pad2: self.pads.state2,
                                pad2_pressed: self.pads.edges2,
                                mouse,
                                typed: &self.typed,
                            },
                            step,
                        );
                        self.pressed.clear();
                        self.typed.clear();
                        self.pads.edges = pad::Pad::default();
                        self.pads.edges2 = pad::Pad::default();
                        // Flanken und Rad gelten genau einen Schritt
                        mouse.left_pressed = false;
                        mouse.right_pressed = false;
                        mouse.left_released = false;
                        mouse.wheel = 0.;
                        mouse.moved = false;
                        self.accumulator -= step;
                    }
                    if stepped {
                        self.mouse.left_pressed = false;
                        self.mouse.right_pressed = false;
                        self.mouse.left_released = false;
                        self.mouse.wheel = 0.;
                        self.mouse.moved = false;
                    }
                    if game.quit() {
                        event_loop.exit();
                        return;
                    }
                    game.interpolate(self.accumulator / step);
                    let (position, zoom) = game.camera();
                    self.camera.position = position;
                    self.camera.zoom = (zoom * self.zoom_factor).clamp(0.5, 3.2);
                    // Koop: aus beiden Wunschkameras ein gemeinsames oder geteiltes Bild
                    self.views = match game.camera2() {
                        Some((p2, z2)) => {
                            let cam2 = Camera {
                                position: p2,
                                zoom: (z2 * self.zoom_factor).clamp(0.5, 3.2),
                                scale: self.camera.scale,
                            };
                            split::split_views(&self.camera, &cam2, renderer.viewport())
                        }
                        None => split::Views::single(self.camera.clone()),
                    };
                    self.camera = self.views.cams[0].clone();
                    game.set_views(&self.views, renderer.viewport());
                    self.bodies.clear();
                    game.bodies(&mut self.bodies);
                    renderer.set_bodies(&self.bodies);
                    self.bodies.clear();
                    game.silhouettes(&mut self.bodies);
                    renderer.set_silhouettes(&self.bodies);
                    self.bodies.clear();
                    game.effects(&mut self.bodies);
                    renderer.set_effects(&self.bodies);
                    renderer.set_graphics(game.graphics());
                    renderer.set_lighting(game.lighting().unwrap_or(self.lighting));
                    self.lights.clear();
                    game.lights(&mut self.lights);
                    renderer.set_lights(&self.lights);
                    let viewport = renderer.viewport();
                    if let Some((px, w, h)) = game.take_vehicle_atlas() {
                        renderer.set_vehicle_atlas(&px, w, h);
                    }
                    if let Some(mesh) = game.take_overview() {
                        renderer.set_overview(&mesh);
                    }
                    if let Some(r) = game.rumble() {
                        self.pads.rumble(0, r.strong, r.weak, r.ms);
                    }
                    if let Some(r) = game.rumble2() {
                        self.pads.rumble(1, r.strong, r.weak, r.ms);
                    }
                    game.frame_stats(FrameStats {
                        dt,
                        work_ms: self.work_ms,
                    });
                    let mut overlay = hud::Hud::new([viewport.x, viewport.y]);
                    // HD: Schrift aus dem Abstandsfeld, Pixel: Bitmapschrift
                    overlay.sdf = game.graphics().mode == graphics::GraphicsMode::Hd;
                    game.hud(&self.camera, viewport, &mut overlay);
                    game.end_frame();
                    renderer.set_hud(&overlay.items, overlay.map, overlay.map2);
                } else if self.focused {
                    self.camera.position += movement.normalize_or_zero()
                        * (if self.keys.contains(&KeyCode::ShiftLeft) {
                            6000.0
                        } else {
                            1200.0
                        })
                        * dt
                        / self.camera.zoom;
                    self.camera.position = self.camera.position.clamp(
                        Vec2::ZERO,
                        Vec2::new(self.index.meta.width, self.index.meta.height),
                    );
                }
                if self.game.is_none() {
                    self.views = split::Views::single(self.camera.clone());
                }
                let second = (self.views.count == 2).then(|| {
                    let c = &self.views.cams[1];
                    (c.position, renderer.view_bounds(c))
                });
                self.streamer.focus(Focus {
                    position: self.camera.position,
                    view: renderer.view_bounds(&self.camera),
                    second,
                });
                if let Some(snapshot) = self.streamer.latest() {
                    renderer.sync(&snapshot);
                    self.status = snapshot.status;
                    let title = match &self.game {
                        Some(game) => format!(
                            "GTA Berlin · {} · {}/{} Kacheln · {}",
                            game.status(),
                            self.status.ready,
                            self.status.needed,
                            self.index.meta.attribution
                        ),
                        None => format!(
                            "GTA Berlin · {}/{} Kacheln bereit · {} geladen · WASD / Shift / Mausrad · {}",
                            self.status.ready,
                            self.status.needed,
                            self.status.resident,
                            self.index.meta.attribution
                        ),
                    };
                    renderer.window.set_title(&title);
                    if self.smoke_frames.is_some() && !snapshot.errors.is_empty() {
                        self.error = Some(anyhow::anyhow!(snapshot.errors.join("\n")));
                        event_loop.exit();
                        return;
                    }
                }
                // Frist: 30 s zum Laden plus die Bildzahl bei 30 Bildern/s (lange Aufnahmen)
                let limit = 30 + self.smoke_frames.unwrap_or(0) as u64 / 30;
                if self.smoke_frames.is_some()
                    && self.smoke_started.elapsed() > Duration::from_secs(limit)
                {
                    self.error = Some(anyhow::anyhow!(
                        "Karten-Smoke-Test: Timeout nach {limit} Sekunden"
                    ));
                    event_loop.exit();
                    return;
                }
                let rendered = renderer.render_views(&self.views);
                self.work_ms = now.elapsed().as_secs_f32() * 1000.;
                if self.metrics.is_some() && matches!(rendered, Ok(true)) {
                    // erst nach der Aufwärmphase (Kacheln hochladen, Kamera eingeschwungen) zählen
                    if self.frames < METRICS_WARMUP {
                        if let Some(g) = renderer.gpu_samples() {
                            g.clear();
                        }
                    } else {
                        self.cpu_samples
                            .push((self.work_ms - renderer.acquire_ms).max(0.));
                    }
                }
                match rendered {
                    Ok(true) => {
                        if self.status.ready() && self.game.as_ref().is_none_or(|g| g.ready()) {
                            self.frames += 1;
                        }
                        if self.smoke_frames.is_some_and(|limit| self.frames >= limit) {
                            self.done = true;
                            if let Some(path) = self.metrics.take() {
                                let gpu = renderer.gpu_samples().map(std::mem::take);
                                if let Err(error) =
                                    write_metrics(&path, &self.cpu_samples, gpu.as_deref())
                                {
                                    self.error = Some(error);
                                }
                            }
                            if let Some(path) = self.capture.take()
                                && let Err(error) = renderer.capture(&self.camera, &path)
                            {
                                self.error = Some(error);
                            }
                            eprintln!(
                                "Karten-Smoke-Test: {} Frames, {} Kacheln, {} Dreiecke, {} Sprites, {:.1} MiB Meshdaten",
                                self.frames,
                                self.status.resident,
                                self.status.triangles,
                                self.status.sprites,
                                self.status.mesh_bytes as f64 / 1048576.0
                            );
                            event_loop.exit();
                        }
                    }
                    Ok(false) => {}
                    Err(error) => {
                        self.error = Some(error);
                        event_loop.exit();
                    }
                }
                self.next = now + self.interval;
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(renderer) = &self.renderer {
            if renderer.drawable() {
                if Instant::now() >= self.next {
                    renderer.window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
            } else {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
    }
}

/// Bilder nach „bereit“, die nicht gemessen werden (Hochladen der Kacheln, Kamera schwingt ein).
const METRICS_WARMUP: u32 = 60;

/// Median und 95. Perzentil (nächster Rang) einer Messreihe; `None` bei leerer Reihe.
pub fn median_p95(samples: &[f32]) -> Option<(f32, f32)> {
    if samples.is_empty() {
        return None;
    }
    let mut v = samples.to_vec();
    v.sort_by(f32::total_cmp);
    let rank = |p: f64| v[((p * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1];
    Some((rank(0.5), rank(0.95)))
}

/// Messung als JSON: Anzahl, Median, P95 je für CPU und GPU (`null`, wenn keine Zeitstempel verfügbar sind).
fn write_metrics(path: &std::path::Path, cpu: &[f32], gpu: Option<&[f32]>) -> Result<()> {
    let block = |s: &[f32]| match median_p95(s) {
        Some((m, p)) => format!(
            "{{\"frames\": {}, \"median_ms\": {m:.3}, \"p95_ms\": {p:.3}}}",
            s.len()
        ),
        None => "null".into(),
    };
    let gpu_text = gpu.map_or("null".into(), block);
    let note = if gpu.is_some() {
        "Zeitstempel Anfang erster bis Ende letzter Durchgang"
    } else {
        "keine Zeitstempel-Abfragen auf diesem Adapter"
    };
    let json = format!(
        "{{\n  \"cpu\": {},\n  \"gpu\": {gpu_text},\n  \"gpu_quelle\": \"{note}\"\n}}\n",
        block(cpu)
    );
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, json)?;
    eprintln!("Messung: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod metric_tests {
    use super::median_p95;
    #[test]
    fn median_and_p95_use_nearest_rank() {
        assert_eq!(median_p95(&[]), None);
        assert_eq!(median_p95(&[3.]), Some((3., 3.)));
        let v: Vec<f32> = (1..=100).rev().map(|i| i as f32).collect();
        assert_eq!(median_p95(&v), Some((50., 95.)));
    }
}
