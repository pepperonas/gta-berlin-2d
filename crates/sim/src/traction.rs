//! Wetter auf der Straße (Port des Kerns von `traction.js`): aus Nässe, Schneedecke und Glätte am Ort folgen
//! Faktoren für Bremsen, Anfahren, Seitenhalt und Lenkung. Überdachte Stellen (Durchfahrt, Boden unter einer
//! Brücke) sind trocken, Brücken frieren zuerst. Pfützen/Aquaplaning-Erkennung und Sturmböen folgen mit dem
//! Wettersystem (Phase 4); die Aquaplaning-Wirkung selbst ist in `car.rs`/`dynamics.rs` bereits enthalten.
use crate::city::{City, seg_dist2};
use crate::collision::Rect;
use std::collections::HashSet;

/// Haftungsfaktoren (1 = trocken).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Traction {
    pub brake: f64,
    pub accel: f64,
    pub lat: f64,
    pub steer: f64,
}
pub const DRY: Traction = Traction {
    brake: 1.,
    accel: 1.,
    lat: 1.,
    steer: 1.,
};
pub const WET: Traction = Traction {
    brake: 0.77,
    accel: 0.85,
    lat: 0.82,
    steer: 0.95,
};
pub const SNOW: Traction = Traction {
    brake: 0.5,
    accel: 0.55,
    lat: 0.55,
    steer: 0.8,
};
pub const ICE: Traction = Traction {
    brake: 0.33,
    accel: 0.4,
    lat: 0.35,
    steer: 0.65,
};
pub const BRIDGE_ICE: f64 = 1.5;
pub const RAIL_WET: f64 = 0.75;
pub const RAIL_ICE: f64 = 0.6;

/// Aquaplaning: ab Tempo (px/s), Dauer (s) und Restfaktoren.
pub struct Aqua;
impl Aqua {
    pub const SPEED: f64 = 70. / 0.36;
    pub const TIME: f64 = 0.35;
    pub const LAT: f64 = 0.15;
    pub const STEER: f64 = 0.2;
    pub const BRAKE: f64 = 0.3;
    pub const YAW: f64 = 0.6;
}

/// Zustand der Straße an einem Punkt.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Condition {
    pub wet: f64,
    pub snow: f64,
    pub ice: f64,
    pub covered: bool,
    pub bridge: bool,
}

/// Wetter am Boden (von der Welt fortgeschrieben, 0…1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GroundWeather {
    pub wet: f64,
    pub snow: f64,
    pub ice: f64,
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0., 1.)
}
/// Zwischen 1 (trocken) und dem Tabellenwert f nach Stärke mischen; die Ränder exakt.
fn mix(amount: f64, f: f64) -> f64 {
    let a = clamp01(amount);
    if a == 0. {
        1.
    } else if a == 1. {
        f
    } else {
        1. - (1. - f) * a
    }
}

/// Ist (x, y) auf Ebene `lvl` überdacht? Eine höhere Fahrbahn, die wirklich darüber hinweggeht (nicht der Anfang
/// einer Brücke, die hier an die eigene Ebene anschließt), oder eine Durchfahrt durch ein Haus.
pub fn covered(city: &mut City, x: f64, y: f64, lvl: i8) -> bool {
    let mut over = Vec::new();
    let mut nodes = HashSet::new();
    let mut cov = false;
    for h in city.edge_segs.query(&Rect::around(x, y, 40.)) {
        let s = *city.edge_segs.get(h);
        let Some(e) = s.edge.and_then(|id| city.edges.get(&id)) else {
            continue;
        };
        if e.lvl == lvl {
            nodes.insert(e.a);
            nodes.insert(e.b);
        }
        let half = e.w / 2.;
        if seg_dist2(x, y, s.ax, s.ay, s.bx, s.by) > half * half {
            continue;
        }
        if e.lvl > lvl {
            over.push((e.a, e.b));
        }
        if e.passage && e.lvl == lvl {
            cov = true;
        }
    }
    cov || over
        .iter()
        .any(|(a, b)| !nodes.contains(a) && !nodes.contains(b))
}

pub fn road_condition(city: &mut City, w: &GroundWeather, x: f64, y: f64, lvl: i8) -> Condition {
    let bridge = lvl >= 1;
    if covered(city, x, y, lvl) {
        return Condition {
            covered: true,
            bridge,
            ..Default::default()
        };
    }
    Condition {
        wet: clamp01(w.wet),
        snow: clamp01(w.snow),
        ice: (clamp01(w.ice) * if bridge { BRIDGE_ICE } else { 1. }).min(1.),
        covered: false,
        bridge,
    }
}

pub fn traction_of(c: &Condition) -> Traction {
    let f = |sel: fn(&Traction) -> f64| {
        sel(&ICE).max(mix(c.wet, sel(&WET)) * mix(c.snow, sel(&SNOW)) * mix(c.ice, sel(&ICE)))
    };
    Traction {
        brake: f(|t| t.brake),
        accel: f(|t| t.accel),
        lat: f(|t| t.lat),
        steer: f(|t| t.steer),
    }
}
pub fn adhesion_of(c: &Condition) -> f64 {
    RAIL_ICE.max(mix(c.wet, RAIL_WET) * mix(c.ice, RAIL_ICE))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixing_keeps_dry_exact_and_floors_at_ice() {
        assert_eq!(traction_of(&Condition::default()), DRY);
        let wet = traction_of(&Condition {
            wet: 1.,
            ..Default::default()
        });
        assert_eq!(wet, WET);
        let half = traction_of(&Condition {
            wet: 0.5,
            ..Default::default()
        });
        assert!((half.brake - (1. - 0.23 * 0.5)).abs() < 1e-12);
        // Nässe + Schnee + Glätte zusammen fallen nie unter die reine Glätte
        let worst = traction_of(&Condition {
            wet: 1.,
            snow: 1.,
            ice: 1.,
            ..Default::default()
        });
        assert_eq!(worst, ICE);
        assert_eq!(
            adhesion_of(&Condition {
                wet: 1.,
                ice: 1.,
                ..Default::default()
            }),
            RAIL_ICE
        );
    }
}
