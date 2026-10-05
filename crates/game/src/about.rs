//! Seite „Über das Spiel“ (Titel- und Pausenmenü): Entwicklung und Hinweise, Daten und Lizenzen, Changelog.
//! Drei Reiter (←/→, A/D, LB/RB, Steuerkreuz, Klick), der Inhalt rollt (↑/↓, W/S, Stick, Mausrad, Bild↑/↓,
//! Pos1/Ende). Alles kommt aus Dateien des Repos, damit nichts veraltet: die Version aus `package.json`, die
//! Einträge aus `CHANGELOG.md`, die Rust-Pakete samt Lizenz aus `thirdparty.tsv` (`node tools/thirdparty.mjs`,
//! ein Test prüft den Abgleich mit `Cargo.lock`).
use berlin_engine::hud::{Align, Hud};
use berlin_engine::{KeyCode, Keys};

use crate::menu::YELLOW;

const PACKAGE_JSON: &str = include_str!("../../../package.json");
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");
const THIRDPARTY: &str = include_str!("thirdparty.tsv");

/// Spielversion nach SemVer (wie im Browser-Spiel, `package.json`); die Crate-Version ist intern.
pub fn game_version() -> &'static str {
    PACKAGE_JSON
        .lines()
        .find_map(|l| {
            let l = l.trim();
            let v = l.strip_prefix("\"version\":")?.trim();
            Some(v.trim_end_matches(',').trim_matches('"'))
        })
        .unwrap_or(env!("CARGO_PKG_VERSION"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    About,
    Licenses,
    Changelog,
}
pub const TABS: [(Tab, &str); 3] = [
    (Tab::About, "Spiel"),
    (Tab::Licenses, "Lizenzen"),
    (Tab::Changelog, "Changelog"),
];

/// Ein Baustein des Inhalts, vor dem Umbrechen.
#[derive(Debug, Clone, PartialEq)]
enum Block {
    Head(String),
    Sub(String),
    Text(String),
    Bullet(String),
    /// Tabellenzeile: links, rechts (wird nicht umbrochen)
    Row(String, String),
    Gap,
}

/// Eine gezeichnete Zeile (HUD-Einheiten).
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub right: Option<String>,
    pub size: f32,
    pub color: [f32; 4],
    pub indent: f32,
    /// Abstand bis zur nächsten Zeile
    pub step: f32,
    pub bold: bool,
    /// Aufzählungspunkt vor der Zeile (gezeichnet, die Schrift hat kein •)
    pub bullet: bool,
}

const WHITE: [f32; 4] = [0.93, 0.93, 0.93, 1.];
const GREY: [f32; 4] = [0.66, 0.66, 0.68, 1.];
const SUB: [f32; 4] = [0.55, 0.78, 1., 1.];

/// Ein Paket aus `thirdparty.tsv`.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub name: &'static str,
    pub version: &'static str,
    pub license: &'static str,
}
pub fn packages() -> Vec<Package> {
    THIRDPARTY
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some(Package {
                name: f.next()?,
                version: f.next()?,
                license: f.next()?,
            })
        })
        .collect()
}

/// Lizenzausdruck vereinheitlicht: „MIT/Apache-2.0“, „Apache-2.0 OR MIT“ → „Apache-2.0 oder MIT“.
pub fn normalize_license(s: &str) -> String {
    if s.contains("AND") || s.contains("WITH") || s.contains('(') {
        return s.replace(" OR ", " oder ").replace(" AND ", " und ");
    }
    let mut parts: Vec<&str> = s
        .split(" OR ")
        .flat_map(|p| p.split('/'))
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    parts.sort_unstable();
    parts.dedup();
    parts.join(" oder ")
}

/// Markdown aus dem Changelog in Klartext: Code-Striche, Fettdruck und Links (`[Text](Ziel)` → Text).
fn plain(s: &str) -> String {
    let s = s.replace("**", "").replace('`', "");
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after
            .find("](")
            .and_then(|j| after[j..].find(')').map(|k| (j, j + k)))
        {
            Some((j, k)) => {
                out.push_str(&after[..j]);
                rest = &after[k + 1..];
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Changelog in Bausteine: Versionen als Überschrift, Rubriken, Punkte (mit Folgezeilen), Absätze.
fn changelog_blocks(md: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut started = false;
    let mut para: Option<(bool, String)> = None; // (Punkt?, Text)
    let flush = |para: &mut Option<(bool, String)>, out: &mut Vec<Block>| {
        if let Some((bullet, t)) = para.take() {
            let t = plain(&t);
            out.push(if bullet {
                Block::Bullet(t)
            } else {
                Block::Text(t)
            });
        }
    };
    for raw in md.lines() {
        let l = raw.trim_end();
        if let Some(h) = l.strip_prefix("## ") {
            flush(&mut para, &mut out);
            if started {
                out.push(Block::Gap);
            }
            started = true;
            out.push(Block::Head(plain(h).replace(['[', ']'], "")));
            continue;
        }
        if !started {
            continue; // Titel und Einleitung
        }
        if let Some(h) = l.strip_prefix("### ") {
            flush(&mut para, &mut out);
            out.push(Block::Sub(plain(h)));
        } else if let Some(b) = l.strip_prefix("- ") {
            flush(&mut para, &mut out);
            para = Some((true, b.trim().to_owned()));
        } else if l.trim().is_empty() {
            flush(&mut para, &mut out);
        } else {
            match &mut para {
                Some((_, t)) => {
                    t.push(' ');
                    t.push_str(l.trim());
                }
                None => para = Some((false, l.trim().to_owned())),
            }
        }
    }
    flush(&mut para, &mut out);
    out
}

fn about_blocks() -> Vec<Block> {
    let n = packages().len();
    let b = |s: &str| Block::Bullet(s.to_owned());
    vec![
        Block::Head(format!("GTA Berlin · Version {}", game_version())),
        Block::Text(
            "Top-down-Open-World in ganz Berlin im Maßstab 1:1, gebaut aus OpenStreetMap. Native Fassung in \
             Rust; die Browser-Fassung (HTML5 Canvas, Web Audio) bleibt die Referenz."
                .into(),
        ),
        Block::Gap,
        Block::Sub("Entwicklung".into()),
        b("Idee, Entwicklung und Gestaltung: Martin Pfeffer · celox.io"),
        b("Programmiert mit Unterstützung von Claude Code (Anthropic)"),
        Block::Gap,
        Block::Sub("Hinweise".into()),
        b(
            "Inoffizielles Fanprojekt ohne Verbindung zu Rockstar Games oder Take-Two Interactive; \
             „Grand Theft Auto“ ist deren Marke.",
        ),
        b(
            "Grafiken, Fahrzeuge, Figuren und Musik erzeugt das Spiel selbst; Geräusche stammen aus frei \
             lizenzierten Aufnahmen (Reiter „Lizenzen“) – keine Inhalte aus anderen Spielen.",
        ),
        b("Quellcode: privates Projekt, keine öffentliche Lizenz."),
        Block::Gap,
        Block::Sub("Technik".into()),
        b("Rust 2024 · wgpu (Metal, DirectX 12) · winit · cpal · gilrs · glam"),
        b(&format!("{n} Rust-Pakete von Dritten, Lizenzen im Reiter „Lizenzen“")),
        b("Browser-Fassung ohne Abhängigkeiten (reines JavaScript)"),
        Block::Gap,
        Block::Sub("Versionierung".into()),
        b("Semantic Versioning; Änderungen im Reiter „Changelog“ (Keep a Changelog)"),
    ]
}

/// Quellen der Geräusch-Samples aus dem Manifest von tools/audio/build_sfx.py: „Titel – Urheber, Lizenz (Seite)“.
fn sfx_credits() -> Vec<String> {
    let man: serde_json::Value =
        serde_json::from_str(include_str!("../../../data/audio/sfx/manifest.json"))
            .unwrap_or_default();
    let Some(q) = man.get("quellen").and_then(|q| q.as_object()) else {
        return Vec::new();
    };
    let s = |v: &serde_json::Value, k: &str| {
        v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
    };
    let mut out: Vec<String> = q
        .values()
        .map(|v| {
            let seite = s(v, "seite");
            let seite = seite
                .trim_start_matches("https://")
                .trim_start_matches("www.");
            format!(
                "{} – {}, {} – {}",
                s(v, "titel"),
                s(v, "urheber"),
                s(v, "lizenz"),
                seite
            )
        })
        .collect();
    out.sort();
    out
}

/// Quellen der Motor-Bänke aus Freesound (Manifeste von build_engine_sounds.py mit Lizenz und Urheber).
fn engine_credits() -> Vec<String> {
    let manifests = [
        include_str!("../../../data/audio/engine/r4/manifest.json"),
        include_str!("../../../data/audio/engine/d4/manifest.json"),
        include_str!("../../../data/audio/engine/d6/manifest.json"),
    ];
    manifests
        .iter()
        .filter_map(|m| {
            let v: serde_json::Value = serde_json::from_str(m).ok()?;
            let lic = v["lizenz"].as_str()?;
            let who = v["urheber"].as_str()?;
            let titles: Vec<&str> = v["quelle"]
                .as_array()?
                .iter()
                .filter_map(|t| t["titel"].as_str())
                .collect();
            Some(format!(
                "Motor ({}): {} – {who}, {lic} – freesound.org",
                v["bank"].as_str().unwrap_or("?"),
                titles.join(", ")
            ))
        })
        .collect()
}

/// Boden-, Dach- und Fassadentexturen aus dem Manifest von tools/gfx/build_materials.py: „Zweck: Vorlage – Urheber, Lizenz (Seite)“.
fn material_credits() -> Vec<String> {
    let man: serde_json::Value =
        serde_json::from_str(berlin_engine::MATERIAL_MANIFEST).unwrap_or_default();
    let Some(m) = man.get("materialien").and_then(|q| q.as_object()) else {
        return Vec::new();
    };
    let s = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or("").to_string();
    m.values()
        .map(|v| {
            format!(
                "Textur {} ({}): {}, {} – {}",
                s(v, "quelle"),
                s(v, "zweck"),
                s(v, "urheber"),
                s(v, "lizenz"),
                s(v, "seite").trim_start_matches("https://")
            )
        })
        .collect()
}

/// HD-Schrift aus ihrem Manifest (Name, Urheber, Lizenz, Seite).
fn font_credit() -> String {
    let man: serde_json::Value =
        serde_json::from_str(berlin_engine::FONT_MANIFEST).unwrap_or_default();
    let f = &man["schrift"];
    let s = |k: &str| f[k].as_str().unwrap_or("").to_string();
    format!(
        "{} {} (HD-Modus): {}, {} – {}",
        s("name"),
        s("version"),
        s("urheber"),
        s("lizenz"),
        s("seite").trim_start_matches("https://")
    )
}

fn license_blocks() -> Vec<Block> {
    let pk = packages();
    let mut out = vec![
        Block::Head("Daten".into()),
        Block::Bullet(
            "Kartendaten © OpenStreetMap-Mitwirkende, Open Database License (ODbL) 1.0 – \
             openstreetmap.org/copyright (Auszug von Geofabrik)"
                .into(),
        ),
        Block::Bullet(
            "Stadtgrenze und Bezirke (LOR 2021), Baumbestand, Einwohnerdichte 2022, Verkehrsmengen 2019: \
             Geoportal Berlin, Datenlizenz Deutschland – Zero – Version 2.0"
                .into(),
        ),
        Block::Bullet("Fahrplandaten: VBB Verkehrsverbund Berlin-Brandenburg GmbH, CC BY 3.0".into()),
        Block::Bullet("Bar-Auslastung (nur mit eingeschaltetem Bar-Feed): gostumblr.com".into()),
        Block::Bullet(
            "Schussgeräusche: The Free Firearm Sound Library (Ben Jaszczak, Brian Nelson, Kevin Heras, \
             Matthew Nanney), CC0 1.0 – opengameart.org"
                .into(),
        ),
        Block::Bullet("Weitere Geräusche (Liste aus data/audio/sfx/manifest.json):".into()),
    ];
    out.extend(sfx_credits().into_iter().map(Block::Bullet));
    out.extend(engine_credits().into_iter().map(Block::Bullet));
    out.extend(material_credits().into_iter().map(Block::Bullet));
    out.extend([
        Block::Gap,
        Block::Head("Schrift".into()),
        Block::Bullet(font_credit()),
        Block::Bullet(
            "Bitmapschrift (Pixel-Modus) aus dem Paket font8x8 (MIT), ergänzt um eigene Zeichen"
                .into(),
        ),
        Block::Gap,
        Block::Head(format!("Rust-Pakete ({})", pk.len())),
    ]);
    // Übersicht: wie viele Pakete unter welcher Lizenz
    let mut counts: Vec<(String, usize)> = Vec::new();
    for p in &pk {
        let l = normalize_license(p.license);
        match counts.iter_mut().find(|(k, _)| *k == l) {
            Some((_, c)) => *c += 1,
            None => counts.push((l, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out.extend(
        counts
            .into_iter()
            .map(|(l, c)| Block::Row(l, format!("{c} Pakete").replace("1 Pakete", "1 Paket"))),
    );
    out.push(Block::Gap);
    out.push(Block::Sub("Alle Pakete".into()));
    out.extend(pk.iter().map(|p| {
        Block::Row(
            format!("{} {}", p.name, p.version),
            normalize_license(p.license),
        )
    }));
    out
}

fn blocks(tab: Tab) -> Vec<Block> {
    match tab {
        Tab::About => about_blocks(),
        Tab::Licenses => license_blocks(),
        Tab::Changelog => changelog_blocks(CHANGELOG),
    }
}

/// Text in Zeilen der Breite `width` umbrechen (an Leerzeichen; ein zu langes Wort steht allein).
pub fn wrap(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        let cand = if cur.is_empty() {
            w.to_owned()
        } else {
            format!("{cur} {w}")
        };
        if measure(&cand) <= width || cur.is_empty() {
            cur = cand;
        } else {
            lines.push(std::mem::replace(&mut cur, w.to_owned()));
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Bausteine in Zeilen: `measure(text, size)` misst in HUD-Einheiten.
pub fn layout(tab: Tab, width: f32, measure: &dyn Fn(&str, f32) -> f32) -> Vec<Line> {
    let mut out = Vec::new();
    let line = |text: String, size: f32, color, indent, step, bold| Line {
        text,
        right: None,
        size,
        color,
        indent,
        step,
        bold,
        bullet: false,
    };
    for b in blocks(tab) {
        match b {
            Block::Head(t) => out.push(line(t, 20., YELLOW, 0., 30., true)),
            Block::Sub(t) => out.push(line(t.to_uppercase(), 13., SUB, 0., 22., true)),
            Block::Text(t) => {
                for l in wrap(&t, width, &|s| measure(s, 14.)) {
                    out.push(line(l, 14., WHITE, 0., 20., false));
                }
            }
            Block::Bullet(t) => {
                let ls = wrap(&t, width - 22., &|s| measure(s, 14.));
                for (i, l) in ls.into_iter().enumerate() {
                    out.push(Line {
                        bullet: i == 0,
                        ..line(l, 14., WHITE, 22., 20., false)
                    });
                }
            }
            Block::Row(l, r) => out.push(Line {
                right: Some(r),
                ..line(l, 12., WHITE, 0., 17., false)
            }),
            Block::Gap => {
                if let Some(last) = out.last_mut() {
                    last.step += 14.;
                }
            }
        }
    }
    out
}

/// Gesamthöhe des Inhalts.
pub fn content_height(lines: &[Line]) -> f32 {
    lines.iter().map(|l| l.step).sum()
}

/// Was ein Schritt auf der Seite bewirkt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Out {
    Stay,
    Back,
    /// Reiter gewechselt (Klang)
    Switched,
}

/// Eingaben der Seite, aus Tastatur, Controller und Maus.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AboutKeys {
    pub prev: bool,
    pub next: bool,
    pub back: bool,
    /// laufendes Rollen −1…1 (gehaltene Pfeile, Stick)
    pub scroll: f32,
    /// einmaliges Rollen in HUD-Einheiten (Mausrad, Bild↑/↓)
    pub jump: f32,
    pub home: bool,
    pub end: bool,
    /// Klick auf diesen Reiter
    pub click_tab: Option<usize>,
}
impl AboutKeys {
    pub fn from(keys: &Keys, tab_at: impl Fn(glam::Vec2) -> Option<usize>) -> Self {
        let p = |k: KeyCode| keys.pressed.contains(&k);
        let held = |k: KeyCode| keys.held.contains(&k);
        let (pad, pe) = (&keys.pad, &keys.pad_pressed);
        let mut scroll = (held(KeyCode::ArrowDown) || held(KeyCode::KeyS) || pad.down) as i32
            as f32
            - (held(KeyCode::ArrowUp) || held(KeyCode::KeyW) || pad.up) as i32 as f32;
        if pad.ly.abs() > 0.2 {
            scroll += pad.ly;
        }
        if pad.ry.abs() > 0.2 {
            scroll += pad.ry;
        }
        let m = &keys.mouse;
        Self {
            prev: p(KeyCode::ArrowLeft) || p(KeyCode::KeyA) || pe.left || pe.lb,
            next: p(KeyCode::ArrowRight) || p(KeyCode::KeyD) || pe.right || pe.rb,
            back: p(KeyCode::Escape) || p(KeyCode::Backspace) || pe.b || m.right_pressed,
            scroll: scroll.clamp(-1.5, 1.5),
            jump: -m.wheel * 60.
                + if p(KeyCode::PageDown) { 400. } else { 0. }
                + if p(KeyCode::PageUp) { -400. } else { 0. },
            home: p(KeyCode::Home),
            end: p(KeyCode::End),
            click_tab: m.left_pressed.then(|| m.hud.and_then(&tab_at)).flatten(),
        }
    }
}

/// Fläche der Seite in HUD-Einheiten.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub w: f32,
    pub top: f32,
    pub bottom: f32,
}
pub fn frame(hud_width: f32) -> Frame {
    let w = (hud_width - 160.).min(1040.);
    Frame {
        x: (hud_width - w) / 2.,
        w,
        top: 156.,
        bottom: 720. - 78.,
    }
}
/// Reiterknöpfe (x, y, Breite, Höhe).
pub fn tab_rects(hud_width: f32) -> [[f32; 4]; 3] {
    let (w, gap) = (190., 14.);
    let x0 = hud_width / 2. - (3. * w + 2. * gap) / 2.;
    std::array::from_fn(|i| [x0 + i as f32 * (w + gap), 92., w, 38.])
}
pub fn tab_at(hud_width: f32, p: glam::Vec2) -> Option<usize> {
    tab_rects(hud_width)
        .iter()
        .position(|r| p.x >= r[0] && p.x <= r[0] + r[2] && p.y >= r[1] && p.y <= r[1] + r[3])
}

#[derive(Debug, Default)]
pub struct About {
    pub tab: Tab,
    pub scroll: f32,
    /// größter Rollwert, aus der letzten Vermessung (`prepare`)
    pub max: f32,
    /// umbrochene Zeilen je (Reiter, Breite)
    cache: Option<(Tab, f32, Vec<Line>)>,
}

impl About {
    /// Zeilen umbrechen (gecacht) und den größten Rollwert bestimmen; `draw` ruft es mit der echten Schrift.
    pub fn prepare(&mut self, hud_width: f32, measure: &dyn Fn(&str, f32) -> f32) {
        let f = frame(hud_width);
        let width = f.w - 40.;
        let fresh = matches!(&self.cache, Some((t, w, _)) if *t == self.tab && *w == width);
        if !fresh {
            self.cache = Some((self.tab, width, layout(self.tab, width, measure)));
        }
        let h = content_height(&self.cache.as_ref().unwrap().2);
        self.max = (h - (f.bottom - f.top - 24.)).max(0.);
        self.scroll = self.scroll.clamp(0., self.max);
    }
    pub fn step(&mut self, k: AboutKeys, dt: f32) -> Out {
        if k.back {
            return Out::Back;
        }
        let i = TABS.iter().position(|(t, _)| *t == self.tab).unwrap_or(0);
        let j = if let Some(c) = k.click_tab {
            c
        } else if k.prev {
            (i + 2) % 3
        } else if k.next {
            (i + 1) % 3
        } else {
            i
        };
        if j != i {
            self.tab = TABS[j].0;
            self.scroll = 0.;
            return Out::Switched;
        }
        self.scroll += k.scroll * 700. * dt + k.jump;
        if k.home {
            self.scroll = 0.;
        }
        if k.end {
            self.scroll = self.max;
        }
        self.scroll = self.scroll.clamp(0., self.max);
        Out::Stay
    }
    pub fn draw(&mut self, h: &mut Hud, in_game: bool) {
        let vw = h.width;
        h.rect(
            0.,
            0.,
            vw,
            720.,
            [0.02, 0.024, 0.04, if in_game { 0.86 } else { 0.94 }],
            0.,
        );
        h.text(
            "ÜBER DAS SPIEL",
            vw / 2.,
            62.,
            36.,
            YELLOW,
            Align::Center,
            true,
        );
        for (i, r) in tab_rects(vw).iter().enumerate() {
            let sel = TABS[i].0 == self.tab;
            let bg = if sel {
                YELLOW
            } else {
                [0.06, 0.067, 0.094, 0.8]
            };
            h.rect(r[0], r[1], r[2], r[3], bg, 10.);
            let c = if sel {
                [0.067, 0.067, 0.067, 1.]
            } else {
                [0.85, 0.85, 0.85, 1.]
            };
            h.text(
                TABS[i].1,
                r[0] + r[2] / 2.,
                r[1] + 26.,
                18.,
                c,
                Align::Center,
                !sel,
            );
        }
        let f = frame(vw);
        h.rect(
            f.x,
            f.top,
            f.w,
            f.bottom - f.top,
            [0.05, 0.055, 0.075, 0.85],
            12.,
        );
        {
            let measure = |s: &str, size: f32| h.text_width(s, size);
            self.prepare(vw, &measure);
        }
        let (scroll, max) = (self.scroll, self.max);
        let lines = self.cache.as_ref().map(|c| c.2.clone()).unwrap_or_default();
        // nur Zeilen, die ganz in der Fläche liegen (der HUD-Text kennt kein Abschneiden)
        let mut y = f.top + 30. - scroll;
        let (lo, hi) = (f.top + 6., f.bottom - 8.);
        for l in &lines {
            if y - l.size * 0.9 >= lo && y + 4. <= hi {
                let x = f.x + 20. + l.indent;
                if l.bullet {
                    h.rect(x - 15., y - l.size * 0.45, 5., 5., SUB, 1.);
                }
                h.text(&l.text, x, y, l.size, l.color, Align::Left, l.bold);
                if let Some(r) = &l.right {
                    h.text(r, f.x + f.w - 20., y, l.size, GREY, Align::Right, false);
                }
            }
            y += l.step;
        }
        // Rollbalken
        if max > 0. {
            let track = f.bottom - f.top - 24.;
            let view = track / (track + max);
            let bh = (track * view).max(30.);
            let by = f.top + 12. + (track - bh) * (scroll / max);
            h.rect(f.x + f.w - 8., by, 4., bh, [1., 1., 1., 0.35], 2.);
        }
        h.text(
            "Links/Rechts, LB/RB: Reiter  ·  Hoch/Runter, Stick, Mausrad: rollen  ·  Esc / B: Zurück",
            vw / 2.,
            720. - 36.,
            15.,
            [0.85, 0.85, 0.85, 1.],
            Align::Center,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(s: &str, size: f32) -> f32 {
        s.chars().count() as f32 * size * 0.6
    }

    #[test]
    fn ground_textures_are_credited_with_license() {
        let f = font_credit();
        assert!(
            f.contains("Inter") && f.contains("Open Font License") && f.contains("rsms.me"),
            "{f}"
        );
        let c = material_credits();
        let man: serde_json::Value =
            serde_json::from_str(berlin_engine::MATERIAL_MANIFEST).unwrap();
        assert_eq!(
            c.len(),
            man["materialien"].as_object().unwrap().len(),
            "{c:?}"
        );
        assert!(c.len() >= 12);
        assert!(
            c.iter()
                .all(|l| l.contains("CC0 1.0") && l.contains("ambientcg.com"))
        );
        let text: Vec<String> = license_blocks()
            .iter()
            .filter_map(|b| match b {
                Block::Bullet(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert!(c.iter().all(|l| text.contains(l)), "im Lizenz-Reiter");
    }
    #[test]
    fn sound_sources_are_credited_with_license() {
        let c = sfx_credits();
        assert!(!c.is_empty(), "Manifest ohne Quellen");
        assert!(c.iter().all(|l| l.contains("CC")), "{c:?}");
        // CC-BY verlangt die Namensnennung der Urheber
        assert!(
            c.iter()
                .any(|l| l.contains("CC BY") && l.contains("EminYILDIRIM"))
        );
        let all = license_blocks();
        assert!(
            all.iter()
                .any(|b| matches!(b, Block::Bullet(t) if t.starts_with("Kenney")))
        );
    }

    #[test]
    fn version_comes_from_package_json() {
        let v = game_version();
        assert!(
            v.split('.').count() == 3 && v.split('.').all(|p| p.parse::<u32>().is_ok()),
            "{v}"
        );
        assert!(PACKAGE_JSON.contains(&format!("\"version\": \"{v}\"")));
    }

    #[test]
    fn package_list_matches_cargo_lock() {
        // Jedes fremde Paket aus Cargo.lock steht mit Version in thirdparty.tsv und umgekehrt – sonst:
        // node tools/thirdparty.mjs
        let lock = include_str!("../../../Cargo.lock");
        // eigene Pakete (Arbeitsbereich) haben in Cargo.lock keine `source`-Zeile – so bleibt die Liste ohne Pflege
        let mut locked: Vec<(String, String)> = Vec::new();
        for block in lock.split("[[package]]").skip(1) {
            let field = |k: &str| {
                block
                    .lines()
                    .find_map(|l| l.strip_prefix(k))
                    .map(|v| v.trim_matches('"').to_owned())
            };
            if field("source = ").is_none() {
                continue;
            }
            if let (Some(n), Some(v)) = (field("name = "), field("version = ")) {
                locked.push((n, v));
            }
        }
        let mut listed: Vec<(String, String)> = packages()
            .iter()
            .map(|p| (p.name.to_owned(), p.version.to_owned()))
            .collect();
        locked.sort();
        listed.sort();
        assert_eq!(
            listed, locked,
            "thirdparty.tsv veraltet: node tools/thirdparty.mjs"
        );
        assert!(packages().iter().all(|p| !p.license.is_empty()));
    }

    #[test]
    fn licenses_read_the_same_however_they_are_written() {
        let n = normalize_license;
        assert_eq!(n("MIT OR Apache-2.0"), "Apache-2.0 oder MIT");
        assert_eq!(n("Apache-2.0/MIT"), "Apache-2.0 oder MIT");
        assert_eq!(n("MIT/Apache-2.0"), "Apache-2.0 oder MIT");
        assert_eq!(n("Apache-2.0 / MIT"), "Apache-2.0 oder MIT");
        assert_eq!(n("MIT"), "MIT");
        assert_eq!(
            n("(MIT OR Apache-2.0) AND Unicode-3.0"),
            "(MIT oder Apache-2.0) und Unicode-3.0"
        );
    }

    #[test]
    fn changelog_becomes_versions_sections_and_bullets() {
        let md = "# Changelog\n\nEinleitung.\n\n## [1.2.0] – 2026-01-02\n\n### Neu\n\n- Erster **Punkt** mit \
                  `Code`\n  und Folgezeile, [Link](docs/x.md).\n- Zweiter\n\n## [1.1.0] – 2026-01-01\n\nAbsatz.\n";
        let b = changelog_blocks(md);
        assert_eq!(b[0], Block::Head("1.2.0 – 2026-01-02".into()));
        assert_eq!(b[1], Block::Sub("Neu".into()));
        assert_eq!(
            b[2],
            Block::Bullet("Erster Punkt mit Code und Folgezeile, Link.".into())
        );
        assert_eq!(b[3], Block::Bullet("Zweiter".into()));
        assert_eq!(b[4], Block::Gap);
        assert_eq!(b[5], Block::Head("1.1.0 – 2026-01-01".into()));
        assert_eq!(b[6], Block::Text("Absatz.".into()));
        assert!(
            !b.iter()
                .any(|x| matches!(x, Block::Text(t) if t.contains("Einleitung")))
        );
        // das echte Changelog: die oberste Überschrift ist der aktuelle Stand
        let real = changelog_blocks(CHANGELOG);
        assert!(
            matches!(&real[0], Block::Head(t) if t.starts_with("Unreleased") || t.starts_with(game_version()))
        );
    }

    #[test]
    fn every_line_fits_the_page() {
        for (tab, _) in TABS {
            let ls = layout(tab, 900., &measure);
            assert!(ls.len() > 5, "{tab:?}");
            for l in &ls {
                let w = l.indent + measure(&l.text, l.size);
                // ein einzelnes überlanges Wort darf überstehen, sonst nichts
                assert!(w <= 900. || !l.text.contains(' '), "{tab:?}: {}", l.text);
                if let Some(r) = &l.right {
                    assert!(
                        measure(&l.text, l.size) + measure(r, l.size) + 20. <= 900.,
                        "{}",
                        l.text
                    );
                }
            }
        }
        let about = layout(Tab::About, 900., &measure);
        assert!(about.iter().any(|l| l.text.contains("Martin Pfeffer")));
        let lic = layout(Tab::Licenses, 900., &measure);
        assert!(lic.iter().any(|l| l.text.contains("OpenStreetMap")));
        assert!(lic.iter().any(|l| l.text.starts_with("wgpu ")));
    }

    #[test]
    fn tabs_switch_and_scroll_stays_in_range() {
        let mut a = About::default();
        let k = AboutKeys::default;
        assert_eq!(
            a.step(AboutKeys { next: true, ..k() }, 0.016),
            Out::Switched
        );
        assert_eq!(a.tab, Tab::Licenses);
        assert_eq!(
            a.step(AboutKeys { prev: true, ..k() }, 0.016),
            Out::Switched
        );
        assert_eq!(
            a.step(AboutKeys { prev: true, ..k() }, 0.016),
            Out::Switched
        );
        assert_eq!(a.tab, Tab::Changelog, "links vom ersten Reiter: der letzte");
        // lange Seite: rollt, bleibt aber in den Grenzen
        a.prepare(1280., &measure);
        assert!(a.max > 1000., "{}", a.max);
        a.step(AboutKeys { jump: 1e6, ..k() }, 0.016);
        assert_eq!(a.scroll, a.max);
        a.step(AboutKeys { scroll: -1., ..k() }, 0.1);
        assert!(a.scroll < a.max);
        a.step(AboutKeys { home: true, ..k() }, 0.016);
        assert_eq!(a.scroll, 0.);
        a.step(AboutKeys { scroll: -1., ..k() }, 0.1);
        assert_eq!(a.scroll, 0., "nicht über den Anfang");
        a.step(AboutKeys { end: true, ..k() }, 0.016);
        assert_eq!(a.scroll, a.max);
        // Reiterwechsel beginnt oben, die kurze Seite rollt nicht weiter als nötig
        assert_eq!(
            a.step(
                AboutKeys {
                    click_tab: Some(0),
                    ..k()
                },
                0.016
            ),
            Out::Switched
        );
        assert_eq!((a.tab, a.scroll), (Tab::About, 0.));
        a.prepare(1280., &measure);
        a.step(AboutKeys { jump: 1e6, ..k() }, 0.016);
        assert_eq!(a.scroll, a.max);
        assert_eq!(a.step(AboutKeys { back: true, ..k() }, 0.016), Out::Back);
        let r = tab_rects(1280.)[2];
        assert_eq!(
            tab_at(1280., glam::Vec2::new(r[0] + 5., r[1] + 5.)),
            Some(2)
        );
        assert_eq!(tab_at(1280., glam::Vec2::new(5., 5.)), None);
    }
}
