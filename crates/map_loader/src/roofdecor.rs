//! Dachaufbauten und Gauben (Port von `roofs.js roofDecor` und dem Gaubenteil von `roofGeometry`): Schornsteine auf
//! dem First von Wohnhäusern, Gauben auf langen Traufseiten; auf Flachdächern Schornsteine, Lüftungsschächte,
//! Oberlichter, Klimageräte, Solarflächen und Dachterrassen – jedes Teil ganz im Grundriss (exakter Rechtecktest,
//! auch gegen Einbuchtungen und kleine Höfe), beim Berliner Dach nur auf der flachen Mitte. Deterministisch aus dem
//! Gebäude-Seed.
use crate::citycodes::building_kind as K;
use crate::format::Building;
use crate::roofs::{Style, footprint_m2, oriented_box};
use glam::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Chimney,
    Shaft,
    Skylight,
    Ac,
    Solar,
    Terrace,
    Dormer,
}
impl Kind {
    /// Maße in m (Länge entlang der Hauptachse × Breite)
    pub fn size(self) -> (f32, f32) {
        match self {
            Kind::Chimney => (0.7, 0.7),
            Kind::Shaft => (1.8, 1.6),
            Kind::Skylight => (1.4, 0.9),
            Kind::Ac => (2.2, 1.4),
            Kind::Solar => (5., 2.2),
            Kind::Terrace => (4.5, 3.5),
            Kind::Dormer => (1.6, 1.5),
        }
    }
    /// Rand und Innenfläche (`render.js DECOR_COLOR`)
    pub fn colors(self) -> (u32, u32) {
        match self {
            Kind::Chimney => (0x7a4a3a, 0x3a2620),
            Kind::Shaft => (0xc8c3ba, 0x2a2c30),
            Kind::Skylight => (0xd0d4d8, 0x8fb3c9),
            Kind::Ac => (0xb9bdc1, 0x8a8f94),
            Kind::Solar => (0x5b7390, 0x223047),
            Kind::Terrace => (0x8a6a48, 0xa47e56),
            Kind::Dormer => (0, 0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decor {
    pub kind: Kind,
    pub center: Vec2,
    /// halbe Länge (entlang `angle`) und halbe Breite
    pub half: Vec2,
    pub angle: f32,
    /// Gaube: Richtung zur Traufe (Stirnseite)
    pub out: Vec2,
}

/// mulberry32 wie `rng.js`
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f64 / 4294967296.) as f32
    }
}

fn point_in_rings(p: Vec2, rings: &[Vec<Vec2>]) -> bool {
    let mut inside = false;
    for r in rings {
        let n = r.len();
        for i in 0..n {
            let (a, b) = (r[i], r[(i + 1) % n]);
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Liegt das gedrehte Rechteck ganz im Grundriss? Mitte drin und keine Grundrisskante schneidet es (Liang–Barsky).
pub fn rect_inside(rings: &[Vec<Vec2>], c: Vec2, hx: f32, hy: f32, axis: Vec2) -> bool {
    if !point_in_rings(c, rings) {
        return false;
    }
    let (ca, sa) = (axis.x, axis.y);
    for r in rings {
        let n = r.len();
        for i in 0..n {
            let (a, b) = (r[i] - c, r[(i + 1) % n] - c);
            let (x0, y0) = (a.x * ca + a.y * sa, -a.x * sa + a.y * ca);
            let (x1, y1) = (b.x * ca + b.y * sa, -b.x * sa + b.y * ca);
            let (dx, dy) = (x1 - x0, y1 - y0);
            let (mut t0, mut t1) = (0f32, 1f32);
            let mut clip = |pq: f32, qq: f32| -> bool {
                if pq == 0. {
                    return qq >= 0.;
                }
                let t = qq / pq;
                if pq < 0. {
                    if t > t1 {
                        return false;
                    }
                    if t > t0 {
                        t0 = t;
                    }
                } else {
                    if t < t0 {
                        return false;
                    }
                    if t < t1 {
                        t1 = t;
                    }
                }
                true
            };
            if clip(-dx, x0 + hx)
                && clip(dx, hx - x0)
                && clip(-dy, y0 + hy)
                && clip(dy, hy - y0)
                && t0 <= t1
            {
                return false;
            }
        }
    }
    true
}

fn edge_dist(rings: &[Vec<Vec2>], p: Vec2) -> f32 {
    let mut best = f32::INFINITY;
    for r in rings {
        let n = r.len();
        for i in 0..n {
            let (a, b) = (r[i], r[(i + 1) % n]);
            let e = b - a;
            let t = ((p - a).dot(e) / e.length_squared().max(1e-6)).clamp(0., 1.);
            best = best.min((a + e * t - p).length());
        }
    }
    best
}

/// Aufbauten und Gauben eines Gebäudes.
pub fn roof_decor(b: &Building, style: Style, scale: f32) -> Vec<Decor> {
    let area = footprint_m2(b, scale) as f32;
    let mut rnd = Rng(b.seed ^ 0x5bd1e995);
    let o = oriented_box(b);
    let axis = o.axis;
    let rings = &b.polygon.rings;
    let mut out: Vec<Decor> = Vec::new();
    if b.kind == K::SMALL || area < 40. {
        return out;
    }
    let mk = |kind: Kind, c: Vec2| {
        let (l, w) = kind.size();
        Decor {
            kind,
            center: c,
            half: Vec2::new(l * scale / 2., w * scale / 2.),
            angle: axis.y.atan2(axis.x),
            out: Vec2::ZERO,
        }
    };
    if style.pitched() || style == Style::Dome {
        if b.kind != K::HOUSE || matches!(style, Style::Skillion | Style::Round | Style::Dome) {
            return out;
        }
        // Schornsteine auf dem First (Mittellinie entlang der Hauptachse)
        let mid = (o.min.y + o.max.y) / 2.;
        let n = 3.min(1 + (area / 160. * rnd.next()) as usize);
        let half = Kind::Chimney.size().0 * scale / 2. + 0.3 * scale;
        for _ in 0..n {
            let u = 0.2 + rnd.next() * 0.6;
            let c = o.point(Vec2::new(o.min.x + (o.max.x - o.min.x) * u, mid));
            if !rect_inside(rings, c, half, half, axis)
                || out.iter().any(|d| (d.center - c).length() < 3. * scale)
            {
                continue;
            }
            out.push(mk(Kind::Chimney, c));
        }
        // Gauben auf den Traufseiten (zur Hälfte der Tiefe, ganz im Grundriss)
        let chance = if style == Style::Berlin { 0.45 } else { 0.55 };
        if rnd.next() < chance {
            let (gw, gl) = (1.6 * scale, 1.5 * scale);
            let len = o.max.x - o.min.x;
            let depth = (o.max.y - o.min.y) / 2.;
            if len >= 7. * scale && depth >= 2.4 * scale {
                let step = (4.5 + rnd.next() * 2.) * scale;
                let n = ((len - 3. * scale) / step).floor().max(0.) as usize;
                let off = (len - (n.max(1) - 1) as f32 * step) / 2.;
                for side in [-1f32, 1.] {
                    let eave = if side < 0. { o.min.y } else { o.max.y };
                    for k in 0..n {
                        let x = o.min.x + off + k as f32 * step;
                        let y = eave - side * depth * 0.45;
                        let c = o.point(Vec2::new(x, y));
                        if !rect_inside(
                            rings,
                            c,
                            gw / 2. + 0.2 * scale,
                            gl / 2. + 0.2 * scale,
                            axis,
                        ) {
                            continue;
                        }
                        if out
                            .iter()
                            .any(|d| d.kind == Kind::Dormer && (d.center - c).length() < gw * 1.6)
                        {
                            continue;
                        }
                        let mut d = mk(Kind::Dormer, c);
                        d.out = o.normal * side;
                        out.push(d);
                        if out.len() >= 24 {
                            return out;
                        }
                    }
                }
            }
        }
        return out;
    }
    if area < 60. {
        return out;
    }
    let mut wish = Vec::new();
    let mut n = |per: f32, max: usize| max.min((area / per + rnd.next()) as usize);
    if style == Style::Corrugated {
        let (a, s) = (n(250., 8), n(400., 6));
        wish.extend(std::iter::repeat_n(Kind::Ac, a));
        wish.extend(std::iter::repeat_n(Kind::Skylight, s));
        if rnd.next() < 0.2 {
            wish.extend([Kind::Solar, Kind::Solar]);
        }
    } else {
        let (c, sh, sk) = (n(55., 14), n(260., 4), n(180., 5));
        wish.extend(std::iter::repeat_n(Kind::Chimney, c));
        wish.extend(std::iter::repeat_n(Kind::Shaft, sh));
        wish.extend(std::iter::repeat_n(Kind::Skylight, sk));
        if b.kind == K::PUBLIC {
            let a = n(300., 5);
            wish.extend(std::iter::repeat_n(Kind::Ac, a));
        }
        if rnd.next() < 0.15 {
            wish.extend([Kind::Solar, Kind::Solar, Kind::Solar]);
        }
        if rnd.next() < 0.1 {
            wish.push(Kind::Terrace);
        }
    }
    // Berliner Dach: nur auf der flachen Mitte, hinter dem geneigten Streifen
    let band = if style == Style::Berlin {
        (4.5 * scale).min((o.max.y - o.min.y) * 0.3)
    } else {
        0.
    };
    let (lo, hi) = (o.min, o.max);
    for kind in wish {
        let (l, w) = kind.size();
        let (l, w) = (l * scale, w * scale);
        for _ in 0..12 {
            let c = o.point(Vec2::new(
                lo.x + rnd.next() * (hi.x - lo.x),
                lo.y + rnd.next() * (hi.y - lo.y),
            ));
            if !rect_inside(rings, c, l / 2. + 0.4 * scale, w / 2. + 0.4 * scale, axis) {
                continue;
            }
            if band > 0. && edge_dist(rings, c) < band + l.max(w) / 2. + 0.3 * scale {
                continue;
            }
            if out.iter().any(|d| {
                (d.center - c).length() < (d.half.max_element() * 2. + l.max(w)) / 2. + 0.5 * scale
            }) {
                continue;
            }
            out.push(mk(kind, c));
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Polygon;

    fn building(kind: u8, w: f32, h: f32, seed: u32) -> Building {
        let ring = vec![
            Vec2::ZERO,
            Vec2::new(w, 0.),
            Vec2::new(w, h),
            Vec2::new(0., h),
        ];
        Building {
            id: 1,
            height: 200.,
            meters: 20.,
            kind,
            polygon: Polygon {
                rings: vec![ring],
                outer: vec![true],
            },
            walls: None,
            look: 0,
            roof_rgb: None,
            wall_rgb: None,
            center: Vec2::new(w / 2., h / 2.),
            seed,
            doors: Vec::new(),
        }
    }

    #[test]
    fn flat_roofs_get_fixtures_inside_the_footprint() {
        let mut total = 0;
        for seed in 0..20 {
            let b = building(K::HOUSE, 300., 200., seed);
            let d = roof_decor(&b, Style::Flat, 10.);
            total += d.len();
            for x in &d {
                assert!(
                    rect_inside(&b.polygon.rings, x.center, x.half.x, x.half.y, Vec2::X),
                    "{x:?}"
                );
            }
        }
        assert!(total > 100, "Aufbauten auf 20 Flachdächern: {total}");
    }

    #[test]
    fn pitched_houses_get_chimneys_and_dormers_small_buildings_nothing() {
        let (mut chimneys, mut dormers) = (0, 0);
        for seed in 0..30 {
            let d = roof_decor(&building(K::HOUSE, 200., 100., seed), Style::Gabled, 10.);
            chimneys += d.iter().filter(|x| x.kind == Kind::Chimney).count();
            dormers += d.iter().filter(|x| x.kind == Kind::Dormer).count();
        }
        assert!(chimneys >= 20 && dormers >= 10, "{chimneys} {dormers}");
        assert!(roof_decor(&building(K::SMALL, 200., 100., 1), Style::Flat, 10.).is_empty());
        // Einbuchtung: ein Rechteck über die Kante ist nicht drin
        let rings = vec![vec![
            Vec2::ZERO,
            Vec2::new(10., 0.),
            Vec2::new(10., 10.),
            Vec2::new(0., 10.),
        ]];
        assert!(rect_inside(&rings, Vec2::new(5., 5.), 2., 2., Vec2::X));
        assert!(!rect_inside(&rings, Vec2::new(9., 5.), 2., 2., Vec2::X));
    }
}
