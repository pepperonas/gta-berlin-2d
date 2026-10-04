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
    Stats,
    /// Ergebnis: weiter (Auftrag zurücksetzen, Spieler bleibt), erneut versuchen, frei weiterspielen
    Next,
    Retry,
    Free,
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
    /// Eintrag unter dem Punkt (HUD-Einheiten), so wie `draw_menu` ihn zeichnet.
    pub fn at(&self, cx: f32, y: f32, p: glam::Vec2) -> Option<usize> {
        if (p.x - cx).abs() > 190. {
            return None;
        }
        let rel = p.y - (y - 24.);
        let i = (rel / 58.).floor();
        (i >= 0. && rel - i * 58. <= 48. && (i as usize) < self.items.len()).then_some(i as usize)
    }
    /// Maus: Zeigen wählt aus, Klicken bestätigt (deaktivierte Einträge reagieren nicht).
    pub fn mouse(&mut self, cx: f32, y: f32, m: &berlin_engine::Mouse) -> Option<Pick> {
        let i = m.hud.and_then(|p| self.at(cx, y, p))?;
        if !self.items[i].enabled {
            return None;
        }
        if m.left_pressed {
            self.index = i;
            return Some(Pick::Choose(self.items[i].action));
        }
        if m.moved && i != self.index {
            self.index = i;
            return Some(Pick::Move);
        }
        None
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
        item(Action::Stats, "Statistik"),
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
        item(Action::Stats, "Statistik"),
        item(Action::Title, "Zum Hauptmenü"),
    ])
}

/// Nach dem Auftrag (game.js resultMenu).
pub fn result_menu(success: bool) -> Menu {
    Menu::new(if success {
        vec![item(Action::Next, "Weiter")]
    } else {
        vec![
            item(Action::Retry, "Erneut versuchen"),
            item(Action::Free, "Frei weiterspielen"),
        ]
    })
}

/// Statistik (hud.js drawStats): Abschnitte in zwei Spalten, je „dieses Spiel“ und „insgesamt“.
pub fn draw_stats(
    h: &mut Hud,
    game: &berlin_sim::stats::Stats,
    total: &berlin_sim::stats::Stats,
    in_game: bool,
) {
    use berlin_sim::stats::{SECTIONS, format_stat};
    let vw = h.width;
    h.rect(
        0.,
        0.,
        vw,
        720.,
        [0.02, 0.024, 0.04, if in_game { 0.82 } else { 0.94 }],
        0.,
    );
    h.text("STATISTIK", vw / 2., 70., 36., YELLOW, Align::Center, true);
    let cols: [&[usize]; 2] = [&[0, 2], &[1, 3]];
    let (col_w, gap, voff, size) = (600., 24., 150., 15.);
    let x0 = vw / 2. - (2. * col_w + gap) / 2.;
    let grey = [0.6, 0.6, 0.6, 1.];
    let mut rows = 0;
    for (ci, secs) in cols.iter().enumerate() {
        let x = x0 + ci as f32 * (col_w + gap);
        let mut y = 120.;
        h.text(
            "dieses Spiel",
            x + col_w - voff,
            y,
            13.,
            grey,
            Align::Right,
            true,
        );
        h.text(
            "insgesamt",
            x + col_w - 8.,
            y,
            13.,
            grey,
            Align::Right,
            true,
        );
        for &si in secs.iter() {
            let (title, list) = SECTIONS[si];
            y += 34.;
            h.text(&title.to_uppercase(), x, y, size, YELLOW, Align::Left, true);
            for (k, label, f) in list.iter() {
                y += 26.;
                if rows % 2 == 0 {
                    h.rect(x - 6., y - 19., col_w + 12., 26., [1., 1., 1., 0.04], 0.);
                }
                rows += 1;
                h.text(label, x, y, size, [1.; 4], Align::Left, false);
                h.text(
                    &format_stat(game.get(k), *f),
                    x + col_w - voff,
                    y,
                    size,
                    [1.; 4],
                    Align::Right,
                    false,
                );
                h.text(
                    &format_stat(total.get(k), *f),
                    x + col_w - 8.,
                    y,
                    size,
                    [0.8, 0.8, 0.8, 1.],
                    Align::Right,
                    false,
                );
            }
        }
    }
    footer(h, "Esc / B: Zurück");
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

/// Knopfflächen der Teleport-Rückfrage (x, y, Breite, Höhe): Ja, Nein.
pub fn teleport_buttons(vw: f32) -> ([f32; 4], [f32; 4]) {
    let (h, bw, bh) = (190., 200., 46.);
    let y = 360. - h / 2.;
    let by = y + h - bh - 22.;
    (
        [vw / 2. - bw - 12., by, bw, bh],
        [vw / 2. + 12., by, bw, bh],
    )
}

/// Teleport-Rückfrage (hud.js drawTeleportDialog); `name` = Zielort, `None` solange der Stadtteil lädt.
pub fn draw_teleport(h: &mut Hud, name: Option<&str>) {
    let vw = h.width;
    let (w, ht) = (560., 190.);
    let (x, y) = (vw / 2. - w / 2., 360. - ht / 2.);
    h.rect(0., 0., vw, 720., [0., 0., 0., 0.45], 0.);
    h.rect(x, y, w, ht, [0.06, 0.07, 0.09, 0.92], 12.);
    h.text(
        "HIERHIN TELEPORTIEREN?",
        vw / 2.,
        y + 44.,
        22.,
        YELLOW,
        Align::Center,
        true,
    );
    h.text(
        name.unwrap_or("Lade den Stadtteil …"),
        vw / 2.,
        y + 84.,
        18.,
        if name.is_some() {
            [1.; 4]
        } else {
            [0.73, 0.73, 0.73, 1.]
        },
        Align::Center,
        true,
    );
    let (yes, no) = teleport_buttons(vw);
    h.rect(yes[0], yes[1], yes[2], yes[3], YELLOW, 10.);
    h.text(
        "Ja (Enter/A)",
        yes[0] + yes[2] / 2.,
        yes[1] + 30.,
        18.,
        [0.07, 0.07, 0.07, 1.],
        Align::Center,
        false,
    );
    h.rect(no[0], no[1], no[2], no[3], [1., 1., 1., 0.12], 10.);
    h.text(
        "Nein (Esc/B)",
        no[0] + no[2] / 2.,
        no[1] + 30.,
        18.,
        [0.93, 0.93, 0.93, 1.],
        Align::Center,
        true,
    );
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
    ("Hupe (im Auto) · Nachladen (zu Fuß)", "X", "H · R"),
    ("Angreifen / Schießen · Treten", "RT · B", "Strg · V"),
    ("Waffe wechseln", "RB / LB", "Q · 1–6"),
    ("Zielen", "Rechter Stick", "Maus (Diablo: Strg)"),
    ("Stadtplan", "Ansicht-Taste", "Tab"),
    ("Uhr +1 Stunde · Wetter wechseln", "–", "T · N"),
    ("Ton umschalten · Speichern", "–", "M · F5"),
    ("Zoom", "–", "Mausrad"),
    ("Pause", "Menü-Taste", "Esc / P"),
    ("Menüs", "Steuerkreuz, A / B", "Pfeile, Enter / Esc"),
];

pub fn draw_controls(h: &mut Hud, diablo: bool) {
    let vw = h.width;
    h.rect(0., 0., vw, 720., [0.02, 0.024, 0.04, 0.9], 0.);
    h.text("STEUERUNG", vw / 2., 100., 44., YELLOW, Align::Center, true);
    let scheme = if diablo {
        "Zu Fuß am PC:  ‹ Diablo (Klick) ›   ← / → wechselt"
    } else {
        "Zu Fuß am PC:  ‹ Klassisch (WASD + Maus) ›   ← / → wechselt"
    };
    h.text(
        scheme,
        vw / 2.,
        136.,
        16.,
        [0.87, 0.87, 0.87, 1.],
        Align::Center,
        true,
    );
    let mouse_rows: [(&str, &str, &str); 2] = if diablo {
        [
            (
                "Maus links: laufen · Person: angreifen · Auto: einsteigen",
                "",
                "",
            ),
            ("Strg + Klick: am Platz angreifen · rechts: treten", "", ""),
        ]
    } else {
        [
            (
                "Maus zielt · links: angreifen/schießen · rechts: treten",
                "",
                "",
            ),
            ("", "", ""),
        ]
    };
    for (i, (t, _, _)) in mouse_rows.iter().enumerate() {
        h.text(
            t,
            vw / 2.,
            160. + i as f32 * 20.,
            14.,
            [0.75, 0.75, 0.75, 1.],
            Align::Center,
            true,
        );
    }
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
    h.text("Controller", x0 + wa, 214., size, grey, Align::Left, true);
    h.text(
        "Tastatur",
        x0 + wa + wb,
        214.,
        size,
        grey,
        Align::Left,
        true,
    );
    let row = 25.;
    for (i, (a, b, k)) in CONTROLS.iter().enumerate() {
        let y = 240. + i as f32 * row;
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
    fn mouse_points_and_clicks_entries() {
        use glam::Vec2;
        let mut m = pause_menu();
        // Einträge bei y = 280 + i·58, je 48 hoch, 380 breit um cx
        assert_eq!(m.at(640., 280., Vec2::new(640., 280.)), Some(0));
        assert_eq!(m.at(640., 280., Vec2::new(500., 280. + 58. * 2.)), Some(2));
        assert_eq!(
            m.at(640., 280., Vec2::new(640., 280. + 30.)),
            None,
            "Lücke zwischen zwei Einträgen"
        );
        assert_eq!(m.at(640., 280., Vec2::new(900., 280.)), None);
        let mut mouse = berlin_engine::Mouse {
            hud: Some(Vec2::new(640., 280. + 58.)),
            moved: true,
            ..Default::default()
        };
        assert_eq!(m.mouse(640., 280., &mouse), Some(Pick::Move));
        assert_eq!(m.index, 1);
        mouse.moved = false;
        mouse.left_pressed = true;
        assert_eq!(
            m.mouse(640., 280., &mouse),
            Some(Pick::Choose(Action::Save))
        );
        // deaktivierte Einträge reagieren nicht
        let mut t = title_menu(false);
        mouse.hud = Some(Vec2::new(640., 350.));
        assert_eq!(t.mouse(640., 350., &mouse), None);
    }
    #[test]
    fn screens_draw_within_the_frame() {
        let mut h = Hud::new([1280., 720.]);
        draw_title(&mut h, &title_menu(true), false);
        draw_pause(&mut h, &pause_menu(), 3, Some(95.));
        draw_controls(&mut h, true);
        draw_stats(&mut h, &Default::default(), &Default::default(), true);
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
        // Statistik: längste Beschriftung endet vor der Spalte „dieses Spiel“ (600 − 150 − Zahlbreite)
        let longest = berlin_sim::stats::SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter())
            .map(|r| h.text_width(r.1, 15.))
            .fold(0., f32::max);
        assert!(
            longest + h.text_width("888 km/h", 15.) < 600. - 150. + 4.,
            "{longest}"
        );
        assert!(2. * 600. + 24. < h.width, "zwei Spalten passen in 16:9");
        assert!(
            h.items
                .iter()
                .all(|i| i.center[1] >= -1. && i.center[1] <= 721.)
        );
    }
}
