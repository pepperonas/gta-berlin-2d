//! Straßenlaternen (Port von `lamps.js`): deterministische Standorte je Straßenkante, am Bordstein auf dem
//! Gehweg – nie auf einer Fahrbahn, in einem Haus oder im Wasser. Die Engine zeichnet daraus Lichtflecke.
use crate::city::{City, Ground, point_along};
use crate::math::hash01;
use std::collections::HashMap;

pub const MAIN_SPACING: f64 = 25.;
pub const SIDE_SPACING: f64 = 30.;
pub const CURB_GAP: f64 = 0.7;
pub const BOTH_SIDES_FROM: f64 = 10.;
pub const CORNER_GAP: f64 = 6.;
pub const MAX_CLASS: u8 = 8;
/// Lichtfarben: Gaslaternen warm, Hauptstraßen hell-neutral, Nebenstraßen warmweiß.
pub const RGB_GAS: [u8; 3] = [255, 186, 110];
pub const RGB_MAIN: [u8; 3] = [255, 236, 200];
pub const RGB_SIDE: [u8; 3] = [255, 214, 158];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lamp {
    pub x: f64,
    pub y: f64,
    /// Arm zeigt zur Fahrbahn
    pub nx: f64,
    pub ny: f64,
    pub rgb: [u8; 3],
    pub gas: bool,
    pub main: bool,
}

/// Laternen einer Kante (ohne Zwischenspeicher; siehe [`LampCache`]).
pub fn edge_lamps(city: &mut City, eid: i64) -> Vec<Lamp> {
    let mut out = Vec::new();
    let Some(e) = city.edges.get(&eid).cloned() else {
        return out;
    };
    if !(e.inside && e.cls <= MAX_CLASS && !e.bridge && !e.passage) {
        return out;
    }
    let s = city.scale;
    let main = e.cls <= 5;
    let step = if main { MAIN_SPACING } else { SIDE_SPACING } * s;
    let both = e.cs.width >= BOTH_SIDES_FROM * s;
    let sides: Vec<f64> = if both {
        vec![-1., 1.]
    } else {
        vec![if hash01((e.id * 131 + 7) as f64) < 0.5 {
            -1.
        } else {
            1.
        }]
    };
    let corner = |n: i64| {
        let Some(nd) = city.nodes.get(&n) else {
            return s;
        };
        let (mut r, mut streets) = (0f64, 0);
        for k in &nd.edges {
            if let Some(o) = city.edges.get(k)
                && o.id != e.id
                && o.cls <= MAX_CLASS
            {
                streets += 1;
                r = r.max(o.w / 2.);
            }
        }
        if streets >= 2 { r + CORNER_GAP * s } else { s }
    };
    let (s0, s1) = (corner(e.a), e.len - corner(e.b));
    let rgb = if e.cs.gaslight {
        RGB_GAS
    } else if main {
        RGB_MAIN
    } else {
        RGB_SIDE
    };
    let avail = s1 - s0;
    for side in sides {
        if avail <= 0. {
            break;
        }
        let off = side * (e.w / 2. + CURB_GAP * s);
        let n = (avail / step + hash01((e.id * 17) as f64 + side * 5. + 3.)).floor() as i64;
        let gap = avail / n.max(1) as f64;
        let phase = gap
            * if both && side > 0. { 0.25 } else { 0.75 }
            * (0.6 + 0.8 * hash01((e.id * 29) as f64 + side));
        for i in 0..n {
            let sv = s0 + avail.min((phase + i as f64 * gap) % avail);
            let p = point_along(&e.pts, sv);
            let (x, y) = (p.x - p.uy * off, p.y + p.ux * off);
            if city.tree_on_road(x, y, 0.3 * s)
                || city.in_building(x, y).is_some()
                || city.surface_at(x, y, None) == Ground::Water
            {
                continue;
            }
            out.push(Lamp {
                x,
                y,
                nx: p.uy * side,
                ny: -p.ux * side,
                rgb,
                gas: e.cs.gaslight,
                main,
            });
        }
    }
    out
}

/// Laternen je Kante, einmal berechnet (wie `e._lamps`); entfernte Kanten fallen beim nächsten Aufräumen weg.
#[derive(Default)]
pub struct LampCache {
    map: HashMap<i64, Vec<Lamp>>,
}
impl LampCache {
    /// Alle Laternen der Kanten im Umkreis.
    pub fn near(&mut self, city: &mut City, x: f64, y: f64, r: f64) -> Vec<Lamp> {
        let mut ids: Vec<i64> = city
            .edge_segs
            .query(&crate::collision::Rect::around(x, y, r))
            .into_iter()
            .filter_map(|h| city.edge_segs.get(h).edge)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let mut out = Vec::new();
        for id in ids {
            let lamps = match self.map.entry(id) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(v) => v.insert(edge_lamps(city, id)),
            };
            out.extend(
                lamps
                    .iter()
                    .filter(|l| (l.x - x).abs() < r && (l.y - y).abs() < r),
            );
        }
        if self.map.len() > 20_000 {
            self.map.retain(|id, _| city.edges.contains_key(id));
        }
        out
    }
}
