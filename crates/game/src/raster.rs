//! Kleiner Vektor-Rasterer für die Fahrzeugbilder (statt Canvas 2D): Pfade aus Linien und Bézierkurven, Füllen mit
//! Nonzero-Regel über Scanlinien (vier Unterzeilen je Pixelreihe, waagerecht exakte Abdeckung), Striche als Vierecke.
//! Koordinaten in Modell-Einheiten; `scale` und `origin` bilden sie auf Pixel ab. Keine Abhängigkeiten.

pub type Pt = (f32, f32);

/// Pfad aus geschlossenen Teilpfaden (Punkte in Modell-Einheiten).
#[derive(Debug, Clone, Default)]
pub struct Path {
    pub rings: Vec<Vec<Pt>>,
}
impl Path {
    pub fn new() -> Self {
        Self::default()
    }
    #[allow(dead_code)] // Pfadaufbau für freie Umrisse (die Fahrzeugbilder nutzen derzeit Polygone)
    pub fn move_to(&mut self, p: Pt) -> &mut Self {
        self.rings.push(vec![p]);
        self
    }
    #[allow(dead_code)] // Pfadaufbau für freie Umrisse (die Fahrzeugbilder nutzen derzeit Polygone)
    pub fn line_to(&mut self, p: Pt) -> &mut Self {
        if self.rings.is_empty() {
            self.rings.push(Vec::new());
        }
        self.rings.last_mut().expect("Teilpfad").push(p);
        self
    }
    /// Kubische Bézierkurve vom letzten Punkt (in 12 Stücke zerlegt).
    #[allow(dead_code)] // Pfadaufbau für freie Umrisse (die Fahrzeugbilder nutzen derzeit Polygone)
    pub fn cubic_to(&mut self, c1: Pt, c2: Pt, p: Pt) -> &mut Self {
        let p0 = *self.rings.last().and_then(|r| r.last()).unwrap_or(&p);
        for i in 1..=12 {
            let t = i as f32 / 12.;
            let u = 1. - t;
            let (a, b, c, d) = (u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t);
            self.line_to((
                a * p0.0 + b * c1.0 + c * c2.0 + d * p.0,
                a * p0.1 + b * c1.1 + c * c2.1 + d * p.1,
            ));
        }
        self
    }
    pub fn poly(pts: &[Pt]) -> Self {
        let mut p = Self::new();
        p.rings.push(pts.to_vec());
        p
    }
    /// Abgerundetes Rechteck (x, y = linke obere Ecke).
    pub fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Self {
        let r = r.clamp(0., (w.abs() / 2.).min(h.abs() / 2.));
        let mut ring = Vec::new();
        let corner = |cx: f32, cy: f32, a0: f32, ring: &mut Vec<Pt>| {
            for i in 0..=4 {
                let a = a0 + i as f32 / 4. * std::f32::consts::FRAC_PI_2;
                ring.push((cx + a.cos() * r, cy + a.sin() * r));
            }
        };
        use std::f32::consts::PI;
        corner(x + w - r, y + r, -PI / 2., &mut ring);
        corner(x + w - r, y + h - r, 0., &mut ring);
        corner(x + r, y + h - r, PI / 2., &mut ring);
        corner(x + r, y + r, PI, &mut ring);
        Self { rings: vec![ring] }
    }
    pub fn circle(cx: f32, cy: f32, r: f32) -> Self {
        let ring = (0..20)
            .map(|i| {
                let a = i as f32 / 20. * std::f32::consts::TAU;
                (cx + a.cos() * r, cy + a.sin() * r)
            })
            .collect();
        Self { rings: vec![ring] }
    }
    /// Linienzug als Fläche (Breite w): je Stück ein Viereck, gleich ausgerichtet (Nonzero vereinigt sie).
    pub fn stroke(pts: &[Pt], w: f32) -> Self {
        let mut p = Self::new();
        for s in pts.windows(2) {
            let ((ax, ay), (bx, by)) = (s[0], s[1]);
            let (dx, dy) = (bx - ax, by - ay);
            let l = dx.hypot(dy);
            if l < 1e-6 {
                continue;
            }
            let (nx, ny) = (-dy / l * w / 2., dx / l * w / 2.);
            let (ex, ey) = (dx / l * w / 4., dy / l * w / 4.);
            p.rings.push(vec![
                (ax - ex + nx, ay - ey + ny),
                (bx + ex + nx, by + ey + ny),
                (bx + ex - nx, by + ey - ny),
                (ax - ex - nx, ay - ey - ny),
            ]);
        }
        p
    }
}

/// Bild mit vormultiplizierten Farben (0…1).
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 4]>,
    pub scale: Pt,
    pub origin: Pt,
}
impl Canvas {
    pub fn new(w: usize, h: usize, scale: Pt, origin: Pt) -> Self {
        Self {
            w,
            h,
            px: vec![[0.; 4]; w * h],
            scale,
            origin,
        }
    }
    fn to_px(&self, p: Pt) -> Pt {
        (
            (p.0 + self.origin.0) * self.scale.0,
            (p.1 + self.origin.1) * self.scale.1,
        )
    }
    /// Pixelmitte zurück in Modell-Einheiten.
    pub fn to_model(&self, x: usize, y: usize) -> Pt {
        (
            (x as f32 + 0.5) / self.scale.0 - self.origin.0,
            (y as f32 + 0.5) / self.scale.1 - self.origin.1,
        )
    }
    /// Abdeckung (0…1) je Pixel für einen Pfad: Nonzero-Regel, 4 Unterzeilen.
    pub fn coverage(&self, path: &Path) -> Vec<(usize, f32)> {
        let mut edges = Vec::new();
        let (mut y0, mut y1) = (f32::MAX, f32::MIN);
        for ring in &path.rings {
            if ring.len() < 2 {
                continue;
            }
            for i in 0..ring.len() {
                let a = self.to_px(ring[i]);
                let b = self.to_px(ring[(i + 1) % ring.len()]);
                if (a.1 - b.1).abs() < 1e-9 {
                    continue;
                }
                y0 = y0.min(a.1.min(b.1));
                y1 = y1.max(a.1.max(b.1));
                edges.push((a, b));
            }
        }
        let mut acc = vec![0f32; self.w * self.h];
        let mut touched = Vec::new();
        if edges.is_empty() {
            return Vec::new();
        }
        const SUB: usize = 4;
        let ry0 = (y0.floor().max(0.)) as usize;
        let ry1 = (y1.ceil().min(self.h as f32)) as usize;
        let mut xs: Vec<(f32, i32)> = Vec::new();
        for row in ry0..ry1 {
            for s in 0..SUB {
                let sy = row as f32 + (s as f32 + 0.5) / SUB as f32;
                xs.clear();
                for &((ax, ay), (bx, by)) in &edges {
                    let (lo, hi) = if ay < by { (ay, by) } else { (by, ay) };
                    if sy < lo || sy >= hi {
                        continue;
                    }
                    let t = (sy - ay) / (by - ay);
                    xs.push((ax + (bx - ax) * t, if by > ay { 1 } else { -1 }));
                }
                if xs.len() < 2 {
                    continue;
                }
                xs.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut wind = 0;
                for i in 0..xs.len() - 1 {
                    wind += xs[i].1;
                    if wind == 0 {
                        continue;
                    }
                    let (a, b) = (xs[i].0.max(0.), xs[i + 1].0.min(self.w as f32));
                    if b <= a {
                        continue;
                    }
                    let (ia, ib) = (a.floor() as usize, (b.ceil() as usize).min(self.w));
                    for x in ia..ib {
                        let cov = (b.min(x as f32 + 1.) - a.max(x as f32)).max(0.);
                        let k = row * self.w + x;
                        if acc[k] == 0. {
                            touched.push(k);
                        }
                        acc[k] += cov / SUB as f32;
                    }
                }
            }
        }
        touched.into_iter().map(|k| (k, acc[k].min(1.))).collect()
    }
    /// Füllen; `paint(x, y)` liefert die (gerade, nicht vormultiplizierte) Farbe an einem Modellpunkt.
    pub fn fill(&mut self, path: &Path, paint: impl Fn(Pt) -> [f32; 4]) {
        for (k, cov) in self.coverage(path) {
            let (x, y) = (k % self.w, k / self.w);
            let c = paint(self.to_model(x, y));
            let a = c[3] * cov;
            let d = &mut self.px[k];
            for i in 0..3 {
                d[i] = c[i] * a + d[i] * (1. - a);
            }
            d[3] = a + d[3] * (1. - a);
        }
    }
    /// Deckkraft unter einem Pfad mindern (eine Lackfläche verdeckt Details darunter).
    pub fn erase(&mut self, cov: &[(usize, f32)], a: f32) {
        for &(k, c) in cov {
            let f = 1. - c * a;
            for v in &mut self.px[k] {
                *v *= f;
            }
        }
    }
    /// RGBA8 mit gerader (nicht vormultiplizierter) Farbe.
    pub fn rgba8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.w * self.h * 4);
        for p in &self.px {
            let a = p[3];
            let un = |v: f32| if a > 1e-4 { (v / a).clamp(0., 1.) } else { 0. };
            out.extend([
                (un(p[0]) * 255.).round() as u8,
                (un(p[1]) * 255.).round() as u8,
                (un(p[2]) * 255.).round() as u8,
                (a.clamp(0., 1.) * 255.).round() as u8,
            ]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_a_square_with_exact_coverage() {
        let mut c = Canvas::new(10, 10, (1., 1.), (0., 0.));
        c.fill(
            &Path::poly(&[(2., 2.), (6., 2.), (6., 6.), (2., 6.)]),
            |_| [1., 0., 0., 1.],
        );
        assert_eq!(c.px[3 * 10 + 3], [1., 0., 0., 1.]);
        assert_eq!(c.px[10 + 1][3], 0.);
        // halbe Pixel am Rand
        let mut d = Canvas::new(10, 10, (1., 1.), (0., 0.));
        d.fill(
            &Path::poly(&[(2.5, 2.), (6., 2.), (6., 6.), (2.5, 6.)]),
            |_| [1.; 4],
        );
        assert!((d.px[3 * 10 + 2][3] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn circle_area_and_stroke_union() {
        let mut c = Canvas::new(40, 40, (1., 1.), (0., 0.));
        c.fill(&Path::circle(20., 20., 10.), |_| [1.; 4]);
        let area: f32 = c.px.iter().map(|p| p[3]).sum();
        assert!((area - std::f32::consts::PI * 100.).abs() < 8., "{area}");
        // überlappende Strichstücke decken nicht doppelt
        let mut s = Canvas::new(40, 10, (1., 1.), (0., 0.));
        s.fill(&Path::stroke(&[(2., 5.), (20., 5.), (38., 5.)], 2.), |_| {
            [1.; 4]
        });
        assert!(s.px.iter().all(|p| p[3] <= 1.0001));
        assert!((s.px[5 * 40 + 20][3] - 1.).abs() < 1e-4);
    }

    #[test]
    fn bezier_reaches_its_end_and_erase_reduces() {
        let mut p = Path::new();
        p.move_to((0., 0.)).cubic_to((1., 0.), (2., 1.), (3., 3.));
        assert_eq!(*p.rings[0].last().unwrap(), (3., 3.));
        let mut c = Canvas::new(8, 8, (1., 1.), (0., 0.));
        let sq = Path::rrect(1., 1., 6., 6., 1.);
        c.fill(&sq, |_| [0., 1., 0., 1.]);
        let cov = c.coverage(&sq);
        c.erase(&cov, 1.);
        // innen ganz weg, am Rand bleibt höchstens c·(1 − c) ≤ 0,25
        assert!(c.px[4 * 8 + 4][3] < 1e-5);
        assert!(c.px.iter().all(|p| p[3] <= 0.2501));
    }
}
