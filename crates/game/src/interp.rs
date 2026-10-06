//! Bild-Interpolation: Die Welt rechnet mit festen 60 Hz, gezeichnet wird mit der Bildrate des Bildschirms.
//! Damit sich Fahrzeuge, Figuren und Kamera auf 120-Hz-Bildschirmen nicht in 60-Hz-Stufen bewegen, zeichnet das
//! Spiel zwischen dem vorigen und dem aktuellen Simulationsstand (`alpha` = Anteil des angebrochenen Schritts).
//!
//! Die Zeichenfunktionen lesen die Welt direkt. Statt jede Stelle anzupassen, schreibt `apply` die
//! interpolierten Lagen vor dem Bild in die Welt und `restore` danach die exakten Werte zurück – die Simulation
//! sieht nie einen interpolierten Wert, sie bleibt bitgenau.
use berlin_sim::world::World;
use std::collections::HashMap;

/// Weiter als so viele Pixel je Schritt = Teleport: nicht interpolieren.
pub const JUMP: f64 = 150.;

type Pose = (f64, f64, f64);

#[derive(Default)]
pub struct Interp {
    cars: HashMap<u32, Pose>,
    peds: HashMap<u32, Pose>,
    player: Option<Pose>,
    camera: Option<(f64, f64)>,
    /// Spieler 2 (Koop): Figur und Kamera
    p2: Option<(Pose, (f64, f64))>,
    saved: Option<Saved>,
}

struct Saved {
    cars: Vec<(usize, Pose)>,
    peds: Vec<(usize, Pose)>,
    player: Pose,
    camera: (f64, f64),
    p2: Option<(Pose, (f64, f64))>,
}

/// Winkel auf dem kürzesten Weg mischen.
pub fn lerp_angle(a: f64, b: f64, t: f64) -> f64 {
    let d = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    a + d * t
}

fn mix(p: Pose, c: Pose, t: f64) -> Option<Pose> {
    if (c.0 - p.0).hypot(c.1 - p.1) > JUMP {
        return None;
    }
    Some((
        p.0 + (c.0 - p.0) * t,
        p.1 + (c.1 - p.1) * t,
        lerp_angle(p.2, c.2, t),
    ))
}

impl Interp {
    /// Stand vor einem Simulationsschritt merken.
    pub fn record(&mut self, w: &World) {
        self.cars.clear();
        self.cars
            .extend(w.cars.iter().map(|c| (c.id, (c.x, c.y, c.angle))));
        self.peds.clear();
        self.peds
            .extend(w.peds.iter().map(|p| (p.id, (p.x, p.y, p.facing))));
        self.player = Some((w.player.x, w.player.y, w.player.angle));
        self.camera = Some((w.camera.x, w.camera.y));
        self.p2 = w.p2.as_ref().map(|s| {
            (
                (s.player.x, s.player.y, s.player.angle),
                (s.camera.x, s.camera.y),
            )
        });
    }

    /// Interpolierte Lagen für das Bild einsetzen (`alpha` 0…1); `restore` macht es rückgängig.
    pub fn apply(&mut self, w: &mut World, alpha: f64) {
        if self.saved.is_some() {
            self.restore(w);
        }
        let t = alpha.clamp(0., 1.);
        let mut saved = Saved {
            cars: Vec::new(),
            peds: Vec::new(),
            player: (w.player.x, w.player.y, w.player.angle),
            camera: (w.camera.x, w.camera.y),
            p2: w.p2.as_ref().map(|s| {
                (
                    (s.player.x, s.player.y, s.player.angle),
                    (s.camera.x, s.camera.y),
                )
            }),
        };
        for (i, c) in w.cars.iter_mut().enumerate() {
            let cur = (c.x, c.y, c.angle);
            if let Some(m) = self.cars.get(&c.id).and_then(|&p| mix(p, cur, t)) {
                saved.cars.push((i, cur));
                (c.x, c.y, c.angle) = m;
            }
        }
        for (i, p) in w.peds.iter_mut().enumerate() {
            let cur = (p.x, p.y, p.facing);
            if let Some(m) = self.peds.get(&p.id).and_then(|&q| mix(q, cur, t)) {
                saved.peds.push((i, cur));
                (p.x, p.y, p.facing) = m;
            }
        }
        if let Some(m) = self.player.and_then(|p| mix(p, saved.player, t)) {
            (w.player.x, w.player.y, w.player.angle) = m;
        }
        if let Some((px, py)) = self.camera {
            let (cx, cy) = saved.camera;
            if (cx - px).hypot(cy - py) <= JUMP {
                w.camera.x = px + (cx - px) * t;
                w.camera.y = py + (cy - py) * t;
            }
        }
        if let (Some(s), Some((pp, pc)), Some((cp, cc))) = (w.p2.as_mut(), self.p2, saved.p2) {
            if let Some(m) = mix(pp, cp, t) {
                (s.player.x, s.player.y, s.player.angle) = m;
            }
            if (cc.0 - pc.0).hypot(cc.1 - pc.1) <= JUMP {
                s.camera.x = pc.0 + (cc.0 - pc.0) * t;
                s.camera.y = pc.1 + (cc.1 - pc.1) * t;
            }
        }
        self.saved = Some(saved);
    }

    /// Exakte Lagen zurückschreiben.
    pub fn restore(&mut self, w: &mut World) {
        let Some(s) = self.saved.take() else {
            return;
        };
        for (i, (x, y, a)) in s.cars {
            if let Some(c) = w.cars.get_mut(i) {
                (c.x, c.y, c.angle) = (x, y, a);
            }
        }
        for (i, (x, y, a)) in s.peds {
            if let Some(p) = w.peds.get_mut(i) {
                (p.x, p.y, p.facing) = (x, y, a);
            }
        }
        (w.player.x, w.player.y, w.player.angle) = s.player;
        (w.camera.x, w.camera.y) = s.camera;
        if let (Some(p2), Some((pose, cam))) = (w.p2.as_mut(), s.p2) {
            (p2.player.x, p2.player.y, p2.player.angle) = pose;
            (p2.camera.x, p2.camera.y) = cam;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_mix_the_short_way() {
        let a = lerp_angle(3.1, -3.1, 0.5);
        assert!((a.abs() - std::f64::consts::PI).abs() < 0.01, "{a}");
        assert!((lerp_angle(0., 1., 0.25) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn teleports_are_not_interpolated() {
        assert!(mix((0., 0., 0.), (JUMP + 1., 0., 0.), 0.5).is_none());
        assert_eq!(mix((0., 0., 0.), (10., 0., 0.), 0.5), Some((5., 0., 0.)));
    }
}
