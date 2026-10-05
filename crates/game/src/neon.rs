//! Ladenlicht und Schilder am Eingang (Nachfolger von `render.js shopGlowPoint`/`neonSigns`): Lokale, Spätis,
//! Imbisse, Cafés, Hotels, Märkte und Bahnhöfe werfen nachts warmes Licht aus dem Schaufenster auf den Gehweg.
//! Kneipen, Bars, Clubs, Spätis, Imbisse, Cafés und Hotels tragen dazu ein Schild – kein Kartensymbol, sondern
//! ein Ding am Haus, und jedes Haus seins: Röhrenschrift, Neon im Rahmen, Leuchtkasten, Glühbirnentafel mit
//! Lauflicht, senkrechtes Nasenschild, Schrift mit Symbol (Glas, Krug, Tasse, Note, Pfeil zur Tür) oder Kreidetafel.
//! Tagsüber matt, ab der Dämmerung leuchtend, manche Röhren flackern. Ärzte, Dienstleister, Restaurants und
//! kleine Läden bleiben ohne Schild und Licht (die Daten bleiben – das Stadtleben nutzt sie weiter).
//! Alles aus Ort-Hashes, nie aus dem Welt-Zufall. Nur Darstellung.
use berlin_engine::camera::Camera;
use berlin_engine::hud::{Align, Hud};
use berlin_sim::city::{City, Poi};
use berlin_sim::math::hash01;
use glam::Vec2;
use std::collections::HashMap;

/// Kategorien mit Schaufensterlicht. Kleine Läden (`shop`), Dienstleister (`service`) und Kultur bleiben dunkel.
pub const SHOP_GLOW: [&str; 8] = [
    "mall",
    "supermarket",
    "food",
    "drink",
    "cafe",
    "hotel",
    "ubahn",
    "sbahn",
];
pub const MAX_SIGNS: usize = 40;
/// Röhrenfarben (Neon, Argon, Helium …)
const NEON: [u32; 8] = [
    0xff3cac, 0x3cf0ff, 0x57ff6b, 0xffae3c, 0xb76bff, 0xff4b4b, 0xfff04a, 0xff7ad9,
];
/// Leuchtkästen: Grund der Fläche (die Schrift ist dunkel)
const BOX: [u32; 6] = [0xfff4d6, 0xffd23f, 0xe8342e, 0x2fa84f, 0x2f6fd8, 0xff8a1f];
/// Tafeln: Schiefer, Flaschengrün, Holz, Bordeaux
const BOARD: [u32; 4] = [0x2a2d31, 0x1f3d2b, 0x4a3122, 0x4b1f2a];

/// `wetfx.js h(...n)`: Hash über gerundete Zahlen.
pub fn h(ns: &[f64]) -> f64 {
    hash01(ns.iter().fold(5., |a, b| a * 31. + b.round()))
}

fn rgb(c: u32) -> [f32; 3] {
    [
        ((c >> 16) & 255) as f32 / 255.,
        ((c >> 8) & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
    ]
}
fn pick<T: Copy>(list: &[T], u: f64) -> T {
    list[((u * list.len() as f64) as usize).min(list.len() - 1)]
}

/// Name in Großbuchstaben, höchstens `n` Zeichen; gekürzt wird an Wortgrenzen („SAHARA“ statt „SAHARA IMBIS“),
/// nur ein einzelnes zu langes Wort wird abgeschnitten.
fn short(s: &str, n: usize) -> String {
    let mut out = String::new();
    for word in s.split_whitespace() {
        let word = word.to_uppercase();
        let len = out.chars().count() + usize::from(!out.is_empty()) + word.chars().count();
        if len > n {
            if out.is_empty() {
                out = word.chars().take(n).collect();
            }
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&word);
    }
    out
}

/// Bauart eines Schilds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    /// Röhrenschrift ohne Grund, bei zwei Wörtern zweifarbig
    Tube,
    /// Röhrenschrift in einem Neonrahmen
    Frame,
    /// Leuchtkasten: farbige Fläche, dunkle Schrift
    Box,
    /// dunkle Tafel mit Glühbirnenrand und Lauflicht
    Bulbs,
    /// senkrechtes Nasenschild, Buchstaben übereinander
    Blade,
    /// Röhrenschrift mit gezeichnetem Symbol davor
    Icon(Glyph),
    /// Kreidetafel, abends von einer kleinen Lampe angestrahlt
    Board,
}

/// Gezeichnete Symbole neben der Schrift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Glyph {
    Cocktail,
    Beer,
    Cup,
    Note,
    Arrow,
    Star,
}

/// Was an einem Ort hängt: Text, Bauart, Farben, Größe, ob die Röhre flackert.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub text: String,
    pub style: Style,
    pub color: [f32; 3],
    pub color2: [f32; 3],
    pub scale: f32,
    pub flicker: bool,
}

/// Schriftzug eines Schilds oder `None` (nur Nachtleben, Spätis, Imbisse, Cafés, Hotels).
pub fn sign_text(q: &Poi) -> Option<String> {
    let name = q.name.trim();
    let low = name.to_lowercase();
    let u = |k: f64| h(&[q.x, q.y, k]);
    match q.cat {
        "drink" => Some(
            if !name.is_empty() && (q.kind == "nightclub" || u(1.) < 0.75) {
                short(name, 14)
            } else {
                match q.kind.as_str() {
                    "nightclub" => "CLUB",
                    "pub" => "KNEIPE",
                    "biergarten" => "BIERGARTEN",
                    _ => {
                        if u(7.) < 0.5 {
                            "BAR"
                        } else {
                            "COCKTAILS"
                        }
                    }
                }
                .into()
            },
        ),
        "supermarket" if q.kind == "convenience" || q.kind == "kiosk" => {
            Some(pick(&["SPÄTI", "SPÄTKAUF", "24/7", "SPÄTI", "KIOSK"], u(1.)).into())
        }
        "food" => {
            if low.contains("döner") || low.contains("doner") || low.contains("kebab") {
                Some("DÖNER".into())
            } else if low.contains("pizz") {
                Some("PIZZA".into())
            } else if low.contains("curry") {
                Some("CURRYWURST".into())
            } else if q.kind == "ice_cream" {
                Some(if name.is_empty() || u(2.) < 0.5 {
                    "EIS".into()
                } else {
                    short(name, 12)
                })
            } else if q.kind == "fast_food" {
                Some(if name.is_empty() || u(2.) < 0.4 {
                    "IMBISS".into()
                } else {
                    short(name, 12)
                })
            } else {
                // Restaurants: nur Licht, kein Schild
                None
            }
        }
        "cafe" => Some(if name.is_empty() || u(2.) < 0.3 {
            "CAFÉ".into()
        } else {
            short(name, 12)
        }),
        "hotel" => Some(if name.is_empty() || u(2.) < 0.55 {
            if q.kind == "hostel" {
                "HOSTEL".into()
            } else {
                "HOTEL".into()
            }
        } else {
            short(name, 12)
        }),
        _ => None,
    }
}

/// Gewichtete Auswahl der Bauart je Art des Lokals.
fn style_for(q: &Poi, text: &str, u: f64) -> Style {
    use Glyph::*;
    use Style::*;
    let table: &[(Style, f64)] = match (q.cat, q.kind.as_str()) {
        ("drink", "nightclub") => &[(Frame, 3.), (Tube, 3.), (Icon(Note), 2.), (Icon(Star), 1.)],
        ("drink", "pub") => &[
            (Tube, 3.),
            (Icon(Beer), 3.),
            (Box, 2.),
            (Blade, 2.),
            (Bulbs, 1.),
        ],
        ("drink", "biergarten") => &[(Board, 3.), (Icon(Beer), 3.)],
        ("drink", _) => &[
            (Tube, 3.),
            (Icon(Cocktail), 3.),
            (Frame, 2.),
            (Bulbs, 2.),
            (Icon(Arrow), 2.),
            (Blade, 1.),
        ],
        ("supermarket", _) => &[(Box, 5.), (Tube, 2.), (Icon(Arrow), 1.)],
        ("food", "ice_cream") => &[(Box, 2.), (Icon(Star), 2.), (Board, 1.)],
        ("food", _) => &[(Box, 5.), (Tube, 2.), (Icon(Arrow), 1.), (Bulbs, 1.)],
        ("cafe", _) => &[(Board, 4.), (Icon(Cup), 4.), (Blade, 1.)],
        ("hotel", _) => &[(Blade, 5.), (Tube, 2.), (Box, 1.), (Frame, 1.)],
        _ => &[(Tube, 1.)],
    };
    // lange Namen passen nicht übereinander
    let fits = |s: &Style| *s != Blade || text.chars().count() <= 8;
    let total: f64 = table.iter().filter(|e| fits(&e.0)).map(|e| e.1).sum();
    let mut acc = u * total;
    for &(s, w) in table.iter().filter(|e| fits(&e.0)) {
        if acc < w {
            return s;
        }
        acc -= w;
    }
    Tube
}

/// Schild eines POI oder `None`. Rein aus dem Ort, also bei jedem Besuch dasselbe.
pub fn sign_spec(q: &Poi) -> Option<Spec> {
    let text = sign_text(q)?;
    let u = |k: f64| h(&[q.x, q.y, k]);
    let style = style_for(q, &text, u(6.));
    let color = match style {
        Style::Box => rgb(pick(&BOX, u(4.))),
        Style::Board => rgb(pick(&BOARD, u(4.))),
        _ => rgb(pick(&NEON, u(4.))),
    };
    // zweite Farbe: nie dieselbe wie die erste
    let mut c2 = pick(&NEON, u(8.));
    if rgb(c2) == color {
        c2 = NEON[(NEON.iter().position(|&c| c == c2).unwrap_or(0) + 3) % NEON.len()];
    }
    Some(Spec {
        text,
        style,
        color,
        color2: rgb(c2),
        scale: 0.85 + 0.45 * u(9.) as f32,
        flicker: u(5.) < 0.12,
    })
}

/// Flackernde Röhren (etwa jede achte), sonst an.
pub fn neon_on(q: &Poi, t: f64) -> bool {
    h(&[q.x, q.y, 5.]) > 0.12 || h(&[q.x, q.y, (t * 9.).floor()]) > 0.3
}

#[derive(Debug, Clone)]
pub struct Sign {
    pub x: f64,
    pub y: f64,
    pub spec: Spec,
    /// Röhre gerade an (Flackern)
    pub on: bool,
    /// Zeit für das Lauflicht
    pub t: f64,
}

/// Schaufenster-Punkt je POI: am Gehweg vor dem Laden (zur Fahrbahn hin), zwischengespeichert.
#[derive(Default)]
pub struct Neon {
    glow: HashMap<(i64, i64), Option<(f64, f64)>>,
    pub signs: Vec<Sign>,
    /// Leuchtkraft (setzt mit der Dämmerung ein); 0 = Tag, die Schilder sind dann matt
    pub alpha: f32,
}
impl Neon {
    pub fn glow_point(&mut self, city: &mut City, q: &Poi) -> Option<(f64, f64)> {
        let key = (q.x.round() as i64, q.y.round() as i64);
        if let Some(&g) = self.glow.get(&key) {
            return g;
        }
        let s = city.scale;
        let g = city
            .nearest_edge(q.x, q.y, 40. * s, |e| e.cls <= 8 && !e.bridge)
            .and_then(|ne| {
                let w = city.edges.get(&ne.edge).map_or(0., |e| e.w);
                let (dx, dy) = (q.x - ne.x, q.y - ne.y);
                let d = dx.hypot(dy);
                if d <= 1. {
                    return None;
                }
                let k = (w / 2. + 2.2 * s) / d;
                (k < 1.).then_some((ne.x + dx * k, ne.y + dy * k))
            });
        if self.glow.len() > 20000 {
            self.glow.clear();
        }
        self.glow.insert(key, g);
        g
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
fn rgba(c: [f32; 3], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a]
}
const DARK: [f32; 3] = [0.07, 0.07, 0.09];
const WHITE: [f32; 3] = [1., 1., 1.];

/// Röhrenschrift: nachts farbiger Schein (versetzte Kopien) unter einem hellen Kern, tags matte Glasröhre.
/// `split` färbt das zweite Wort in der zweiten Farbe.
#[allow(clippy::too_many_arguments)]
fn tube_text(
    h: &mut Hud,
    text: &str,
    cx: f32,
    base: f32,
    size: f32,
    c1: [f32; 3],
    c2: Option<[f32; 3]>,
    lit: f32,
    a: f32,
) {
    let (first, second) = match (c2, text.split_once(' ')) {
        (Some(c2), Some((f, s))) => (format!("{f} "), Some((s.to_owned(), c2))),
        _ => (text.to_owned(), None),
    };
    let w = h.text_width(text, size);
    let mut x = cx - w / 2.;
    let mut parts = vec![(first, c1)];
    if let Some(p) = second {
        parts.push(p);
    }
    for (part, c) in parts {
        if lit > 0.02 {
            let o = size * 0.12;
            for (dx, dy) in [(-o, 0.), (o, 0.), (0., -o), (0., o)] {
                h.text(
                    &part,
                    x + dx,
                    base + dy,
                    size,
                    rgba(c, 0.45 * lit * a),
                    Align::Left,
                    false,
                );
            }
        }
        // Kern: tags die gefärbte Röhre, nachts fast weiß
        let core = mix(mix(c, DARK, 0.35), mix(c, WHITE, 0.7), lit);
        x += h.text(&part, x, base, size, rgba(core, a), Align::Left, false);
    }
}

/// Neonrahmen bzw. Kasten-Kontur aus vier Strichen.
fn outline(h: &mut Hud, x: f32, y: f32, w: f32, hh: f32, width: f32, c: [f32; 4]) {
    h.line(x, y, x + w, y, width, c);
    h.line(x + w, y, x + w, y + hh, width, c);
    h.line(x + w, y + hh, x, y + hh, width, c);
    h.line(x, y + hh, x, y, width, c);
}

/// Symbol in einem Feld der Kantenlänge `s` um (cx, cy).
fn glyph(h: &mut Hud, g: Glyph, cx: f32, cy: f32, s: f32, c: [f32; 4]) {
    let lw = (s * 0.11).max(1.);
    let l = |h: &mut Hud, x0: f32, y0: f32, x1: f32, y1: f32| {
        h.line(cx + x0 * s, cy + y0 * s, cx + x1 * s, cy + y1 * s, lw, c)
    };
    match g {
        Glyph::Cocktail => {
            l(h, -0.42, -0.4, 0.42, -0.4);
            l(h, -0.42, -0.4, 0., 0.05);
            l(h, 0.42, -0.4, 0., 0.05);
            l(h, 0., 0.05, 0., 0.4);
            l(h, -0.25, 0.42, 0.25, 0.42);
            l(h, 0.15, -0.52, 0.32, -0.2); // Spieß
        }
        Glyph::Beer => {
            l(h, -0.35, -0.38, -0.35, 0.42);
            l(h, 0.2, -0.38, 0.2, 0.42);
            l(h, -0.35, 0.42, 0.2, 0.42);
            l(h, -0.4, -0.38, 0.25, -0.38);
            h.arc(cx + 0.2 * s, cy, 0.2 * s, lw, -1.4, 1.4, c);
            l(h, -0.18, -0.15, -0.18, 0.25);
            l(h, 0.02, -0.15, 0.02, 0.25);
        }
        Glyph::Cup => {
            l(h, -0.4, -0.1, 0.25, -0.1);
            l(h, -0.4, -0.1, -0.3, 0.38);
            l(h, 0.25, -0.1, 0.15, 0.38);
            l(h, -0.3, 0.38, 0.15, 0.38);
            h.arc(cx + 0.24 * s, cy + 0.1 * s, 0.13 * s, lw, -1.5, 1.5, c);
            l(h, -0.15, -0.25, -0.05, -0.5);
            l(h, 0.05, -0.25, 0.15, -0.5);
        }
        Glyph::Note => {
            h.ellipse(cx - 0.2 * s, cy + 0.3 * s, 0.17 * s, 0.13 * s, c);
            l(h, -0.05, 0.3, -0.05, -0.45);
            l(h, -0.05, -0.45, 0.35, -0.3);
        }
        Glyph::Arrow => {
            // zeigt zur Tür (nach unten)
            l(h, 0., -0.45, 0., 0.4);
            l(h, 0., 0.45, -0.3, 0.12);
            l(h, 0., 0.45, 0.3, 0.12);
        }
        Glyph::Star => {
            for i in 0..5 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.;
                let b = a + std::f32::consts::TAU * 2. / 5.;
                l(
                    h,
                    a.cos() * 0.45,
                    a.sin() * 0.45,
                    b.cos() * 0.45,
                    b.sin() * 0.45,
                );
            }
        }
    }
}

/// Ein Schild an (px, py) in Basiseinheiten; `size` = Schriftgröße, `lit` 0 (Tag) … 1 (Nacht, Röhre an),
/// `glow` = Leuchtkraft der Nacht (auch für ausgefallene Röhren), `t` Zeit für das Lauflicht.
#[allow(clippy::too_many_arguments)]
pub fn draw_sign(h: &mut Hud, s: &Spec, px: f32, py: f32, size: f32, lit: f32, glow: f32, t: f64) {
    let size = size * s.scale;
    let gh = size * 7. / 8.;
    let base = py + gh / 2.;
    let w = h.text_width(&s.text, size);
    let pad = size * 0.35;
    // Trägerplatte der Röhren: tags trägt sie die matte Glasschrift, nachts tritt sie hinter dem Schein zurück
    let plate = |h: &mut Hud, w: f32| {
        let (bw, bh) = (w + 1.6 * pad, gh + 1.4 * pad);
        h.rect(
            px - bw / 2.,
            py - bh / 2.,
            bw,
            bh,
            rgba(DARK, 0.6 - 0.35 * lit),
            3.,
        );
    };
    match s.style {
        Style::Tube => {
            plate(h, w);
            let two = (h01(&s.text) < 0.5).then_some(s.color2);
            tube_text(h, &s.text, px, base, size, s.color, two, lit, 1.);
        }
        Style::Frame => {
            let (bw, bh) = (w + 2. * pad, gh + 2. * pad);
            let fc = mix(mix(s.color2, DARK, 0.4), mix(s.color2, WHITE, 0.4), lit);
            if lit > 0.02 {
                outline(
                    h,
                    px - bw / 2.,
                    py - bh / 2.,
                    bw,
                    bh,
                    size * 0.3,
                    rgba(s.color2, 0.3 * lit),
                );
            }
            outline(
                h,
                px - bw / 2.,
                py - bh / 2.,
                bw,
                bh,
                (size * 0.1).max(1.),
                rgba(fc, 1.),
            );
            tube_text(h, &s.text, px, base, size, s.color, None, lit, 1.);
        }
        Style::Box => {
            let (bw, bh) = (w + 2. * pad, gh + 2. * pad);
            if glow > 0.02 {
                h.rect(
                    px - bw / 2. - pad,
                    py - bh / 2. - pad,
                    bw + 2. * pad,
                    bh + 2. * pad,
                    rgba(s.color, 0.25 * glow),
                    pad * 1.5,
                );
            }
            // tags matter Kunststoff, nachts von innen durchleuchtet
            let face = mix(
                mix(s.color, DARK, 0.25),
                mix(s.color, WHITE, 0.2),
                lit.max(0.3 * glow),
            );
            h.rect(
                px - bw / 2. - 1.,
                py - bh / 2. - 1.,
                bw + 2.,
                bh + 2.,
                rgba(DARK, 0.9),
                3.,
            );
            h.rect(px - bw / 2., py - bh / 2., bw, bh, rgba(face, 1.), 2.);
            let ink = if s.color.iter().sum::<f32>() > 1.6 {
                DARK
            } else {
                [0.98, 0.97, 0.92]
            };
            h.text(&s.text, px, base, size, rgba(ink, 1.), Align::Center, false);
        }
        Style::Bulbs => {
            let (bw, bh) = (w + 3. * pad, gh + 3. * pad);
            let (x0, y0) = (px - bw / 2., py - bh / 2.);
            h.rect(x0, y0, bw, bh, rgba([0.12, 0.1, 0.09], 0.95), 2.);
            // Glühbirnen rundum, jede dritte läuft vorweg
            let step = (size * 0.55).max(3.);
            let (nx, ny) = (
                (bw / step).round().max(2.) as i32,
                (bh / step).round().max(1.) as i32,
            );
            let mut pts = Vec::new();
            for i in 0..nx {
                let x = x0 + bw * i as f32 / nx as f32;
                pts.push((x, y0));
                pts.push((x0 + bw - (x - x0), y0 + bh));
            }
            for j in 0..ny {
                let y = y0 + bh * j as f32 / ny as f32;
                pts.push((x0 + bw, y));
                pts.push((x0, y0 + bh - (y - y0)));
            }
            let chase = (t * 6.).floor() as i64;
            let warm = [1., 0.86, 0.55];
            for (i, (x, y)) in pts.into_iter().enumerate() {
                let hot = (i as i64 + chase).rem_euclid(3) == 0;
                let k = if hot { lit } else { lit * 0.55 };
                let c = mix([0.55, 0.52, 0.45], warm, k);
                if k > 0.05 {
                    h.ellipse(x, y, step * 0.45, step * 0.45, rgba(warm, 0.35 * k));
                }
                h.ellipse(x, y, step * 0.2, step * 0.2, rgba(c, 1.));
            }
            let ink = mix([0.85, 0.8, 0.7], [1., 0.95, 0.85], lit);
            h.text(&s.text, px, base, size, rgba(ink, 1.), Align::Center, false);
        }
        Style::Blade => {
            // Nasenschild: Buchstaben übereinander auf einer schmalen Tafel, die Röhre leuchtet nachts
            let n = s.text.chars().filter(|c| *c != ' ').count().max(1) as f32;
            let lh = gh * 1.25;
            let bw = size * 1.4;
            let bh = n * lh + pad;
            let (x0, y0) = (px - bw / 2., py - bh / 2.);
            h.rect(
                x0 - 1.,
                y0 - 1.,
                bw + 2.,
                bh + 2.,
                rgba(mix(s.color2, DARK, 0.6), 1.),
                2.,
            );
            h.rect(x0, y0, bw, bh, rgba([0.09, 0.09, 0.11], 1.), 2.);
            let mut y = y0 + pad / 2. + gh;
            for ch in s.text.chars().filter(|c| *c != ' ') {
                tube_text(h, &ch.to_string(), px, y, size, s.color, None, lit, 1.);
                y += lh;
            }
        }
        Style::Icon(g) => {
            let gs = gh * 1.9;
            let total = gs + pad + w;
            plate(h, total);
            let gx = px - total / 2. + gs / 2.;
            let core = mix(mix(s.color2, DARK, 0.35), mix(s.color2, WHITE, 0.6), lit);
            if lit > 0.02 {
                glyph(h, g, gx, py, gs * 1.08, rgba(s.color2, 0.35 * lit));
            }
            glyph(h, g, gx, py, gs, rgba(core, 1.));
            tube_text(
                h,
                &s.text,
                px + (gs + pad) / 2.,
                base,
                size,
                s.color,
                None,
                lit,
                1.,
            );
        }
        Style::Board => {
            let (bw, bh) = (w + 2. * pad, gh + 2.2 * pad);
            let (x0, y0) = (px - bw / 2., py - bh / 2.);
            // kleine Lampe über der Tafel strahlt sie abends an
            if glow > 0.02 {
                let k = h.scale;
                h.blob_px(
                    px * k,
                    (y0 + bh * 0.2) * k,
                    bw * 0.75 * k,
                    bh * 1.1 * k,
                    0.,
                    rgba([1., 0.85, 0.6], 0.35 * glow),
                );
            }
            let lamp = 0.35 * glow;
            h.rect(
                x0 - 1.5,
                y0 - 1.5,
                bw + 3.,
                bh + 3.,
                rgba([0.55, 0.4, 0.25], 1.),
                2.,
            );
            h.rect(
                x0,
                y0,
                bw,
                bh,
                rgba(mix(s.color, [1., 0.9, 0.7], lamp * 0.3), 1.),
                1.,
            );
            let chalk = mix([0.86, 0.85, 0.8], [1., 0.96, 0.85], lamp);
            h.text(
                &s.text,
                px,
                base,
                size,
                rgba(chalk, 1.),
                Align::Center,
                false,
            );
        }
    }
}

/// Musterseite: jede Bauart links matt (Tag), rechts leuchtend (Nacht) – zum Prüfen der Formen.
pub fn draw_lab(h: &mut Hud, t: f64) {
    use Glyph::*;
    let vw = h.width;
    h.rect(0., 0., vw / 2., 720., [0.55, 0.58, 0.52, 1.], 0.);
    h.rect(vw / 2., 0., vw / 2., 720., [0.05, 0.06, 0.09, 1.], 0.);
    let rows: [(Style, &str, u32, u32); 12] = [
        (Style::Tube, "ZUR QUELLE", NEON[2], NEON[0]),
        (Style::Tube, "WEISSER HASE", NEON[1], NEON[3]),
        (Style::Frame, "CLUB", NEON[4], NEON[1]),
        (Style::Box, "SPÄTI", BOX[1], NEON[0]),
        (Style::Box, "DÖNER", BOX[2], NEON[0]),
        (Style::Bulbs, "BAR", NEON[6], NEON[0]),
        (Style::Blade, "HOTEL", NEON[5], NEON[3]),
        (Style::Board, "CAFÉ", BOARD[1], NEON[0]),
        (Style::Icon(Cocktail), "COCKTAILS", NEON[0], NEON[1]),
        (Style::Icon(Beer), "KNEIPE", NEON[2], NEON[3]),
        (Style::Icon(Cup), "KAFFEE", NEON[3], NEON[7]),
        (Style::Icon(Note), "TANZBAR", NEON[4], NEON[6]),
    ];
    for (i, (style, text, c1, c2)) in rows.into_iter().enumerate() {
        let spec = Spec {
            text: text.into(),
            style,
            color: rgb(c1),
            color2: rgb(c2),
            scale: 1.,
            flicker: false,
        };
        let (col, row) = (i % 3, i / 3);
        let y = 110. + row as f32 * 160.;
        for night in [false, true] {
            let x0 = if night { vw / 2. } else { 0. };
            let x = x0 + vw / 12. + col as f32 * vw / 6.;
            let k = if night { 1. } else { 0. };
            draw_sign(h, &spec, x, y, 18., k, k, t);
        }
    }
    h.text(
        "Tag (matt)",
        20.,
        30.,
        16.,
        [0.1, 0.1, 0.1, 1.],
        Align::Left,
        false,
    );
    h.text(
        "Nacht (leuchtend)",
        vw / 2. + 20.,
        30.,
        16.,
        [0.9, 0.9, 0.9, 1.],
        Align::Left,
        false,
    );
}

fn h01(s: &str) -> f64 {
    hash01(s.chars().fold(7., |a, c| (a * 31. + c as u32 as f64) % 1e9))
}

/// Schilder im Bildraum (über dem Boden, unter dem HUD).
pub fn draw(neon: &Neon, camera: &Camera, viewport: Vec2, h: &mut Hud) {
    let glow = neon.alpha;
    for s in &neon.signs {
        let p = camera.world_to_screen(Vec2::new(s.x as f32, s.y as f32), 0., viewport) / h.scale;
        let q =
            camera.world_to_screen(Vec2::new(s.x as f32 + 12., s.y as f32), 0., viewport) / h.scale;
        let size = (q - p).length().clamp(7., 24.);
        let lit = if s.on { glow } else { 0. };
        draw_sign(h, &s.spec, p.x, p.y, size, lit, glow, s.t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poi_at(cat: &'static str, kind: &str, name: &str, x: f64, y: f64) -> Poi {
        Poi {
            x,
            y,
            cat,
            name: name.into(),
            kind: kind.into(),
        }
    }
    fn poi(cat: &'static str, kind: &str, name: &str) -> Poi {
        poi_at(cat, kind, name, 1234., 5678.)
    }

    #[test]
    fn only_nightlife_snacks_cafes_and_hotels_get_a_sign() {
        assert!(
            ["SPÄTI", "SPÄTKAUF", "24/7", "KIOSK"].contains(
                &sign_text(&poi("supermarket", "convenience", "Späti 24"))
                    .unwrap()
                    .as_str()
            )
        );
        let imbiss = sign_text(&poi("food", "fast_food", "Curry Eck")).unwrap();
        assert_eq!(imbiss, "CURRYWURST");
        assert_eq!(
            sign_text(&poi("food", "restaurant", "Döner König")).as_deref(),
            Some("DÖNER")
        );
        assert_eq!(
            sign_text(&poi("food", "restaurant", "Pizzeria Roma")).as_deref(),
            Some("PIZZA")
        );
        assert_eq!(
            sign_text(&poi("drink", "nightclub", "")).as_deref(),
            Some("CLUB")
        );
        assert_eq!(
            sign_text(&poi("drink", "nightclub", "Berghain  Panorama Bar")).as_deref(),
            Some("BERGHAIN")
        );
        let hotel = sign_text(&poi("hotel", "hotel", "Adlon")).unwrap();
        assert!(hotel == "HOTEL" || hotel == "ADLON", "{hotel}");
        assert!(sign_text(&poi("cafe", "cafe", "")).is_some());
        // gestrichen: Ärzte, Dienstleister, kleine Läden, Restaurants, Kultur, Haltestellen, Supermärkte
        for (cat, kind) in [
            ("service", "doctors"),
            ("service", "bank"),
            ("shop", "hairdresser"),
            ("shop", "bakery"),
            ("food", "restaurant"),
            ("culture", "cinema"),
            ("bus", ""),
            ("supermarket", "supermarket"),
        ] {
            assert!(sign_text(&poi(cat, kind, "Name")).is_none(), "{cat}/{kind}");
        }
        assert!(!SHOP_GLOW.contains(&"shop") && !SHOP_GLOW.contains(&"service"));
    }

    #[test]
    fn bars_look_different_from_each_other() {
        let mut styles = std::collections::HashSet::new();
        let mut colors = std::collections::HashSet::new();
        let mut glyphs = std::collections::HashSet::new();
        for i in 0..300 {
            let q = poi_at(
                "drink",
                "bar",
                "Zur Quelle",
                1000. + i as f64 * 37.,
                2000. + i as f64 * 11.,
            );
            let s = sign_spec(&q).unwrap();
            assert_eq!(s, sign_spec(&q).unwrap(), "stabil");
            assert_ne!(s.color, s.color2);
            assert!((0.85..=1.3).contains(&s.scale));
            colors.insert(s.color.map(|c| (c * 255.) as u8));
            if let Style::Icon(g) = s.style {
                glyphs.insert(g);
            }
            styles.insert(std::mem::discriminant(&s.style));
        }
        assert!(styles.len() >= 5, "{styles:?}");
        assert!(colors.len() >= 6);
        assert!(glyphs.len() >= 2);
        // lange Namen nie senkrecht
        for i in 0..300 {
            let q = poi_at(
                "hotel",
                "hotel",
                "Grand Hotel Esplanade",
                i as f64 * 13.,
                0.,
            );
            let s = sign_spec(&q).unwrap();
            assert!(
                s.style != Style::Blade || s.text.chars().count() <= 8,
                "{s:?}"
            );
        }
    }

    #[test]
    fn every_style_draws_by_day_and_night() {
        use Glyph::*;
        for style in [
            Style::Tube,
            Style::Frame,
            Style::Box,
            Style::Bulbs,
            Style::Blade,
            Style::Board,
            Style::Icon(Cocktail),
            Style::Icon(Beer),
            Style::Icon(Cup),
            Style::Icon(Note),
            Style::Icon(Arrow),
            Style::Icon(Star),
        ] {
            let spec = Spec {
                text: "ZUR QUELLE".into(),
                style,
                color: rgb(NEON[0]),
                color2: rgb(NEON[1]),
                scale: 1.,
                flicker: false,
            };
            let mut day = Hud::new([1280., 720.]);
            draw_sign(&mut day, &spec, 640., 360., 14., 0., 0., 0.);
            let mut night = Hud::new([1280., 720.]);
            draw_sign(&mut night, &spec, 640., 360., 14., 1., 1., 0.);
            assert!(!day.items.is_empty(), "{style:?}");
            // nachts kommt Schein dazu (Tafeln und Kästen werden heller, Röhren bekommen Halo)
            assert!(
                night.items.len() >= day.items.len(),
                "{style:?}: {} < {}",
                night.items.len(),
                day.items.len()
            );
            assert_ne!(
                day.items.iter().map(|i| i.color).collect::<Vec<_>>(),
                night.items.iter().map(|i| i.color).collect::<Vec<_>>(),
                "{style:?} sieht tags und nachts gleich aus"
            );
        }
    }

    #[test]
    fn names_are_cut_at_word_boundaries() {
        assert_eq!(short("Sahara  Imbiss", 12), "SAHARA");
        assert_eq!(short("Zur Quelle", 12), "ZUR QUELLE");
        assert_eq!(short("Kumpelnestbarundmehr", 8), "KUMPELNE");
        assert_eq!(short("", 8), "");
    }

    #[test]
    fn flicker_is_stable_and_rare() {
        let q = poi("drink", "bar", "A");
        let on = (0..900).filter(|i| neon_on(&q, *i as f64 / 9.)).count();
        assert!(on > 600, "{on}");
        assert!((h(&[1., 2., 3.]) - h(&[1., 2., 3.])).abs() < 1e-15);
    }
}
