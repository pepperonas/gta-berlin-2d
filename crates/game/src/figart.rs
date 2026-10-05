//! Figurenteile in Draufsicht (Grafik-Überarbeitung Phase 6, Variante B): gemalte Rümpfe, Köpfe, Frisuren,
//! Kopfbedeckungen, Taschen, Kinderwagen und Hund statt flacher Ellipsen. Wie bei den Fahrzeugen (`carart.rs`) zwei
//! Ebenen: die **Lackebene** trägt nur die Schattierung (Falten, Strähnen, Wölbung; R = (s+1)/2), dazu Glanz (G) und
//! Material (B) – die Farbe der einzelnen Person setzt der Shader ein; die **Detailebene** trägt Feststehendes
//! (Räder, Schnallen, Hundenase). Jedes Teil liegt in einer Teilzelle eines eigenen Zellenpaars im Fahrzeugatlas
//! (`vehatlas::FIG_*`). Gangbild und Farben bleiben in `figure.rs`; die Teile werden wie bisher verschoben.
//!
//! Zeichenraum je Teil: u ∈ [−1, 1] nach vorn, v ∈ [−1, 1] nach rechts; die Zeichnung reicht bis ±1 und belegt
//! `FIG_ART` der Teilzelle. Köpfe und alles darauf haben den Kopf mit Radius `HEAD_R`, damit Frisuren, Krempen und
//! langes Haar Platz haben.
use crate::raster::{Canvas, Path, Pt};
use berlin_engine::vehatlas::{CELL_H, CELL_W, FIG_ART, FIG_CELL, FIG_COLS, FIG_PARTS};

/// Kopfradius im Zeichenraum der Kopfteile
pub const HEAD_R: f32 = 0.62;

/// Teile (Index = Teilzelle)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    TorsoSlim,
    TorsoMid,
    TorsoBroad,
    Head,
    HairShort,
    HairLong,
    HairBun,
    HairCurly,
    HairMohawk,
    HairPony,
    HatCap,
    HatBeanie,
    HatHard,
    HatScarf,
    HatSun,
    Bag,
    Backpack,
    Briefcase,
    Shopping,
    Stroller,
    Dog,
}
pub const PARTS: [Part; 21] = [
    Part::TorsoSlim,
    Part::TorsoMid,
    Part::TorsoBroad,
    Part::Head,
    Part::HairShort,
    Part::HairLong,
    Part::HairBun,
    Part::HairCurly,
    Part::HairMohawk,
    Part::HairPony,
    Part::HatCap,
    Part::HatBeanie,
    Part::HatHard,
    Part::HatScarf,
    Part::HatSun,
    Part::Bag,
    Part::Backpack,
    Part::Briefcase,
    Part::Shopping,
    Part::Stroller,
    Part::Dog,
];

/// Zwei Ebenen eines Teils, 64 × 64 Bildpunkte
struct Layers {
    paint: Canvas,
    detail: Canvas,
}
impl Layers {
    fn new() -> Self {
        let n = FIG_CELL as f32;
        // Zeichenraum ±1 auf die inneren FIG_ART der Zelle
        let scale = n / 2. * FIG_ART;
        let origin = 1. / FIG_ART;
        Self {
            paint: Canvas::new(FIG_CELL, FIG_CELL, (scale, scale), (origin, origin)),
            detail: Canvas::new(FIG_CELL, FIG_CELL, (scale, scale), (origin, origin)),
        }
    }
    /// Fläche in Personenfarbe mit Schattierung s (−1 … 1), Glanz und Material; verdeckt Details darunter.
    fn paint(&mut self, path: &Path, s: impl Fn(Pt) -> f32, gloss: f32) {
        let cov = self.paint.coverage(path);
        self.detail.erase(&cov, 1.);
        for (k, c) in cov {
            let (x, y) = (k % self.paint.w, k / self.paint.w);
            let v = ((s(self.paint.to_model(x, y)) + 1.) / 2.).clamp(0., 1.);
            let d = &mut self.paint.px[k];
            for (ch, t) in d[..3].iter_mut().zip([v, gloss, 0.]) {
                *ch = t * c + *ch * (1. - c);
            }
            d[3] = c + d[3] * (1. - c);
        }
    }
    /// Linie in Personenfarbe (Naht, Strähne, Falte) mit fester Schattierung
    fn crease(&mut self, pts: &[Pt], s: f32, w: f32, gloss: f32) {
        self.paint(&Path::stroke(pts, w), |_| s, gloss);
    }
    fn fixed(&mut self, path: &Path, c: [f32; 4]) {
        self.detail.fill(path, |_| c);
    }
}

/// Superellipse |u/a|^n + |v/b|^n = 1 als Polygon
fn superellipse(a: f32, b: f32, n: f32, cu: f32, cv: f32) -> Path {
    let pts: Vec<Pt> = (0..48)
        .map(|i| {
            let t = i as f32 / 48. * std::f32::consts::TAU;
            let (c, s) = (t.cos(), t.sin());
            (
                cu + a * c.signum() * c.abs().powf(2. / n),
                cv + b * s.signum() * s.abs().powf(2. / n),
            )
        })
        .collect();
    Path::poly(&pts)
}
/// Kreis mit gewelltem Rand (Locken, Fell)
fn lumpy(r: f32, lobes: f32, amp: f32, cu: f32, cv: f32) -> Path {
    let pts: Vec<Pt> = (0..64)
        .map(|i| {
            let t = i as f32 / 64. * std::f32::consts::TAU;
            let rr = r * (1. + amp * (t * lobes).sin());
            (cu + rr * t.cos(), cv + rr * t.sin())
        })
        .collect();
    Path::poly(&pts)
}
/// Wölbung: Mitte hell, Rand dunkel (Radius r um (cu, cv))
fn dome(r: f32, cu: f32, cv: f32, k: f32) -> impl Fn(Pt) -> f32 {
    move |(u, v)| {
        let d = (((u - cu).powi(2) + (v - cv).powi(2)).sqrt() / r).min(1.);
        k * (0.35 - 0.9 * d * d)
    }
}

const CLOTH: f32 = 0.12;
const HAIR: f32 = 0.3;
const SKIN: f32 = 0.18;

fn torso(l: &mut Layers, width: f32, depth: f32) {
    // Schultern quer (v), Brust/Rücken längs (u); flacher Bauch, Schultern gerundet
    let body = superellipse(depth, width, 2.6, 0., 0.);
    l.paint(
        &body,
        move |(u, v)| {
            let across = (v / width).abs();
            let along = (u / depth).abs();
            0.25 - 0.55 * across.powi(4) - 0.35 * along.powi(3) - 0.1 * (u / depth).min(0.)
        },
        CLOTH,
    );
    // Schulternähte und Ausschnitt vorn, Rückennaht hinten
    for side in [-1f32, 1.] {
        l.crease(
            &[
                (-0.5 * depth, side * 0.62 * width),
                (0.5 * depth, side * 0.6 * width),
            ],
            -0.45,
            0.06,
            CLOTH,
        );
        // Falten zur Armbeuge
        l.crease(
            &[
                (0.15 * depth, side * 0.8 * width),
                (0.45 * depth, side * 0.55 * width),
            ],
            -0.3,
            0.045,
            CLOTH,
        );
    }
    l.crease(
        &[(-0.95 * depth, 0.), (-0.2 * depth, 0.)],
        -0.35,
        0.05,
        CLOTH,
    );
    l.paint(
        &Path::poly(&[
            (0.55 * depth, -0.28),
            (0.98 * depth, -0.3),
            (0.98 * depth, 0.3),
            (0.55 * depth, 0.28),
            (0.78 * depth, 0.),
        ]),
        |_| -0.5,
        CLOTH,
    );
}

fn head(l: &mut Layers) {
    let r = HEAD_R;
    for side in [-1f32, 1.] {
        l.paint(
            &Path::circle(0.02, side * r * 1.02, r * 0.2),
            |_| -0.25,
            SKIN,
        );
    }
    l.paint(&Path::circle(0.05, 0., r), dome(r, 0.05, 0., 0.9), SKIN);
    // Nase und Augenbrauen-Schatten vorn
    l.paint(&Path::circle(0.05 + r * 0.95, 0., r * 0.16), |_| 0.1, SKIN);
    l.crease(
        &[(r * 0.55, -r * 0.45), (r * 0.62, -r * 0.12)],
        -0.4,
        0.05,
        SKIN,
    );
    l.crease(
        &[(r * 0.55, r * 0.45), (r * 0.62, r * 0.12)],
        -0.4,
        0.05,
        SKIN,
    );
}

/// Haarkappe: Kreis etwas nach hinten, Haaransatz vorn als Bogen, Strähnen vom Wirbel aus
fn hair_cap(l: &mut Layers, r: f32, back: f32, strands: usize) {
    let cu = -back;
    let mut ring = Vec::new();
    for i in 0..=48 {
        let t = i as f32 / 48. * std::f32::consts::TAU;
        let (u, v) = (cu + r * t.cos(), r * t.sin());
        // vorn eine Stirn frei lassen: Haaransatz als Bogen
        let line = r * 0.42 + 0.18 * (v / r).powi(2);
        ring.push((u.min(line), v));
    }
    l.paint(&Path::poly(&ring), dome(r, cu - 0.05, 0., 1.), HAIR);
    let whorl = (cu - r * 0.35, r * 0.1);
    for i in 0..strands {
        let t = i as f32 / strands as f32 * std::f32::consts::TAU + 0.3;
        let end = (cu + r * 0.92 * t.cos(), r * 0.92 * t.sin());
        if end.0 > r * 0.45 {
            continue;
        }
        let mid = (
            (whorl.0 + end.0) / 2. + 0.05 * t.sin(),
            (whorl.1 + end.1) / 2. + 0.05 * t.cos(),
        );
        l.crease(&[whorl, mid, end], -0.2, 0.03, HAIR);
    }
}

fn paint_part(p: Part) -> Layers {
    let mut l = Layers::new();
    let r = HEAD_R;
    match p {
        Part::TorsoSlim => torso(&mut l, 0.86, 0.62),
        Part::TorsoMid => torso(&mut l, 0.93, 0.68),
        Part::TorsoBroad => torso(&mut l, 1.0, 0.74),
        Part::Head => head(&mut l),
        Part::HairShort => hair_cap(&mut l, r * 0.98, 0.08, 14),
        Part::HairLong => {
            // über Schultern und Rücken fallend
            l.paint(
                &superellipse(0.36, r * 1.25, 2.4, -r * 0.95, 0.),
                dome(0.8, -r, 0., 0.8),
                HAIR,
            );
            for i in -3..=3 {
                let v = i as f32 * 0.16;
                l.crease(
                    &[(-r * 0.6, v * 0.7), (-r * 1.4, v * 0.95)],
                    -0.35,
                    0.035,
                    HAIR,
                );
            }
            hair_cap(&mut l, r * 1.02, 0.06, 16);
        }
        Part::HairBun => {
            hair_cap(&mut l, r * 0.98, 0.08, 12);
            l.paint(
                &Path::circle(-r * 1.15, 0., r * 0.38),
                dome(r * 0.38, -r * 1.15, 0., 1.),
                HAIR,
            );
            l.crease(
                &[(-r * 0.85, -r * 0.25), (-r * 0.85, r * 0.25)],
                -0.5,
                0.05,
                HAIR,
            );
        }
        Part::HairCurly => {
            l.paint(
                &lumpy(r * 1.12, 11., 0.07, -0.06, 0.),
                dome(r * 1.1, -0.06, 0., 1.),
                HAIR,
            );
            for i in 0..22 {
                let t = i as f32 * 2.39996;
                let d = r * 0.95 * ((i as f32 + 0.5) / 22.).sqrt();
                l.paint(
                    &Path::circle(-0.06 + d * t.cos(), d * t.sin(), r * 0.13),
                    |_| -0.3,
                    HAIR,
                );
            }
        }
        Part::HairMohawk => {
            l.paint(
                &superellipse(r * 1.05, r * 0.2, 2.2, -0.05, 0.),
                |(u, _)| 0.3 - 0.4 * (u / r).abs(),
                HAIR,
            );
            for i in 0..8 {
                let u = -r * 0.9 + i as f32 * r * 0.26;
                l.crease(&[(u, -r * 0.15), (u + r * 0.1, r * 0.15)], -0.5, 0.04, HAIR);
            }
        }
        Part::HairPony => {
            l.paint(
                &superellipse(r * 0.4, r * 0.22, 2.2, -r * 1.15, 0.),
                dome(r * 0.4, -r * 1.15, 0., 0.9),
                HAIR,
            );
            hair_cap(&mut l, r * 0.98, 0.08, 12);
            l.crease(
                &[(-r * 0.95, -r * 0.12), (-r * 0.95, r * 0.12)],
                -0.6,
                0.07,
                HAIR,
            );
        }
        Part::HatCap => {
            // Schirm nach vorn, darauf die Kappe mit Nähten und Knopf
            l.paint(
                &superellipse(r * 0.55, r * 0.78, 2.2, r * 0.85, 0.),
                |(u, _)| 0.05 - 0.25 * (u / r - 0.9).abs(),
                CLOTH,
            );
            l.paint(
                &Path::circle(-0.04, 0., r * 1.02),
                dome(r, -0.04, 0., 1.),
                CLOTH,
            );
            for i in 0..6 {
                let t = i as f32 / 6. * std::f32::consts::TAU;
                l.crease(
                    &[(-0.04, 0.), (-0.04 + r * t.cos(), r * t.sin())],
                    -0.3,
                    0.035,
                    CLOTH,
                );
            }
            l.paint(&Path::circle(-0.04, 0., r * 0.1), |_| 0.4, CLOTH);
        }
        Part::HatBeanie => {
            l.paint(
                &Path::circle(-0.03, 0., r * 1.04),
                dome(r * 1.04, -0.03, 0., 1.),
                CLOTH,
            );
            // Rippen der Umschlagkante
            for i in 0..36 {
                let t = i as f32 / 36. * std::f32::consts::TAU;
                l.crease(
                    &[
                        (-0.03 + r * 0.8 * t.cos(), r * 0.8 * t.sin()),
                        (-0.03 + r * 1.0 * t.cos(), r * 1.0 * t.sin()),
                    ],
                    -0.35,
                    0.03,
                    CLOTH,
                );
            }
            l.paint(&Path::circle(-0.03, 0., r * 0.22), |_| 0.25, CLOTH);
        }
        Part::HatHard => {
            l.paint(
                &Path::circle(0., 0., r * 1.2),
                |(u, v)| -0.35 + 0.1 * (u / r) - 0.05 * (v / r).abs(),
                0.6,
            );
            l.paint(
                &Path::circle(-0.02, 0., r * 0.98),
                dome(r, -0.02, 0., 1.2),
                0.85,
            );
            l.crease(&[(-r * 0.95, 0.), (r * 0.95, 0.)], 0.45, 0.12, 0.85);
        }
        Part::HatScarf => {
            // Tuch um den Kopf, hinten über die Schultern gelegt, mit Falten
            l.paint(
                &superellipse(r * 0.7, r * 1.2, 2.3, -r * 0.8, 0.),
                dome(r * 1.2, -r * 0.8, 0., 0.8),
                CLOTH,
            );
            l.paint(
                &Path::circle(0., 0., r * 1.05),
                dome(r * 1.05, 0., 0., 1.),
                CLOTH,
            );
            for side in [-1f32, 1.] {
                for k in 0..3 {
                    let v = side * r * (0.45 + 0.2 * k as f32);
                    l.crease(
                        &[(r * 0.3, v * 0.9), (-r * 1.3, v * 1.2)],
                        -0.35,
                        0.035,
                        CLOTH,
                    );
                }
            }
            // Gesicht frei
            l.paint(
                &superellipse(r * 0.28, r * 0.48, 2.4, r * 0.82, 0.),
                |_| -0.75,
                CLOTH,
            );
        }
        Part::HatSun => {
            l.paint(
                &Path::circle(0., 0., 0.98),
                |(u, v)| {
                    let d = (u * u + v * v).sqrt();
                    0.15 - 0.45 * d + 0.08 * (d * 30.).sin()
                },
                CLOTH,
            );
            l.paint(
                &Path::circle(0., 0., r * 0.82),
                dome(r * 0.82, 0., 0., 1.),
                CLOTH,
            );
            l.paint(&Path::circle(0., 0., r * 0.86), |_| -0.55, CLOTH);
            l.paint(
                &Path::circle(0., 0., r * 0.76),
                dome(r * 0.76, -0.05, 0., 1.),
                CLOTH,
            );
        }
        Part::Bag => {
            l.paint(
                &Path::rrect(-0.85, -0.95, 1.7, 1.9, 0.35),
                |(u, _)| 0.1 - 0.3 * u.abs(),
                0.25,
            );
            l.crease(&[(-0.2, -0.9), (-0.2, 0.9)], -0.5, 0.08, 0.25);
            l.fixed(
                &Path::rrect(0.05, -0.2, 0.3, 0.4, 0.08),
                [0.72, 0.68, 0.55, 1.],
            );
        }
        Part::Backpack => {
            l.paint(
                &Path::rrect(-0.95, -0.9, 1.9, 1.8, 0.45),
                |(u, v)| 0.2 - 0.3 * (u * u + v * v),
                CLOTH,
            );
            l.paint(&Path::rrect(-0.85, -0.55, 0.7, 1.1, 0.25), |_| -0.15, CLOTH);
            l.crease(&[(-0.5, -0.5), (-0.5, 0.5)], -0.5, 0.06, CLOTH);
            for side in [-1f32, 1.] {
                l.fixed(
                    &Path::rrect(0.55, side * 0.55 - 0.12, 0.45, 0.24, 0.06),
                    [0.12, 0.12, 0.13, 1.],
                );
            }
        }
        Part::Briefcase => {
            l.paint(
                &Path::rrect(-0.95, -0.85, 1.9, 1.7, 0.18),
                |(u, v)| 0.15 - 0.25 * (u.abs() + v.abs()),
                0.5,
            );
            l.crease(&[(-0.9, 0.), (0.9, 0.)], -0.45, 0.06, 0.5);
            for side in [-1f32, 1.] {
                l.fixed(
                    &Path::rrect(side * 0.5 - 0.1, -0.12, 0.2, 0.24, 0.04),
                    [0.78, 0.74, 0.6, 1.],
                );
            }
        }
        Part::Shopping => {
            l.paint(
                &Path::rrect(-0.9, -0.9, 1.8, 1.8, 0.1),
                |(u, v)| 0.1 - 0.2 * (u.abs() + v.abs()) + 0.1 * (v * 9.).sin(),
                0.08,
            );
            l.crease(&[(-0.85, -0.3), (0.85, -0.3)], -0.4, 0.06, 0.08);
            l.crease(&[(-0.85, 0.3), (0.85, 0.3)], -0.4, 0.06, 0.08);
        }
        Part::Stroller => {
            // Räder an den Ecken (fest), Wanne in Wagenfarbe, Verdeck hinten mit Bügeln, Schiebebügel vorn hell
            for (u, v) in [(-0.62, -0.85), (-0.62, 0.85), (0.62, -0.85), (0.62, 0.85)] {
                l.fixed(
                    &Path::rrect(u - 0.18, v - 0.1, 0.36, 0.2, 0.08),
                    [0.1, 0.1, 0.11, 1.],
                );
            }
            l.paint(
                &Path::rrect(-0.85, -0.7, 1.7, 1.4, 0.5),
                |(u, v)| 0.2 - 0.35 * (u * u + v * v),
                0.45,
            );
            l.paint(
                &superellipse(0.48, 0.66, 2.5, -0.35, 0.),
                dome(0.66, -0.4, 0., 1.1),
                0.3,
            );
            for k in 0..3 {
                let u = -0.65 + k as f32 * 0.22;
                l.crease(&[(u, -0.62), (u + 0.04, 0.62)], -0.45, 0.04, 0.3);
            }
            l.paint(&Path::rrect(0.15, -0.45, 0.6, 0.9, 0.3), |_| -0.45, 0.2);
        }
        Part::Dog => {
            // Rumpf, Kopf vorn mit Ohren, Nase fest
            l.paint(
                &superellipse(0.62, 0.42, 2.3, -0.2, 0.),
                |(u, v)| 0.25 - 0.6 * (v / 0.42).powi(2) - 0.2 * ((u + 0.2) / 0.62).powi(2),
                0.1,
            );
            l.crease(&[(-0.75, 0.), (0.3, 0.)], 0.25, 0.08, 0.1);
            for side in [-1f32, 1.] {
                l.paint(&Path::circle(0.55, side * 0.27, 0.14), |_| -0.45, 0.1);
            }
            l.paint(&Path::circle(0.62, 0., 0.27), dome(0.27, 0.62, 0., 1.), 0.1);
            l.paint(&superellipse(0.16, 0.13, 2.2, 0.86, 0.), |_| 0.15, 0.1);
            l.fixed(&Path::circle(0.98, 0., 0.06), [0.05, 0.05, 0.06, 1.]);
        }
    }
    l
}

/// Lack- und Detailzelle des Figurenpaars (je CELL_W × CELL_H, RGBA8 gerade Farbe).
pub fn cells() -> (Vec<u8>, Vec<u8>) {
    let mut paint = vec![0u8; CELL_W * CELL_H * 4];
    let mut detail = vec![0u8; CELL_W * CELL_H * 4];
    for (i, p) in PARTS.iter().enumerate() {
        let l = paint_part(*p);
        let (cx, cy) = (
            (i % FIG_COLS as usize) * FIG_CELL,
            (i / FIG_COLS as usize) * FIG_CELL,
        );
        for (dst, src) in [
            (&mut paint, l.paint.rgba8()),
            (&mut detail, l.detail.rgba8()),
        ] {
            for y in 0..FIG_CELL {
                let o = ((cy + y) * CELL_W + cx) * 4;
                dst[o..o + FIG_CELL * 4]
                    .copy_from_slice(&src[y * FIG_CELL * 4..(y + 1) * FIG_CELL * 4]);
            }
        }
    }
    (paint, detail)
}

/// Body-Form eines Teils (`vehatlas::FIG_BASE` + Paar · Teile + Index).
pub fn shape(p: Part) -> f32 {
    let i = PARTS.iter().position(|q| *q == p).unwrap_or(0) as u32;
    (berlin_engine::vehatlas::FIG_BASE + crate::carart::figure_pair() as u32 * FIG_PARTS + i) as f32
}
/// Halbe Body-Größe, damit die Zeichnung (±1 im Zeichenraum) `half` Weltpixel belegt.
pub fn half(half: [f32; 2]) -> [f32; 2] {
    [half[0] / FIG_ART, half[1] / FIG_ART]
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Vorschau der Teile, eingefärbt, als PPM: `GTA_FIG_DUMP=x.ppm cargo test -p gta-berlin dump_figures -- --ignored`
    #[test]
    #[ignore]
    fn dump_figures() {
        let Ok(path) = std::env::var("GTA_FIG_DUMP") else {
            return;
        };
        let (paint, detail) = cells();
        let tint = [0.75f32, 0.45, 0.25];
        let mut f = format!("P6\n{} {}\n255\n", CELL_W * 2, CELL_H * 2).into_bytes();
        for y in 0..CELL_H * 2 {
            for x in 0..CELL_W * 2 {
                let i = ((y / 2) * CELL_W + x / 2) * 4;
                let (ba, da) = (paint[i + 3] as f32 / 255., detail[i + 3] as f32 / 255.);
                let s = paint[i] as f32 / 255. * 2. - 1.;
                let mut c = [0.35f32, 0.42, 0.33];
                for k in 0..3 {
                    let v = if s >= 0. {
                        tint[k] + (1. - tint[k]) * s
                    } else {
                        tint[k] * (1. + s)
                    };
                    c[k] = c[k] * (1. - ba) + v * ba;
                    c[k] = c[k] * (1. - da) + detail[i + k] as f32 / 255. * da;
                }
                f.extend(c.map(|v| (v.clamp(0., 1.) * 255.) as u8));
            }
        }
        std::fs::write(path, f).unwrap();
    }
    #[test]
    fn every_part_is_painted_inside_its_cell() {
        assert!(PARTS.len() as u32 <= FIG_PARTS);
        let (paint, detail) = cells();
        for (i, p) in PARTS.iter().enumerate() {
            let (cx, cy) = (
                (i % FIG_COLS as usize) * FIG_CELL,
                (i / FIG_COLS as usize) * FIG_CELL,
            );
            let a = |x: usize, y: usize| {
                paint[((cy + y) * CELL_W + cx + x) * 4 + 3]
                    .max(detail[((cy + y) * CELL_W + cx + x) * 4 + 3])
            };
            let n = (0..FIG_CELL * FIG_CELL)
                .filter(|k| a(k % FIG_CELL, k / FIG_CELL) > 128)
                .count();
            assert!(n > 150, "{p:?}: nur {n} Bildpunkte");
            // Rand der Teilzelle frei (2 px): Mip-Stufen bluten sonst in den Nachbarn
            for t in 0..FIG_CELL {
                for (x, y) in [(t, 0), (t, 1), (t, FIG_CELL - 1), (0, t), (FIG_CELL - 1, t)] {
                    assert!(a(x, y) < 8, "{p:?} randet bei ({x}, {y})");
                }
            }
        }
    }
    #[test]
    fn shading_varies_and_hair_is_glossier_than_cloth() {
        let (paint, _) = cells();
        let at = |i: usize, x: usize, y: usize| {
            let (cx, cy) = (
                (i % FIG_COLS as usize) * FIG_CELL,
                (i / FIG_COLS as usize) * FIG_CELL,
            );
            &paint[((cy + y) * CELL_W + cx + x) * 4..((cy + y) * CELL_W + cx + x) * 4 + 4]
        };
        let c = FIG_CELL / 2;
        let torso = PARTS.iter().position(|p| *p == Part::TorsoMid).unwrap();
        let hair = PARTS.iter().position(|p| *p == Part::HairShort).unwrap();
        // Schattierung: Mitte heller als Schulterkante
        assert!(at(torso, c, c)[0] > at(torso, c, FIG_CELL / 8 + 3)[0]);
        assert!(at(hair, c - 4, c)[1] > at(torso, c, c)[1]);
    }
}
