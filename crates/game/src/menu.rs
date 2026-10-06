//! Menüs und Bildschirme (`game.js` + `hud.js drawTitle/drawPause/drawControls`): Titel mit Skyline,
//! Pausenmenü und Steuerungstafel. Auswahl mit ↑/↓ (W/S, Steuerkreuz, Stick), Enter/Leertaste/A bestätigt,
//! Esc/B geht zurück.
use berlin_engine::hud::{Align, Hud};
use berlin_engine::{KeyCode, Keys};

pub const YELLOW: [f32; 4] = [1., 0.827, 0.239, 1.];
/// Mitte des ersten Eintrags im Titelmenü.
pub const TITLE_MENU_Y: f32 = 316.;

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
    /// Seite „Über das Spiel“
    About,
    /// Ergebnis: weiter (Auftrag zurücksetzen, Spieler bleibt), erneut versuchen, frei weiterspielen
    Next,
    Retry,
    Free,
    /// Grafik HD / Pixel umschalten (Beschriftung zeigt den aktuellen Modus)
    Graphics,
    /// Spieler 2 beitreten lassen bzw. verabschieden (Beschriftung zeigt, was geschieht)
    Coop,
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
    /// Zeilenabstand und Knopfhöhe: ab sieben Einträgen enger, damit das Menü über der Fußzeile endet.
    pub fn spacing(&self) -> (f32, f32) {
        if self.items.len() >= 9 {
            (44., 38.)
        } else if self.items.len() >= 7 {
            (50., 42.)
        } else {
            (58., 48.)
        }
    }
    /// Eintrag unter dem Punkt (HUD-Einheiten), so wie `draw_menu` ihn zeichnet.
    pub fn at(&self, cx: f32, y: f32, p: glam::Vec2) -> Option<usize> {
        if (p.x - cx).abs() > 190. {
            return None;
        }
        let (step, height) = self.spacing();
        let rel = p.y - (y - height / 2.);
        let i = (rel / step).floor();
        (i >= 0. && rel - i * step <= height && (i as usize) < self.items.len())
            .then_some(i as usize)
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

const GRAPHICS_HD: &str = "Grafik: HD";
const COOP_JOIN: &str = "Spieler 2 beitreten";
const COOP_LEAVE: &str = "Spieler 2 verlassen";
const GRAPHICS_PIXEL: &str = "Grafik: Pixel";

impl Menu {
    /// Beschriftung des Grafik-Eintrags an den aktuellen Modus anpassen.
    pub fn with_graphics(mut self, mode: berlin_engine::graphics::GraphicsMode) -> Self {
        self.set_graphics(mode);
        self
    }
    /// Beschriftung des Koop-Eintrags: beitreten oder verlassen.
    pub fn with_coop(mut self, coop: bool) -> Self {
        for it in &mut self.items {
            if it.action == Action::Coop {
                it.label = if coop { COOP_LEAVE } else { COOP_JOIN };
            }
        }
        self
    }
    pub fn set_graphics(&mut self, mode: berlin_engine::graphics::GraphicsMode) {
        use berlin_engine::graphics::GraphicsMode;
        for it in &mut self.items {
            if it.action == Action::Graphics {
                it.label = match mode {
                    GraphicsMode::Hd => GRAPHICS_HD,
                    GraphicsMode::Pixel => GRAPHICS_PIXEL,
                };
            }
        }
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
        item(Action::Graphics, GRAPHICS_HD),
        item(Action::Stats, "Statistik"),
        item(Action::About, "Über das Spiel"),
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
        item(Action::Coop, COOP_JOIN),
        item(Action::Controls, "Steuerung"),
        item(Action::Graphics, GRAPHICS_HD),
        item(Action::Stats, "Statistik"),
        item(Action::About, "Über das Spiel"),
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
    h.text("STATISTIK", vw / 2., 58., 36., YELLOW, Align::Center, true);
    // Spalten wie hud.js drawStats: Unterwegs+Verkehr | Kampf+Aufträge | Nahverkehr
    let sec = |name: &str| SECTIONS.iter().position(|(n, _)| *n == name).unwrap_or(0);
    let cols: [Vec<usize>; 3] = [
        vec![sec("Unterwegs"), sec("Verkehr")],
        vec![sec("Kampf"), sec("Aufträge")],
        vec![sec("Nahverkehr")],
    ];
    let (col_w, gap, voff, size) = (410., 16., 118., 11.);
    let x0 = vw / 2. - (3. * col_w + 2. * gap) / 2.;
    let grey = [0.6, 0.6, 0.6, 1.];
    let mut rows = 0;
    let mut bottom = 0f32;
    for (ci, secs) in cols.iter().enumerate() {
        let x = x0 + ci as f32 * (col_w + gap);
        let mut y = 96.;
        h.text("Spiel", x + col_w - voff, y, 11., grey, Align::Right, true);
        h.text("gesamt", x + col_w - 8., y, 11., grey, Align::Right, true);
        for &si in secs.iter() {
            let (title, list) = SECTIONS[si];
            y += 26.;
            h.text(
                &title.to_uppercase(),
                x,
                y,
                size + 1.,
                YELLOW,
                Align::Left,
                true,
            );
            for (k, label, f) in list.iter() {
                y += 20.;
                if rows % 2 == 0 {
                    h.rect(x - 6., y - 14., col_w + 12., 20., [1., 1., 1., 0.04], 0.);
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
        bottom = bottom.max(y);
    }
    // Waffen: direkt unter den Spalten
    use berlin_sim::stats::{Fmt, accuracy, weapon_key};
    let mut y = bottom + 34.;
    let wx = x0;
    h.text("WAFFEN", wx, y, size + 1., YELLOW, Align::Left, true);
    let heads = ["Schüsse/Schläge", "Kugeln", "Treffer", "Quote", "Tote"];
    for (i, t) in heads.iter().enumerate() {
        h.text(
            t,
            wx + 420. + i as f32 * 190.,
            y,
            10.,
            grey,
            Align::Right,
            true,
        );
    }
    let note_y = y + 19. * (berlin_sim::combat::WEAPONS.len() as f32 + 1.);
    h.text(
        "je Zelle: dieses Spiel / insgesamt",
        wx,
        note_y,
        10.,
        [0.47, 0.47, 0.47, 1.],
        Align::Left,
        false,
    );
    for wp in berlin_sim::combat::WEAPONS.iter() {
        y += 19.;
        h.text(wp.name, wx, y, size, [1.; 4], Align::Left, false);
        let n = |s: &berlin_sim::stats::Stats, f: &str| {
            format_stat(s.get(&weapon_key(wp.id, f)), Fmt::N)
        };
        let cells = [
            (n(game, "shots"), n(total, "shots")),
            (n(game, "bullets"), n(total, "bullets")),
            (n(game, "hits"), n(total, "hits")),
            (
                format!("{} %", accuracy(game, wp.id)),
                format!("{} %", accuracy(total, wp.id)),
            ),
            (n(game, "kills"), n(total, "kills")),
        ];
        for (i, (a, b)) in cells.iter().enumerate() {
            h.text(
                &format!("{a} / {b}"),
                wx + 420. + i as f32 * 190.,
                y,
                size,
                [1.; 4],
                Align::Right,
                false,
            );
        }
    }
    footer(h, "Esc / B: Zurück");
}

/// Menüeinträge untereinander, ab y (Mitte der ersten Zeile), zentriert bei cx.
pub fn draw_menu(h: &mut Hud, m: &Menu, cx: f32, y: f32) {
    let width = 380.;
    let (step, height) = m.spacing();
    for (i, it) in m.items.iter().enumerate() {
        let yy = y + i as f32 * step;
        let sel = i == m.index;
        let bg = if sel {
            YELLOW
        } else {
            [0.06, 0.067, 0.094, 0.7]
        };
        h.rect(cx - width / 2., yy - height / 2., width, height, bg, 10.);
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

/// Entwickler-Zeile unten links auf dem Titelbild; der GitHub-Teil ist ein Link.
pub const CREDIT: &str = "Entwickelt von Martin Pfeffer · celox.io · ";
pub const CREDIT_LINK: &str = "github.com/pepperonas";
/// Widmung hinter dem Auftritt (das Herz zeichnet `heart`)
pub const DEDICATION: &str = "  |  inspired by Anna";
pub const CREDIT_URL: &str = "https://github.com/pepperonas";
const CREDIT_SIZE: f32 = 12.;
const CREDIT_Y: f32 = 720. - 16.;

/// Klickfläche des GitHub-Links (x, y, Breite, Höhe in Basiseinheiten).
pub fn credit_link_rect(h: &Hud) -> [f32; 4] {
    let x = 36. + h.text_width(CREDIT, CREDIT_SIZE);
    let w = h.text_width(CREDIT_LINK, CREDIT_SIZE);
    [
        x - 4.,
        CREDIT_Y - CREDIT_SIZE - 4.,
        w + 8.,
        CREDIT_SIZE + 10.,
    ]
}

/// Liegt der Mauszeiger (HUD-Koordinaten) auf dem Link?
pub fn in_rect(r: [f32; 4], p: glam::Vec2) -> bool {
    p.x >= r[0] && p.x <= r[0] + r[2] && p.y >= r[1] && p.y <= r[1] + r[3]
}

/// Öffnet eine Adresse im Standardbrowser (ohne zusätzliche Abhängigkeit).
pub fn open_url(url: &str) {
    if cfg!(test) {
        return;
    }
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    let _ = cmd.arg(url).spawn();
}

pub fn draw_title(h: &mut Hud, m: &Menu, loading: bool, link_hover: bool) -> [f32; 4] {
    let vw = h.width;
    // abgedunkelt, unten stärker (ein Verlauf aus Streifen zeigte Nähte zwischen den Kanten)
    h.rect(0., 0., vw, 720., [0.04, 0.03, 0.1, 0.4], 0.);
    h.rect(0., 430., vw, 290., [0.04, 0.03, 0.08, 0.35], 0.);
    skyline(h);
    // Logo: Anton mit Verlauf, Kontur, 3D-Extrusion und Schatten (HD), Bitmap mit Extrusion (Pixel)
    h.logo(
        "GTA",
        vw / 2. - 4.,
        124.,
        50.,
        [1.; 4],
        [0.13, 0.16, 0.27, 1.],
    );
    h.logo(
        "BERLIN",
        vw / 2.,
        226.,
        86.,
        [1., 0.82, 0.18, 1.],
        [0.48, 0.12, 0.06, 1.],
    );
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
        draw_menu(h, m, vw / 2., TITLE_MENU_Y);
        footer(h, "Enter / A: Auswählen");
    }
    let grey = [0.6, 0.6, 0.6, 1.];
    h.text(
        &format!(
            "v{} · Rust {}",
            crate::about::game_version(),
            crate::about::native_version()
        ),
        vw - 36.,
        720. - 16.,
        14.,
        grey,
        Align::Right,
        true,
    );
    let w = h.text(CREDIT, 36., CREDIT_Y, CREDIT_SIZE, grey, Align::Left, true);
    let link = if link_hover {
        YELLOW
    } else {
        [0.62, 0.74, 1., 1.]
    };
    let lw = h.text(
        CREDIT_LINK,
        36. + w,
        CREDIT_Y,
        CREDIT_SIZE,
        link,
        Align::Left,
        true,
    );
    h.rect(36. + w, CREDIT_Y + 2., lw, 1., link, 0.);
    // Widmung hinter dem Auftritt, mit Trennstrich und Herz
    let x = 36. + w + lw;
    let dw = h.text(
        DEDICATION,
        x,
        CREDIT_Y,
        CREDIT_SIZE,
        grey,
        Align::Left,
        true,
    );
    heart(h, x + dw + 9., CREDIT_Y - 4.5, 6.);
    credit_link_rect(h)
}

/// Rosa Herz (💗): auf die Spitze gestelltes Quadrat mit zwei Kreisen auf den oberen Kanten – dunkler Rand, rosa
/// Fläche, hellerer Kern, Glanzpunkt. `r` = halbe Breite (Basiseinheiten).
fn heart(h: &mut Hud, cx: f32, cy: f32, r: f32) {
    let s = h.scale;
    for (k, c) in [
        (1.0, [0.06, 0.02, 0.05, 0.9]),
        (0.8, [1., 0.33, 0.6, 1.]),
        (0.42, [1., 0.66, 0.82, 1.]),
    ] {
        let a = r * k / 0.854;
        let y = cy + 0.07 * r / 0.854 + (1. - k) * r * 0.12;
        let m = a / (2. * std::f32::consts::SQRT_2);
        h.items.push(berlin_engine::hud::HudItem {
            center: [cx * s, y * s],
            half: [a / 2. * s, a / 2. * s],
            angle: std::f32::consts::FRAC_PI_4,
            shape: 0.,
            color: c,
            extra: [0.; 4],
        });
        h.ellipse(cx - m, y - m, a / 2., a / 2., c);
        h.ellipse(cx + m, y - m, a / 2., a / 2., c);
    }
    h.ellipse(
        cx - r * 0.42,
        cy - r * 0.38,
        r * 0.15,
        r * 0.15,
        [1., 1., 1., 0.9],
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

/// Übersicht der Steuerung aus der aktuellen Belegung: Aufgabe, Controller, Tastatur (je Aktion die erste Taste).
pub fn control_rows(b: &crate::bindings::Bindings) -> Vec<(String, String, String)> {
    use crate::bindings::Action as A;
    let key = |a: A| {
        b.keys_of(a)
            .iter()
            .flatten()
            .find_map(|k| crate::bindings::key_name(*k))
            .unwrap_or("–")
    };
    let pad = |a: A| b.pad_of(a).map_or("–", |p| p.name());
    let join = |xs: &[&str]| xs.join(" · ");
    let both = |label: &str, acts: &[A]| {
        (
            label.to_string(),
            join(&acts.iter().map(|&a| pad(a)).collect::<Vec<_>>()),
            join(&acts.iter().map(|&a| key(a)).collect::<Vec<_>>()),
        )
    };
    vec![
        (
            "Laufen / Lenken".into(),
            "Linker Stick".into(),
            [A::Up, A::Left, A::Down, A::Right].map(key).join(" "),
        ),
        both("Sprinten · langsam/ruhig zielen", &[A::Sprint, A::Slow]),
        both("Gas · Bremse/rückwärts", &[A::Throttle, A::Brake]),
        both("Handbremse · Hupe", &[A::Handbrake, A::Horn]),
        both("ESP · ABS (im Auto)", &[A::Esp, A::Abs]),
        both("Ein-/Aussteigen · Aktion", &[A::EnterExit, A::Use]),
        both("Mitfahren (Bus, Bahn)", &[A::Ride]),
        both(
            "Angreifen · Treten · Nachladen",
            &[A::Fire, A::Kick, A::Reload],
        ),
        {
            let (l, p, k) = both(
                "Waffe: nächste · Waffenrad",
                &[A::NextWeapon, A::WeaponWheel],
            );
            (l, p, format!("{k} · 1–6"))
        },
        ("Zielen".into(), "Rechter Stick".into(), "Maus".into()),
        both("Kamera näher · weiter", &[A::ZoomIn, A::ZoomOut]),
        both(
            "Stadtplan · Pause · Befehlszeile",
            &[A::Map, A::Pause, A::Console],
        ),
        both(
            "Uhr · Wetter · Ton · Speichern",
            &[A::Clock, A::Weather, A::Mute, A::Save],
        ),
        (
            "Menüs".into(),
            "Steuerkreuz, A / B".into(),
            "Pfeile, Enter / Esc".into(),
        ),
    ]
}

pub fn draw_controls(h: &mut Hud, diablo: bool, b: &crate::bindings::Bindings) {
    let controls = control_rows(b);
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
                "Maus links: laufen · Auto: einsteigen (links schießt nie)",
                "",
                "",
            ),
            (
                "rechts: schießen/schlagen zum Zeiger · beide Tasten: Waffenrad",
                "",
                "",
            ),
        ]
    } else {
        [
            (
                "Maus zielt · rechts: schießen/schlagen (links schießt nie)",
                "",
                "",
            ),
            ("beide Maustasten: Waffenrad", "", ""),
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
        controls
            .iter()
            .map(|r| h.text_width([&r.0, &r.1, &r.2][n], size))
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
    for (i, (a, b, k)) in controls.iter().enumerate() {
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
    footer(h, "Enter / A: Belegung ändern · Esc / B: Zurück");
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
        // neun Einträge: eng gesetzt (y = 280 + i·44, je 38 hoch), 380 breit um cx
        assert_eq!(m.spacing(), (44., 38.));
        assert_eq!(
            result_menu(false).spacing(),
            (58., 48.),
            "kurze Menüs behalten den weiten Abstand"
        );
        assert_eq!(m.at(640., 280., Vec2::new(640., 280.)), Some(0));
        assert_eq!(m.at(640., 280., Vec2::new(500., 280. + 44. * 2.)), Some(2));
        assert_eq!(
            m.at(640., 280., Vec2::new(640., 280. + 22.)),
            None,
            "Lücke zwischen zwei Einträgen"
        );
        // das Pausenmenü endet über der Fußzeile
        let (step, height) = m.spacing();
        assert!(280. + (m.items.len() - 1) as f32 * step + height / 2. < 720. - 36. - 10.);
        // das Titelmenü endet über der Fußzeile
        let t = title_menu(true);
        let (step, height) = t.spacing();
        assert!(
            TITLE_MENU_Y + (t.items.len() - 1) as f32 * step + height / 2. < 720. - 36. - 10. - 16.
        );
        assert_eq!(m.at(640., 280., Vec2::new(900., 280.)), None);
        let mut mouse = berlin_engine::Mouse {
            hud: Some(Vec2::new(640., 280. + 50.)),
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
        let r = draw_title(&mut h, &title_menu(true), false, false);
        let ver = h.text_width(
            &format!(
                "v{} · Rust {}",
                crate::about::game_version(),
                crate::about::native_version()
            ),
            14.,
        );
        assert!(
            r[0] > 36. && r[0] + r[2] < 1280. - 36. - ver - 20.,
            "Link vor der Versionszeile: {r:?}"
        );
        assert!(r[1] + r[3] <= 720.);
        draw_pause(&mut h, &pause_menu(), 3, Some(95.));
        let binds = crate::bindings::Bindings::default();
        draw_controls(&mut h, true, &binds);
        draw_stats(&mut h, &Default::default(), &Default::default(), true);
        assert!(h.items.len() > 500);
        // Steuerungstafel: jede Spalte endet vor der nächsten
        let size = 16.;
        let wa = control_rows(&binds)
            .iter()
            .map(|r| h.text_width(&r.0, size))
            .fold(0., f32::max);
        let wb = control_rows(&binds)
            .iter()
            .map(|r| h.text_width(&r.1, size))
            .fold(0., f32::max);
        let wc = control_rows(&binds)
            .iter()
            .map(|r| h.text_width(&r.2, size))
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
