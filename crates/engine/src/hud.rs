//! HUD im Bildschirmraum: Rechtecke (abgerundet), Ellipsen, Kreisbögen, Dreiecke und Schriftzeichen als
//! Instanzen einer Pipeline. Zwei Schriften: im Pixel-Modus der gemeinfreie 8×8-Bitmapfont (`font8x8`, Basic Latin
//! und Latin‑1 mit Umlauten und ß), proportional gesetzt (Breite je Zeichen aus der Bitmap; €, → und ✓ sind hier selbst
//! gezeichnet); im HD-Modus Inter SemiBold als Abstandsfeld (SDF, `data/gfx/font/`, erzeugt von
//! `tools/gfx/build_font.py`) mit Laufweiten aus der Schrift – Zeichen, die Inter fehlen, kommen aus der Bitmap.
//! Beide liegen in einem Atlas (SDF oben, Bitmap darunter ab `HUD_BITMAP_Y`). Das Spiel legt die Anzeigen in
//! Basiseinheiten (720 Zeilen hoch) an, `scale` passt sie an die Fensterhöhe an.
use font8x8::legacy::{BASIC_LEGACY, LATIN_LEGACY};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Ein HUD-Element (Bildschirm-Pixel).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudItem {
    pub center: [f32; 2],
    pub half: [f32; 2],
    pub angle: f32,
    /// 0 Rechteck (extra.x = Eckradius), 1 Ellipse, 2 Bogen (extra = a0, a1, Dicke), 3 Bitmapzeichen (extra.x =
    /// Zelle), 4 Dreieck (Spitze nach +x), 6 SDF-Zeichen, 7 dessen Kontur (extra = Atlas-Rechteck x, y, w, h)
    pub shape: f32,
    pub color: [f32; 4],
    pub extra: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub const ATLAS: u32 = 128;
const EXTRA: &[(char, [u8; 8])] = &[
    // € (Bit 0 = linkes Pixel)
    ('€', [0x38, 0x44, 0x1F, 0x04, 0x1F, 0x44, 0x38, 0x00]),
    ('→', [0x00, 0x10, 0x30, 0x7F, 0x30, 0x10, 0x00, 0x00]),
    ('✓', [0x00, 0x40, 0x60, 0x31, 0x1B, 0x0E, 0x04, 0x00]),
    ('▣', [0x7F, 0x41, 0x5D, 0x5D, 0x5D, 0x41, 0x7F, 0x00]),
    ('⚠', [0x08, 0x1C, 0x14, 0x36, 0x22, 0x6B, 0x41, 0x7F]),
    ('☀', [0x49, 0x2A, 0x1C, 0x7F, 0x1C, 0x2A, 0x49, 0x00]),
    ('☾', [0x1C, 0x06, 0x03, 0x03, 0x03, 0x06, 0x1C, 0x00]),
    ('★', [0x08, 0x08, 0x7F, 0x3E, 0x1C, 0x36, 0x22, 0x00]),
    // Treppen zu anderen Ebenen
    ('▲', [0x08, 0x08, 0x1C, 0x1C, 0x3E, 0x3E, 0x7F, 0x00]),
    ('▼', [0x7F, 0x3E, 0x3E, 0x1C, 0x1C, 0x08, 0x08, 0x00]),
    ('↑', [0x08, 0x1C, 0x3E, 0x08, 0x08, 0x08, 0x08, 0x00]),
    ('↓', [0x08, 0x08, 0x08, 0x08, 0x3E, 0x1C, 0x08, 0x00]),
];
/// Ersatz für typografische Zeichen ohne eigene Bitmap.
fn fold(c: char) -> char {
    match c {
        '–' | '—' | '−' => '-',
        '„' | '“' | '”' => '"',
        '‚' | '‘' | '’' => '\'',
        '…' => '.',
        '‹' | '←' => '<',
        '›' => '>',
        _ => c,
    }
}
/// Zelle im Atlas für ein Zeichen.
pub fn cell(c: char) -> Option<u32> {
    let c = fold(c);
    let u = c as u32;
    match u {
        32..=126 => Some(u),
        0xA0..=0xFF => Some(128 + u - 0xA0),
        _ => EXTRA
            .iter()
            .position(|(e, _)| *e == c)
            .map(|i| 224 + i as u32),
    }
}
fn bitmap(cell: u32) -> [u8; 8] {
    match cell {
        0..=127 => BASIC_LEGACY[cell as usize],
        128..=223 => LATIN_LEGACY[(cell - 128) as usize],
        _ => EXTRA
            .get((cell - 224) as usize)
            .map(|e| e.1)
            .unwrap_or([0; 8]),
    }
}
/// Ein Zeichen der SDF-Schrift: Atlas-Rechteck (Pixel), linke obere Ecke relativ zum Ursprung auf der Grundlinie
/// und Vorschub (in em).
#[derive(Debug, Clone, Copy)]
struct Glyph {
    rect: [f32; 4],
    left: f32,
    top: f32,
    adv: f32,
}
struct Sdf {
    /// Atlas-Pixel je em
    em: f32,
    /// Atlas-Pixel von der Kante bis zum vollen Feldwert
    spread: f32,
    width: u32,
    height: u32,
    cap: f32,
    glyphs: HashMap<char, Glyph>,
    px: Vec<u8>,
}
fn sdf() -> &'static Sdf {
    static S: OnceLock<Sdf> = OnceLock::new();
    S.get_or_init(|| {
        let meta: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/gfx/font/hud_sdf.json"))
                .expect("hud_sdf.json");
        let (rgba, width, height) =
            crate::materials::decode(include_bytes!("../../../data/gfx/font/hud_sdf.png"))
                .expect("hud_sdf.png");
        let f = |v: &serde_json::Value| v.as_f64().unwrap_or(0.) as f32;
        let mut glyphs = HashMap::new();
        for (k, v) in meta["zeichen"].as_object().expect("zeichen") {
            let c = char::from_u32(k.parse().expect("Zeichencode")).expect("Zeichen");
            let a: Vec<f32> = v.as_array().expect("Metrik").iter().map(f).collect();
            glyphs.insert(
                c,
                Glyph {
                    rect: [a[0], a[1], a[2], a[3]],
                    left: a[4],
                    top: a[5],
                    adv: a[6],
                },
            );
        }
        Sdf {
            em: f(&meta["em"]),
            spread: f(&meta["spread"]),
            width,
            height,
            cap: f(&meta["cap"]),
            glyphs,
            px: rgba.iter().step_by(4).copied().collect(),
        }
    })
}
/// Schriftgröße der SDF-Schrift in em je Basiseinheit `size`: Großbuchstaben so hoch wie die der Bitmapschrift
/// (7 von 8 Zeilen).
fn sdf_em(size: f32) -> f32 {
    size * 0.875 / sdf().cap
}
/// WGSL-Konstanten der Schrift (Lage der Bitmapzellen im gemeinsamen Atlas).
pub fn shader_constants() -> String {
    format!(
        "const HUD_BITMAP_Y: f32 = {}.0;\nconst HUD_SDF_SPREAD: f32 = {:?};\n",
        sdf().height,
        sdf().spread
    )
}

/// Gemeinsamer Atlas (ein Kanal): SDF-Schrift oben, darunter die 16 × 16 Bitmapzellen à 8 × 8. Liefert Pixel,
/// Breite, Höhe.
pub fn atlas() -> (Vec<u8>, u32, u32) {
    let s = sdf();
    let (w, h) = (s.width.max(ATLAS), s.height + ATLAS);
    let mut out = vec![0u8; (w * h) as usize];
    for y in 0..s.height {
        let row = (y * s.width) as usize;
        out[(y * w) as usize..(y * w + s.width) as usize]
            .copy_from_slice(&s.px[row..row + s.width as usize]);
    }
    let bits = bitmap_atlas();
    for y in 0..ATLAS {
        let src = (y * ATLAS) as usize;
        let dst = ((s.height + y) * w) as usize;
        out[dst..dst + ATLAS as usize].copy_from_slice(&bits[src..src + ATLAS as usize]);
    }
    (out, w, h)
}
/// Bitmapzellen (128 × 128, ein Kanal): 16 × 16 Zellen à 8 × 8.
fn bitmap_atlas() -> Vec<u8> {
    let mut px = vec![0u8; (ATLAS * ATLAS) as usize];
    for c in 0..256 {
        let b = bitmap(c);
        let (cx, cy) = ((c % 16) * 8, (c / 16) * 8);
        for (y, row) in b.iter().enumerate() {
            for x in 0..8 {
                if row & (1 << x) != 0 {
                    px[((cy + y as u32) * ATLAS + cx + x) as usize] = 255;
                }
            }
        }
    }
    px
}
/// Vorschub je Zeichen in Font-Pixeln (proportional: rechtestes gesetztes Pixel + 1 Abstand).
fn advances() -> &'static [u8; 256] {
    static ADV: OnceLock<[u8; 256]> = OnceLock::new();
    ADV.get_or_init(|| {
        let mut a = [0u8; 256];
        for (c, v) in a.iter_mut().enumerate() {
            let b = bitmap(c as u32);
            let mut right = 0;
            let mut left = 8;
            for row in b {
                for x in 0..8 {
                    if row & (1 << x) != 0 {
                        right = right.max(x + 1);
                        left = left.min(x);
                    }
                }
            }
            *v = if right == 0 {
                4
            } else {
                (right - left.min(1)) as u8 + 1
            };
        }
        a
    })
}
/// Linker Leerraum je Zeichen (damit proportional gesetzte Zeichen nicht nach rechts kippen).
fn lefts() -> &'static [u8; 256] {
    static L: OnceLock<[u8; 256]> = OnceLock::new();
    L.get_or_init(|| {
        let mut l = [0u8; 256];
        for (c, v) in l.iter_mut().enumerate() {
            let b = bitmap(c as u32);
            let left = b
                .iter()
                .flat_map(|row| (0..8).filter(move |x| row & (1 << x) != 0))
                .min()
                .unwrap_or(0);
            *v = left.min(1);
        }
        l
    })
}

/// Sammelt die HUD-Elemente eines Bildes. Koordinaten in Basiseinheiten (Fensterhöhe = 720).
/// Kartenausschnitt im HUD (Minikarte): die Kartenmeshes mit eigener Kamera in einem Rechteck des Bildes.
/// HUD-Elemente vor `split` liegen darunter, alle späteren darüber.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapInset {
    /// Rechteck in Bildschirm-Pixeln (x, y, Breite, Höhe)
    pub rect: [f32; 4],
    /// Kartenmitte (Welt-px)
    pub center: [f32; 2],
    /// Weltbreite des Ausschnitts (px)
    pub span: f32,
    pub split: u32,
    /// Stadtplan (`Renderer::set_overview`) statt der Kartenkacheln
    pub overview: bool,
    /// feinere/dickere Linienstufe des Stadtplans
    pub detail: bool,
    /// HUD-Maßstab (Pixel je Basiseinheit) für Linienbreiten
    pub px: f32,
}
pub struct Hud {
    pub items: Vec<HudItem>,
    pub map: Option<MapInset>,
    pub scale: f32,
    /// Breite in Basiseinheiten
    pub width: f32,
    pub height: f32,
    /// SDF-Schrift (HD-Modus) statt der Bitmapschrift
    pub sdf: bool,
}
impl Hud {
    pub fn new(viewport: [f32; 2]) -> Self {
        let scale = (viewport[1] / 720.).max(0.25);
        Self {
            items: Vec::new(),
            map: None,
            scale,
            width: viewport[0] / scale,
            height: 720.,
            sdf: false,
        }
    }
    /// Minikarte in das Rechteck (Basiseinheiten) legen; was danach gezeichnet wird, liegt über der Karte.
    pub fn map_inset(&mut self, x: f32, y: f32, w: f32, h: f32, center: [f32; 2], span: f32) {
        let s = self.scale;
        self.map = Some(MapInset {
            rect: [x * s, y * s, w * s, h * s],
            center,
            span,
            split: self.items.len() as u32,
            overview: false,
            detail: false,
            px: s,
        });
    }
    /// Stadtplan in das Rechteck (Basiseinheiten); `span` = Weltbreite des Rechtecks.
    pub fn overview_inset(&mut self, rect: [f32; 4], center: [f32; 2], span: f32, detail: bool) {
        let [x, y, w, h] = rect;
        self.map_inset(x, y, w, h, center, span);
        if let Some(m) = self.map.as_mut() {
            m.overview = true;
            m.detail = detail;
        }
    }
    /// Strich von (x0, y0) nach (x1, y1) mit runden Enden.
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: [f32; 4]) {
        let s = self.scale;
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = dx.hypot(dy);
        self.items.push(HudItem {
            center: [(x0 + x1) / 2. * s, (y0 + y1) / 2. * s],
            half: [(len + width) / 2. * s, width / 2. * s],
            angle: dy.atan2(dx),
            shape: 0.,
            color,
            extra: [width / 2. * s, 0., 0., 0.],
        });
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4], radius: f32) {
        let s = self.scale;
        self.items.push(HudItem {
            center: [(x + w / 2.) * s, (y + h / 2.) * s],
            half: [w / 2. * s, h / 2. * s],
            angle: 0.,
            shape: 0.,
            color,
            extra: [radius * s, 0., 0., 0.],
        });
    }
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, color: [f32; 4]) {
        let s = self.scale;
        self.items.push(HudItem {
            center: [cx * s, cy * s],
            half: [rx * s, ry * s],
            angle: 0.,
            shape: 1.,
            color,
            extra: [0.; 4],
        });
    }
    /// Gedrehtes Rechteck in Bildschirmpixeln (Mitte, halbe Ausdehnung, Winkel, Eckenradius).
    #[allow(clippy::too_many_arguments)]
    pub fn quad_px(
        &mut self,
        cx: f32,
        cy: f32,
        hw: f32,
        hh: f32,
        angle: f32,
        color: [f32; 4],
        radius: f32,
    ) {
        self.items.push(HudItem {
            center: [cx, cy],
            half: [hw, hh],
            angle,
            shape: 0.,
            color,
            extra: [radius, 0., 0., 0.],
        });
    }
    /// Gedrehte Ellipse in Bildschirmpixeln.
    pub fn ellipse_px(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, angle: f32, color: [f32; 4]) {
        self.items.push(HudItem {
            center: [cx, cy],
            half: [rx, ry],
            angle,
            shape: 1.,
            color,
            extra: [0.; 4],
        });
    }
    /// Weicher Fleck in Bildschirmpixeln (Wolkenschatten, Nebel): Deckkraft fällt zum Rand auf 0.
    pub fn blob_px(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, angle: f32, color: [f32; 4]) {
        self.items.push(HudItem {
            center: [cx, cy],
            half: [rx, ry],
            angle,
            shape: 5.,
            color,
            extra: [0.; 4],
        });
    }
    /// Kreisbogen von a0 bis a1 (rad, im Uhrzeigersinn ab +x), Radius r, Dicke.
    #[allow(clippy::too_many_arguments)]
    pub fn arc(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        thickness: f32,
        a0: f32,
        a1: f32,
        color: [f32; 4],
    ) {
        let s = self.scale;
        let e = r + thickness;
        self.items.push(HudItem {
            center: [cx * s, cy * s],
            half: [e * s, e * s],
            angle: 0.,
            shape: 2.,
            color,
            extra: [a0, a1, r * s, thickness * s],
        });
    }
    /// Dreieck (Pfeil) mit Spitze in Richtung `angle`.
    pub fn triangle(&mut self, cx: f32, cy: f32, size: f32, angle: f32, color: [f32; 4]) {
        let s = self.scale;
        self.items.push(HudItem {
            center: [cx * s, cy * s],
            half: [size * s, size * 0.75 * s],
            angle,
            shape: 4.,
            color,
            extra: [0.; 4],
        });
    }
    fn px(&self, size: f32) -> f32 {
        // ganzzahlige Pixelgröße für eine scharfe Bitmapschrift
        (size * self.scale / 8.).round().max(1.)
    }
    /// Breite eines Texts in Basiseinheiten.
    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        if self.sdf {
            return self.sdf_width(text, size);
        }
        let adv = advances();
        let w: u32 = text
            .chars()
            .filter_map(cell)
            .map(|c| adv[c as usize] as u32)
            .sum();
        w as f32 * self.px(size) / self.scale
    }
    /// Text mit Grundlinie bei y (Basiseinheiten); `outline` = dunkle Kontur für Lesbarkeit ohne Kasten.
    /// Liefert die Breite.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
        align: Align,
        outline: bool,
    ) -> f32 {
        let w = self.text_width(text, size);
        if self.sdf {
            let x0 = match align {
                Align::Left => x,
                Align::Center => x - w / 2.,
                Align::Right => x - w,
            } * self.scale;
            let base = y * self.scale;
            self.sdf_line(text, (x0, base), size, 0., color, outline);
            return w;
        }
        let p = self.px(size);
        let x0 = match align {
            Align::Left => x,
            Align::Center => x - w / 2.,
            Align::Right => x - w,
        } * self.scale;
        let top = y * self.scale - 7. * p;
        let (adv, lefts) = (advances(), lefts());
        let shadow = [0.02, 0.02, 0.03, color[3] * 0.85];
        let offsets: &[(f32, f32)] = if outline {
            &[(-1., 0.), (1., 0.), (0., -1.), (0., 1.), (1., 1.)]
        } else {
            &[]
        };
        for &(ox, oy) in offsets.iter() {
            self.glyph_run(
                text,
                x0.round() + ox * p,
                top.round() + oy * p,
                p,
                shadow,
                adv,
                lefts,
            );
        }
        self.glyph_run(text, x0.round(), top.round(), p, color, adv, lefts);
        w
    }
    /// Gedrehter Text, mittig um (cx, cy) in Basiseinheiten (Straßennamen entlang der Straße); `outline` = Kontur.
    #[allow(clippy::too_many_arguments)]
    pub fn text_rotated(
        &mut self,
        text: &str,
        cx: f32,
        cy: f32,
        size: f32,
        angle: f32,
        color: [f32; 4],
        outline: bool,
    ) {
        let p = self.px(size);
        let w = self.text_width(text, size) * self.scale;
        if self.sdf {
            // Grundlinie so, dass die Großbuchstaben mittig auf (cx, cy) liegen
            let (ca, sa) = (angle.cos(), angle.sin());
            let cap = 0.875 * size * self.scale;
            let (bx, by) = (-w / 2., cap / 2.);
            let start = (
                cx * self.scale + ca * bx - sa * by,
                cy * self.scale + sa * bx + ca * by,
            );
            self.sdf_line(text, start, size, angle, color, outline);
            return;
        }
        let (adv, lefts) = (advances(), lefts());
        let (ca, sa) = (angle.cos(), angle.sin());
        let (dir, perp) = ((ca, sa), (-sa, ca));
        let c = (cx * self.scale, cy * self.scale);
        let shadow = [0.02, 0.02, 0.03, color[3] * 0.85];
        let mut runs: Vec<((f32, f32), [f32; 4])> = Vec::new();
        if outline {
            for o in [(-1., 0.), (1., 0.), (0., -1.), (0., 1.), (1., 1.)] {
                runs.push((o, shadow));
            }
        }
        runs.push(((0., 0.), color));
        for ((ox, oy), col) in runs {
            let mut pen = -w / 2.;
            for ch in text.chars().filter_map(cell) {
                if ch != 32 {
                    // Glyphmitte im Textsystem: entlang (Schrift-Vorlauf) und quer (Mitte der Zelle auf der Linie)
                    let along = pen - lefts[ch as usize] as f32 * p + 4. * p + ox * p;
                    let across = -0.5 * p + oy * p;
                    self.items.push(HudItem {
                        center: [
                            c.0 + dir.0 * along + perp.0 * across,
                            c.1 + dir.1 * along + perp.1 * across,
                        ],
                        half: [4. * p, 4. * p],
                        angle,
                        shape: 3.,
                        color: col,
                        extra: [ch as f32, 0., 0., 0.],
                    });
                }
                pen += adv[ch as usize] as f32 * p;
            }
        }
    }
    /// Breite in Basiseinheiten mit der SDF-Schrift (fehlende Zeichen mit der Bitmap-Laufweite).
    fn sdf_width(&self, text: &str, size: f32) -> f32 {
        let s = sdf();
        let em = sdf_em(size) * self.scale;
        let (p, adv) = (self.px(size), advances());
        let w: f32 = text
            .chars()
            .map(|c| match s.glyphs.get(&c) {
                Some(g) => g.adv * em,
                None => cell(c).map_or(0., |k| adv[k as usize] as f32 * p),
            })
            .sum();
        w / self.scale
    }
    /// Eine Zeile SDF-Text ab `start` (Bildschirm-Pixel, Ursprung auf der Grundlinie) in Richtung `angle`. Kontur =
    /// dieselben Zeichen, dicker und dunkel, vorher. Zeichen, die Inter fehlt, als Bitmapzeichen.
    fn sdf_line(
        &mut self,
        text: &str,
        start: (f32, f32),
        size: f32,
        angle: f32,
        color: [f32; 4],
        outline: bool,
    ) {
        let s = sdf();
        let em = sdf_em(size) * self.scale;
        let k = em / s.em; // Bildschirm-Pixel je Atlas-Pixel
        let (p, adv, lefts) = (self.px(size), advances(), lefts());
        let (ca, sa) = (angle.cos(), angle.sin());
        let at = |along: f32, across: f32| {
            [
                start.0 + ca * along - sa * across,
                start.1 + sa * along + ca * across,
            ]
        };
        let shadow = [0.02, 0.02, 0.03, color[3] * 0.85];
        let passes: &[(f32, [f32; 4])] = if outline {
            &[(7., shadow), (6., color)]
        } else {
            &[(6., color)]
        };
        for &(shape, col) in passes {
            let mut pen = 0.;
            for c in text.chars() {
                if let Some(g) = s.glyphs.get(&c) {
                    if g.rect[2] > 0. {
                        let (w, h) = (g.rect[2] * k, g.rect[3] * k);
                        self.items.push(HudItem {
                            center: at(pen + g.left * em + w / 2., g.top * em + h / 2.),
                            half: [w / 2., h / 2.],
                            angle,
                            shape,
                            color: col,
                            extra: g.rect,
                        });
                    }
                    pen += g.adv * em;
                } else if let Some(cl) = cell(c) {
                    // Bitmapzeichen auf derselben Grundlinie (Zeile 7 von 8)
                    if shape == 6. && cl != 32 {
                        let along = pen - lefts[cl as usize] as f32 * p + 4. * p;
                        self.items.push(HudItem {
                            center: at(along, -3. * p),
                            half: [4. * p, 4. * p],
                            angle,
                            shape: 3.,
                            color: col,
                            extra: [cl as f32, 0., 0., 0.],
                        });
                    }
                    pen += adv[cl as usize] as f32 * p;
                }
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn glyph_run(
        &mut self,
        text: &str,
        x: f32,
        top: f32,
        p: f32,
        color: [f32; 4],
        adv: &[u8; 256],
        lefts: &[u8; 256],
    ) {
        let mut pen = x;
        for c in text.chars().filter_map(cell) {
            if c != 32 {
                let gx = pen - lefts[c as usize] as f32 * p;
                self.items.push(HudItem {
                    center: [gx + 4. * p, top + 4. * p],
                    half: [4. * p, 4. * p],
                    angle: 0.,
                    shape: 3.,
                    color,
                    extra: [c as f32, 0., 0., 0.],
                });
            }
            pen += adv[c as usize] as f32 * p;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cells_cover_german_text() {
        for c in "Kisten für den Kiez – Größe +/− ‹›←  ÄÖÜäöüß 12:30 € → ✓ ▲▼↑↓".chars()
        {
            assert!(cell(c).is_some(), "Zeichen {c:?} fehlt");
        }
        assert_eq!(cell('A'), Some(65));
        assert_eq!(cell('ä'), Some(128 + 0xE4 - 0xA0));
        assert!(cell('漢').is_none());
        let a = bitmap_atlas();
        assert_eq!(a.len(), 128 * 128);
        // 'A' hat gesetzte Pixel in seiner Zelle, das Leerzeichen keine
        let filled = |c: u32| {
            (0..64)
                .filter(|i| a[(((c / 16) * 8 + i / 8) * 128 + (c % 16) * 8 + i % 8) as usize] > 0)
                .count()
        };
        assert!(filled(65) > 10 && filled(32) == 0 && filled(224) > 10);
    }
    #[test]
    fn proportional_widths_and_alignment() {
        let mut h = Hud::new([1280., 720.]);
        assert!(h.text_width("iii", 16.) < h.text_width("WWW", 16.));
        assert_eq!(h.text_width("", 16.), 0.);
        let w = h.text("Hallo", 640., 100., 16., [1.; 4], Align::Center, false);
        let xs: Vec<f32> = h.items.iter().map(|i| i.center[0]).collect();
        let (min, max) = (
            xs.iter().cloned().fold(f32::MAX, f32::min),
            xs.iter().cloned().fold(f32::MIN, f32::max),
        );
        assert!((min - (640. - w / 2.)).abs() < 12. && max < 640. + w / 2.);
        h.items.clear();
        h.text("ab", 0., 0., 16., [1.; 4], Align::Left, true);
        assert_eq!(h.items.len(), 2 * 6, "Kontur (5×) + Zeichen");
        // doppelt so hohes Fenster → doppelt so große Elemente
        let mut big = Hud::new([2560., 1440.]);
        big.rect(10., 10., 100., 20., [1.; 4], 0.);
        assert_eq!(big.items[0].half, [100., 20.]);
    }
    #[test]
    fn sdf_font_metrics_and_atlas() {
        let s = sdf();
        // alle deutschen Zeichen in Inter, nur einzelne Eigenzeichen fallen auf die Bitmap zurück
        for c in "Kisten für den Kiez – Größe ÄÖÜäöüß 12:30 € → ✓ ▲▼↑↓".chars()
        {
            assert!(s.glyphs.contains_key(&c), "{c:?} fehlt in der SDF-Schrift");
        }
        assert!(s.cap > 0.6 && s.cap < 0.8 && s.em > 16.);
        let (px, w, h) = atlas();
        assert_eq!(px.len(), (w * h) as usize);
        assert_eq!(h, s.height + ATLAS);
        assert!(shader_constants().contains(&format!("HUD_BITMAP_Y: f32 = {}.0", s.height)));
        // Bitmap liegt unter dem SDF-Teil: 'A' (Zelle 65) hat dort Pixel
        let filled = (0..64)
            .filter(|i| px[((s.height + 32 + i / 8) * w + 8 + i % 8) as usize] > 0)
            .count();
        assert!(filled > 10);
    }
    #[test]
    fn sdf_text_uses_font_advances_and_falls_back() {
        let mut h = Hud::new([1280., 720.]);
        h.sdf = true;
        assert!(h.text_width("iii", 16.) < h.text_width("WWW", 16.));
        let w = h.text("Hallo", 100., 100., 16., [1.; 4], Align::Left, true);
        assert!((w - h.text_width("Hallo", 16.)).abs() < 1e-4);
        assert_eq!(h.items.iter().filter(|i| i.shape == 6.).count(), 5);
        assert_eq!(h.items.iter().filter(|i| i.shape == 7.).count(), 5);
        // Großbuchstabe sitzt auf der Grundlinie und ist so hoch wie die Bitmap-Großbuchstaben (7/8 der Größe)
        let hh = h.items.iter().find(|i| i.shape == 6.).unwrap();
        let bottom = hh.center[1] + hh.half[1];
        assert!((bottom - 100.).abs() < 3., "Grundlinie {bottom}");
        // ☾ fehlt Inter: Bitmapzeichen
        h.items.clear();
        h.text("☾", 0., 50., 16., [1.; 4], Align::Left, false);
        assert_eq!(h.items[0].shape, 3.);
    }
}
