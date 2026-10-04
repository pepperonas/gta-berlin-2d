//! Belegungstafel (Einstellungen → Steuerung → Enter/A): oben Vibration, Lenkempfindlichkeit und „alles zurück“,
//! darunter jede Aktion mit zwei Tastaturtasten und einer Controller-Taste. Bestätigen auf einer Zelle wartet auf die
//! neue Taste (Esc bricht ab, Rücktaste/Entf löscht, Controller-Zellen nehmen die nächste Controller-Taste und
//! geben nach 6 s auf). Doppelt belegte Aktionen stehen rot mit dem Partner daneben.
//! Die Bedienung ist ein reiner Zustandsautomat über `BindKeys` (testbar ohne Fenster).
use crate::bindings::{ACTIONS, Bindings, PadButton, bindable_key, info_of, key_name};
use berlin_engine::KeyCode;
use berlin_engine::Keys;
use berlin_engine::hud::{Align, Hud};
use glam::Vec2;

const SETTINGS: usize = 3;
pub const ROW_H: f32 = 21.;
const TOP: f32 = 168.;
const VISIBLE: usize = 23;
const CAPTURE_S: f64 = 6.;

/// Zeilen: 0 Vibration, 1 Lenkempfindlichkeit, 2 Standard, danach je eine Aktion.
pub fn rows() -> usize {
    SETTINGS + ACTIONS.len()
}

/// Eingaben eines Schritts für die Tafel.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BindKeys {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub confirm: bool,
    pub back: bool,
    /// Rücktaste/Entf (bzw. Controller X): Zelle leeren
    pub clear: bool,
    /// erste neu gedrückte belegbare Taste
    pub key: Option<KeyCode>,
    pub escape: bool,
    /// erste neu gedrückte Controller-Taste
    pub pad: Option<PadButton>,
    /// Mauszeiger (HUD) und Linksklick
    pub mouse: Option<Vec2>,
    pub click: bool,
}
impl BindKeys {
    pub fn from(keys: &Keys, stick_prev: (f32, f32)) -> Self {
        let p = |k: KeyCode| keys.pressed.contains(&k);
        let (pad, pe) = (&keys.pad, &keys.pad_pressed);
        let stick = |v: f32, prev: f32, s: f32| v * s > 0.6 && prev * s <= 0.6;
        Self {
            up: p(KeyCode::ArrowUp) || pe.up || stick(pad.ly, stick_prev.1, -1.),
            down: p(KeyCode::ArrowDown) || pe.down || stick(pad.ly, stick_prev.1, 1.),
            left: p(KeyCode::ArrowLeft) || pe.left || stick(pad.lx, stick_prev.0, -1.),
            right: p(KeyCode::ArrowRight) || pe.right || stick(pad.lx, stick_prev.0, 1.),
            confirm: p(KeyCode::Enter) || p(KeyCode::NumpadEnter) || pe.a,
            back: p(KeyCode::Escape) || pe.b,
            clear: p(KeyCode::Backspace) || p(KeyCode::Delete) || pe.x,
            key: keys
                .pressed
                .iter()
                .copied()
                .filter(|&k| bindable_key(k))
                .min_by_key(|k| *k as u32),
            escape: p(KeyCode::Escape),
            pad: Bindings::first_pad_press(pe),
            mouse: keys.mouse.hud,
            click: keys.mouse.left_pressed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Out {
    Stay,
    /// Belegung geändert (speichern)
    Changed,
    Back,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BindMenu {
    pub row: usize,
    /// 0, 1 = Tastatur, 2 = Controller
    pub col: usize,
    /// wartet auf eine Taste: seit wann
    pub capture: Option<f64>,
    pub scroll: usize,
}

/// Spaltenmitten (HUD-Einheiten) der drei Belegungsspalten bei Breite `w`.
fn col_x(w: f32, c: usize) -> f32 {
    w / 2. + 140. + c as f32 * 140.
}

impl BindMenu {
    fn action_row(&self) -> Option<usize> {
        self.row.checked_sub(SETTINGS)
    }
    fn keep_visible(&mut self) {
        if self.row < self.scroll {
            self.scroll = self.row;
        }
        if self.row >= self.scroll + VISIBLE {
            self.scroll = self.row + 1 - VISIBLE;
        }
    }
    /// Zeile und Spalte unter dem Mauszeiger.
    fn hit(&self, p: Vec2, w: f32) -> Option<(usize, usize)> {
        let r = ((p.y - (TOP - ROW_H * 0.75)) / ROW_H).floor();
        if r < 0. || r >= VISIBLE as f32 {
            return None;
        }
        let row = self.scroll + r as usize;
        if row >= rows() {
            return None;
        }
        let col = (0..3).find(|&c| (p.x - col_x(w, c)).abs() < 70.);
        Some((row, col.unwrap_or(self.col)))
    }

    pub fn step(&mut self, k: BindKeys, b: &mut Bindings, t: f64, w: f32) -> Out {
        if let Some(since) = self.capture {
            let a = ACTIONS[self.action_row().expect("Erfassung nur in Aktionszeilen")].action;
            if self.col < 2 {
                if k.escape || k.pad.is_some() {
                    self.capture = None;
                } else if k.clear
                    && k.key
                        .is_some_and(|c| matches!(c, KeyCode::Backspace | KeyCode::Delete))
                {
                    b.set_key(a, self.col, None);
                    self.capture = None;
                    return Out::Changed;
                } else if let Some(key) = k.key {
                    b.set_key(a, self.col, Some(key));
                    self.capture = None;
                    return Out::Changed;
                }
            } else if k.escape || t - since > CAPTURE_S {
                self.capture = None;
            } else if let Some(p) = k.pad {
                b.set_pad(a, Some(p));
                self.capture = None;
                return Out::Changed;
            }
            return Out::Stay;
        }
        if k.click
            && let Some(p) = k.mouse
            && let Some((row, col)) = self.hit(p, w)
        {
            self.row = row;
            self.col = col;
            return self.activate(b, t);
        }
        if k.back {
            return Out::Back;
        }
        if k.up {
            self.row = (self.row + rows() - 1) % rows();
        }
        if k.down {
            self.row = (self.row + 1) % rows();
        }
        self.keep_visible();
        match self.row {
            0 if k.left || k.right => {
                b.rumble = !b.rumble;
                return Out::Changed;
            }
            1 if k.left || k.right => {
                let d = if k.right { 0.1 } else { -0.1 };
                b.steer_sens = ((b.steer_sens + d) * 10.).round() / 10.;
                b.steer_sens = b.steer_sens.clamp(0.5, 1.5);
                return Out::Changed;
            }
            0..=2 => {}
            _ => {
                if k.left {
                    self.col = self.col.saturating_sub(1);
                }
                if k.right {
                    self.col = (self.col + 1).min(2);
                }
            }
        }
        if k.clear
            && let Some(i) = self.action_row()
        {
            let a = ACTIONS[i].action;
            if self.col < 2 {
                b.set_key(a, self.col, None);
            } else {
                b.set_pad(a, None);
            }
            return Out::Changed;
        }
        if k.confirm {
            return self.activate(b, t);
        }
        Out::Stay
    }
    fn activate(&mut self, b: &mut Bindings, t: f64) -> Out {
        match self.row {
            0 => {
                b.rumble = !b.rumble;
                Out::Changed
            }
            1 => Out::Stay,
            2 => {
                *b = Bindings::default();
                Out::Changed
            }
            _ => {
                let i = self.row - SETTINGS;
                if self.col == 2 && ACTIONS[i].pad_fixed.is_some() {
                    return Out::Stay;
                }
                self.capture = Some(t);
                Out::Stay
            }
        }
    }

    pub fn draw(&self, h: &mut Hud, b: &Bindings) {
        let w = h.width;
        h.rect(0., 0., w, 720., [0.02, 0.024, 0.04, 0.94], 0.);
        h.text(
            "BELEGUNG",
            w / 2.,
            64.,
            40.,
            crate::menu::YELLOW,
            Align::Center,
            true,
        );
        let hint = if self.capture.is_some() {
            if self.col < 2 {
                "Neue Taste drücken … · Rücktaste/Entf: leeren · Esc: abbrechen"
            } else {
                "Controller-Taste drücken … · Esc: abbrechen"
            }
        } else {
            "↑/↓ wählen · ←/→ Spalte bzw. Wert · Enter/A: belegen · Rücktaste/Entf/X: leeren · Esc/B: zurück"
        };
        h.text(
            hint,
            w / 2.,
            96.,
            13.,
            [0.8, 0.8, 0.8, 1.],
            Align::Center,
            true,
        );
        let x0 = w / 2. - 470.;
        let head = [0.62, 0.62, 0.62, 1.];
        h.text("Aktion", x0, 138., 14., head, Align::Left, true);
        for (c, t) in ["Taste 1", "Taste 2", "Controller"].iter().enumerate() {
            h.text(t, col_x(w, c), 138., 14., head, Align::Center, true);
        }
        let conflicts = b.conflicts();
        for vis in 0..VISIBLE {
            let row = self.scroll + vis;
            if row >= rows() {
                break;
            }
            let y = TOP + vis as f32 * ROW_H;
            let sel = row == self.row;
            if sel {
                h.rect(
                    x0 - 10.,
                    y - ROW_H * 0.72,
                    940. + 20.,
                    ROW_H - 1.,
                    [1., 1., 1., 0.07],
                    4.,
                );
            }
            let white = [0.92, 0.92, 0.92, 1.];
            let val = |h: &mut Hud, c: usize, text: &str, color: [f32; 4]| {
                if sel && self.row >= SETTINGS && self.col == c {
                    let cap = self.capture.is_some();
                    h.rect(
                        col_x(w, c) - 68.,
                        y - ROW_H * 0.72,
                        136.,
                        ROW_H - 1.,
                        if cap {
                            [1., 0.83, 0.24, 0.35]
                        } else {
                            [1., 0.83, 0.24, 0.16]
                        },
                        4.,
                    );
                }
                h.text(text, col_x(w, c), y, 13., color, Align::Center, true);
            };
            match row {
                0 => {
                    h.text(
                        "Vibration (Controller)",
                        x0,
                        y,
                        14.,
                        white,
                        Align::Left,
                        true,
                    );
                    let t = if b.rumble {
                        "‹ an ›"
                    } else {
                        "‹ aus ›"
                    };
                    h.text(t, col_x(w, 1), y, 14., white, Align::Center, true);
                }
                1 => {
                    h.text(
                        "Lenkempfindlichkeit (Stick)",
                        x0,
                        y,
                        14.,
                        white,
                        Align::Left,
                        true,
                    );
                    let t = format!("‹ {:.0} % ›", b.steer_sens * 100.);
                    h.text(&t, col_x(w, 1), y, 14., white, Align::Center, true);
                }
                2 => {
                    h.text("Alles auf Standard", x0, y, 14., white, Align::Left, true);
                    h.text(
                        "Enter/A",
                        col_x(w, 1),
                        y,
                        13.,
                        [0.7, 0.7, 0.7, 1.],
                        Align::Center,
                        true,
                    );
                }
                _ => {
                    let info = &ACTIONS[row - SETTINGS];
                    let clash = conflicts
                        .iter()
                        .find(|(x, y)| *x == info.action || *y == info.action)
                        .map(|&(x, y)| if x == info.action { y } else { x });
                    let color = if clash.is_some() {
                        [1., 0.45, 0.42, 1.]
                    } else {
                        white
                    };
                    h.text(info.label, x0, y, 13., color, Align::Left, true);
                    // Konflikt der gewählten Zeile unten ausgeschrieben (neben der Beschriftung wäre kein Platz)
                    if sel && let Some(o) = clash {
                        let t = format!("Doppelt belegt mit „{}“", info_of(o).label);
                        h.text(&t, w / 2., 678., 13., color, Align::Center, true);
                    }
                    let keys = b.keys_of(info.action);
                    let dim = [0.5, 0.5, 0.5, 1.];
                    for (c, k) in keys.iter().enumerate() {
                        let (t, col) = match k.and_then(key_name) {
                            Some(n) => (n, color),
                            None => ("–", dim),
                        };
                        val(h, c, t, col);
                    }
                    match info.pad_fixed {
                        Some(f) => val(h, 2, f, dim),
                        None => match b.pad_of(info.action) {
                            Some(p) => val(h, 2, p.name(), color),
                            None => val(h, 2, "–", dim),
                        },
                    }
                }
            }
        }
        if rows() > VISIBLE {
            let total = rows() as f32;
            let (y0, hgt) = (TOP - ROW_H * 0.7, VISIBLE as f32 * ROW_H);
            h.rect(x0 + 945., y0, 3., hgt, [1., 1., 1., 0.1], 1.);
            h.rect(
                x0 + 945.,
                y0 + hgt * self.scroll as f32 / total,
                3.,
                hgt * VISIBLE as f32 / total,
                [1., 1., 1., 0.45],
                1.,
            );
        }
        h.text(
            "Fest: Menüs (Pfeile, Enter, Esc, A/B), Waffen 1–6, Maus und Sticks",
            w / 2.,
            700.,
            12.,
            [0.55, 0.55, 0.55, 1.],
            Align::Center,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::Action;

    fn go(m: &mut BindMenu, b: &mut Bindings, k: BindKeys, t: f64) -> Out {
        m.step(k, b, t, 1280.)
    }
    fn row_of(a: Action) -> usize {
        SETTINGS + ACTIONS.iter().position(|i| i.action == a).unwrap()
    }

    #[test]
    fn capture_sets_clears_and_cancels() {
        let (mut m, mut b) = (BindMenu::default(), Bindings::default());
        m.row = row_of(Action::Horn);
        // Enter: wartet; die nächste Taste belegt Spalte 1
        assert_eq!(
            go(
                &mut m,
                &mut b,
                BindKeys {
                    confirm: true,
                    ..Default::default()
                },
                0.
            ),
            Out::Stay
        );
        assert!(m.capture.is_some());
        let out = go(
            &mut m,
            &mut b,
            BindKeys {
                key: Some(KeyCode::KeyJ),
                ..Default::default()
            },
            0.1,
        );
        assert_eq!(out, Out::Changed);
        assert_eq!(b.keys_of(Action::Horn)[0], Some(KeyCode::KeyJ));
        // Esc bricht ab, ändert nichts (und verlässt die Tafel nicht)
        go(
            &mut m,
            &mut b,
            BindKeys {
                confirm: true,
                ..Default::default()
            },
            1.,
        );
        let out = go(
            &mut m,
            &mut b,
            BindKeys {
                escape: true,
                back: true,
                key: Some(KeyCode::Escape),
                ..Default::default()
            },
            1.1,
        );
        assert_eq!(out, Out::Stay);
        assert!(m.capture.is_none() && b.keys_of(Action::Horn)[0] == Some(KeyCode::KeyJ));
        // Rücktaste leert die Zelle
        let out = go(
            &mut m,
            &mut b,
            BindKeys {
                clear: true,
                ..Default::default()
            },
            2.,
        );
        assert_eq!(out, Out::Changed);
        assert_eq!(b.keys_of(Action::Horn)[0], None);
        // Controller-Spalte: nächste Controller-Taste; Tastatur zählt dort nicht
        go(
            &mut m,
            &mut b,
            BindKeys {
                right: true,
                ..Default::default()
            },
            3.,
        );
        go(
            &mut m,
            &mut b,
            BindKeys {
                right: true,
                ..Default::default()
            },
            3.,
        );
        assert_eq!(m.col, 2);
        go(
            &mut m,
            &mut b,
            BindKeys {
                confirm: true,
                ..Default::default()
            },
            4.,
        );
        assert_eq!(
            go(
                &mut m,
                &mut b,
                BindKeys {
                    key: Some(KeyCode::KeyK),
                    ..Default::default()
                },
                4.1
            ),
            Out::Stay
        );
        assert_eq!(
            go(
                &mut m,
                &mut b,
                BindKeys {
                    pad: Some(PadButton::LS),
                    ..Default::default()
                },
                4.2
            ),
            Out::Changed
        );
        assert_eq!(b.pad_of(Action::Horn), Some(PadButton::LS));
        // Zeitlimit beim Warten auf den Controller
        go(
            &mut m,
            &mut b,
            BindKeys {
                confirm: true,
                ..Default::default()
            },
            10.,
        );
        go(&mut m, &mut b, BindKeys::default(), 10. + CAPTURE_S + 0.1);
        assert!(m.capture.is_none());
    }

    #[test]
    fn settings_rows_navigation_and_back() {
        let (mut m, mut b) = (BindMenu::default(), Bindings::default());
        assert_eq!(
            go(
                &mut m,
                &mut b,
                BindKeys {
                    right: true,
                    ..Default::default()
                },
                0.
            ),
            Out::Changed
        );
        assert!(!b.rumble, "Vibration umgeschaltet");
        m.row = 1;
        go(
            &mut m,
            &mut b,
            BindKeys {
                left: true,
                ..Default::default()
            },
            0.,
        );
        assert!((b.steer_sens - 0.9).abs() < 1e-6);
        for _ in 0..10 {
            go(
                &mut m,
                &mut b,
                BindKeys {
                    left: true,
                    ..Default::default()
                },
                0.,
            );
        }
        assert_eq!(b.steer_sens, 0.5, "geklemmt");
        // Standard setzt alles zurück
        b.set_key(Action::Horn, 0, None);
        m.row = 2;
        assert_eq!(
            go(
                &mut m,
                &mut b,
                BindKeys {
                    confirm: true,
                    ..Default::default()
                },
                0.
            ),
            Out::Changed
        );
        assert_eq!(b, Bindings::default());
        // Stick-Aktionen: Controller-Zelle nicht belegbar
        m.row = row_of(Action::Up);
        m.col = 2;
        go(
            &mut m,
            &mut b,
            BindKeys {
                confirm: true,
                ..Default::default()
            },
            0.,
        );
        assert!(m.capture.is_none());
        // nach oben über die erste Zeile läuft auf die letzte, und der Ausschnitt folgt
        m.row = 0;
        go(
            &mut m,
            &mut b,
            BindKeys {
                up: true,
                ..Default::default()
            },
            0.,
        );
        assert_eq!(m.row, rows() - 1);
        assert!(m.scroll + VISIBLE > m.row);
        assert_eq!(
            go(
                &mut m,
                &mut b,
                BindKeys {
                    back: true,
                    escape: true,
                    ..Default::default()
                },
                0.
            ),
            Out::Back
        );
    }

    #[test]
    fn table_draws_and_labels_end_before_the_first_column() {
        let mut h = Hud::new([1280., 720.]);
        let mut b = Bindings::default();
        b.set_key(Action::Horn, 0, Some(KeyCode::KeyF)); // Konflikt mit Einsteigen
        let mut m = BindMenu::default();
        m.draw(&mut h, &b);
        let n = h.items.len();
        m.row = rows() - 1;
        m.keep_visible();
        m.capture = Some(0.);
        m.draw(&mut h, &b);
        assert!(h.items.len() > n + 200, "beide Ausschnitte gezeichnet");
        let x0 = h.width / 2. - 470.;
        let widest = ACTIONS
            .iter()
            .map(|i| h.text_width(i.label, 13.))
            .fold(0., f32::max);
        assert!(
            x0 + widest < col_x(h.width, 0) - 70.,
            "Beschriftung {widest} läuft in die Tastenspalte"
        );
    }

    #[test]
    fn mouse_click_selects_the_cell_and_starts_capture() {
        let (mut m, mut b) = (BindMenu::default(), Bindings::default());
        let row = row_of(Action::Kick);
        let y = TOP + row as f32 * ROW_H - 3.;
        let p = Vec2::new(col_x(1280., 1), y);
        go(
            &mut m,
            &mut b,
            BindKeys {
                mouse: Some(p),
                click: true,
                ..Default::default()
            },
            0.,
        );
        assert_eq!((m.row, m.col), (row, 1));
        assert!(m.capture.is_some());
    }
}
