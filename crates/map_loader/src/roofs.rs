//! Deterministic roof/facade selection and GPU facet geometry, based on roofs.js.
use crate::{
    citycodes::{
        building_kind as k, building_sub as sub, roof_mat, roof_shape as rs, unpack_look, wall_mat,
    },
    format::Building,
    geom::{edges, point_in_rings, signed_area},
};
use glam::Vec2;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Flat,
    Berlin,
    Gabled,
    Hipped,
    Pyramidal,
    Mansard,
    Skillion,
    Dome,
    Round,
    Corrugated,
}
impl Style {
    pub fn pitched(self) -> bool {
        matches!(
            self,
            Self::Gabled
                | Self::Hipped
                | Self::Pyramidal
                | Self::Mansard
                | Self::Skillion
                | Self::Round
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facade {
    Altbau,
    Platte,
    Modern,
    Industry,
}
pub fn footprint_m2(b: &Building, scale: f32) -> f64 {
    b.polygon
        .rings
        .iter()
        .enumerate()
        .map(|(i, r)| signed_area(r).abs() * if b.polygon.outer[i] { 1.0 } else { -1.0 })
        .sum::<f64>()
        / (scale as f64).powi(2)
}
pub fn roof_style(b: &Building, scale: f32) -> Style {
    let r = (b.seed % 1000) as f32 / 1000.;
    let m = b.meters;
    let area = footprint_m2(b, scale);
    let lk = unpack_look(b.look);
    if b.kind == k::SPAETI {
        return Style::Flat;
    }
    if b.kind == k::WAREHOUSE {
        return Style::Corrugated;
    }
    match lk.shape {
        rs::FLAT => {
            return if b.kind == k::INDUSTRIAL && lk.rmat != roof_mat::GREEN {
                Style::Corrugated
            } else {
                Style::Flat
            };
        }
        rs::GABLED => return Style::Gabled,
        rs::HIPPED => return Style::Hipped,
        rs::PYRAMIDAL => return Style::Pyramidal,
        rs::MANSARD => return Style::Mansard,
        rs::SKILLION => return Style::Skillion,
        rs::DOME => return Style::Dome,
        rs::ROUND => return Style::Round,
        _ => {}
    }
    if lk.rmat == roof_mat::GREEN || lk.rmat == roof_mat::TAR {
        return Style::Flat;
    }
    if b.kind == k::CHURCH {
        return if r < 0.85 {
            Style::Gabled
        } else {
            Style::Hipped
        };
    }
    if b.kind == k::INDUSTRIAL {
        return if r < 0.12 {
            Style::Gabled
        } else {
            Style::Corrugated
        };
    }
    if b.kind == k::SMALL {
        return if r < 0.45 {
            Style::Gabled
        } else if r < 0.55 {
            Style::Skillion
        } else {
            Style::Flat
        };
    }
    if lk.sub == sub::GARAGE {
        return if r < 0.2 { Style::Gabled } else { Style::Flat };
    }
    if b.kind == k::HOUSE {
        if lk.sub == sub::TERRACE {
            return if r < 0.8 { Style::Gabled } else { Style::Flat };
        }
        let villa = lk.sub == sub::VILLA || (m <= 10. && area < 350.);
        if villa && m <= 14. {
            return if r < 0.42 {
                Style::Hipped
            } else if r < 0.84 {
                Style::Gabled
            } else if r < 0.9 {
                Style::Mansard
            } else {
                Style::Flat
            };
        }
        if [3, 4].contains(&lk.bez) && m >= 14. {
            return Style::Flat;
        }
        if lk.bez == 6 && (12.0..=26.0).contains(&m) {
            return if r < 0.85 {
                Style::Gabled
            } else if r < 0.93 {
                Style::Berlin
            } else {
                Style::Flat
            };
        }
        if (12.0..=26.0).contains(&m) && r < 0.62 {
            return Style::Berlin;
        }
        if m > 9. && m < 12. && r < 0.35 {
            return Style::Hipped;
        }
    }
    if b.kind == k::PUBLIC && m <= 22. && r < 0.3 {
        return if r < 0.15 {
            Style::Hipped
        } else {
            Style::Mansard
        };
    }
    Style::Flat
}
pub fn facade_style(b: &Building) -> Facade {
    let r = ((b.seed / 1000) % 1000) as f32 / 1000.;
    let m = b.meters;
    let lk = unpack_look(b.look);
    if b.kind == k::INDUSTRIAL || b.kind == k::WAREHOUSE {
        return Facade::Industry;
    }
    if lk.wmat == wall_mat::GLASS {
        return Facade::Modern;
    }
    if lk.wmat == wall_mat::CONCRETE && b.kind == k::HOUSE && m >= 14. {
        return Facade::Platte;
    }
    if b.kind == k::PUBLIC {
        return if r < 0.6 {
            Facade::Modern
        } else {
            Facade::Altbau
        };
    }
    if b.kind == k::HOUSE {
        if [3, 4].contains(&lk.bez) && m >= 14. && lk.sub != sub::VILLA {
            return if r < 0.85 {
                Facade::Platte
            } else {
                Facade::Modern
            };
        }
        if m > 30. {
            return if r < 0.7 {
                Facade::Platte
            } else {
                Facade::Modern
            };
        }
        if m > 24. {
            return if r < 0.4 {
                Facade::Platte
            } else if r < 0.7 {
                Facade::Modern
            } else {
                Facade::Altbau
            };
        }
        return if r < 0.8 {
            Facade::Altbau
        } else {
            Facade::Modern
        };
    }
    Facade::Altbau
}
#[derive(Debug, Clone, Copy)]
pub struct OrientedBox {
    pub center: Vec2,
    pub axis: Vec2,
    pub normal: Vec2,
    pub min: Vec2,
    pub max: Vec2,
}
impl OrientedBox {
    pub fn point(self, p: Vec2) -> Vec2 {
        self.center + self.axis * p.x + self.normal * p.y
    }
    pub fn local(self, p: Vec2) -> Vec2 {
        let d = p - self.center;
        Vec2::new(d.dot(self.axis), d.dot(self.normal))
    }
}
pub fn oriented_box(b: &Building) -> OrientedBox {
    let d = edges(&b.polygon.rings[0])
        .map(|(a, c)| c - a)
        .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .unwrap_or(Vec2::X)
        .normalize_or_zero();
    let axis = if d == Vec2::ZERO { Vec2::X } else { d };
    let normal = Vec2::new(-axis.y, axis.x);
    let mut o = OrientedBox {
        center: b.center,
        axis,
        normal,
        min: Vec2::splat(f32::INFINITY),
        max: Vec2::splat(f32::NEG_INFINITY),
    };
    for &p in &b.polygon.rings[0] {
        let p = o.local(p);
        o.min = o.min.min(p);
        o.max = o.max.max(p);
    }
    o
}
#[derive(Debug, Clone)]
pub struct Facet {
    pub points: Vec<Vec2>,
    pub outward: Vec2,
}
fn ray_to_wall(rings: &[Vec<Vec2>], p: Vec2, d: Vec2) -> f32 {
    let mut best = f32::INFINITY;
    for ring in rings {
        for (a, b) in edges(ring) {
            let e = b - a;
            let den = d.perp_dot(e);
            if den.abs() < 1e-9 {
                continue;
            }
            let t = (a - p).perp_dot(e) / den;
            let u = (a - p).perp_dot(d) / den;
            if t > 0.5 && (0.0..=1.0).contains(&u) {
                best = best.min(t);
            }
        }
    }
    best
}
fn band_facets(
    ring: &[Vec2],
    rings: &[Vec<Vec2>],
    depth: impl Fn(f32) -> f32,
    out: &mut Vec<Facet>,
) {
    let n = ring.len();
    if n < 3 {
        return;
    }
    let mut side = 0.;
    for (a, b) in edges(ring) {
        let d = b - a;
        if d.length() < 4. {
            continue;
        }
        let left = Vec2::new(-d.y, d.x).normalize_or_zero() * 1.5;
        let mid = (a + b) * 0.5;
        let p = point_in_rings(mid + left, rings);
        let q = point_in_rings(mid - left, rings);
        if p != q {
            side = if p { 1. } else { -1. };
            break;
        }
    }
    if side == 0. {
        return;
    }
    let normals: Vec<_> = edges(ring)
        .map(|(a, b)| {
            let d = (b - a).normalize_or_zero();
            Vec2::new(-d.y, d.x) * side
        })
        .collect();
    let mut depths = Vec::new();
    for (i, (a, b)) in edges(ring).enumerate() {
        let mut samples = [0.25, 0.5, 0.75].map(|f| ray_to_wall(rings, a.lerp(b, f), normals[i]));
        samples.sort_by(f32::total_cmp);
        depths.push(depth(samples[1]));
    }
    let q: Vec<_> = (0..n)
        .map(|i| {
            let j = (i + n - 1) % n;
            let a = normals[j];
            let b = normals[i];
            let dot = 1. + a.dot(b);
            let mut m = if dot < 0.15 { b } else { (a + b) / dot };
            if m.length() > 2.2 {
                m = m.normalize() * 2.2;
            }
            ring[i] + m * depths[j].min(depths[i])
        })
        .collect();
    for i in 0..n {
        let k = (i + 1) % n;
        let a = ring[i];
        let b = ring[k];
        if a.distance(b) < 1. {
            continue;
        }
        let dd = depths[i].min(depths[(i + n - 1) % n]).min(depths[k]);
        let mut qa = q[i];
        let mut qb = q[k];
        if (qa - a).dot(normals[i]) < 0.2 * dd {
            qa = a + normals[i] * dd;
        }
        if (qb - b).dot(normals[i]) < 0.2 * dd {
            qb = b + normals[i] * dd;
        }
        if (qb - qa).dot(b - a) < 0. {
            let mid = (qa + qb) * 0.5;
            qa = mid;
            qb = mid;
        }
        out.push(Facet {
            points: vec![a, b, qb, qa],
            outward: -normals[i],
        });
    }
}
pub fn roof_facets(b: &Building, style: Style, scale: f32) -> Vec<Facet> {
    if !style.pitched() && style != Style::Berlin {
        return Vec::new();
    }
    let o = oriented_box(b);
    let size = o.max - o.min;
    let half = size.y * 0.5;
    let middle = (o.min.y + o.max.y) * 0.5;
    let box_shape = b.polygon.rings.len() == 1
        && size.x * size.y > 0.
        && footprint_m2(b, scale) * scale as f64 * scale as f64 / (size.x * size.y) as f64 >= 0.82;
    let mut out = Vec::new();
    if box_shape && [Style::Gabled, Style::Round, Style::Skillion].contains(&style) {
        let x0 = o.min.x - 2.;
        let x1 = o.max.x + 2.;
        let facet = |y0, y1, normal| Facet {
            points: vec![
                o.point(Vec2::new(x0, y0)),
                o.point(Vec2::new(x1, y0)),
                o.point(Vec2::new(x1, y1)),
                o.point(Vec2::new(x0, y1)),
            ],
            outward: normal,
        };
        if style == Style::Skillion {
            out.push(facet(o.min.y - 2., o.max.y + 2., o.normal));
        } else {
            out.push(facet(o.min.y - 2., middle, -o.normal));
            out.push(facet(middle, o.max.y + 2., o.normal));
        }
        return out;
    }
    let box_ring = vec![
        o.point(o.min),
        o.point(Vec2::new(o.max.x, o.min.y)),
        o.point(o.max),
        o.point(Vec2::new(o.min.x, o.max.y)),
    ];
    let box_rings = vec![box_ring];
    let rings = if box_shape {
        &box_rings
    } else {
        &b.polygon.rings
    };
    let lo = 0.8 * scale;
    for r in rings {
        band_facets(
            r,
            rings,
            |d| {
                if box_shape {
                    lo.max(match style {
                        Style::Mansard => (half * 0.35).min(2.8 * scale),
                        Style::Berlin => (half * 0.6).min(4.5 * scale),
                        _ => half,
                    })
                } else {
                    let (fraction, max) = match style {
                        Style::Mansard => (0.22, 2.8),
                        Style::Berlin => (0.38, 4.5),
                        _ => (0.5, 8.),
                    };
                    lo.max(
                        (if d.is_finite() {
                            d * fraction
                        } else {
                            max * scale
                        })
                        .min(max * scale)
                        .min(half),
                    )
                }
            },
            &mut out,
        );
    }
    out
}
