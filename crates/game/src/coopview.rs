//! Koop-Darstellung: welche Bildausschnitte gerade zu sehen sind (Aussortieren von Körpern, Lichtern, Schildern)
//! und wo ein Weltpunkt auf dem Bildschirm liegt, wenn das Bild geteilt ist.

use berlin_engine::camera::Camera;
use berlin_engine::split::Views;

/// Ausschnitte dieses Bildes: Mitte und Zoom je Ansicht (ohne Koop genau die Kamera der Welt).
#[derive(Debug, Clone, Copy)]
pub struct Spots {
    s: [(f64, f64, f64); 2],
    n: usize,
}

impl Spots {
    pub fn one(x: f64, y: f64, zoom: f64) -> Self {
        Self {
            s: [(x, y, zoom); 2],
            n: 1,
        }
    }
    pub fn from_views(v: &Views) -> Self {
        let p = |c: &Camera| (c.position.x as f64, c.position.y as f64, c.zoom as f64);
        Self {
            s: [p(&v.cams[0]), p(&v.cams[1])],
            n: v.count.clamp(1, 2),
        }
    }
    pub fn all(&self) -> &[(f64, f64, f64)] {
        &self.s[..self.n]
    }
    /// Liegt (x, y) nahe irgendeiner Ansicht? `half(zoom)` = halbe Breite und Höhe des Bereichs.
    pub fn near(&self, x: f64, y: f64, half: impl Fn(f64) -> (f64, f64)) -> bool {
        self.all().iter().any(|&(cx, cy, z)| {
            let (hx, hy) = half(z);
            (x - cx).abs() < hx && (y - cy).abs() < hy
        })
    }
    /// Abstand zur nächsten Bildmitte.
    pub fn dist(&self, x: f64, y: f64) -> f64 {
        self.all()
            .iter()
            .map(|&(cx, cy, _)| (x - cx).hypot(y - cy))
            .fold(f64::INFINITY, f64::min)
    }
    /// Abfrage je Ansicht, zusammengeführt ohne Doppelte (gleicher Ort). Bei einer Ansicht genau eine Abfrage.
    pub fn gather<T>(
        &self,
        mut query: impl FnMut(f64, f64, f64) -> Vec<T>,
        pos: impl Fn(&T) -> (f64, f64),
    ) -> Vec<T> {
        if self.n == 1 {
            let (x, y, z) = self.s[0];
            return query(x, y, z);
        }
        let mut out: Vec<T> = Vec::new();
        for &(x, y, z) in self.all() {
            for t in query(x, y, z) {
                let p = pos(&t);
                if !out.iter().any(|o| pos(o) == p) {
                    out.push(t);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use berlin_engine::split::split_views;
    use glam::Vec2;

    fn cam(x: f32, y: f32) -> Camera {
        Camera {
            position: Vec2::new(x, y),
            zoom: 1.,
            scale: 1.,
        }
    }

    #[test]
    fn single_spot_queries_once_and_keeps_order() {
        let s = Spots::one(0., 0., 1.);
        let mut calls = 0;
        let v = s.gather(
            |_, _, _| {
                calls += 1;
                vec![(1., 1.), (1., 1.)]
            },
            |p| *p,
        );
        assert_eq!(calls, 1);
        assert_eq!(v.len(), 2, "eine Ansicht: unverändert, auch Doppelte");
    }

    #[test]
    fn two_spots_merge_without_duplicates() {
        let v = split_views(&cam(0., 0.), &cam(9000., 0.), Vec2::new(1280., 720.));
        let s = Spots::from_views(&v);
        assert_eq!(s.all().len(), 2);
        let got = s.gather(|x, _, _| vec![(x, 0.), (5., 5.)], |p| *p);
        assert_eq!(got.len(), 3, "der gemeinsame Punkt nur einmal");
        assert!(
            s.near(9000., 0., |_| (400., 400.)),
            "Spieler 2 sitzt in seiner Bildhälfte"
        );
        assert!(!s.near(4500., 2000., |_| (100., 100.)));
        assert!(s.dist(9000., 10.) < 700.);
    }
}
