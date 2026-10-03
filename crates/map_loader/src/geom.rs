//! Pure geometry port of geom.js; coordinates are Vec2 instead of flat arrays.
use glam::Vec2;
use std::collections::HashMap;
#[derive(Debug, Clone, Copy, Default)]
pub struct Bounds {
    pub min: Vec2,
    pub max: Vec2,
}
impl Bounds {
    pub fn of(points: &[Vec2]) -> Self {
        points.iter().fold(
            Self {
                min: Vec2::splat(f32::INFINITY),
                max: Vec2::splat(f32::NEG_INFINITY),
            },
            |b, &p| Self {
                min: b.min.min(p),
                max: b.max.max(p),
            },
        )
    }
    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
    pub fn contains(self, p: Vec2) -> bool {
        p.cmpge(self.min).all() && p.cmple(self.max).all()
    }
    pub fn expand(self, amount: f32) -> Self {
        Self {
            min: self.min - Vec2::splat(amount),
            max: self.max + Vec2::splat(amount),
        }
    }
}
pub fn point_in_ring(p: Vec2, ring: &[Vec2]) -> bool {
    let mut inside = false;
    for (a, b) in edges(ring) {
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}
pub fn point_in_rings(p: Vec2, rings: &[Vec<Vec2>]) -> bool {
    rings.iter().filter(|r| point_in_ring(p, r)).count() % 2 == 1
}
pub fn edges(ring: &[Vec2]) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
    ring.iter()
        .copied()
        .zip(ring.iter().copied().cycle().skip(1))
        .take(ring.len())
}
pub fn signed_area(ring: &[Vec2]) -> f64 {
    // Translate before products to preserve precision at Berlin's large map coordinates.
    let origin = ring.first().copied().unwrap_or_default();
    edges(ring)
        .map(|(a, b)| {
            let a = a - origin;
            let b = b - origin;
            a.x as f64 * b.y as f64 - a.y as f64 * b.x as f64
        })
        .sum::<f64>()
        * 0.5
}
pub fn seg_dist2(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let d = b - a;
    let t = if d.length_squared() > 0.0 {
        ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance_squared(a + d * t)
}
pub fn polyline_length(points: &[Vec2]) -> f32 {
    points.windows(2).map(|s| s[0].distance(s[1])).sum()
}
pub fn cum_lengths(points: &[Vec2]) -> Vec<f32> {
    let mut out = Vec::with_capacity(points.len());
    let mut sum = 0.0;
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            sum += p.distance(points[i - 1]);
        }
        out.push(sum);
    }
    out
}
#[derive(Debug, Clone, Copy)]
pub struct Along {
    pub point: Vec2,
    pub direction: Vec2,
    pub s: f32,
    pub d2: f32,
}
pub fn point_along(points: &[Vec2], s: f32) -> Option<Along> {
    point_along_cum(points, &cum_lengths(points), s)
}
pub fn point_along_cum(points: &[Vec2], cum: &[f32], s: f32) -> Option<Along> {
    if points.len() != cum.len() || points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some(Along {
            point: points[0],
            direction: Vec2::X,
            s: 0.0,
            d2: 0.0,
        });
    }
    let s = s.clamp(0.0, *cum.last()?);
    let i = cum
        .partition_point(|&v| v <= s)
        .saturating_sub(1)
        .min(points.len() - 2);
    let d = points[i + 1] - points[i];
    let len = cum[i + 1] - cum[i];
    let t = if len > 0.0 { (s - cum[i]) / len } else { 0.0 };
    Some(Along {
        point: points[i] + d * t,
        direction: if len > 0.0 { d / len } else { Vec2::X },
        s,
        d2: 0.0,
    })
}
pub fn project_on_polyline(points: &[Vec2], p: Vec2) -> Option<Along> {
    let mut best: Option<Along> = None;
    let mut acc = 0.0;
    for segment in points.windows(2) {
        let a = segment[0];
        let d = segment[1] - a;
        let len = d.length();
        let t = if len > 0.0 {
            ((p - a).dot(d) / (len * len)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let point = a + d * t;
        let d2 = point.distance_squared(p);
        if best.is_none_or(|b| d2 < b.d2) {
            best = Some(Along {
                point,
                direction: if len > 0.0 { d / len } else { Vec2::X },
                s: acc + len * t,
                d2,
            });
        }
        acc += len;
    }
    best
}
pub fn offset_polyline(points: &[Vec2], distance: f32) -> Vec<Vec2> {
    points
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let d = (points[(i + 1).min(points.len() - 1)] - points[i.saturating_sub(1)])
                .normalize_or_zero();
            p + Vec2::new(-d.y, d.x) * distance
        })
        .collect()
}
pub fn delta(points: &[Vec2]) -> Vec<Vec2> {
    points
        .iter()
        .enumerate()
        .map(|(i, &p)| if i == 0 { p } else { p - points[i - 1] })
        .collect()
}
pub fn undelta(flat: &[f32]) -> anyhow::Result<Vec<Vec2>> {
    anyhow::ensure!(flat.len().is_multiple_of(2), "Ungerade Koordinatenliste");
    let mut point = Vec2::ZERO;
    let mut out = Vec::with_capacity(flat.len() / 2);
    for xy in flat.as_chunks::<2>().0 {
        point += Vec2::new(xy[0], xy[1]);
        anyhow::ensure!(point.is_finite(), "Nicht-endliche Koordinate");
        out.push(point);
    }
    Ok(out)
}
pub struct RingIndex {
    band: f32,
    bounds: Bounds,
    bands: HashMap<i32, Vec<(Vec2, Vec2)>>,
}
impl RingIndex {
    pub fn new(rings: &[Vec<Vec2>], band: f32) -> Self {
        let band = if band.is_finite() && band > 0.0 {
            band
        } else {
            2000.0
        };
        let bounds = Bounds::of(&rings.iter().flatten().copied().collect::<Vec<_>>());
        let mut bands: HashMap<i32, Vec<(Vec2, Vec2)>> = HashMap::new();
        for ring in rings {
            for (a, b) in edges(ring) {
                if a.y == b.y {
                    continue;
                }
                for k in (a.y.min(b.y) / band).floor() as i32..=(a.y.max(b.y) / band).floor() as i32
                {
                    bands.entry(k).or_default().push((a, b));
                }
            }
        }
        Self {
            band,
            bounds,
            bands,
        }
    }
    pub fn contains(&self, p: Vec2) -> bool {
        if !self.bounds.contains(p) {
            return false;
        }
        let Some(list) = self.bands.get(&((p.y / self.band).floor() as i32)) else {
            return false;
        };
        let mut inside = false;
        for &(a, b) in list {
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
        }
        inside
    }
}
pub fn clip_ring(ring: &[Vec2], bounds: Bounds) -> Vec<Vec2> {
    let mut points = ring.to_vec();
    for (axis, value, sign) in [
        (0, bounds.min.x, 1.0),
        (0, bounds.max.x, -1.0),
        (1, bounds.min.y, 1.0),
        (1, bounds.max.y, -1.0),
    ] {
        let mut out = Vec::new();
        for (p, c) in edges(&points) {
            let pin = (p[axis] - value) * sign >= 0.0;
            let cin = (c[axis] - value) * sign >= 0.0;
            if cin != pin {
                let t = (value - p[axis]) / (c[axis] - p[axis]);
                out.push(p.lerp(c, t));
            }
            if cin {
                out.push(c);
            }
        }
        points = out;
    }
    points
}
/// Containment tree for Canvas even/odd fill, including disconnected building rings.
pub fn ring_parents(rings: &[Vec<Vec2>]) -> Vec<Option<usize>> {
    let areas: Vec<_> = rings.iter().map(|r| signed_area(r).abs()).collect();
    rings
        .iter()
        .enumerate()
        .map(|(i, r)| {
            rings
                .iter()
                .enumerate()
                .filter(|(j, outer)| {
                    *j != i
                        && areas[*j] > areas[i]
                        && r.iter().copied().any(|p| point_in_ring(p, outer))
                })
                .min_by(|(a, _), (b, _)| areas[*a].total_cmp(&areas[*b]))
                .map(|(j, _)| j)
        })
        .collect()
}
pub fn ring_depths(parents: &[Option<usize>]) -> Vec<usize> {
    (0..parents.len())
        .map(|i| {
            let mut depth = 0;
            let mut parent = parents[i];
            while let Some(i) = parent {
                depth += 1;
                parent = parents[i];
            }
            depth
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn holes_index_and_clip() {
        let outer = vec![
            Vec2::ZERO,
            Vec2::new(10., 0.),
            Vec2::splat(10.),
            Vec2::new(0., 10.),
        ];
        let hole = vec![
            Vec2::splat(3.),
            Vec2::new(7., 3.),
            Vec2::splat(7.),
            Vec2::new(3., 7.),
        ];
        let rings = vec![outer.clone(), hole];
        let ix = RingIndex::new(&rings, 2.);
        for p in [Vec2::splat(1.), Vec2::splat(5.), Vec2::new(-1., 5.)] {
            assert_eq!(point_in_rings(p, &rings), ix.contains(p));
        }
        assert!(!ix.contains(Vec2::splat(5.)));
        assert_eq!(signed_area(&outer), 100.);
        assert_eq!(
            signed_area(&clip_ring(
                &outer,
                Bounds {
                    min: Vec2::splat(2.),
                    max: Vec2::splat(8.)
                }
            )),
            36.
        );
    }
    #[test]
    fn polyline_and_delta() {
        let p = vec![
            Vec2::new(200000., 100000.),
            Vec2::new(200003., 100000.),
            Vec2::new(200003., 100004.),
        ];
        assert_eq!(polyline_length(&p), 7.);
        assert_eq!(point_along(&p, 5.).unwrap().point, p[1] + Vec2::new(0., 2.));
        assert_eq!(point_along(&p, 100.).unwrap().point, p[2]);
        let q = project_on_polyline(&p, p[1] + Vec2::new(2., 2.)).unwrap();
        assert_eq!(q.s, 5.);
        assert_eq!(q.d2, 4.);
        let flat: Vec<_> = delta(&p).iter().flat_map(|p| [p.x, p.y]).collect();
        assert_eq!(undelta(&flat).unwrap(), p);
        assert!(point_along(&[], 0.).is_none());
        assert!(undelta(&[1.]).is_err());
    }
}
