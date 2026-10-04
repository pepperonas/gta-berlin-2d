//! Reifenspuren im Schnee (Port von `snowtracks.js`, nur Darstellung): je Auto die Spuren der Räder als Stücke in
//! einem Ringpuffer mit Zeitstempel (Weltzeit, damit Pause und Zeitlupe stimmen). Sie verblassen mit der Zeit und
//! schneller, solange es schneit; ohne Schneedecke sind sie unsichtbar. Die Simulation bleibt unberührt.
use berlin_sim::car::Car;
use std::collections::HashMap;

pub const MAX: usize = 12000;
/// px: neues Stück erst nach dieser Strecke
pub const STEP: f64 = 6.;
pub const INSET: f64 = 1.2;
pub const REAR: f64 = 0.57;
pub const FRONT: f64 = 0.62;
/// px: weiter = Sprung (Teleport), keine Spur
pub const JUMP: f64 = 150.;
/// s Lebensdauer ohne Schneefall
pub const LIFE: f64 = 240.;
pub const MIN_DEPTH: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub a: (f64, f64),
    pub b: (f64, f64),
    pub t: f64,
}

#[derive(Debug, Default)]
pub struct Trails {
    pub pieces: Vec<Piece>,
    head: usize,
    last: HashMap<u32, (Vec<(f64, f64)>, u64)>,
    frame: u64,
}

fn two_wheeled(kind: &str) -> bool {
    matches!(kind, "bicycle" | "escooter" | "motorcycle" | "scooter")
}

/// Aufstandspunkte der Räder (car.hw = halbe Länge, car.hh = halbe Breite): vier, ein Zweirad zwei.
pub fn wheels(c: &Car) -> Vec<(f64, f64)> {
    let (ca, sa) = (c.angle.cos(), c.angle.sin());
    let (r, f) = (c.hw * REAR, c.hw * FRONT);
    if two_wheeled(c.kind) {
        return vec![(c.x - ca * r, c.y - sa * r), (c.x + ca * f, c.y + sa * f)];
    }
    let off = (c.hh - INSET).max(1.);
    let (ox, oy) = (-sa * off, ca * off);
    let (rx, ry, fx, fy) = (c.x - ca * r, c.y - sa * r, c.x + ca * f, c.y + sa * f);
    vec![
        (rx + ox, ry + oy),
        (rx - ox, ry - oy),
        (fx + ox, fy + oy),
        (fx - ox, fy - oy),
    ]
}

impl Trails {
    fn push(&mut self, a: (f64, f64), b: (f64, f64), t: f64) {
        let p = Piece { a, b, t };
        if self.pieces.len() < MAX {
            self.pieces.push(p);
        } else {
            self.pieces[self.head] = p;
        }
        self.head = (self.head + 1) % MAX;
    }
    /// Einmal je Bild: neue Stücke für alle Autos am Boden, die seit dem letzten Stück weit genug gefahren sind.
    pub fn record(&mut self, cars: &[Car], t: f64, depth: f64) {
        self.frame += 1;
        let frame = self.frame;
        if depth < MIN_DEPTH {
            self.last.clear();
            return;
        }
        for c in cars {
            if c.lvl() != 0 {
                self.last.remove(&c.id);
                continue;
            }
            let w = wheels(c);
            let Some((prev, g)) = self.last.get_mut(&c.id) else {
                self.last.insert(c.id, (w, frame));
                continue;
            };
            *g = frame;
            if prev.len() != w.len() {
                *prev = w;
                continue;
            }
            let d = prev
                .iter()
                .zip(&w)
                .map(|(p, q)| (p.0 - q.0).hypot(p.1 - q.1))
                .fold(0., f64::max);
            if d < STEP {
                continue;
            }
            let old = std::mem::replace(prev, w.clone());
            if d < JUMP {
                for (p, q) in old.into_iter().zip(w) {
                    self.push(p, q, t);
                }
            }
        }
        if frame.is_multiple_of(60) {
            self.last.retain(|_, (_, g)| *g == frame);
        }
    }
    pub fn clear(&mut self) {
        self.pieces.clear();
        self.head = 0;
        self.last.clear();
    }
}

/// Deckkraft eines Stücks: blasser mit dem Alter, Lebenszeit kürzer bei Schneefall.
pub fn alpha(age: f64, depth: f64, snowfall: f64) -> f64 {
    if depth < MIN_DEPTH {
        return 0.;
    }
    let life = LIFE / (1. + 4. * snowfall.max(0.));
    if age >= life {
        return 0.;
    }
    (depth * 1.4).min(1.) * 0.55 * (1. - age / life).powf(1.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use berlin_sim::car::Role;
    #[test]
    fn tracks_follow_the_wheels_and_fade() {
        let mut c = Car::new(1, 0., 0., 0., 0, Role::Traffic, "car");
        let mut tr = Trails::default();
        tr.record(std::slice::from_ref(&c), 0., 0.5);
        assert!(
            tr.pieces.is_empty(),
            "erst der zweite Punkt zieht eine Spur"
        );
        c.x = 4.;
        tr.record(std::slice::from_ref(&c), 0.1, 0.5);
        assert!(tr.pieces.is_empty(), "unter 6 px kein Stück");
        c.x = 10.;
        tr.record(std::slice::from_ref(&c), 0.2, 0.5);
        assert_eq!(tr.pieces.len(), 4, "vier Räder");
        c.x = 500.;
        tr.record(std::slice::from_ref(&c), 0.3, 0.5);
        assert_eq!(tr.pieces.len(), 4, "Sprung = keine Spur");
        tr.record(std::slice::from_ref(&c), 0.4, 0.);
        assert!(alpha(0., 0.5, 0.) > 0.3);
        assert!(alpha(100., 0.5, 1.) == 0., "Schneefall deckt zu");
        assert!(alpha(10., 0., 0.) == 0., "ohne Schneedecke unsichtbar");
        let mut m = Car::new(2, 0., 0., 0., 0, Role::Traffic, "motorcycle");
        m.kind = "motorcycle";
        assert_eq!(wheels(&m).len(), 2);
    }
}
