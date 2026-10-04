//! Unter Tage (Port von `tunnel.js`): U- und S-Bahn fahren dort im Tunnel, wo die Karte kein sichtbares Gleis hat
//! (der Build verwirft Tunnelgleise). Dieselbe Regel entscheidet, ob Züge oben gezeichnet werden. Eine noch nicht
//! geladene Kachel gilt als oberirdisch. Ein Gleis zählt nur, wenn seine Richtung zur Fahrtrichtung passt (±35°):
//! am Kottbusser Tor liegt der U8-Tunnel nur ~5 m unter dem querenden U1-Viadukt.
use crate::city::{City, seg_dist2};
use crate::collision::Rect;
use crate::transit::{Mode, Pattern, Shape, point_on_shape};
use std::collections::HashMap;

/// px: so nah muss ein Gleis sein
pub const PROBE: f64 = 50.;
/// px: Raster des Zwischenspeichers je Muster
pub const STEP: f64 = 60.;
pub const MAX_ANGLE_DEG: f64 = 35.;

/// Liegt ein (oberirdisches) Gleis bei (x, y)? Mit Fahrtrichtung zählt nur ein passend ausgerichtetes.
pub fn rail_at(city: &mut City, x: f64, y: f64, dir: Option<(f64, f64)>, r: f64) -> bool {
    let max_cos = MAX_ANGLE_DEG.to_radians().cos();
    let (hx, hy) = dir.map_or((0., 0.), |(dx, dy)| {
        let l = dx.hypot(dy);
        if l > 1e-9 { (dx / l, dy / l) } else { (0., 0.) }
    });
    let any = hx == 0. && hy == 0.;
    for h in city.rails.query(&Rect::around(x, y, r)) {
        let p = &city.rails.get(h).pts;
        for w in p.windows(2) {
            let (a, b) = (w[0], w[1]);
            if seg_dist2(x, y, a.0, a.1, b.0, b.1) >= r * r {
                continue;
            }
            if any {
                return true;
            }
            let (sx, sy) = (b.0 - a.0, b.1 - a.1);
            let sl = sx.hypot(sy);
            if sl > 1e-9 && ((sx * hx + sy * hy) / sl).abs() >= max_cos {
                return true;
            }
        }
    }
    false
}

/// Ebene des nächsten Gleises bei (x, y) (ohne Richtung), `None` = kein Gleis.
pub fn rail_level_at(city: &mut City, x: f64, y: f64, r: f64) -> Option<i8> {
    let mut best: Option<(f64, i8)> = None;
    for h in city.rails.query(&Rect::around(x, y, r)) {
        let l = city.rails.get(h);
        for w in l.pts.windows(2) {
            let d = seg_dist2(x, y, w[0].0, w[0].1, w[1].0, w[1].1);
            if d < r * r && best.is_none_or(|b| d < b.0) {
                best = Some((d, l.lvl.max(i8::from(l.bridge))));
            }
        }
    }
    best.map(|b| b.1)
}

pub fn underground_at(
    city: &mut City,
    mode: Mode,
    x: f64,
    y: f64,
    dir: Option<(f64, f64)>,
) -> bool {
    if !mode.rail() || !city.ready(x, y, PROBE) {
        return false;
    }
    !rail_at(city, x, y, dir, PROBE)
}

/// Zwischenspeicher je Muster und 60-px-Stück, gültig bis sich der Kachelstand ändert.
#[derive(Debug, Clone, Default)]
pub struct Cache {
    generation: u64,
    map: HashMap<(usize, i64), bool>,
}
impl Cache {
    pub fn underground_at_s(&mut self, city: &mut City, p: &Pattern, sh: &Shape, s: f64) -> bool {
        if !p.mode.rail() {
            return false;
        }
        if self.generation != city.generation {
            self.generation = city.generation;
            self.map.clear();
        }
        let k = (s / STEP).round() as i64;
        if let Some(&v) = self.map.get(&(p.id, k)) {
            return v;
        }
        let (x, y, a) = point_on_shape(sh, k as f64 * STEP);
        let v = underground_at(city, p.mode, x, y, Some((a.cos(), a.sin())));
        if city.ready(x, y, PROBE) {
            self.map.insert((p.id, k), v); // Unfertiges nicht merken
        }
        v
    }
}
