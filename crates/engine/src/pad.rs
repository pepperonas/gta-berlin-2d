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
    /// Sticks gedrückt (L3/R3)
    pub ls: bool,
    pub rs: bool,
}
impl Pad {
    /// Tasten, die in `now` gedrückt sind und in `before` nicht (Flanken), in `self` sammeln.
    pub fn latch_edges(&mut self, before: &Pad, now: &Pad) {
        macro_rules! edge {
            ($($f:ident),*) => { $( self.$f |= now.$f && !before.$f; )* };
        }
        edge!(
            a, b, x, y, lb, rb, view, menu, up, down, left, right, ls, rs
        );
        // Trigger: Flanke beim Überschreiten des halben Wegs (als 1 vermerkt)
        if now.lt > 0.5 && before.lt <= 0.5 {
            self.lt = 1.;
        }
        if now.rt > 0.5 && before.rt <= 0.5 {
            self.rt = 1.;
        }
    }
}

pub(crate) struct Gamepads {
    gilrs: Option<gilrs::Gilrs>,
    pub state: Pad,
    pub edges: Pad,
    /// laufende Vibration (muss leben, solange sie spielt)
    effect: Option<gilrs::ff::Effect>,
    /// Vibration ist mit diesem Controller nicht möglich (einmal gemeldet, dann still)
    ff_failed: bool,
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
            effect: None,
            ff_failed: false,
        }
    }
    /// Vibration: starker (tiefer) und schwacher (heller) Motor 0…1 für `ms` Millisekunden. Ersetzt eine laufende.
    /// Ohne Force-Feedback (z. B. unter macOS, die gilrs dort nicht anbietet) geschieht nichts.
    pub fn rumble(&mut self, strong: f32, weak: f32, ms: u32) {
        use gilrs::ff::{BaseEffect, BaseEffectType, EffectBuilder, Replay, Ticks};
        if self.ff_failed {
            return;
        }
        let Some(g) = self.gilrs.as_mut() else { return };
        let ids: Vec<_> = g
            .gamepads()
            .filter(|(_, gp)| gp.is_connected() && gp.is_ff_supported())
            .map(|(id, _)| id)
            .take(1)
            .collect();
        if ids.is_empty() {
            return;
        }
        let mag = |v: f32| (v.clamp(0., 1.) * u16::MAX as f32) as u16;
        let replay = Replay {
            play_for: Ticks::from_ms(ms),
            ..Default::default()
        };
        let built = EffectBuilder::new()
            .add_effect(BaseEffect {
                kind: BaseEffectType::Strong {
                    magnitude: mag(strong),
                },
                scheduling: replay,
                ..Default::default()
            })
            .add_effect(BaseEffect {
                kind: BaseEffectType::Weak {
                    magnitude: mag(weak),
                },
                scheduling: replay,
                ..Default::default()
            })
            .gamepads(&ids)
            .finish(g);
        match built.and_then(|e| e.play().map(|_| e)) {
            Ok(e) => self.effect = Some(e),
            Err(e) => {
                eprintln!("Vibration nicht verfügbar: {e}");
                self.ff_failed = true;
            }
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
                ls: btn(Button::LeftThumb),
                rs: btn(Button::RightThumb),
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
