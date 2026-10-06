//! Bildschirmtastatur für die Befehlszeile am Controller (geöffnet mit LB + RB).
//!
//! Steuerkreuz bzw. linker Stick bewegen die Auswahl, A tippt das Zeichen, X löscht, Y setzt ein Leerzeichen,
//! Start führt aus, B schließt. LT/RT gehen durch die Vorschläge, RB übernimmt den gezeigten Vorschlag.
use crate::console::Key;
use berlin_engine::hud::{Align, Hud};
use berlin_engine::pad::Pad;

/// Zeichenreihen (Befehle sind klein geschrieben; Ziffern, Doppelpunkt für Uhrzeiten).
pub const ROWS: [&str; 4] = ["1234567890", "qwertzuiop", "asdfghjkl:", "yxcvbnm.,-"];
/// Untere Reihe: Leerzeichen (Spalten 0–5), Löschen (6–7), Ausführen (8–9).
pub const COLS: usize = 10;

/// Taste an einer Stelle des Rasters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Char(char),
    Space,
    Back,
    Enter,
}
pub fn cell(row: usize, col: usize) -> Cell {
    match ROWS.get(row) {
        Some(r) => Cell::Char(r.chars().nth(col.min(COLS - 1)).unwrap_or(' ')),
        None => match col {
            0..=5 => Cell::Space,
            6 | 7 => Cell::Back,
            _ => Cell::Enter,
        },
    }
}

/// Stick als Taste: Flanke beim Überschreiten von 0,6.
fn stick_edge(v: f32, prev: f32, sign: f32) -> bool {
    v * sign > 0.6 && prev * sign <= 0.6
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PadKbd {
    /// sichtbar und aktiv
    pub on: bool,
    /// Controller, der sie bedient (0 = erster, 1 = zweiter)
    pub slot: u8,
    pub row: usize,
    pub col: usize,
    prev: (f32, f32),
}

impl PadKbd {
    pub fn new(slot: u8) -> Self {
        Self {
            on: true,
            slot,
            row: 1,
            col: 0,
            prev: (0., 0.),
        }
    }
    /// Ein Schritt: Bewegung und Tasten → Eingaben für die Befehlszeile.
    pub fn step(&mut self, pad: &Pad, e: &Pad) -> Vec<Key> {
        let mut out = Vec::new();
        if !self.on {
            return out;
        }
        let (px, py) = self.prev;
        self.prev = (pad.lx, pad.ly);
        let rows = ROWS.len() + 1;
        if e.up || stick_edge(pad.ly, py, -1.) {
            self.row = (self.row + rows - 1) % rows;
        }
        if e.down || stick_edge(pad.ly, py, 1.) {
            self.row = (self.row + 1) % rows;
        }
        if e.left || stick_edge(pad.lx, px, -1.) {
            self.col = (self.col + COLS - 1) % COLS;
        }
        if e.right || stick_edge(pad.lx, px, 1.) {
            self.col = (self.col + 1) % COLS;
        }
        if e.a {
            out.push(match cell(self.row, self.col) {
                Cell::Char(c) => Key::Char(c),
                Cell::Space => Key::Char(' '),
                Cell::Back => Key::Backspace,
                Cell::Enter => Key::Enter,
            });
        }
        if e.x {
            out.push(Key::Backspace);
        }
        if e.y {
            out.push(Key::Char(' '));
        }
        if e.lt > 0.5 {
            out.push(Key::Up);
        }
        if e.rt > 0.5 {
            out.push(Key::Down);
        }
        if e.rb {
            out.push(Key::Tab);
        }
        if e.menu {
            out.push(Key::Enter);
        }
        if e.b {
            out.push(Key::Escape);
        }
        out
    }
}

/// Tastatur in der oberen Bildhälfte (über der Befehlszeile, die unten liegt).
pub fn draw(h: &mut Hud, k: &PadKbd) {
    if !k.on {
        return;
    }
    let (kw, kh, gap) = (44., 40., 6.);
    let w = COLS as f32 * (kw + gap) - gap;
    let (x0, y0) = (h.width / 2. - w / 2., 150.);
    let rows = ROWS.len() + 1;
    h.rect(
        x0 - 16.,
        y0 - 16.,
        w + 32.,
        rows as f32 * (kh + gap) - gap + 84.,
        [0.05, 0.06, 0.08, 0.9],
        10.,
    );
    let yellow = [1., 0.83, 0.24, 1.];
    for r in 0..rows {
        let y = y0 + r as f32 * (kh + gap);
        let mut c = 0;
        while c < COLS {
            let cl = cell(r, c);
            // breite Tasten der unteren Reihe
            let span = match cl {
                Cell::Space => 6,
                Cell::Back | Cell::Enter => 2,
                Cell::Char(_) => 1,
            };
            let x = x0 + c as f32 * (kw + gap);
            let bw = span as f32 * (kw + gap) - gap;
            let on = k.row == r && (c..c + span).contains(&k.col);
            h.rect(
                x,
                y,
                bw,
                kh,
                if on { yellow } else { [0.13, 0.14, 0.18, 1.] },
                6.,
            );
            let label = match cl {
                Cell::Char(ch) => ch.to_string(),
                Cell::Space => "Leerzeichen".into(),
                Cell::Back => "Löschen".into(),
                Cell::Enter => "OK".into(),
            };
            h.text(
                &label,
                x + bw / 2.,
                y + 27.,
                18.,
                if on {
                    [0.07, 0.07, 0.07, 1.]
                } else {
                    [0.93, 0.93, 0.95, 1.]
                },
                Align::Center,
                false,
            );
            c += span;
        }
    }
    let hy = y0 + rows as f32 * (kh + gap) + 16.;
    for (i, line) in [
        "A tippen · X löschen · Y Leerzeichen · Start ausführen",
        "LT/RT Vorschläge · RB übernehmen · B schließen",
    ]
    .iter()
    .enumerate()
    {
        h.text(
            line,
            h.width / 2.,
            hy + i as f32 * 18.,
            13.,
            [0.8, 0.8, 0.82, 1.],
            Align::Center,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(f: impl Fn(&mut Pad)) -> Pad {
        let mut p = Pad::default();
        f(&mut p);
        p
    }

    #[test]
    fn types_commands_with_the_pad() {
        let mut k = PadKbd::new(0);
        let idle = Pad::default();
        // „q“ steht in Reihe 1, Spalte 0
        assert_eq!(k.step(&idle, &edge(|p| p.a = true)), vec![Key::Char('q')]);
        // rechts, rechts → „e“
        k.step(&idle, &edge(|p| p.right = true));
        k.step(&idle, &edge(|p| p.right = true));
        assert_eq!(k.step(&idle, &edge(|p| p.a = true)), vec![Key::Char('e')]);
        // Stick hoch (Flanke) → Ziffernreihe, Spalte 2 = „3“
        let up = Pad {
            ly: -1.,
            ..Default::default()
        };
        k.step(&up, &idle);
        assert_eq!(k.step(&up, &edge(|p| p.a = true)), vec![Key::Char('3')]);
        // ganz nach unten: Sonderreihe, Spalte 2 = Leerzeichen; Spalte 9 = Ausführen
        k.row = ROWS.len();
        assert_eq!(k.step(&idle, &edge(|p| p.a = true)), vec![Key::Char(' ')]);
        k.col = 9;
        assert_eq!(k.step(&idle, &edge(|p| p.a = true)), vec![Key::Enter]);
        // Kurzwege
        assert_eq!(k.step(&idle, &edge(|p| p.x = true)), vec![Key::Backspace]);
        assert_eq!(k.step(&idle, &edge(|p| p.menu = true)), vec![Key::Enter]);
        assert_eq!(k.step(&idle, &edge(|p| p.b = true)), vec![Key::Escape]);
        assert_eq!(k.step(&idle, &edge(|p| p.rb = true)), vec![Key::Tab]);
        // Ränder: links von Spalte 0 geht es zur letzten
        k.col = 0;
        k.step(&idle, &edge(|p| p.left = true));
        assert_eq!(k.col, COLS - 1);
    }

    #[test]
    fn every_cell_is_defined() {
        for r in 0..=ROWS.len() {
            for c in 0..COLS {
                if let Cell::Char(ch) = cell(r, c) {
                    assert_ne!(ch, ' ');
                }
            }
        }
        assert_eq!(cell(ROWS.len(), 0), Cell::Space);
        assert_eq!(cell(ROWS.len(), 7), Cell::Back);
        assert_eq!(cell(ROWS.len(), 9), Cell::Enter);
    }
}
