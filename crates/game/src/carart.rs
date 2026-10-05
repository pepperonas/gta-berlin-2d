//! Fahrzeugbilder in Draufsicht (Port von `vehicleart.js paintPassenger`/`paintLamps` und `vehicles.js paintSpecial`),
//! einmal beim Start in einen Atlas gerastert. Je Modell zwei Zellen: die **Lackebene** trägt nur, wie stark der Lack
//! an jeder Stelle aufgehellt oder abgedunkelt ist (`assets.js shade(farbe, s)`, gespeichert als (s+1)/2) – die Farbe
//! des einzelnen Autos setzt der Shader ein; die **Detailebene** trägt alles mit fester Farbe (Scheiben, Leuchten,
//! Gummi, Chrom) und liegt darüber. Lack, der später gemalt wird, deckt die Details darunter ab.
use crate::raster::{Canvas, Path, Pt};

pub use berlin_engine::vehatlas::{CELL_H, CELL_W, COLS, MAT_CHROME, MAT_GLASS};
/// Glanzstärke je Material (G der Lackzelle, `vehatlas`)
pub const GLOSS_PAINT: f32 = 0.7;
pub const GLOSS_GLASS: f32 = 1.0;
pub const GLOSS_CHROME: f32 = 0.9;
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
/// Zwei Ebenen eines Fahrzeugs
pub struct Art {
    pub body: Canvas,
    pub detail: Canvas,
}
impl Art {
    pub fn new(l: f32, w: f32) -> Self {
        Self::sized(l, w, CELL_W, CELL_H)
    }
    /// Zellgröße frei (Test: gleiche Deckung bei jeder Auflösung)
    pub fn sized(l: f32, w: f32, px_w: usize, px_h: usize) -> Self {
        let (cw, ch) = (l + 2. * PAD, w + 2. * PAD);
        let scale = (px_w as f32 / cw, px_h as f32 / ch);
        let origin = (cw / 2., ch / 2.);
        Self {
            body: Canvas::new(px_w, px_h, scale, origin),
            detail: Canvas::new(px_w, px_h, scale, origin),
        }
    }
    /// Material der Detailebene (Glas, Chrom) in die Lackzelle darunter schreiben: G = Glanz, B = Klasse. Die
    /// Kanäle sind vormultipliziert wie der Rest; wo kein Lack liegt, bleibt nichts übrig (dort glänzt nichts).
    fn material(&mut self, path: &Path, gloss: f32, class: f32) {
        for (k, c) in self.body.coverage(path) {
            let d = &mut self.body.px[k];
            let a = d[3];
            d[1] += (gloss * a - d[1]) * c;
            d[2] += (class * a - d[2]) * c;
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
            // R = Schattierung, G = Glanz, B = Material (Lack = 0)
            for (x, t) in d[..3].iter_mut().zip([v, GLOSS_PAINT, 0.]) {
                *x = t * al + *x * (1. - al);
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
    /// Scheibe als Kasten: Detailfarbe und Glas in der Glanzmaske
    fn glass_bx(&mut self, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]) {
        if w > 0. && h > 0. {
            self.fix(&Path::rrect(x, y, w, h, 0.), c);
            self.material(&Path::rrect(x, y, w, h, 0.), GLOSS_GLASS, MAT_GLASS);
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
        a.glass_bx(xs + len - 5., y0 + 2.4, 3.2, w - 4.8, glass);
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
            a.glass_bx(l / 2. - 5., y0 + 2.4, 3., w - 4.8, glass);
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
            paint_car(a, model, l, w, axles("limousine", l));
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
            a.glass_bx(l / 2. - 4., y0 + 2., 3., w - 4., glass);
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
            a.glass_bx(
                cx,
                if side < 0. { y0 + 1.4 } else { -y0 - 3.1 },
                4.,
                1.7,
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

// ---------------------------------------------------------------------------------------------------------------
// Pkw nach Karosserieform (seit 05.10.2026): eigene Proportionen je Form statt eines Einheitsumrisses.
// Längsanteile (vorn → hinten): Haube bzw. Bug bis Scheibenfuß, Frontscheibe (in Draufsicht), Dach, Heckscheibe, Heck.
// ---------------------------------------------------------------------------------------------------------------

/// Karosserieform
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// Schrägheck (Kleinwagen, Kompakte)
    Hatch,
    /// Stufenheck-Limousine
    Sedan,
    /// Kombi: langes Dach, steile Heckklappe
    Wagon,
    /// SUV: hoch, kastig, kurze Heckscheibe
    Suv,
    /// Geländewagen mit Kastenaufbau (G-Klasse, Defender, Niva)
    Boxy,
    /// Coupé, Muscle-Car: lange Haube, kurze Kabine
    Coupe,
    /// Mittelmotor: kurze Fronthaube, Motorabdeckung hinter der Kabine, breite Hinterbacken
    Mid,
    /// Roadster: offen, Überrollbügel
    Roadster,
    /// Transporter, Kleinbus: fast keine Haube
    Van,
    /// Pickup: Kabine und Ladefläche
    Pickup,
    /// Oldtimer: runde Kotflügel, Chrom
    Classic,
}

/// Leuchtengrafik
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lamps {
    /// schmale LED-Streifen (modern)
    Slim,
    /// breite Scheinwerfer
    Wide,
    /// rund (Oldtimer, Geländewagen)
    Round,
    /// eckig (Transporter, Kastenwagen)
    Square,
}

/// Proportionen und Merkmale eines Modells
#[derive(Debug, Clone, Copy)]
pub struct Prop {
    pub form: Form,
    /// Längsanteile: Haube, Frontscheibe, Dach, Heckscheibe (Rest = Heck)
    pub hood: f32,
    pub ws: f32,
    pub roof: f32,
    pub rw: f32,
    /// Breite des Glashauses (Anteil der Karosseriebreite) und des Dachs (Anteil des Glashauses)
    pub cab: f32,
    pub top: f32,
    /// Eckradius vorn/hinten (Anteil der Breite), Einzug vorn/hinten (Anteil der halben Breite)
    pub cf: f32,
    pub cr: f32,
    pub taper_f: f32,
    pub taper_r: f32,
    /// Kotflügel-Ausbuchtung an den Achsen (Anteil der halben Breite)
    pub arch: f32,
    pub lamps: Lamps,
    pub doors: u8,
    pub rails: bool,
    pub glass_roof: bool,
    pub sunroof: bool,
    pub stripes: bool,
    pub hood_vent: bool,
    pub wing: bool,
    pub spare: bool,
    pub chrome: bool,
    pub roof_color: Option<u32>,
}

const fn prop(form: Form, hood: f32, ws: f32, roof: f32, rw: f32, lamps: Lamps) -> Prop {
    Prop {
        form,
        hood,
        ws,
        roof,
        rw,
        cab: 0.8,
        top: 0.84,
        cf: 0.26,
        cr: 0.2,
        taper_f: 0.08,
        taper_r: 0.04,
        arch: 0.,
        lamps,
        doors: 4,
        rails: false,
        glass_roof: false,
        sunroof: false,
        stripes: false,
        hood_vent: false,
        wing: false,
        spare: false,
        chrome: false,
        roof_color: None,
    }
}

/// Proportionen je Modell (Vorbilder in Klammern, nur als Formvorlage)
pub fn proportions(model: &str) -> Prop {
    use Form::*;
    use Lamps::*;
    let hatch = prop(Hatch, 0.25, 0.15, 0.38, 0.1, Slim);
    let sedan = prop(Sedan, 0.28, 0.15, 0.27, 0.12, Slim);
    let wagon = Prop {
        rails: true,
        ..prop(Wagon, 0.27, 0.15, 0.40, 0.06, Slim)
    };
    let suv = Prop {
        rails: true,
        cf: 0.2,
        cr: 0.16,
        cab: 0.84,
        top: 0.86,
        ..prop(Suv, 0.27, 0.13, 0.41, 0.06, Wide)
    };
    let coupe = Prop {
        doors: 2,
        cab: 0.72,
        top: 0.8,
        arch: 0.04,
        ..prop(Coupe, 0.36, 0.16, 0.18, 0.14, Slim)
    };
    let mid = Prop {
        doors: 2,
        cab: 0.66,
        top: 0.78,
        cf: 0.3,
        cr: 0.16,
        taper_f: 0.14,
        taper_r: 0.,
        arch: 0.07,
        ..prop(Mid, 0.24, 0.17, 0.15, 0.24, Slim)
    };
    let van = Prop {
        cab: 0.9,
        top: 0.94,
        cf: 0.16,
        cr: 0.08,
        taper_f: 0.06,
        taper_r: 0.,
        ..prop(Van, 0.1, 0.1, 0.74, 0.02, Square)
    };
    let boxy = Prop {
        cab: 0.88,
        top: 0.94,
        cf: 0.1,
        cr: 0.08,
        taper_f: 0.02,
        taper_r: 0.,
        arch: 0.03,
        spare: true,
        ..prop(Boxy, 0.27, 0.07, 0.48, 0.03, Round)
    };
    match model {
        "kleinwagen" => Prop {
            cf: 0.32,
            cr: 0.26,
            ..prop(Hatch, 0.22, 0.16, 0.38, 0.12, Wide)
        },
        "kompakt" => hatch,
        "hothatch" => Prop {
            stripes: true,
            hood_vent: true,
            arch: 0.03,
            ..hatch
        },
        "elektro" => Prop {
            glass_roof: true,
            cf: 0.32,
            ..prop(Hatch, 0.22, 0.2, 0.34, 0.14, Slim)
        },
        "elektrosport" => Prop {
            glass_roof: true,
            doors: 4,
            cab: 0.76,
            cf: 0.32,
            ..prop(Sedan, 0.24, 0.2, 0.26, 0.18, Slim)
        },
        "limousine" | "taxi" | "business" => sedan,
        "luxus" => Prop {
            chrome: true,
            sunroof: true,
            ..prop(Sedan, 0.3, 0.14, 0.28, 0.11, Wide)
        },
        "sportlimo" => Prop {
            hood_vent: true,
            arch: 0.03,
            ..sedan
        },
        "kombi" | "familienkombi" => wagon,
        "powerkombi" => Prop {
            hood_vent: true,
            arch: 0.03,
            ..wagon
        },
        "kompaktsuv" => Prop { cf: 0.24, ..suv },
        "sportsuv" => Prop { arch: 0.04, ..suv },
        "grosssuv" => Prop {
            glass_roof: true,
            ..suv
        },
        "gelaende" => Prop {
            spare: true,
            cf: 0.14,
            ..suv
        },
        "gklasse" => boxy,
        "defender" => Prop {
            roof_color: Some(0xf0efe9),
            ..boxy
        },
        "niva" => Prop {
            spare: false,
            ..boxy
        },
        "coupe" | "gtcoupe" | "leichtcoupe" => coupe,
        "heckcoupe" => Prop {
            ..prop(Coupe, 0.26, 0.16, 0.18, 0.2, Slim)
        }
        .with2(),
        "musclecar" => Prop {
            stripes: true,
            hood_vent: true,
            lamps: Round,
            cf: 0.12,
            cr: 0.1,
            ..coupe
        },
        "sportwagen" | "supersport" => Prop {
            wing: model == "supersport",
            ..mid
        },
        "leichtbau" => Prop { cab: 0.62, ..mid },
        "roadster" => Prop {
            doors: 2,
            cab: 0.74,
            arch: 0.04,
            cf: 0.34,
            ..prop(Roadster, 0.34, 0.1, 0.22, 0., Round)
        },
        "rallye" => Prop {
            wing: true,
            hood_vent: true,
            arch: 0.05,
            ..hatch
        },
        "transporter" | "sprinter" => van,
        "hochdach" => Prop {
            hood: 0.18,
            ws: 0.12,
            roof: 0.62,
            cf: 0.22,
            rails: true,
            ..van
        },
        "kleinbus" => Prop {
            roof_color: Some(0xefeee8),
            cf: 0.34,
            cr: 0.3,
            lamps: Round,
            chrome: true,
            ..prop(Van, 0.06, 0.1, 0.78, 0.02, Round)
        },
        "pickup" => Prop {
            cf: 0.16,
            cr: 0.06,
            cab: 0.86,
            ..prop(Pickup, 0.27, 0.12, 0.2, 0.03, Wide)
        },
        "oldtimer" => Prop {
            chrome: true,
            arch: 0.1,
            cab: 0.74,
            cf: 0.3,
            cr: 0.32,
            taper_f: 0.12,
            taper_r: 0.08,
            ..prop(Classic, 0.3, 0.12, 0.26, 0.12, Round)
        },
        "zweitakter" => Prop {
            roof_color: Some(0xecebe4),
            doors: 2,
            cf: 0.3,
            cr: 0.26,
            lamps: Round,
            ..prop(Sedan, 0.27, 0.14, 0.27, 0.13, Round)
        },
        "police" => sedan,
        _ => sedan,
    }
}

impl Prop {
    fn with2(self) -> Self {
        Prop {
            doors: 2,
            cab: 0.74,
            top: 0.8,
            ..self
        }
    }
    /// Längslagen (x, vorn = +l/2): Scheibenfuß vorn, Dachvorderkante, Dachhinterkante, Heckscheibenfuß
    fn stations(&self, l: f32) -> (f32, f32, f32, f32) {
        let front = l / 2.;
        let ws0 = front - self.hood * l;
        let rf = ws0 - self.ws * l;
        let rr = rf - self.roof * l;
        let rw0 = rr - self.rw * l;
        (ws0, rf, rr, rw0)
    }
}

/// Halbe Karosseriebreite an der Längslage x: Einzug zu den Enden, gerundete Ecken, Kotflügel an den Achsen.
fn half_at(p: &Prop, l: f32, w: f32, axles: (f32, f32), x: f32) -> f32 {
    let (front, rear) = (l / 2., -l / 2.);
    let smooth = |t: f32| {
        let t = t.clamp(0., 1.);
        t * t * (3. - 2. * t)
    };
    let hb = w / 2.;
    let mut h = hb
        * (1.
            - p.taper_f * smooth((x - (front - 0.32 * l)) / (0.32 * l))
            - p.taper_r * smooth(((rear + 0.3 * l) - x) / (0.3 * l)));
    // Kotflügel: Glocke um jede Achse
    if p.arch > 0. {
        for ax in [axles.0, axles.1] {
            let d = (x - ax) / (0.11 * l);
            h += hb * p.arch * (-d * d).exp();
        }
        h = h.min(hb);
    }
    // gerundete Ecken
    for (end, r, dir) in [(front, p.cf * w, 1f32), (rear, p.cr * w, -1.)] {
        let into = (x - (end - dir * r)) * dir;
        if into > 0. && r > 0. {
            let dx = into.min(r);
            h = h.min(h - r + (r * r - dx * dx).max(0.).sqrt());
        }
    }
    h.max(0.2)
}

/// Pkw zeichnen: Umriss, Lack mit Licht von oben links, Glashaus aus Front-, Heck- und Seitenscheiben mit Säulen,
/// Dach, Fugen, Spiegel, Griffe, Leuchten, Stoßfänger, Kennzeichen und die Merkmale des Modells.
pub fn paint_car(a: &mut Art, model: &str, l: f32, w: f32, axles: (f32, f32)) {
    let p = proportions(model);
    let (front, rear) = (l / 2., -l / 2.);
    let (ws0, rf, rr, rw0) = p.stations(l);
    let half = |x: f32| half_at(&p, l, w, axles, x);
    // Umriss
    let n = 64;
    let xs: Vec<f32> = (0..=n).map(|i| rear + l * i as f32 / n as f32).collect();
    let mut ring: Vec<Pt> = xs.iter().map(|&x| (x, -half(x))).collect();
    ring.extend(xs.iter().rev().map(|&x| (x, half(x))));
    let body = Path::poly(&ring);
    // Lack: quer gewölbt (Licht von oben links), zu den Enden und Rändern dunkler
    let lack_s = move |(x, y): Pt, hx: f32| {
        let t = (y / hx).clamp(-1.2, 1.2);
        let mut s = 0.2 - 0.62 * t * t - 0.12 * t;
        let e = (front - x).min(x - rear);
        s -= 0.28 * (1. - e / 2.4).max(0.);
        s
    };
    a.lack(&body, |pt| lack_s(pt, half(pt.0)));
    let mut edge = ring.clone();
    edge.push(ring[0]);
    a.line_lack(&edge, -0.7, 0.5);

    // Glanzkante entlang der Gürtellinie (links hell, rechts dunkel)
    for side in [-1f32, 1.] {
        let pts: Vec<Pt> = (0..=16)
            .map(|i| {
                let x = rw0 - 1. + (ws0 + 1. - (rw0 - 1.)) * i as f32 / 16.;
                (x, side * half(x) * 0.9)
            })
            .collect();
        if side < 0. {
            a.line(&pts, rgba(255, 255, 255, 0.28), 0.45);
        } else {
            a.line(&pts, rgba(0, 0, 0, 0.22), 0.45);
        }
    }

    // Streifen (über Haube, Dach, Heck)
    if p.stripes {
        for y in [-w * 0.13, w * 0.04] {
            a.bx(
                rear + 1.2,
                y,
                l - 2.4,
                w * 0.09,
                0.,
                rgba(238, 236, 228, 0.92),
            );
        }
    }

    // Haube: Fuge vor der Scheibe, Kanten, ggf. Lufthutze
    if p.form != Form::Van {
        let hx = ws0 + 0.8;
        a.line(
            &[
                (hx, -half(hx) * 0.82),
                (hx + 0.4, 0.),
                (hx, half(hx) * 0.82),
            ],
            rgba(0, 0, 0, 0.35),
            0.3,
        );
        for side in [-1f32, 1.] {
            a.line(
                &[
                    (hx + 1., side * half(hx) * 0.8),
                    (front - 2.2, side * half(front - 2.2) * 0.66),
                ],
                rgba(0, 0, 0, 0.25),
                0.28,
            );
            // Haubenwölbung: heller Grat
            a.line(
                &[(hx + 2., side * w * 0.12), (front - 3., side * w * 0.1)],
                rgba(255, 255, 255, if side < 0. { 0.16 } else { 0.07 }),
                0.5,
            );
        }
        if p.hood_vent {
            let (x0, x1) = (hx + 2.5, (front - 4.).max(hx + 4.));
            a.bx(x0, -w * 0.13, x1 - x0, w * 0.26, 0.6, hex(0x1d2326));
            let mut x = x0 + 0.7;
            while x < x1 - 0.3 {
                a.line(&[(x, -w * 0.11), (x, w * 0.11)], hex(0x4c5558), 0.3);
                x += 1.1;
            }
        }
    }

    // Glashaus
    let cab = |x: f32| half(x) * p.cab;
    let top = cab((rf + rr) / 2.) * p.top;
    let gl = glass(rw0, ws0, cab((rf + rr) / 2.));
    let window = |a: &mut Art, pts: &[Pt]| {
        let path = Path::poly(pts);
        a.fix_fn(&path, &gl);
        a.material(&path, GLOSS_GLASS, MAT_GLASS);
        let refl = Path::poly(&[
            (rw0 - 4., -w),
            (rw0 + 1.5, -w),
            (ws0 + 6., w),
            (ws0 + 2.5, w),
        ]);
        let cov_w = a.detail.coverage(&path);
        let cov_r: std::collections::HashMap<usize, f32> =
            a.detail.coverage(&refl).into_iter().collect();
        for (k, c) in cov_w {
            if let Some(r) = cov_r.get(&k) {
                let al = 0.16 * c.min(*r);
                let d = &mut a.detail.px[k];
                let col = [0.82, 0.91, 0.95];
                for i in 0..3 {
                    d[i] = col[i] * al + d[i] * (1. - al);
                }
            }
        }
        let mut r = pts.to_vec();
        r.push(pts[0]);
        a.line(&r, hex(0x0e1416), 0.3);
    };
    let bow = |x0: f32, x1: f32, h0: f32, h1: f32, k: f32| -> Vec<Pt> {
        // Scheibe als Viereck mit gewölbter Vorder- bzw. Hinterkante (k: Wölbung in Fahrtrichtung)
        let mut pts = Vec::new();
        for i in 0..=8 {
            let y = -h0 + 2. * h0 * i as f32 / 8.;
            pts.push((x0 + k * (1. - (y / h0).powi(2)), y));
        }
        pts.push((x1, h1));
        pts.push((x1, -h1));
        pts
    };
    let open = p.form == Form::Roadster;
    // Frontscheibe
    window(a, &bow(ws0, rf, cab(ws0), top, 0.04 * l));
    if !open {
        // Heckscheibe
        if p.rw > 0.005 {
            let mut pts = bow(rw0, rr, cab(rw0) * 0.96, top, -0.025 * l);
            pts.reverse();
            window(a, &pts);
        }
        // Seitenscheiben zwischen Gürtellinie und Dachkante
        for side in [-1f32, 1.] {
            let pts = [
                (ws0 - 0.4, side * cab(ws0)),
                (rf, side * (top + 0.2)),
                (rr, side * (top + 0.2)),
                (rw0 + 0.4, side * cab(rw0) * 0.96),
            ];
            window(a, &pts);
            // Säulen in Wagenfarbe: A, B (bei vier Türen auch C-nahe Säule), C
            let mut pillars = vec![((ws0, rf), 0.42)];
            if p.doors == 4 || matches!(p.form, Form::Wagon | Form::Suv | Form::Boxy | Form::Van) {
                pillars.push(((rf + (rr - rf) * 0.48, rf + (rr - rf) * 0.48), 0.6));
            }
            if matches!(p.form, Form::Wagon | Form::Suv | Form::Boxy) {
                pillars.push(((rf + (rr - rf) * 0.84, rf + (rr - rf) * 0.84), 0.45));
            }
            pillars.push(((rw0, rr), 0.7));
            for ((xa, xb), wd) in pillars {
                let ya = side * cab(xa) * if xa == rw0 { 0.96 } else { 1. };
                // Säulen liegen seitlich und schräg: im Schatten der Dachkante
                a.lack(&Path::stroke(&[(xa, ya), (xb, side * top)], wd), |_| -0.22);
            }
        }
        // Dach
        let roof = Path::rrect(rr, -top, rf - rr, 2. * top, (top * 0.25).min(2.));
        if p.glass_roof {
            a.fix_fn(&roof, &gl);
            a.material(&roof, GLOSS_GLASS, MAT_GLASS);
            a.line(
                &[((rr + rf) / 2., -top), ((rr + rf) / 2., top)],
                hex(0x0f1517),
                0.6,
            );
        } else if let Some(c) = p.roof_color {
            a.fix_fn(&roof, |(_, y)| {
                shade_fixed(c, 0.15 - 0.4 * (y / top).powi(2) - 0.1 * y / top)
            });
        } else {
            let (r0, r1) = (rr, rf);
            a.lack(&roof, move |(x, y)| {
                let along = ((x - r0) / (r1 - r0)).clamp(0., 1.);
                0.18 + 0.16 * along - 0.5 * (y / top).powi(2) - 0.14 * y / top
            });
        }
        let mut rring = roof.rings[0].clone();
        rring.push(rring[0]);
        a.line(&rring, rgba(10, 16, 18, 0.55), 0.3);
        if p.sunroof {
            let (x0, x1) = (rf - (rf - rr) * 0.42, rf - 1.);
            a.fix_fn(&Path::rrect(x0, -top * 0.62, x1 - x0, top * 1.24, 0.6), &gl);
        }
        if p.rails {
            for side in [-1f32, 1.] {
                a.line(
                    &[(rr + 1., side * (top - 0.6)), (rf - 1., side * (top - 0.6))],
                    hex(0x1d2427),
                    0.75,
                );
                a.line(
                    &[
                        (rr + 1.4, side * (top - 0.75)),
                        (rf - 1.4, side * (top - 0.75)),
                    ],
                    hex(0xa9b1b3),
                    0.25,
                );
            }
        }
        if p.form == Form::Van || p.form == Form::Boxy {
            // Sicken im Dach
            let mut x = rr + 2.5;
            while x < rf - 1.5 {
                a.line(&[(x, -top + 1.), (x, top - 1.)], rgba(0, 0, 0, 0.14), 0.35);
                x += 2.4;
            }
        }
        if p.stripes {
            for y in [-w * 0.13, w * 0.04] {
                a.bx(
                    rr + 0.3,
                    y,
                    rf - rr - 0.6,
                    w * 0.09,
                    0.,
                    rgba(238, 236, 228, 0.92),
                );
            }
        }
    } else {
        // offen: Innenraum, Sitze, Überrollbügel
        let (x0, x1) = (rr - 2., rf);
        a.bx(x0, -top * 1.05, x1 - x0, top * 2.1, 1.5, hex(0x15191b));
        for side in [-1f32, 1.] {
            a.bx_stroke(
                x0 + 2.2,
                side * top * 0.5 - 1.8,
                4.2,
                3.6,
                1.,
                hex(0x2b2523),
                hex(0x544643),
            );
            a.bx(
                x0 + 1.4,
                side * top * 0.5 - 1.6,
                1.1,
                3.2,
                0.4,
                hex(0x3d3330),
            );
            a.line(
                &[(x0 + 0.6, side * top * 0.2), (x0 + 0.6, side * top * 0.85)],
                hex(0xb6bcbe),
                0.6,
            );
        }
        a.line(
            &[(x1 - 0.8, -top * 0.95), (x1 - 0.8, top * 0.95)],
            hex(0x2b3134),
            0.7,
        );
    }

    // Spiegel an der A-Säule (stehen über die Karosserie hinaus)
    for side in [-1f32, 1.] {
        let x = ws0 - 0.8;
        let y0 = side * cab(x);
        let y1 = side * (half(x) + w * 0.07);
        a.line(&[(x, y0), (x - 0.4, y1)], hex(0x1c2225), 0.55);
        let p0 = Path::rrect(x - 2.2, y1.min(y1 - side * w * 0.08), 2.2, w * 0.08, 0.5);
        a.lack(&p0, |_| -0.05);
        a.line(
            &[
                (x - 2.0, y1 - side * w * 0.03),
                (x - 0.3, y1 - side * w * 0.03),
            ],
            hex(0xaebfc4),
            0.3,
        );
    }
    // Scheibenwischer
    if !open {
        // ruhen am Scheibenfuß, fast quer zur Fahrtrichtung
        for k in [-0.62f32, 0.05] {
            a.line(
                &[
                    (ws0 - 0.15, k * cab(ws0)),
                    (ws0 - 0.55, (k + 0.48) * cab(ws0)),
                ],
                hex(0x101618),
                0.3,
            );
        }
    }

    // Türfugen und Griffe zwischen Gürtellinie und Rand
    if !matches!(p.form, Form::Van) {
        let mut doors = vec![ws0 - 0.2];
        if p.doors == 4 {
            doors.push(rf + (rr - rf) * 0.48);
        }
        doors.push(if p.doors == 4 {
            rw0 + (rr - rw0) * 0.3
        } else {
            rf + (rr - rf) * 0.75
        });
        for side in [-1f32, 1.] {
            for &x in &doors {
                a.line(
                    &[(x, side * cab(x) * 1.02), (x - 0.3, side * half(x) * 0.97)],
                    rgba(0, 0, 0, 0.42),
                    0.28,
                );
            }
            for win in doors.windows(2) {
                let x = win[1] + 0.8;
                let y = side * (cab(x) + (half(x) - cab(x)) * 0.45);
                let g = Path::rrect(x, y - 0.25, 1.5, 0.5, 0.2);
                if p.chrome {
                    a.fix(&g, hex(0xc9cdcc));
                    a.material(&g, GLOSS_CHROME, MAT_CHROME);
                } else {
                    a.lack(&g, |_| 0.35);
                }
            }
        }
    } else {
        // Schiebetür: Laufschiene rechts
        let y = (half(0.) * 0.97).min(w / 2.);
        a.line(
            &[(rr + 3., y), (rf - (rf - rr) * 0.42, y)],
            rgba(0, 0, 0, 0.45),
            0.4,
        );
    }

    // Ladefläche (Pickup)
    if p.form == Form::Pickup {
        let (x0, x1) = (rear + 1.2, rw0 - 0.8);
        a.bx_stroke(
            x0,
            -half(x0) * 0.82,
            x1 - x0,
            half(x0) * 1.64,
            0.8,
            hex(0x24282a),
            hex(0x7d8280),
        );
        let mut y = -half(x0) * 0.6;
        while y < half(x0) * 0.62 {
            a.line(&[(x0 + 0.8, y), (x1 - 0.8, y)], hex(0x464c4f), 0.35);
            y += 1.5;
        }
    }
    // Mittelmotor: Motorabdeckung mit Lamellen, Lufteinlässe vor den Hinterrädern
    if p.form == Form::Mid {
        let (x0, x1) = (rw0 + 1.2, rr - 0.6);
        if x1 > x0 + 1. {
            a.bx(x0, -top * 0.9, x1 - x0, top * 1.8, 0.8, hex(0x171c1e));
            let mut x = x0 + 0.7;
            while x < x1 - 0.4 {
                a.line(&[(x, -top * 0.75), (x, top * 0.75)], hex(0x3d4548), 0.3);
                x += 1.;
            }
        }
        for side in [-1f32, 1.] {
            let x = axles.1 + 0.18 * l;
            let path = Path::poly(&[
                (x, side * half(x) * 0.98),
                (x - 3.5, side * half(x - 3.5) * 0.98),
                (x - 3., side * half(x - 3.) * 0.8),
                (x - 0.5, side * half(x) * 0.86),
            ]);
            a.fix(&path, hex(0x111517));
        }
    }

    // Heck: Klappenfuge, Stoßfänger, Kennzeichen, Endrohre, Reserverad, Flügel
    if p.form != Form::Pickup && p.form != Form::Mid {
        let tx = rw0 - 0.7;
        if tx > rear + 2. {
            a.line(
                &[(tx, -half(tx) * 0.8), (tx, half(tx) * 0.8)],
                rgba(0, 0, 0, 0.32),
                0.28,
            );
        }
    }
    let bumper = if p.chrome {
        hex(0xc6cbc9)
    } else {
        hex(0x22282b)
    };
    a.bx(
        front - 0.9,
        -half(front - 1.) * 0.7,
        0.9,
        half(front - 1.) * 1.4,
        0.4,
        bumper,
    );
    a.bx(
        rear,
        -half(rear + 1.) * 0.74,
        0.9,
        half(rear + 1.) * 1.48,
        0.4,
        bumper,
    );
    if p.chrome {
        for (x, y) in [
            (front - 0.9, half(front - 1.) * 0.7),
            (rear, half(rear + 1.) * 0.74),
        ] {
            a.material(
                &Path::rrect(x, -y, 0.9, 2. * y, 0.4),
                GLOSS_CHROME,
                MAT_CHROME,
            );
        }
    }
    for x in [rear + 0.15, front - 0.75] {
        a.bx(x, -1.3, 0.6, 2.6, 0.1, hex(0xe2e5e0));
        a.bx(x, -1.3, 0.6, 0.45, 0., hex(0x2f5790));
    }
    if !matches!(model, "elektro" | "elektrosport") {
        let sides: &[f32] = if matches!(p.form, Form::Coupe | Form::Mid) || p.hood_vent {
            &[-1., 1.]
        } else {
            &[1.]
        };
        for &side in sides {
            a.bx_stroke(
                rear - 0.6,
                side * half(rear + 1.) * 0.55 - 0.6,
                1.3,
                1.2,
                0.5,
                hex(0xa9b1b2),
                hex(0x171e21),
            );
        }
    }
    if p.spare {
        a.fix(&Path::circle(rear - 0.9, 0., w * 0.17), hex(0x181c1e));
        a.fix(&Path::circle(rear - 0.9, 0., w * 0.09), hex(0x5c6264));
    }
    if p.wing {
        a.bx(rear + 1.6, -w * 0.3, 0.9, w * 0.6, 0.1, hex(0x161b1e));
        let wp = Path::rrect(rear + 0.8, -w * 0.44, 1.4, w * 0.88, 0.4);
        a.lack(&wp, |(_, y)| 0.15 - 0.5 * (2. * y / w).powi(2));
    }
    if model == "taxi" {
        let x = (rr + rf) / 2.;
        a.bx_stroke(x - 1.2, -3.2, 2.4, 6.4, 0.55, hex(0xeccc69), hex(0x554a2c));
        a.line(&[(x, -1.8), (x, 1.8)], hex(0x443b24), 0.5);
    }

    paint_car_lamps(a, &p, l, w, &half);
}

/// Scheinwerfer, Grill und Rückleuchten nach Leuchtengrafik (Bremslicht und Blinker zeichnet das Spiel darüber).
fn paint_car_lamps(a: &mut Art, p: &Prop, l: f32, w: f32, half: &dyn Fn(f32) -> f32) {
    let (front, rear) = (l / 2., -l / 2.);
    let lens = hex(0xdde6ea);
    let red = hex(0x9c141b);
    let red_hi = hex(0xe23a3f);
    // Grill zwischen den Scheinwerfern
    let gx = front - 1.4;
    let gh = half(gx) * if p.form == Form::Van { 0.62 } else { 0.42 };
    if p.form != Form::Mid {
        a.bx(
            gx,
            -gh,
            1.1,
            2. * gh,
            0.4,
            hex(if p.chrome { 0xa6adae } else { 0x14191b }),
        );
    }
    for side in [-1f32, 1.] {
        match p.lamps {
            Lamps::Slim => {
                let (x0, x1) = (front - 0.05 * l, front - 0.5);
                let pts = [
                    (x0, side * half(x0) * 0.86),
                    (x1, side * half(x1) * 0.92),
                    (x1, side * half(x1) * 0.6),
                    (x0 + 0.6, side * half(x0) * 0.7),
                ];
                a.fix(&Path::poly(&pts), hex(0x8f9a9e));
                a.fix(
                    &Path::poly(&[
                        (x0 + 0.5, side * half(x0) * 0.82),
                        (x1 - 0.2, side * half(x1) * 0.88),
                        (x1 - 0.2, side * half(x1) * 0.7),
                        (x0 + 0.9, side * half(x0) * 0.74),
                    ]),
                    hex(0xe9f2f6),
                );
                a.line(
                    &[
                        (x0 + 0.6, side * half(x0) * 0.68),
                        (x1 - 0.4, side * half(x1) * 0.64),
                    ],
                    hex(0x30383b),
                    0.25,
                );
            }
            Lamps::Wide => {
                let (x0, x1) = (front - 0.065 * l, front - 0.5);
                let pts = [
                    (x0, side * half(x0) * 0.9),
                    (x1, side * half(x1) * 0.92),
                    (x1, side * half(x1) * 0.5),
                    (x0, side * half(x0) * 0.56),
                ];
                a.fix(&Path::poly(&pts), lens);
                a.line(
                    &[
                        (x0 + 0.5, side * half(x0) * 0.75),
                        (x1 - 0.4, side * half(x1) * 0.75),
                    ],
                    hex(0x8c979b),
                    0.35,
                );
            }
            Lamps::Round => {
                let x = front - 0.045 * l;
                let r = w * 0.085;
                a.fix(
                    &Path::circle(x, side * half(x) * 0.7, r + 0.25),
                    hex(0xb7bebf),
                );
                a.fix(&Path::circle(x, side * half(x) * 0.7, r), lens);
                a.fix(
                    &Path::circle(x + r * 0.2, side * half(x) * 0.7 - side * r * 0.2, r * 0.35),
                    hex(0xffffff),
                );
            }
            Lamps::Square => {
                let (x0, x1) = (front - 0.05 * l, front - 0.4);
                a.bx(
                    x0,
                    if side < 0. {
                        -half(x0) * 0.92
                    } else {
                        half(x0) * 0.6
                    },
                    x1 - x0,
                    half(x0) * 0.32,
                    0.3,
                    lens,
                );
            }
        }
        // Rückleuchten: um die Ecke gezogen
        let (x0, x1) = (rear + 0.35, rear + 0.035 * l);
        let pts = [
            (x0, side * half(x0 + 0.4) * 0.88),
            (x1, side * half(x1) * 0.93),
            (x1, side * half(x1) * 0.76),
            (x0, side * half(x0 + 0.4) * 0.62),
        ];
        a.fix(&Path::poly(&pts), red);
        a.line(
            &[
                (x0 + 0.3, side * half(x0 + 0.5) * 0.82),
                (x1 - 0.3, side * half(x1) * 0.85),
            ],
            red_hi,
            0.35,
        );
    }
    // durchgehendes Leuchtband hinten (moderne Schlanke Leuchten)
    if p.lamps == Lamps::Slim && matches!(p.form, Form::Coupe | Form::Mid | Form::Suv) {
        let x = rear + 0.45;
        a.line(
            &[(x, -half(x) * 0.6), (x, half(x) * 0.6)],
            rgba(200, 30, 36, 0.9),
            0.4,
        );
    }
}

/// Achslagen (x, vorn = +l/2) eines Pkw-Modells bei Länge l (px): Radstand aus den Fahrzeugdaten (höchstens 72 % der
/// Länge), Überhänge je Karosserieform verteilt (Mittelmotor hinten länger, Transporter vorn kurz).
pub fn axles(model: &str, l: f32) -> (f32, f32) {
    let wb = berlin_sim::vehdata::game_vehicle(model)
        .map_or(0.6 * l, |v| v.wheelbase as f32 * 10.)
        .min(0.72 * l);
    let front_share = match proportions(model).form {
        Form::Mid => 0.42,
        Form::Van => 0.4,
        Form::Coupe | Form::Classic => 0.52,
        Form::Pickup | Form::Boxy => 0.5,
        _ => 0.56,
    };
    let over = l - wb;
    let fa = l / 2. - over * front_share;
    (fa, fa - wb)
}

/// Räder eines Pkw im Spiel (Hülle hw × hh, px): Achslagen, halbe Spurweite, halbe Reifenlänge und -breite.
pub fn wheels(model: &str, hw: f32, hh: f32) -> (f32, f32, f32, f32, f32) {
    let (fa, ra) = axles(model, 2. * hw);
    let track = berlin_sim::vehdata::game_vehicle(model).map_or(1.6, |v| v.track as f32);
    let (tl, tw) = match proportions(model).form {
        Form::Suv | Form::Boxy | Form::Pickup => (4.0, 1.4),
        Form::Van => (3.6, 1.2),
        Form::Mid | Form::Coupe => (3.6, 1.4),
        _ => (3.3, 1.1),
    };
    // Reifenaußenkante bleibt unter der Karosserie (breite Spur + breite Reifen ragten sonst heraus)
    let ht = (track * 5.).min(hh - tw - 0.15);
    (fa, ra, ht, tl, tw)
}

/// Alle Modelle in Atlasreihenfolge mit ihren Maßen (Länge, Breite der Hülle)
pub fn models() -> Vec<(&'static str, f32, f32)> {
    let mut v: Vec<(&str, f32, f32)> = berlin_sim::carmodels::CAR_MODELS
        .iter()
        .map(|(m, _)| {
            let (l, w) = berlin_sim::carmodels::body_dims(m).unwrap_or((4.2, 2.0));
            (*m, l as f32 * 10., w as f32 * 10.)
        })
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
            paint_car(&mut art, m, *l, *wd, axles(m, *l));
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
    let model = sprite_model(model);
    models()
        .iter()
        .position(|(m, _, _)| *m == model)
        .unwrap_or(2)
}

/// Bild für ein Modell: eigenes, sonst nach Klasse des Datensatzes (Sattelzug → Lkw, Gelenkbus → Bus).
pub fn sprite_model(model: &str) -> &str {
    if models().iter().any(|(m, _, _)| *m == model) {
        return model;
    }
    match berlin_sim::vehdata::shared()
        .get(model)
        .map(|v| v.class.as_str())
    {
        Some("lkw" | "lkw_sattel") => "truck",
        Some("bus" | "bus_gelenk") => "bus",
        Some("transporter" | "van") => "delivery",
        _ => "limousine",
    }
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

    /// Silhouette bleibt: dieselben Modelle bei alter (256 × 128) und neuer Zellgröße decken denselben Anteil der
    /// Zelle (der Umriss kommt aus Modelleinheiten, nicht aus Pixeln) – die Kollisionsform ändert sich nicht.
    #[test]
    fn coverage_does_not_depend_on_cell_size() {
        let share = |m: &str, l: f32, wd: f32, cw: usize, ch: usize| {
            let mut a = Art::sized(l, wd, cw, ch);
            if SPECIAL.contains(&m) {
                paint_special(&mut a, m, l, wd);
            } else {
                paint_car(&mut a, m, l, wd, axles(m, l));
            }
            let n = a
                .body
                .px
                .iter()
                .zip(&a.detail.px)
                .filter(|(b, d)| d[3] + b[3] * (1. - d[3]) > 0.5)
                .count();
            n as f32 / (cw * ch) as f32
        };
        for (m, l, wd) in models() {
            let (old, new) = (share(m, l, wd, 256, 128), share(m, l, wd, CELL_W, CELL_H));
            assert!((old - new).abs() < 0.01, "{m}: {old:.4} → {new:.4}");
        }
    }

    /// Glanzmaske: Lack glänzt überall, wo Lack liegt; Scheiben sind Glas, Chromstoßstangen Chrom; ohne Lack nichts.
    #[test]
    fn gloss_mask_marks_paint_glass_and_chrome() {
        let (px, w, _) = atlas();
        let at = |cell: usize, x: usize, y: usize| {
            let (cx, cy) = ((cell % COLS) * CELL_W, (cell / COLS) * CELL_H);
            let i = ((cy + y) * w as usize + cx + x) * 4;
            [px[i], px[i + 1], px[i + 2], px[i + 3]]
        };
        let class = |m: &str| {
            let i = model_index(m) * 2;
            let (mut paint, mut glass, mut chrome) = (0, 0, 0);
            for y in 0..CELL_H {
                for x in 0..CELL_W {
                    let b = at(i, x, y);
                    if b[3] < 250 {
                        continue;
                    }
                    match b[2] {
                        0 => {
                            let g = b[1] as f32 / 255.;
                            assert!((g - GLOSS_PAINT).abs() < 0.02, "{m}: Lackglanz {g}");
                            paint += 1
                        }
                        100..=160 => glass += 1,
                        220.. => chrome += 1,
                        _ => {}
                    }
                }
            }
            (paint, glass, chrome)
        };
        let (p, g, _) = class("limousine");
        assert!(p > 20_000 && g > 5_000, "Limousine: Lack {p}, Glas {g}");
        let (_, _, c) = class("oldtimer");
        assert!(c > 200, "Oldtimer ohne Chrom: {c}");
        let (_, g, _) = class("bus");
        assert!(g > 500, "Bus ohne Scheiben: {g}");
    }

    #[test]
    fn atlas_order_is_stable() {
        assert_eq!(model_index("limousine"), 2);
    }

    /// Die Formen unterscheiden sich sichtbar: Umriss (Lack) und Details zweier Modelle verschiedener Form weichen
    /// in einem merklichen Teil der Pixel voneinander ab; dieselbe Form mit anderer Leuchtengrafik bleibt nahe.
    #[test]
    fn body_forms_differ_visibly() {
        let (px, w, _) = atlas();
        let ms = models();
        let idx = |m: &str| ms.iter().position(|x| x.0 == m).unwrap();
        // Abweichung zweier Modelle in Prozent der Pixel (Lackmaske oder Detailfarbe deutlich anders)
        let diff = |a: &str, b: &str| {
            let (ia, ib) = (idx(a), idx(b));
            let mut n = 0usize;
            for y in 0..CELL_H {
                for x in 0..CELL_W {
                    let at = |cell: usize, k: usize| {
                        let (cx, cy) = ((cell % COLS) * CELL_W, (cell / COLS) * CELL_H);
                        px[((cy + y) * w as usize + cx + x) * 4 + k] as i32
                    };
                    let body = (at(ia * 2, 3) > 128) != (at(ib * 2, 3) > 128);
                    let det = (0..3)
                        .map(|k| (at(ia * 2 + 1, k) - at(ib * 2 + 1, k)).abs())
                        .sum::<i32>()
                        > 120;
                    if body || det {
                        n += 1;
                    }
                }
            }
            n as f32 / (CELL_W * CELL_H) as f32 * 100.
        };
        for (a, b) in [
            ("kompakt", "transporter"),
            ("limousine", "pickup"),
            ("kombi", "roadster"),
            ("sportwagen", "kompaktsuv"),
            ("gklasse", "coupe"),
            ("kleinwagen", "oldtimer"),
        ] {
            let d = diff(a, b);
            assert!(d > 6., "{a} gegen {b}: nur {d:.1} % verschieden");
        }
        // gleiche Form (Limousine, Taxi ohne Schild wäre identisch): Taxi trägt nur das Dachschild
        assert!(diff("limousine", "taxi") < 3.);
    }

    /// Maße: echte Unterschiede in der Länge, keine Breite über 2,0 m (Fahrstreifen), Räder innerhalb der Hülle.
    #[test]
    fn sizes_vary_and_wheels_sit_inside() {
        let ms = models();
        let cars: Vec<_> = ms.iter().filter(|(m, _, _)| !SPECIAL.contains(m)).collect();
        let (lmin, lmax) = cars
            .iter()
            .fold((f32::MAX, 0f32), |a, c| (a.0.min(c.1), a.1.max(c.1)));
        let (wmin, wmax) = cars
            .iter()
            .fold((f32::MAX, 0f32), |a, c| (a.0.min(c.2), a.1.max(c.2)));
        assert!(lmax - lmin > 14., "Länge {lmin}…{lmax}");
        assert!(
            wmax <= 20. && wmin >= 15. && wmax - wmin > 3.,
            "Breite {wmin}…{wmax}"
        );
        for (m, l, wd) in cars {
            let (fa, ra, ht, tl, tw) = wheels(m, l / 2., wd / 2.);
            assert!(
                fa + tl <= l / 2. + 0.5 && ra - tl >= -l / 2. - 0.5,
                "{m}: Räder außerhalb der Länge"
            );
            assert!(ht + tw <= wd / 2. + 0.01, "{m}: Räder außerhalb der Breite");
            assert!(fa - ra > 0.45 * l, "{m}: Radstand zu kurz");
        }
    }
}

#[cfg(test)]
mod dump {
    /// Atlas als PPM ausgeben (Lack mit Beispielfarbe eingefärbt, Details darüber): `GTA_ATLAS_DUMP=pfad`
    #[test]
    #[ignore]
    fn dump_atlas() {
        let Ok(path) = std::env::var("GTA_ATLAS_DUMP") else {
            return;
        };
        let (px, w, h) = super::atlas();
        let (w, h) = (w as usize, h as usize);
        let cw = super::CELL_W;
        let pairs = w / (2 * cw);
        let out_w = pairs * cw;
        let rows = h / super::CELL_H;
        let mut img = vec![200u8; out_w * rows * super::CELL_H * 3];
        let paint = [0.20f32, 0.45, 0.80];
        for r in 0..rows {
            for pi in 0..pairs {
                for y in 0..super::CELL_H {
                    for x in 0..cw {
                        let gy = r * super::CELL_H + y;
                        let b = (gy * w + pi * 2 * cw + x) * 4;
                        let d = (gy * w + (pi * 2 + 1) * cw + x) * 4;
                        let o = ((gy) * out_w + pi * cw + x) * 3;
                        let mut c = [0.78f32, 0.78, 0.78];
                        let ba = px[b + 3] as f32 / 255.;
                        if ba > 0. {
                            let s = px[b] as f32 / 255. * 2. - 1.;
                            for k in 0..3 {
                                let v = if s >= 0. {
                                    paint[k] + (1. - paint[k]) * s
                                } else {
                                    paint[k] * (1. + s)
                                };
                                c[k] = c[k] * (1. - ba) + v * ba;
                            }
                        }
                        let da = px[d + 3] as f32 / 255.;
                        for k in 0..3 {
                            c[k] = c[k] * (1. - da) + px[d + k] as f32 / 255. * da;
                        }
                        for k in 0..3 {
                            img[o + k] = (c[k].clamp(0., 1.) * 255.) as u8;
                        }
                    }
                }
            }
        }
        let dims: String = super::models()
            .iter()
            .map(|(m, l, w)| format!("{m} {l} {w}\n"))
            .collect();
        std::fs::write(format!("{path}.dims"), dims).unwrap();
        let mut f = format!("P6\n{} {}\n255\n", out_w, rows * super::CELL_H).into_bytes();
        f.extend(img);
        std::fs::write(path, f).unwrap();
    }
}
