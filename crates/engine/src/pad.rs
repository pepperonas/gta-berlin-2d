//! Gamepad über `gilrs` (Xbox-Controller unter macOS und Windows): Achsen roh (Totzone wendet das Spiel an),
//! Tasten gehalten und als Flanken seit dem letzten Simulationsschritt.

/// Zustand des ersten verbundenen Controllers. Achsen −1…1 (y nach unten wie der Bildschirm), Trigger 0…1.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pad {
    pub connected: bool,
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    pub lt: f32,
    pub rt: f32,
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub lb: bool,
    pub rb: bool,
    pub view: bool,
    pub menu: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}
impl Pad {
    /// Tasten, die in `now` gedrückt sind und in `before` nicht (Flanken), in `self` sammeln.
    pub fn latch_edges(&mut self, before: &Pad, now: &Pad) {
        macro_rules! edge {
            ($($f:ident),*) => { $( self.$f |= now.$f && !before.$f; )* };
        }
        edge!(a, b, x, y, lb, rb, view, menu, up, down, left, right);
    }
}

pub(crate) struct Gamepads {
    gilrs: Option<gilrs::Gilrs>,
    pub state: Pad,
    pub edges: Pad,
}
impl Gamepads {
    pub fn new() -> Self {
        let gilrs = match gilrs::Gilrs::new() {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("Gamepad aus: {e}");
                None
            }
        };
        Self {
            gilrs,
            state: Pad::default(),
            edges: Pad::default(),
        }
    }
    /// Ereignisse abholen und den Zustand des ersten verbundenen Controllers lesen.
    pub fn poll(&mut self) {
        let Some(g) = self.gilrs.as_mut() else { return };
        while let Some(ev) = g.next_event() {
            if let gilrs::EventType::Connected = ev.event {
                eprintln!("Gamepad verbunden: {}", g.gamepad(ev.id).name());
            }
        }
        let before = self.state;
        let mut now = Pad::default();
        if let Some((_, gp)) = g.gamepads().find(|(_, gp)| gp.is_connected()) {
            use gilrs::{Axis, Button};
            let axis = |a| gp.axis_data(a).map(|d| d.value()).unwrap_or(0.);
            let btn = |b| gp.is_pressed(b);
            let trig = |b| {
                gp.button_data(b)
                    .map(|d| d.value())
                    .unwrap_or(if gp.is_pressed(b) { 1. } else { 0. })
            };
            now = Pad {
                connected: true,
                lx: axis(Axis::LeftStickX),
                ly: -axis(Axis::LeftStickY),
                rx: axis(Axis::RightStickX),
                ry: -axis(Axis::RightStickY),
                lt: trig(Button::LeftTrigger2),
                rt: trig(Button::RightTrigger2),
                a: btn(Button::South),
                b: btn(Button::East),
                x: btn(Button::West),
                y: btn(Button::North),
                lb: btn(Button::LeftTrigger),
                rb: btn(Button::RightTrigger),
                view: btn(Button::Select),
                menu: btn(Button::Start),
                up: btn(Button::DPadUp),
                down: btn(Button::DPadDown),
                left: btn(Button::DPadLeft),
                right: btn(Button::DPadRight),
            };
        }
        self.edges.latch_edges(&before, &now);
        self.state = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edges_are_latched_until_cleared() {
        let mut edges = Pad::default();
        let up = Pad::default();
        let down = Pad {
            a: true,
            y: true,
            ..Default::default()
        };
        edges.latch_edges(&up, &down);
        edges.latch_edges(&down, &down);
        assert!(
            edges.a && edges.y && !edges.b,
            "Flanke bleibt bis zum nächsten Schritt"
        );
        let mut e2 = Pad::default();
        e2.latch_edges(&down, &up);
        assert!(!e2.a, "Loslassen ist keine Flanke");
    }
}
