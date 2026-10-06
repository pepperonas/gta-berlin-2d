//! Dieselbe Last wie auf der Xbox, hier mit winit (Mac, Windows-Desktop): Vergleichswerte für die Hochrechnung.
//! `cargo run --release -p berlin-probe --example mac` – läuft, bis alle Betriebsarten gemessen sind, und druckt den
//! Bericht (dieselben Zeilen, die die Konsole anzeigt).
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    probe: Option<berlin_probe::Probe>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            el.create_window(
                Window::default_attributes()
                    .with_title("berlin-probe")
                    .with_inner_size(winit::dpi::LogicalSize::new(960, 540)),
            )
            .expect("Fenster"),
        );
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone()).expect("Oberfläche");
        let size = window.inner_size();
        let steps = vec![format!(
            "berlin-probe {} · {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        )];
        self.probe = Some(
            // PROBE_FENCE=1: ohne Zeitstempel messen wie auf der Xbox (Fence), für vergleichbare Werte
            berlin_probe::Probe::new(
                &instance,
                surface,
                size.width,
                size.height,
                steps,
                std::env::var_os("PROBE_FENCE").is_none(),
            )
            .expect("Probe"),
        );
        self.window = Some(window);
        el.set_control_flow(ControlFlow::Poll);
    }
    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(probe) = self.probe.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => probe.resize(s.width, s.height),
            WindowEvent::RedrawRequested => {
                probe.frame().expect("Bild");
                if probe.complete() && !el.exiting() {
                    println!("{}", probe.status());
                    el.exit();
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn main() {
    let mut app = App::default();
    EventLoop::new()
        .expect("Ereignisschleife")
        .run_app(&mut app)
        .expect("Lauf");
}
