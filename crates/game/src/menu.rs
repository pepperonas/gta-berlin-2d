//! Menüs und Bildschirme (`game.js` + `hud.js drawTitle/drawPause/drawControls`): Titel mit Skyline,
//! Pausenmenü und Steuerungstafel. Auswahl mit ↑/↓ (W/S, Steuerkreuz, Stick), Enter/Leertaste/A bestätigt,
//! Esc/B geht zurück.
use berlin_engine::hud::{Align, Hud};
use berlin_engine::{KeyCode, Keys};

pub const YELLOW: [f32; 4] = [1., 0.827, 0.239, 1.];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Continue,
    New,
    Controls,
    Quit,
    Resume,
    Save,
    Restart,
    Title,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub action: Action,
    pub label: &'static str,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub items: Vec<Item>,
    pub index: usize,
}

/// Ergebnis einer Eingabe auf einem Menü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Move,
    Choose(Action),
    Back,
}

/// Tasten der Menüs aus Tastatur und Controller (Flanken).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MenuKeys {
    pub up: bool,
    pub down: bool,
    pub confirm: bool,
    pub back: bool,
}
impl MenuKeys {
    pub fn from(keys: &Keys, stick_prev: f32) -> Self {
        let p = |k: KeyCode| keys.pressed.contains(&k);
        let (pad, pe) = (&keys.pad, &keys.pad_pressed);
        // Stick als Taste: Flanke beim Überschreiten von 0,6
        let stick_up = pad.ly < -0.6 && stick_prev >= -0.6;
        let stick_down = pad.ly > 0.6 && stick_prev <= 0.6;
        Self {
            up: p(KeyCode::ArrowUp) || p(KeyCode::KeyW) || pe.up || stick_up,
            down: p(KeyCode::ArrowDown) || p(KeyCode::KeyS) || pe.down || stick_down,
            confirm: p(KeyCode::Enter)
                || p(KeyCode::NumpadEnter)
                || p(KeyCode::Space)
                || p(KeyCode::KeyE)
                || pe.a,
            back: p(KeyCode::Escape) || p(KeyCode::Backspace) || pe.b,
        }
    }
}

impl Menu {
    pub fn new(items: Vec<Item>) -> Self {
        let index = items.iter().position(|i| i.enabled).unwrap_or(0);
        Self { items, index }
    }
    /// game.js menuInput: Auswahl bewegen (deaktivierte Einträge überspringen), bestätigen oder zurück.
    pub fn input(&mut self, k: MenuKeys) -> Option<Pick> {
        let n = self.items.len();
        let mut step = |d: isize| {
            for s in 1..=n as isize {
                let i = (self.index as isize + d * s).rem_euclid(n as isize) as usize;
                if self.items[i].enabled {
                    self.index = i;
                    return true;
                }
            }
            false
        };
        if n > 0 && k.up && step(-1) {
            return Some(Pick::Move);
        }
        if n > 0 && k.down && step(1) {
            return Some(Pick::Move);
        }
        if k.confirm {
            return self
                .items
                .get(self.index)
                .filter(|i| i.enabled)
                .map(|i| Pick::Choose(i.action));
        }
        k.back.then_some(Pick::Back)
    }
}

const fn item(action: Action, label: &'static str) -> Item {
    Item {
        action,
        label,
        enabled: true,
    }
}

pub fn title_menu(has_save: bool) -> Menu {
    let mut m = Menu::new(vec![
        Item {
            enabled: has_save,
            ..item(Action::Continue, "Fortsetzen")
        },
        item(Action::New, "Neues Spiel"),
        item(Action::Controls, "Steuerung"),
        item(Action::Quit, "Beenden"),
    ]);
    m.index = if has_save { 0 } else { 1 };
    m
}

pub fn pause_menu() -> Menu {
    Menu::new(vec![
        item(Action::Resume, "Weiterspielen"),
        item(Action::Save, "Spiel speichern"),
        item(Action::Restart, "Mission neu starten"),
        item(Action::Controls, "Steuerung"),
        item(Action::Title, "Zum Hauptmenü"),
    ])
}

/// Menüeinträge untereinander, ab y (Mitte der ersten Zeile), zentriert bei cx.
pub fn draw_menu(h: &mut Hud, m: &Menu, cx: f32, y: f32) {
    let width = 380.;
    for (i, it) in m.items.iter().enumerate() {
        let yy = y + i as f32 * 58.;
        let sel = i == m.index;
        let bg = if sel {
            YELLOW
        } else {
            [0.06, 0.067, 0.094, 0.7]
        };
        h.rect(cx - width / 2., yy - 24., width, 48., bg, 10.);
        let color = if sel {
            [0.067, 0.067, 0.067, 1.]
        } else if it.enabled {
            [0.93, 0.93, 0.93, 1.]
        } else {
            [0.4, 0.4, 0.4, 1.]
        };
        h.text(it.label, cx, yy + 8., 24., color, Align::Center, !sel);
    }
}

fn footer(h: &mut Hud, text: &str) {
    h.text(
        text,
        h.width / 2.,
        720. - 36. - 10.,
        16.,
        [0.85, 0.85, 0.85, 1.],
        Align::Center,
        true,
    );
}

/// Silhouette der Stadt am unteren Rand (hud.js drawSkyline): Häuserreihe aus einem festen Zufall und der
/// Fernsehturm mit roter Spitze.
fn skyline(h: &mut Hud) {
    let (vw, vh) = (h.width, 720.);
    let base = vh - 40.;
    let c = [0.031, 0.031, 0.063, 0.85];
    h.rect(0., base, vw, 40., c, 0.);
    let mut seed: u32 = 7;
    let mut rnd = || {
        seed = (seed * 9301 + 49297) % 233280;
        seed as f32 / 233280.
    };
    let mut x = 0.;
    while x < vw {
        let (w, ht) = (30. + rnd() * 60., 40. + rnd() * 110.);
        h.rect(x, base - ht, w.min(vw - x), ht, c, 0.);
        x += w;
    }
    let tx = vw * 0.78;
    h.rect(tx - 5., base - 330., 10., 330., c, 0.);
    h.ellipse(tx, base - 250., 26., 26., c);
    h.rect(tx - 1.5, base - 400., 3., 80., c, 0.);
    h.ellipse(tx, base - 402., 3., 3., [1., 0.231, 0.188, 1.]);
}

pub fn draw_title(h: &mut Hud, m: &Menu, loading: bool) {
    let vw = h.width;
    // abgedunkelt, unten stärker (ein Verlauf aus Streifen zeigte Nähte zwischen den Kanten)
    h.rect(0., 0., vw, 720., [0.04, 0.03, 0.1, 0.4], 0.);
    h.rect(0., 430., vw, 290., [0.04, 0.03, 0.08, 0.35], 0.);
    skyline(h);
    h.text("GTA", vw / 2., 150., 64., [1.; 4], Align::Center, true);
    h.text("BERLIN", vw / 2., 232., 96., YELLOW, Align::Center, true);
    h.text(
        "Kisten für den Kiez",
        vw / 2.,
        272.,
        22.,
        [0.87, 0.87, 0.87, 1.],
        Align::Center,
        true,
    );
    if loading {
        h.text(
            "Lade Berlin …",
            vw / 2.,
            380.,
            22.,
            [0.87, 0.87, 0.87, 1.],
            Align::Center,
            true,
        );
    } else {
        draw_menu(h, m, vw / 2., 350.);
        footer(h, "Enter / A: Auswählen");
    }
    let grey = [0.6, 0.6, 0.6, 1.];
    h.text(
        &format!("v{} · native Fassung", env!("CARGO_PKG_VERSION")),
        vw - 36.,
        720. - 16.,
        14.,
        grey,
        Align::Right,
        true,
    );
    h.text(
        "Kartendaten © OpenStreetMap-Mitwirkende (ODbL)",
        36.,
        720. - 16.,
        12.,
        grey,
        Align::Left,
        true,
    );
}

pub fn draw_pause(h: &mut Hud, m: &Menu, completed: u32, best: Option<f64>) {
    let vw = h.width;
    h.rect(0., 0., vw, 720., [0., 0., 0., 0.6], 0.);
    h.text("PAUSE", vw / 2., 170., 56., YELLOW, Align::Center, true);
    let best = best.map_or("–".to_owned(), crate::hud::fmt_time);
    h.text(
        &format!("Aufträge erledigt: {completed}   ·   Bestzeit: {best}"),
        vw / 2.,
        210.,
        18.,
        [0.8, 0.8, 0.8, 1.],
        Align::Center,
        true,
    );
    draw_menu(h, m, vw / 2., 280.);
    footer(h, "Enter / A: Auswählen  ·  Esc / B: Weiter");
}

/// Belegung der nativen Fassung: Aufgabe, Controller, Tastatur.
pub const CONTROLS: &[(&str, &str, &str)] = &[
    ("Laufen / Lenken", "Linker Stick", "WASD / Pfeile"),
    ("Sprinten · langsam gehen", "A halten", "Umschalt · Alt"),
    ("Gas / Bremse · Rückwärts", "RT / LT", "W / S"),
    ("Handbremse", "RB oder B", "Leertaste"),
    ("ESP · ABS umschalten (im Auto)", "–", "X · Y/Z"),
    ("Einsteigen / Aussteigen", "Y", "F"),
    ("Aktion (Auftrag, Einladen)", "A", "E"),
    ("Hupe", "X", "H"),
    ("Stadtplan", "Ansicht-Taste", "Tab"),
    ("Uhr +1 Stunde · Wetter wechseln", "–", "T · N"),
    ("Ton umschalten · Speichern", "–", "M · F5"),
    ("Zoom", "–", "Mausrad"),
    ("Pause", "Menü-Taste", "Esc / P"),
    ("Menüs", "Steuerkreuz, A / B", "Pfeile, Enter / Esc"),
];

pub fn draw_controls(h: &mut Hud) {
    let vw = h.width;
    h.rect(0., 0., vw, 720., [0.02, 0.024, 0.04, 0.9], 0.);
    h.text("STEUERUNG", vw / 2., 110., 44., YELLOW, Align::Center, true);
    // Spaltenbreiten aus dem Text (die Bitmapschrift läuft breiter als die Browserschrift)
    let size = 16.;
    let col = |n: usize, h: &Hud| {
        CONTROLS
            .iter()
            .map(|r| h.text_width([r.0, r.1, r.2][n], size))
            .fold(0., f32::max)
            + 36.
    };
    let (wa, wb, wc) = (col(0, h), col(1, h), col(2, h));
    let total = wa + wb + wc;
    let x0 = vw / 2. - total / 2.;
    let grey = [0.67, 0.67, 0.67, 1.];
    h.text("Controller", x0 + wa, 170., size, grey, Align::Left, true);
    h.text(
        "Tastatur",
        x0 + wa + wb,
        170.,
        size,
        grey,
        Align::Left,
        true,
    );
    let row = 28.;
    for (i, (a, b, k)) in CONTROLS.iter().enumerate() {
        let y = 200. + i as f32 * row;
        if i % 2 == 0 {
            h.rect(
                x0 - 12.,
                y - row + 9.,
                total + 12.,
                row,
                [1., 1., 1., 0.05],
                0.,
            );
        }
        h.text(a, x0, y, size, [1.; 4], Align::Left, true);
        h.text(
            b,
            x0 + wa,
            y,
            size,
            [0.87, 0.87, 0.87, 1.],
            Align::Left,
            true,
        );
        h.text(
            k,
            x0 + wa + wb,
            y,
            size,
            [0.87, 0.87, 0.87, 1.],
            Align::Left,
            true,
        );
    }
    footer(h, "Esc / B: Zurück");
}

#[cfg(test)]
mod tests {
    use super::*;
    const DOWN: MenuKeys = MenuKeys {
        up: false,
        down: true,
        confirm: false,
        back: false,
    };
    #[test]
    fn menu_skips_disabled_and_wraps() {
        let mut m = title_menu(false);
        assert_eq!(
            m.items[m.index].action,
            Action::New,
            "ohne Spielstand steht Neues Spiel vorn"
        );
        let up = MenuKeys { up: true, ..DOWN };
        let up = MenuKeys { down: false, ..up };
        // nach oben: Fortsetzen ist aus → springt ans Ende (Beenden)
        assert_eq!(m.input(up), Some(Pick::Move));
        assert_eq!(m.items[m.index].action, Action::Quit);
        assert_eq!(m.input(DOWN), Some(Pick::Move));
        assert_eq!(m.items[m.index].action, Action::New);
        let ok = MenuKeys {
            confirm: true,
            down: false,
            ..DOWN
        };
        assert_eq!(m.input(ok), Some(Pick::Choose(Action::New)));
        let back = MenuKeys {
            back: true,
            down: false,
            ..DOWN
        };
        assert_eq!(m.input(back), Some(Pick::Back));
        assert_eq!(m.input(MenuKeys::default()), None);
        let m = title_menu(true);
        assert_eq!(m.items[m.index].action, Action::Continue);
    }
    #[test]
    fn screens_draw_within_the_frame() {
        let mut h = Hud::new([1280., 720.]);
        draw_title(&mut h, &title_menu(true), false);
        draw_pause(&mut h, &pause_menu(), 3, Some(95.));
        draw_controls(&mut h);
        assert!(h.items.len() > 500);
        // Steuerungstafel: jede Spalte endet vor der nächsten
        let size = 16.;
        let wa = CONTROLS
            .iter()
            .map(|r| h.text_width(r.0, size))
            .fold(0., f32::max);
        let wb = CONTROLS
            .iter()
            .map(|r| h.text_width(r.1, size))
            .fold(0., f32::max);
        let wc = CONTROLS
            .iter()
            .map(|r| h.text_width(r.2, size))
            .fold(0., f32::max);
        assert!(wa + wb + wc + 108. < h.width, "Tafel passt in 16:9");
        assert!(
            h.items
                .iter()
                .all(|i| i.center[1] >= -1. && i.center[1] <= 721.)
        );
    }
}
