//! Fahrzeugbilder in Draufsicht (Port von `vehicleart.js paintPassenger`/`paintLamps` und `vehicles.js paintSpecial`),
//! einmal beim Start in einen Atlas gerastert. Je Modell zwei Zellen: die **Lackebene** trägt nur, wie stark der Lack
//! an jeder Stelle aufgehellt oder abgedunkelt ist (`assets.js shade(farbe, s)`, gespeichert als (s+1)/2) – die Farbe
//! des einzelnen Autos setzt der Shader ein; die **Detailebene** trägt alles mit fester Farbe (Scheiben, Leuchten,
//! Gummi, Chrom) und liegt darüber. Lack, der später gemalt wird, deckt die Details darunter ab.
use crate::raster::{Canvas, Path, Pt};

/// Zelle im Atlas (Pixel) und Raster: 8 Zellen nebeneinander.
pub const CELL_W: usize = 256;
pub const CELL_H: usize = 128;
pub const COLS: usize = 8;
/// Rand um das Fahrzeug (Einheiten), damit Spiegel und Reserverad Platz haben
pub const PAD: f32 = 4.;

const SPORT: &[&str] = &[
    "sportwagen",
    "supersport",
    "leichtbau",
    "heckcoupe",
    "gtcoupe",
    "roadster",
    "leichtcoupe",
    "coupe",
    "elektrosport",
];
const CLASSIC: &[&str] = &[
    "oldtimer",
    "zweitakter",
    "kleinbus",
    "niva",
    "gklasse",
    "defender",
    "musclecar",
];
fn two_door(m: &str) -> bool {
    SPORT.contains(&m) || matches!(m, "musclecar" | "zweitakter" | "niva")
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Shape {
    pub len: f32,
    pub inset: f32,
    pub r: f32,
    pub back: f32,
    pub front: f32,
    pub stripes: bool,
    pub vents: Option<&'static str>,
    pub glass_roof: bool,
    pub rails: bool,
    pub spare: bool,
    pub open: bool,
    pub bed: bool,
    pub van: bool,
    pub wing: bool,
    pub chrome: bool,
    pub two_tone: Option<u32>,
    pub roof: Option<u32>,
}
const fn sh(len: f32, inset: f32, r: f32, back: f32, front: f32) -> Shape {
    Shape {
        len,
        inset,
        r,
        back,
        front,
        stripes: false,
        vents: None,
        glass_roof: false,
        rails: false,
        spare: false,
        open: false,
        bed: false,
        van: false,
        wing: false,
        chrome: false,
        two_tone: None,
        roof: None,
    }
}
/// `vehicles.js SHAPES`
pub fn shape(model: &str) -> Shape {
    let b = sh;
    match model {
        "kleinwagen" => b(6., 0., 5.5, 6., 9.),
        "kompakt" => b(3., 0., 5.5, 6., 10.),
        "kombi" => b(0., 0., 5.5, 5., 12.),
        "elektro" => Shape {
            glass_roof: true,
            ..b(0., 0., 6., 6., 11.)
        },
        "gelaende" => Shape {
            rails: true,
            spare: true,
            ..b(0., 0., 3., 4., 11.)
        },
        "sportwagen" => Shape {
            vents: Some("mid"),
            ..b(0., 0., 7., 17., 14.)
        },
        "heckcoupe" => Shape {
            vents: Some("rear"),
            ..b(2., 0.5, 8., 11., 13.)
        },
        "zweitakter" => Shape {
            roof: Some(0xecebe4),
            ..b(9., 1., 7., 6., 9.)
        },
        "hothatch" => Shape {
            stripes: true,
            vents: Some("hood"),
            ..b(3., 0., 5., 6., 10.)
        },
        "roadster" => Shape {
            open: true,
            ..b(5., 0.5, 7., 12., 15.)
        },
        "musclecar" => Shape {
            stripes: true,
            vents: Some("hood"),
            ..b(0., 0., 4., 11., 17.)
        },
        "oldtimer" => Shape {
            chrome: true,
            ..b(0., 0.5, 8., 11., 13.)
        },
        "pickup" => Shape {
            bed: true,
            ..b(0., 0., 3.5, 18., 11.)
        },
        "kleinbus" => Shape {
            two_tone: Some(0xefeee8),
            ..b(3., 0., 6., 3., 5.)
        },
        "rallye" => Shape {
            wing: true,
            vents: Some("hood"),
            ..b(2., 0., 5., 6., 10.)
        },
        "transporter" => Shape {
            van: true,
            ..b(0., 0., 3., 0., 0.)
        },
        "supersport" => Shape {
            vents: Some("mid"),
            ..b(0., 0., 7., 18., 15.)
        },
        "gtcoupe" => b(0., 0., 7., 8., 19.),
        "leichtbau" => Shape {
            vents: Some("mid"),
            ..b(4., 0.5, 7., 15., 12.)
        },
        "elektrosport" => Shape {
            glass_roof: true,
            ..b(0., 0., 7., 9., 13.)
        },
        "sprinter" => Shape {
            van: true,
            ..b(0., 0., 3., 0., 0.)
        },
        "hochdach" => Shape {
            rails: true,
            ..b(2., 0., 4., 3., 9.)
        },
        "powerkombi" => Shape {
            vents: Some("hood"),
            rails: true,
            ..b(0., 0., 5., 5., 12.)
        },
        "familienkombi" => Shape {
            rails: true,
            ..b(0., 0., 5.5, 5., 12.)
        },
        "business" => b(0., 0., 5.5, 9., 13.),
        "sportlimo" => Shape {
            vents: Some("hood"),
            ..b(0., 0., 5., 9., 13.)
        },
        "luxus" => Shape {
            chrome: true,
            ..b(0., 0., 6., 10., 13.)
        },
        "coupe" => b(3., 0., 6., 10., 13.),
        "leichtcoupe" => b(3., 0.5, 6.5, 10., 14.),
        "gklasse" => Shape {
            spare: true,
            rails: true,
            ..b(1., 0., 1.5, 4., 10.)
        },
        "defender" => Shape {
            rails: true,
            spare: true,
            two_tone: Some(0xf2f2ee),
            ..b(0., 0., 2.5, 3., 10.)
        },
        "niva" => Shape {
            rails: true,
            ..b(6., 0.5, 3., 5., 9.)
        },
        "kompaktsuv" => Shape {
            rails: true,
            ..b(2., 0., 5., 5., 10.)
        },
        "sportsuv" => Shape {
            rails: true,
            ..b(0., 0., 6., 5., 11.)
        },
        "grosssuv" => Shape {
            rails: true,
            glass_roof: true,
            ..b(0., 0., 5., 5., 11.)
        },
        _ => b(0., 0., 5.5, 9., 12.), // limousine, taxi, police
    }
}

/// Sonderfahrzeuge mit eigenem Aufbau
pub const SPECIAL: &[&str] = &["truck", "delivery", "garbage", "police", "ambulance", "bus"];

fn hex(c: u32) -> [f32; 4] {
    [
        ((c >> 16) & 255) as f32 / 255.,
        ((c >> 8) & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
        1.,
    ]
}
fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255., g as f32 / 255., b as f32 / 255., a]
}
/// `assets.js shade` für feste Farben
fn shade_fixed(c: u32, f: f32) -> [f32; 4] {
    let mut v = hex(c);
    for x in &mut v[..3] {
        *x = if f < 0. {
            *x * (1. + f)
        } else {
            *x + (1. - *x) * f
        };
    }
    v
}
/// Lackverlauf quer über die Breite (`finish`): Schattierungen je Anteil
const FINISH: [(f32, f32); 7] = [
    (0., -0.48),
    (0.12, -0.12),
    (0.26, 0.28),
    (0.44, 0.08),
    (0.72, -0.08),
    (0.94, -0.36),
    (1., -0.55),
];
fn finish_at(y: f32, y0: f32, w: f32) -> f32 {
    let t = ((y - y0) / w).clamp(0., 1.);
    for i in 1..FINISH.len() {
        if t <= FINISH[i].0 {
            let (a, b) = (FINISH[i - 1], FINISH[i]);
            return a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0);
        }
    }
    FINISH[FINISH.len() - 1].1
}

/// Zwei Ebenen eines Fahrzeugs
pub struct Art {
    pub body: Canvas,
    pub detail: Canvas,
}
impl Art {
    pub fn new(l: f32, w: f32) -> Self {
        let (cw, ch) = (l + 2. * PAD, w + 2. * PAD);
        let scale = (CELL_W as f32 / cw, CELL_H as f32 / ch);
        let origin = (cw / 2., ch / 2.);
        Self {
            body: Canvas::new(CELL_W, CELL_H, scale, origin),
            detail: Canvas::new(CELL_W, CELL_H, scale, origin),
        }
    }
    /// Lack mit Schattierung s (Funktion des Ortes); verdeckt Details darunter.
    fn lack(&mut self, path: &Path, s: impl Fn(Pt) -> f32) {
        self.lack_a(path, s, 1.);
    }
    fn lack_a(&mut self, path: &Path, s: impl Fn(Pt) -> f32, a: f32) {
        let cov = self.body.coverage(path);
        self.detail.erase(&cov, a);
        for (k, c) in cov {
            let (x, y) = (k % self.body.w, k / self.body.w);
            let v = ((s(self.body.to_model(x, y)) + 1.) / 2.).clamp(0., 1.);
            let al = c * a;
            let d = &mut self.body.px[k];
            for x in &mut d[..3] {
                *x = v * al + *x * (1. - al);
            }
            d[3] = al + d[3] * (1. - al);
        }
    }
    fn fix(&mut self, path: &Path, c: [f32; 4]) {
        self.detail.fill(path, |_| c);
    }
    fn fix_fn(&mut self, path: &Path, f: impl Fn(Pt) -> [f32; 4]) {
        self.detail.fill(path, f);
    }
    fn line(&mut self, pts: &[Pt], c: [f32; 4], w: f32) {
        self.detail.fill(&Path::stroke(pts, w.max(0.3)), |_| c);
    }
    fn line_lack(&mut self, pts: &[Pt], s: f32, w: f32) {
        self.lack(&Path::stroke(pts, w.max(0.3)), |_| s);
    }
    fn bx(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: [f32; 4]) {
        if w > 0. && h > 0. {
            self.fix(&Path::rrect(x, y, w, h, r), c);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn bx_stroke(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        fill: [f32; 4],
        stroke: [f32; 4],
    ) {
        if w > 0. && h > 0. {
            let p = Path::rrect(x, y, w, h, r);
            self.fix(&p, fill);
            let ring = &p.rings[0];
            let mut pts = ring.clone();
            pts.push(ring[0]);
            self.line(&pts, stroke, 0.35);
        }
    }
}

fn outline(l: f32, w: f32, boxy: bool, sporty: bool) -> Path {
    let end = if boxy {
        0.43
    } else if sporty {
        0.29
    } else {
        0.34
    };
    let waist = if sporty { 0.425 } else { 0.47 };
    let mut p = Path::new();
    p.move_to((-l / 2., -w * end))
        .cubic_to(
            (-l * 0.48, -w * 0.48),
            (-l * 0.34, -w * 0.51),
            (-l * 0.25, -w * 0.5),
        )
        .cubic_to(
            (-l * 0.1, -w * waist),
            (l * 0.1, -w * waist),
            (l * 0.27, -w * 0.5),
        )
        .cubic_to(
            (l * 0.4, -w * 0.5),
            (l * 0.5, -w * 0.43),
            (l / 2., -w * end),
        )
        .line_to((l / 2., w * end))
        .cubic_to((l * 0.5, w * 0.43), (l * 0.4, w * 0.5), (l * 0.27, w * 0.5))
        .cubic_to(
            (l * 0.1, w * waist),
            (-l * 0.1, w * waist),
            (-l * 0.25, w * 0.5),
        )
        .cubic_to(
            (-l * 0.34, w * 0.51),
            (-l * 0.48, w * 0.48),
            (-l / 2., w * end),
        );
    p
}

/// Glas: dunkler Verlauf schräg über die Kabine mit hellem Band
fn glass(ca: f32, cb: f32, outer: f32) -> impl Fn(Pt) -> [f32; 4] {
    move |(x, y)| {
        let (dx, dy) = (cb - ca, 2. * outer);
        let t = (((x - ca) * dx + (y + outer) * dy) / (dx * dx + dy * dy)).clamp(0., 1.);
        let stops = [
            (0., [0.067, 0.114, 0.145]),
            (0.42, [0.275, 0.38, 0.443]),
            (0.53, [0.149, 0.224, 0.267]),
            (1., [0.063, 0.106, 0.137]),
        ];
        for i in 1..stops.len() {
            if t <= stops[i].0 {
                let (a, b) = (stops[i - 1], stops[i]);
                let k = (t - a.0) / (b.0 - a.0);
                return [
                    a.1[0] + (b.1[0] - a.1[0]) * k,
                    a.1[1] + (b.1[1] - a.1[1]) * k,
                    a.1[2] + (b.1[2] - a.1[2]) * k,
                    1.,
                ];
            }
        }
        [0.063, 0.106, 0.137, 1.]
    }
}

/// Pkw (`paintPassenger`): Umriss mit Taille, Lack mit Glanzband, Kabine aus Front-, Heck- und Seitenscheiben, Dach,
/// Türfugen, Griffe, Spiegel, Stoßfänger, Leuchten und Modellmerkmale.
pub fn paint_passenger(a: &mut Art, model: &str, l: f32, w: f32, s: &Shape) {
    let sport = SPORT.contains(&model);
    let classic = CLASSIC.contains(&model);
    let boxy = s.r <= 3.5 || s.van;
    let (rear, front) = (-l / 2., l / 2.);
    let body = outline(l, w, boxy, sport);
    a.lack(&body, |(_, y)| finish_at(y, -w / 2., w));
    // Kontur
    let mut ring = body.rings[0].clone();
    ring.push(ring[0]);
    a.line_lack(&ring, -0.65, 0.65);
    for side in [-1f32, 1.] {
        let pts = [
            (rear + 3., side * w * 0.4),
            (-l * 0.22, side * w * 0.445),
            (l * 0.23, side * w * 0.445),
            (front - 3., side * w * 0.37),
        ];
        if side < 0. {
            a.line(&pts, rgba(255, 255, 255, 0.42), 0.55);
        } else {
            a.line(&pts, rgba(0, 0, 0, 0.32), 0.55);
        }
        for ax in [-l * 0.29, l * 0.29] {
            let c = if side < 0. {
                rgba(255, 255, 255, 0.2)
            } else {
                rgba(0, 0, 0, 0.2)
            };
            a.line(
                &[
                    (ax - 3., side * w * 0.47),
                    (ax - 1., side * w * 0.405),
                    (ax + 3., side * w * 0.42),
                ],
                c,
                0.65,
            );
        }
        a.bx(
            -l * 0.18,
            if side < 0. { -w * 0.495 } else { w * 0.46 },
            l * 0.35,
            0.5,
            0.2,
            hex(0x24282b),
        );
    }
    if s.stripes {
        for y in [-2.15, 0.75] {
            a.bx(rear + 2., y, l - 4., 1.4, 0., hex(0xd5d4cb));
        }
    }
    let (mut cr, mut cf) = ((-0.5 + s.back / l).max(-0.39), 0.5 - s.front / l);
    if s.vents == Some("mid") {
        (cr, cf) = (-0.16, 0.26);
    }
    if model == "gtcoupe" || model == "musclecar" {
        (cr, cf) = (-0.32, 0.12);
    }
    if s.open {
        (cr, cf) = (-0.25, 0.14);
    }
    if s.bed {
        (cr, cf) = (-0.05, 0.29);
    }
    if s.van {
        (cr, cf) = (-0.43, 0.36);
    }
    let (ca, cb) = (cr * l, cf * l);
    let span = cb - ca;
    let ra = ca
        + span
            * if s.van {
                0.05
            } else if s.rails {
                0.12
            } else {
                0.23
            };
    let rb = cb - span * if s.van { 0.18 } else { 0.27 };
    let outer = w * 0.385;
    let roof_half = w * if sport { 0.265 } else { 0.29 };
    let gl = glass(ca, cb, outer);
    let window = |a: &mut Art, pts: &[Pt]| {
        let p = Path::poly(pts);
        a.fix_fn(&p, &gl);
        // Spiegelung: schräger heller Streifen
        let refl = Path::poly(&[(ca - 4., -w), (ca + 1., -w), (cb + 5., w), (cb + 2., w)]);
        let cov_w = a.detail.coverage(&p);
        let cov_r: std::collections::HashMap<usize, f32> =
            a.detail.coverage(&refl).into_iter().collect();
        for (k, c) in cov_w {
            if let Some(r) = cov_r.get(&k) {
                let al = 0.17 * c.min(*r);
                let d = &mut a.detail.px[k];
                let col = [0.816, 0.91, 0.937];
                for i in 0..3 {
                    d[i] = col[i] * al + d[i] * (1. - al);
                }
            }
        }
        let mut ring = pts.to_vec();
        ring.push(pts[0]);
        a.line(&ring, hex(0x101619), 0.35);
    };
    if s.bed {
        a.bx_stroke(
            rear + 2.,
            -w * 0.36,
            ca - rear - 2.8,
            w * 0.72,
            1.,
            hex(0x292d30),
            hex(0x898b88),
        );
        let mut y = -w * 0.25;
        while y < w * 0.3 {
            a.line(&[(rear + 3., y), (ca - 2., y)], hex(0x51575a), 0.45);
            y += 1.6;
        }
        for side in [-1f32, 1.] {
            a.bx(
                rear + 6.,
                if side < 0. { -w * 0.37 } else { w * 0.26 },
                5.5,
                w * 0.12,
                1.,
                hex(0x414548),
            );
        }
    }
    window(
        a,
        &[
            (ca, -outer * 0.79),
            (ra, -roof_half),
            (ra, roof_half),
            (ca, outer * 0.79),
        ],
    );
    window(
        a,
        &[
            (rb, -roof_half),
            (cb, -outer * 0.84),
            (cb, outer * 0.84),
            (rb, roof_half),
        ],
    );
    for side in [-1f32, 1.] {
        window(
            a,
            &[
                (ca + 0.7, side * outer),
                (cb - 1., side * outer),
                (rb, side * (roof_half + 0.55)),
                (ra, side * (roof_half + 0.55)),
            ],
        );
        if !two_door(model) && !s.bed && !s.van {
            let x = ra + (rb - ra) * 0.53;
            a.line(
                &[(x, side * roof_half), (x, side * outer)],
                hex(0x171c20),
                0.9,
            );
        }
        let doors: Vec<f32> = if two_door(model) || s.bed {
            vec![ca + 1., cb]
        } else {
            vec![ca + 1., (ra + rb) / 2., cb]
        };
        for &x in &doors {
            a.line_lack(
                &[(x, side * outer), (x - 0.4, side * w * 0.46)],
                -0.48,
                0.35,
            );
        }
        for &x in &doors[..doors.len() - 1] {
            let y = if side < 0. { -w * 0.433 } else { w * 0.411 };
            let p = Path::rrect(x + 1.1, y, 1.9, 0.45, 0.2);
            if classic || s.chrome {
                a.fix(&p, hex(0xc5c8c6));
            } else {
                a.lack(&p, |_| 0.3);
            }
        }
    }
    if s.open {
        a.bx(
            ra - 0.9,
            -roof_half,
            rb - ra + 1.4,
            roof_half * 2.,
            1.2,
            hex(0x141b1e),
        );
        for side in [-1f32, 1.] {
            a.bx_stroke(
                ra + 0.4,
                side * w * 0.15 - 1.7,
                4.2,
                3.4,
                0.85,
                hex(0x865641),
                hex(0xb98260),
            );
            a.bx(
                ra + 0.3,
                side * w * 0.15 - 1.35,
                1.,
                2.7,
                0.35,
                hex(0x382b27),
            );
            a.line(
                &[(ra - 0.7, side * w * 0.1), (ra - 0.7, side * w * 0.23)],
                hex(0xb1b7b9),
                0.65,
            );
        }
    } else {
        let roof = Path::rrect(
            ra,
            -roof_half,
            rb - ra,
            roof_half * 2.,
            if sport { 1.3 } else { 0.8 },
        );
        if s.glass_roof {
            a.fix_fn(&roof, &gl);
        } else if let Some(c) = s.two_tone.or(s.roof) {
            a.fix_fn(&roof, |(_, y)| {
                shade_fixed(c, finish_at(y, -roof_half, roof_half * 2.))
            });
        } else {
            a.lack(&roof, |(_, y)| finish_at(y, -roof_half, roof_half * 2.));
        }
        let mut ring = roof.rings[0].clone();
        ring.push(ring[0]);
        a.line(&ring, rgba(12, 19, 22, 0.65), 0.35);
        a.line(
            &[(ra + 0.9, -roof_half + 0.65), (rb - 0.9, -roof_half + 0.65)],
            rgba(255, 255, 255, 0.35),
            0.35,
        );
        if s.stripes {
            for y in [-2.15, 0.75] {
                a.bx(ra + 0.2, y, rb - ra - 0.4, 1.4, 0., hex(0xd5d4cb));
            }
        }
        if s.glass_roof {
            let x = (ra + rb) / 2.;
            a.line(&[(x, -roof_half), (x, roof_half)], hex(0x11181c), 0.7);
        }
        if s.rails {
            for side in [-1f32, 1.] {
                a.line(
                    &[
                        (ra, side * (roof_half - 0.2)),
                        (rb, side * (roof_half - 0.2)),
                    ],
                    hex(0x20282c),
                    1.,
                );
                a.line(
                    &[
                        (ra + 1., side * (roof_half - 0.35)),
                        (rb - 1., side * (roof_half - 0.35)),
                    ],
                    hex(0xafb7b6),
                    0.35,
                );
            }
        }
        if s.van {
            let mut y = -roof_half + 2.;
            while y < roof_half - 1. {
                a.line(&[(ra + 2., y), (rb - 2., y)], rgba(0, 0, 0, 0.17), 0.35);
                y += 2.2;
            }
        }
    }
    // Scheibenwischer, Haubenspalte
    for side in [-1f32, 1.] {
        a.line(
            &[(rb + 0.65, side * w * 0.06), (rb + 1.1, side * w * 0.23)],
            hex(0x10181c),
            0.45,
        );
        a.line(
            &[
                (cb + 0.8, side * outer * 0.85),
                (front - 3., side * w * 0.28),
            ],
            rgba(0, 0, 0, 0.3),
            0.4,
        );
    }
    a.line_lack(
        &[(cb + 0.7, -outer * 0.83), (cb + 0.7, outer * 0.83)],
        -0.3,
        0.35,
    );
    if let Some(v) = s.vents {
        let (x0, x1) = if v == "hood" {
            (cb + 2., front - 5.)
        } else {
            (rear + 3., ca - 1.)
        };
        if x1 > x0 {
            a.bx(x0, -w * 0.2, x1 - x0, w * 0.4, 0.6, hex(0x242b2d));
            let mut x = x0 + 0.8;
            while x < x1 {
                a.line(&[(x, -w * 0.17), (x, w * 0.17)], hex(0x667072), 0.35);
                x += 1.3;
            }
        }
    }
    // Stoßfänger, Grill, Kennzeichen, Endrohre
    a.bx(
        front - 1.7,
        -w * 0.24,
        1.15,
        w * 0.48,
        0.3,
        hex(if classic { 0xbbc1c0 } else { 0x1b242a }),
    );
    a.bx(
        rear + 0.3,
        -w * 0.26,
        1.,
        w * 0.52,
        0.3,
        hex(if s.chrome { 0xc8cdcb } else { 0x272c2d }),
    );
    for x in [rear + 0.4, front - 0.85] {
        a.bx(x, -1.55, 0.65, 3.1, 0.1, hex(0xd5d9d4));
        a.bx(x, -1.55, 0.65, 0.5, 0., hex(0x2f5780));
    }
    if model != "elektro" && model != "elektrosport" {
        let sides: &[f32] = if sport || model == "musclecar" {
            &[-1., 1.]
        } else {
            &[1.]
        };
        for &side in sides {
            a.bx_stroke(
                rear,
                side * w * 0.25 - 0.65,
                1.5,
                1.3,
                0.5,
                hex(0xa7b0b1),
                hex(0x172024),
            );
        }
    }
    // Spiegel an den A-Säulen
    for side in [-1f32, 1.] {
        a.line(
            &[(cb - 1.5, side * w * 0.4), (cb - 2., side * w * 0.54)],
            hex(0x20272b),
            0.7,
        );
        let p = Path::rrect(
            cb - 3.,
            if side < 0. { -w * 0.57 } else { w * 0.48 },
            2.4,
            w * 0.09,
            0.55,
        );
        a.lack(&p, |_| -0.1);
        a.line(
            &[(cb - 2.8, side * w * 0.53), (cb - 1.2, side * w * 0.53)],
            hex(0xb1c4ca),
            0.4,
        );
    }
    if s.wing || model == "supersport" {
        a.bx(rear + 2.2, -w * 0.28, 1.2, w * 0.56, 0.1, hex(0x1b2227));
        let p = Path::rrect(rear + 1.5, -w * 0.43, 1.5, w * 0.86, 0.45);
        a.lack(&p, |(_, y)| finish_at(y, -w / 2., w));
    }
    if s.spare {
        a.bx_stroke(rear - 1.7, -2.7, 2.8, 5.4, 1., hex(0x1b2023), hex(0x616668));
        a.bx(rear - 1.2, -1.6, 1.4, 3.2, 0.6, hex(0x666d6b));
    }
    if model == "taxi" {
        let x = (ra + rb) / 2.;
        a.bx_stroke(x - 1.2, -3.2, 2.4, 6.4, 0.55, hex(0xeccc69), hex(0x554a2c));
        a.line(&[(x, -1.8), (x, 1.8)], hex(0x443b24), 0.5);
    }
    paint_lamps(a, model, l, w);
}

/// Scheinwerfer und Rückleuchten (ruhend; Bremslicht und Blinker zeichnet das Spiel darüber)
pub fn paint_lamps(a: &mut Art, model: &str, l: f32, w: f32) {
    let classic = CLASSIC.contains(&model);
    let sport = SPORT.contains(&model);
    let (nose, rear) = (l / 2., -l / 2.);
    for side in [-1f32, 1.] {
        let y = side * w * 0.3;
        a.bx(
            nose - 3.4,
            y - 1.65,
            2.7,
            3.3,
            if classic { 1.3 } else { 0.6 },
            hex(0x151e24),
        );
        if classic {
            let dys: &[f32] = if model == "musclecar" {
                &[-0.8, 0.8]
            } else {
                &[0.]
            };
            for &dy in dys {
                let r = if model == "musclecar" { 0.65 } else { 1.1 };
                a.fix(&Path::circle(nose - 2.05, y + dy, r), hex(0xe5ddba));
            }
        } else {
            a.fix(
                &Path::poly(&[
                    (nose - 3., y - 1.15),
                    (nose - 1.3, y - 0.95),
                    (nose - 1.1, y + 1.25),
                    (nose - if sport { 2.2 } else { 3. }, y + 0.9),
                ]),
                hex(0xa9c0c8),
            );
            a.line(
                &[
                    (nose - 1.45, y - 1.05),
                    (nose - 1.3, y + 0.9),
                    (nose - 2.2, y + 1.05),
                ],
                hex(0xf2f8ed),
                0.45,
            );
        }
        a.bx(rear + 0.5, y - 1.6, 1.6, 3.2, 0.45, hex(0x271d21));
        if model == "musclecar" {
            for i in -1..=1 {
                a.bx(
                    rear + 0.85,
                    y + i as f32 * 0.9 - 0.26,
                    0.9,
                    0.52,
                    0.1,
                    hex(0xb53131),
                );
            }
        } else {
            a.bx(rear + 0.85, y - 1.3, 0.8, 2.6, 0.25, hex(0xb53131));
        }
    }
    if model == "elektro" || model == "elektrosport" {
        a.line(
            &[(rear + 1.2, -w * 0.24), (rear + 1.2, w * 0.24)],
            hex(0x8f2529),
            0.5,
        );
    }
}

/// Nutzfahrzeuge und Einsatzwagen (`paintSpecial`): Fahrerhaus vorn, Aufbau dahinter.
pub fn paint_special(a: &mut Art, model: &str, l: f32, w: f32) {
    let (x0, y0) = (-l / 2., -w / 2.);
    let glass = hex(0x233140);
    // Kasten mit Licht von oben (Verlauf über die Breite), Kontur
    let boxf = |a: &mut Art, x: f32, len: f32, s0: Option<u32>, r: f32| {
        let p = Path::rrect(x, y0 + 0.6, len, w - 1.2, r);
        let light = move |y: f32| {
            let t = ((y - y0) / w).clamp(0., 1.);
            if t < 0.22 {
                -0.32 + (0.23 + 0.32) * t / 0.22
            } else if t < 0.55 {
                0.23 * (1. - (t - 0.22) / 0.33)
            } else {
                -0.38 * (t - 0.55) / 0.45
            }
        };
        match s0 {
            None => a.lack(&p, move |(_, y)| light(y)),
            Some(c) => a.fix_fn(&p, move |(_, y)| shade_fixed(c, light(y))),
        }
        let mut ring = p.rings[0].clone();
        ring.push(ring[0]);
        a.line(&ring, hex(0x303a3d), 0.6);
    };
    let ribs = |a: &mut Art, xa: f32, xb: f32, step: f32, c: [f32; 4]| {
        let mut x = xa;
        while x < xb {
            a.line(&[(x, y0 + 2.), (x, y0 + w - 2.)], c, 0.7);
            x += step;
        }
    };
    let cabin = |a: &mut Art, xs: f32, len: f32| {
        boxf(a, xs, len, None, 3.5);
        a.bx(xs + len - 5., y0 + 2.4, 3.2, w - 4.8, 0., glass);
        a.lack(&Path::rrect(xs + 2., y0 + 3., len - 8., w - 6., 0.), |_| {
            0.12
        });
    };
    match model {
        "truck" => {
            let cab = 15.;
            cabin(a, l / 2. - cab, cab);
            boxf(a, x0, l - cab - 2., Some(0xdcdad4), 1.5);
            ribs(a, x0 + 6., l / 2. - cab - 4., 7., rgba(0, 0, 0, 0.12));
        }
        "delivery" => {
            let cab = 12.;
            boxf(a, x0, l, None, 3.);
            a.bx(l / 2. - 5., y0 + 2.4, 3., w - 4.8, 0., glass);
            a.lack(
                &Path::rrect(x0 + 1.5, y0 + 1.8, l - cab - 3., w - 3.6, 1.5),
                |_| 0.1,
            );
            a.lack(
                &Path::rrect(x0 + 3., y0 + w / 2. - 1., l - cab - 7., 2., 0.),
                |_| -0.25,
            );
        }
        "garbage" => {
            let cab = 16.;
            cabin(a, l / 2. - cab, cab);
            boxf(a, x0 + 9., l - cab - 11., None, 2.);
            let mut x = x0 + 14.;
            while x < l / 2. - cab - 3. {
                a.line_lack(&[(x, y0 + 2.), (x, y0 + w - 2.)], -0.25, 0.7);
                x += 9.;
            }
            boxf(a, x0, 10., Some(0x4b4f54), 2.);
            for y in [y0 + 2.5, y0 + w - 4.5] {
                a.bx(x0 + 12., y, l - cab - 16., 2., 0., hex(0xf5f5f0));
            }
        }
        "police" => {
            paint_passenger(a, model, l, w, &shape("limousine"));
            let blue = hex(0x1f4e9c);
            a.bx(x0 + 5., y0 + 0.7, l - 10., 1.2, 0., blue);
            a.bx(x0 + 5., y0 + w - 1.9, l - 10., 1.2, 0., blue);
            a.bx(-4., -3., 6., 6., 0., blue);
        }
        "ambulance" => {
            let cab = 13.;
            cabin(a, l / 2. - cab, cab);
            boxf(a, x0, l - cab - 1., None, 2.);
            let red = hex(0xd0102a);
            a.bx(x0 + 1., y0 + 0.8, l - 2., 2.4, 0., red);
            a.bx(x0 + 1., y0 + w - 3.2, l - 2., 2.4, 0., red);
            a.bx(-10., -1.4, 10., 2.8, 0., red);
            a.bx(-6.4, -5., 2.8, 10., 0., red);
            let mut y = y0 + 3.;
            while y < y0 + w - 3. {
                a.bx(x0, y, 1.6, 2., 0., hex(0xf07d00));
                y += 4.;
            }
        }
        "bus" => {
            boxf(a, x0, l, None, 4.);
            a.bx(l / 2. - 4., y0 + 2., 3., w - 4., 0., glass);
            a.bx(x0 + 4., y0 + 0.8, l - 12., 2., 0., hex(0x2b2f36));
            a.bx(x0 + 4., y0 + w - 2.8, l - 12., 2., 0., hex(0x2b2f36));
            a.lack(&Path::rrect(x0 + 3., y0 + 3.5, l - 10., w - 7., 2.), |_| {
                0.12
            });
            a.bx(-18., -6., 26., 12., 2., hex(0xd8d8d2));
            a.bx(x0 + 6., -4., 14., 8., 0., hex(0xb9b9b2));
            for x in [-40., 22., 34.] {
                a.lack(&Path::rrect(x, -3., 5., 6., 0.), |_| -0.2);
            }
            for x in [-11f32, 1.] {
                a.fix(&Path::circle(x, 0., 4.2), hex(0x555f62));
                for y in -3..=3 {
                    a.line(
                        &[(x - 2.7, y as f32), (x + 2.7, y as f32)],
                        hex(0xa3afac),
                        0.4,
                    );
                }
                a.fix(&Path::circle(x, 0., 0.8), hex(0x262e32));
            }
            let mut x = x0 + 6.;
            while x < l / 2. - 8. {
                a.bx(x, y0 + 0.7, 0.6, 2.3, 0., hex(0x6d746f));
                a.bx(x, -y0 - 3., 0.6, 2.3, 0., hex(0x6d746f));
                x += 8.;
            }
        }
        _ => {}
    }
    if model != "police" {
        // Fahrerhaus: Seitenscheiben, Spiegel, Wischer, Scheibenreflex
        let cx = l / 2. - 9.;
        for side in [-1f32, 1.] {
            a.bx(
                cx,
                if side < 0. { y0 + 1.4 } else { -y0 - 3.1 },
                4.,
                1.7,
                0.,
                hex(0x172b37),
            );
            a.bx(
                cx + 2.,
                if side < 0. { y0 - 2. } else { -y0 },
                1.5,
                2.4,
                0.,
                hex(0x242c31),
            );
            a.bx(
                cx + 1.4,
                if side < 0. { y0 - 2.3 } else { -y0 + 0.8 },
                2.6,
                1.,
                0.,
                hex(0x9bb0b5),
            );
            a.line(
                &[
                    (l / 2. - 4.7, side * w * 0.08),
                    (l / 2. - 3.9, side * w * 0.3),
                ],
                hex(0x141d22),
                0.4,
            );
        }
        a.bx(
            l / 2. - 4.8,
            y0 + 3.,
            0.65,
            w * 0.45,
            0.,
            rgba(197, 228, 236, 0.36),
        );
        a.line(
            &[(x0 + 3., y0 + 2.), (l / 2. - 13., y0 + 2.)],
            rgba(255, 255, 255, 0.3),
            0.45,
        );
        if matches!(model, "truck" | "delivery" | "ambulance") {
            a.bx(x0 + 0.7, -0.25, 2., 0.5, 0., hex(0x293237));
            for y in [-w * 0.3, w * 0.3] {
                a.bx(x0 + 0.5, y, 0.8, 1.6, 0., hex(0x9da9a8));
            }
        }
    }
    if model == "police" || model == "ambulance" {
        let x = if model == "police" { -2. } else { l / 2. - 16. };
        a.bx(x - 1.8, -5.2, 3.6, 10.4, 0.7, hex(0x374349));
        a.bx(x - 1.2, -4.6, 2.4, 3.5, 0., hex(0x245791));
        a.bx(x - 1.2, 1.1, 2.4, 3.5, 0., hex(0x245791));
        a.bx(x - 0.8, -1., 1.6, 2., 0., hex(0xb7c7cc));
    }
    paint_lamps(a, model, l, w);
}

/// Länge des eigentlichen Wagenkörpers (`bodyLength`): kürzere Modelle stehen in der Fahrzeughülle mit Abstand.
pub fn body_length(model: &str, l: f32) -> f32 {
    if SPECIAL.contains(&model) {
        l
    } else {
        l - shape(model).len
    }
}

/// Alle Modelle in Atlasreihenfolge mit ihren Maßen (Länge, Breite der Hülle)
pub fn models() -> Vec<(&'static str, f32, f32)> {
    let mut v: Vec<(&str, f32, f32)> = berlin_sim::carmodels::CAR_MODELS
        .iter()
        .map(|(m, _)| (*m, 42., 20.))
        .collect();
    for m in SPECIAL {
        let k = berlin_sim::carmodels::kind(m);
        v.push((m, k.l as f32, k.w as f32));
    }
    v
}

/// Atlas: je Modell zwei Zellen (Lack, Details) nebeneinander. Liefert Pixel, Breite, Höhe.
pub fn atlas() -> (Vec<u8>, u32, u32) {
    let ms = models();
    let cells = ms.len() * 2;
    let rows = cells.div_ceil(COLS);
    let (w, h) = (COLS * CELL_W, rows * CELL_H);
    let mut out = vec![0u8; w * h * 4];
    for (i, (m, l, wd)) in ms.iter().enumerate() {
        let mut art = Art::new(*l, *wd);
        if SPECIAL.contains(m) {
            paint_special(&mut art, m, *l, *wd);
        } else {
            let s = shape(m);
            paint_passenger(&mut art, m, l - s.len, wd - 2. * s.inset, &s);
        }
        for (j, layer) in [&art.body, &art.detail].into_iter().enumerate() {
            let cell = i * 2 + j;
            let (cx, cy) = ((cell % COLS) * CELL_W, (cell / COLS) * CELL_H);
            let px = layer.rgba8();
            for y in 0..CELL_H {
                let src = &px[y * CELL_W * 4..(y + 1) * CELL_W * 4];
                let dst = ((cy + y) * w + cx) * 4;
                out[dst..dst + CELL_W * 4].copy_from_slice(src);
            }
        }
    }
    (out, w as u32, h as u32)
}

/// Index eines Modells im Atlas
pub fn model_index(model: &str) -> usize {
    models()
        .iter()
        .position(|(m, _, _)| *m == model)
        .unwrap_or(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_model_gets_paint_and_detail() {
        let (px, w, h) = atlas();
        assert_eq!(w as usize, COLS * CELL_W);
        let ms = models();
        assert_eq!(
            ms.len(),
            berlin_sim::carmodels::CAR_MODELS.len() + SPECIAL.len()
        );
        let alpha = |cell: usize| {
            let (cx, cy) = ((cell % COLS) * CELL_W, (cell / COLS) * CELL_H);
            let mut n = 0usize;
            for y in 0..CELL_H {
                for x in 0..CELL_W {
                    if px[((cy + y) * w as usize + cx + x) * 4 + 3] > 128 {
                        n += 1;
                    }
                }
            }
            n
        };
        for (i, m) in ms.iter().enumerate() {
            assert!(alpha(i * 2) > 4000, "{}: Lack", m.0);
            assert!(alpha(i * 2 + 1) > 800, "{}: Details", m.0);
        }
        assert!(h > 0);
    }

    #[test]
    fn finish_gradient_matches_stops() {
        assert!((finish_at(-10., -10., 20.) + 0.48).abs() < 1e-6);
        assert!((finish_at(10., -10., 20.) + 0.55).abs() < 1e-6);
        assert!(finish_at(-10. + 0.26 * 20., -10., 20.) > 0.27);
        assert_eq!(model_index("limousine"), 2);
    }
}
