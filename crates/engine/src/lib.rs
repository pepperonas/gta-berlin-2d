mod atlas;
pub mod camera;
mod renderer;
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
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::{Window, WindowId},
};

/// Bewegtes Objekt für die Instanz-Pipeline: Mittelpunkt und halbe Ausdehnung in Kartenpixeln, Drehung (rad),
/// Form (0 = abgerundetes Rechteck, 1 = Ellipse, 2 = Ring), Tiefe (kleiner = weiter vorn), Farbe sRGB + Deckkraft.
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

/// Gehaltene und in diesem Schritt neu gedrückte Tasten.
pub struct Keys<'a> {
    pub held: &'a HashSet<KeyCode>,
    pub pressed: &'a HashSet<KeyCode>,
}

/// Spiel, das die Engine mit festem Schritt antreibt (die Simulation selbst kennt weder Fenster noch GPU).
pub trait Game {
    /// Fester Simulationsschritt in Sekunden.
    fn step_seconds(&self) -> f64;
    fn step(&mut self, keys: &Keys, dt: f64);
    /// Kameraziel (Kartenpixel) und Zoom.
    fn camera(&self) -> (Vec2, f32);
    fn bodies(&self, out: &mut Vec<Body>);
    /// Zeile für den Fenstertitel.
    fn status(&self) -> String;
    /// Ist die Spielwelt geladen (für Smoke-Tests)?
    fn ready(&self) -> bool;
}

pub use winit::keyboard::KeyCode;

pub struct Options {
    pub fps: u32,
    pub smoke_frames: Option<u32>,
    pub data_root: PathBuf,
    pub position: Option<Vec2>,
    pub sun_hour: f32,
    pub capture: Option<PathBuf>,
    pub zoom: f32,
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
        sun_hour,
        capture,
        zoom,
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
        camera,
        index,
        streamer,
        status: Status::default(),
        sun_hour,
        smoke_started: Instant::now(),
        keys: HashSet::new(),
        pressed: HashSet::new(),
        game,
        accumulator: 0.,
        bodies: Vec::new(),
        zoom_factor: 1.,
        interval: Duration::from_secs_f64(1.0 / fps as f64),
        next: Instant::now(),
        last: Instant::now(),
        frames: 0,
        smoke_frames,
        error: None,
        focused: true,
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
    keys: HashSet<KeyCode>,
    pressed: HashSet<KeyCode>,
    game: Option<Box<dyn Game>>,
    accumulator: f64,
    bodies: Vec<Body>,
    zoom_factor: f32,
    interval: Duration,
    next: Instant,
    last: Instant,
    frames: u32,
    smoke_frames: Option<u32>,
    error: Option<anyhow::Error>,
    focused: bool,
    index: Arc<Index>,
    streamer: Streamer,
    status: Status,
    sun_hour: f32,
    smoke_started: Instant,
    capture: Option<PathBuf>,
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
                        .with_inner_size(winit::dpi::LogicalSize::new(1280, 720)),
                )?,
            );
            pollster::block_on(renderer::Renderer::new(
                window,
                self.index.meta.scale,
                self.sun_hour,
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
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        if !event.repeat {
                            self.pressed.insert(key);
                        }
                        self.keys.insert(key);
                        match key {
                            KeyCode::Escape => event_loop.exit(),
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
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.15,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.0015,
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
                if let Some(game) = self.game.as_mut() {
                    // fester Simulationsschritt; kurze Tastendrücke gelten bis zum nächsten Schritt
                    let step = game.step_seconds();
                    self.accumulator = (self.accumulator + dt as f64).min(step * 5.);
                    while self.accumulator >= step {
                        game.step(
                            &Keys {
                                held: &self.keys,
                                pressed: &self.pressed,
                            },
                            step,
                        );
                        self.pressed.clear();
                        self.accumulator -= step;
                    }
                    let (position, zoom) = game.camera();
                    self.camera.position = position;
                    self.camera.zoom = (zoom * self.zoom_factor).clamp(0.5, 3.2);
                    self.bodies.clear();
                    game.bodies(&mut self.bodies);
                    renderer.set_bodies(&self.bodies);
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
                self.streamer.focus(Focus {
                    position: self.camera.position,
                    view: renderer.view_bounds(&self.camera),
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
                if self.smoke_frames.is_some()
                    && self.smoke_started.elapsed() > Duration::from_secs(30)
                {
                    self.error = Some(anyhow::anyhow!(
                        "Karten-Smoke-Test: Timeout nach 30 Sekunden"
                    ));
                    event_loop.exit();
                    return;
                }
                match renderer.render(&self.camera) {
                    Ok(true) => {
                        if self.status.ready() && self.game.as_ref().is_none_or(|g| g.ready()) {
                            self.frames += 1;
                        }
                        if self.smoke_frames.is_some_and(|limit| self.frames >= limit) {
                            self.done = true;
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
