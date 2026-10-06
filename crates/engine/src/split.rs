//! Geteilter Bildschirm für zwei Spieler – ein Bild, das sich nahtlos in zwei teilt.
//!
//! Verfahren („Voronoi-Split“): solange beide Spieler in die innere Hälfte eines gemeinsamen Bildes passen, zeigen
//! beide Ansichten genau dasselbe Bild um die Mitte zwischen ihnen – die Trennung ist unsichtbar. Entfernen sie sich,
//! rückt jede Kamera so weit vor ihren Spieler, dass er in der Mitte seiner Bildhälfte bleibt; die Trennlinie steht
//! senkrecht zur Verbindung der beiden und dreht mit ihr. Alles ist eine stetige Funktion der Spielerorte: kein
//! Sprung beim Teilen, kein Schnitt beim Zusammenführen.
//!
//! Rein (ohne GPU), damit Geometrie und Stetigkeit testbar sind.

use crate::camera::Camera;
use glam::Vec2;

/// Wie weit das gemeinsame Bild höchstens herauszoomt, bevor es sich teilt (Anteil des Wunschzooms).
pub const FIT_MIN: f32 = 0.8;
/// Spanne (Anteil des Zooms unterhalb `FIT_MIN`), über die der Zoom nach dem Teilen zum Wunschzoom zurückkehrt.
pub const FIT_RETURN: f32 = 0.3;
/// Über welche Strecke (Anteil der kürzeren Bildseite) die Trennlinie nach dem Teilen eingeblendet wird.
pub const LINE_FADE: f32 = 0.08;

/// Ergebnis: eine oder zwei Ansichten mit derselben Bildfläche.
#[derive(Debug, Clone)]
pub struct Views {
    /// Kamera je Spieler (bei `count == 1` sind beide gleich).
    pub cams: [Camera; 2],
    /// 1 = gemeinsames Bild, 2 = geteilt.
    pub count: usize,
    /// Bildschirm-Richtung von Spieler 1 zu Spieler 2 (Einheitsvektor, y nach unten). Ein Pixel `p` gehört zu
    /// Spieler 2, wenn `(p − Bildmitte) · normal > 0`.
    pub normal: Vec2,
    /// Deckkraft der Trennlinie 0…1.
    pub line: f32,
}

impl Views {
    /// Eine Ansicht (Einzelspieler).
    pub fn single(cam: Camera) -> Self {
        Self {
            cams: [cam.clone(), cam],
            count: 1,
            normal: Vec2::X,
            line: 0.,
        }
    }
    /// Welche Ansicht zeigt den Bildschirmpunkt `p`?
    pub fn view_at(&self, p: Vec2, viewport: Vec2) -> usize {
        if self.count == 2 && (p - viewport * 0.5).dot(self.normal) > 0. {
            1
        } else {
            0
        }
    }
}

/// Abstand von der Bildmitte zum Bildrand entlang der Richtung `dir` (Einheitsvektor), in Pixeln.
pub fn edge_distance(dir: Vec2, viewport: Vec2) -> f32 {
    let hx = if dir.x.abs() > 1e-6 {
        viewport.x * 0.5 / dir.x.abs()
    } else {
        f32::INFINITY
    };
    let hy = if dir.y.abs() > 1e-6 {
        viewport.y * 0.5 / dir.y.abs()
    } else {
        f32::INFINITY
    };
    hx.min(hy)
}

/// Zoomfaktor (Anteil des Wunschzooms) aus dem Passwert `fit` = Zoom, bei dem beide genau in die innere Hälfte
/// passen, geteilt durch den Wunschzoom. Bis `FIT_MIN` zoomt das Bild heraus, darunter wird geteilt und der Zoom
/// kehrt über `FIT_RETURN` stetig zum Wunschzoom zurück.
pub fn zoom_factor(fit: f32) -> f32 {
    if fit >= 1. {
        1.
    } else if fit >= FIT_MIN {
        fit
    } else {
        FIT_MIN + (1. - FIT_MIN) * ((FIT_MIN - fit) / FIT_RETURN).clamp(0., 1.)
    }
}

/// Ansichten für zwei Spieler. `a`, `b` = Wunschkamera je Spieler (Position = sein Kameraziel, Zoom wie im
/// Einzelspiel); gemeinsamer Zoom ist der kleinere der beiden.
pub fn split_views(a: &Camera, b: &Camera, viewport: Vec2) -> Views {
    let scale = a.scale;
    let want = a.zoom.min(b.zoom);
    let px = want * scale;
    let delta = b.position - a.position;
    let len = delta.length();
    let mid = (a.position + b.position) * 0.5;
    let cam = |position: Vec2, zoom: f32| Camera {
        position,
        zoom,
        scale,
    };
    if len < 1e-3 || px <= 0. || viewport.min_element() <= 0. {
        return Views::single(cam(mid, want));
    }
    let dir = delta / len;
    let edge = edge_distance(dir, viewport);
    // im gemeinsamen Bild stehen die Spieler bei ±Abstand/2 um die Mitte; teilen, sobald einer die halbe
    // Strecke zum Rand überschreitet (dort säße er im geteilten Bild in der Mitte seiner Hälfte)
    let fit = edge / (len * px);
    let zoom = want * zoom_factor(fit);
    let dist = len * zoom * scale;
    let over = dist - edge;
    if over <= 0. {
        let mut v = Views::single(cam(mid, zoom));
        v.normal = dir;
        return v;
    }
    let shift = dir * (edge * 0.5) / (zoom * scale);
    Views {
        cams: [cam(a.position + shift, zoom), cam(b.position - shift, zoom)],
        count: 2,
        normal: dir,
        line: (over / (LINE_FADE * viewport.min_element())).clamp(0., 1.),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VP: Vec2 = Vec2::new(1280., 720.);

    fn cam(x: f32, y: f32) -> Camera {
        Camera {
            position: Vec2::new(x, y),
            zoom: 1.,
            scale: 1.,
        }
    }

    fn screen(v: &Views, i: usize, p: Vec2) -> Vec2 {
        v.cams[i].world_to_screen(p, 0., VP)
    }

    #[test]
    fn close_players_share_one_view_centred_between_them() {
        let (a, b) = (cam(0., 0.), cam(200., 100.));
        let v = split_views(&a, &b, VP);
        assert_eq!(v.count, 1);
        assert_eq!(v.cams[0].position, Vec2::new(100., 50.));
        assert_eq!(v.cams[0].position, v.cams[1].position);
        assert_eq!(v.line, 0.);
        assert_eq!(v.cams[0].zoom, 1.);
    }

    #[test]
    fn view_zooms_out_before_splitting() {
        // 700 px auseinander (waagrecht): passt nur mit Zoom 640/700 in die innere Hälfte
        let v = split_views(&cam(0., 0.), &cam(700., 0.), VP);
        assert_eq!(v.count, 1);
        assert!(
            (v.cams[0].zoom - 640. / 700.).abs() < 1e-4,
            "{}",
            v.cams[0].zoom
        );
    }

    #[test]
    fn far_players_sit_centred_in_their_half() {
        for (dx, dy) in [(5000., 0.), (0., 5000.), (-4000., 3000.), (3000., -3000.)] {
            let (a, b) = (cam(0., 0.), cam(dx, dy));
            let v = split_views(&a, &b, VP);
            assert_eq!(v.count, 2);
            assert_eq!(v.line, 1.);
            // weit auseinander: wieder der Wunschzoom
            assert!((v.cams[0].zoom - 1.).abs() < 1e-5);
            let dir = Vec2::new(dx, dy).normalize();
            let edge = edge_distance(dir, VP);
            let pa = screen(&v, 0, a.position) - VP * 0.5;
            let pb = screen(&v, 1, b.position) - VP * 0.5;
            assert!((pa + dir * edge * 0.5).length() < 0.01, "{pa:?}");
            assert!((pb - dir * edge * 0.5).length() < 0.01, "{pb:?}");
            // jeder Spieler liegt in seiner Hälfte
            assert_eq!(v.view_at(pa + VP * 0.5, VP), 0);
            assert_eq!(v.view_at(pb + VP * 0.5, VP), 1);
        }
    }

    #[test]
    fn line_turns_with_the_players() {
        let side = split_views(&cam(0., 0.), &cam(5000., 0.), VP);
        assert!((side.normal - Vec2::X).length() < 1e-6);
        let above = split_views(&cam(0., 5000.), &cam(0., 0.), VP);
        assert!((above.normal - Vec2::NEG_Y).length() < 1e-6);
        let diag = split_views(&cam(0., 0.), &cam(3000., 3000.), VP);
        assert!((diag.normal - Vec2::splat(0.5f32.sqrt())).length() < 1e-5);
    }

    #[test]
    fn moving_apart_never_jumps() {
        // Spieler 2 entfernt sich in kleinen Schritten über die Teilungsgrenze hinaus: Kameras und Zoom ändern
        // sich je Schritt nur wenig (stetig)
        let a = cam(0., 0.);
        let mut prev = split_views(&a, &cam(10., 0.), VP);
        let mut split_seen = false;
        for i in 1..4000 {
            let x = 10. + i as f32 * 1.;
            let v = split_views(&a, &cam(x, x * 0.3), VP);
            split_seen |= v.count == 2;
            for k in 0..2 {
                let d = v.cams[k].position.distance(prev.cams[k].position);
                assert!(d < 2.5, "Sprung {d} bei x={x}, Ansicht {k}");
            }
            assert!(
                (v.cams[0].zoom - prev.cams[0].zoom).abs() < 0.003,
                "Zoomsprung bei {x}"
            );
            assert!((v.line - prev.line).abs() < 0.05);
            prev = v;
        }
        assert!(split_seen);
    }

    #[test]
    fn smaller_wish_zoom_wins() {
        let a = Camera {
            zoom: 1.6,
            ..cam(0., 0.)
        };
        let b = Camera {
            zoom: 1.1,
            ..cam(10., 0.)
        };
        assert_eq!(split_views(&a, &b, VP).cams[0].zoom, 1.1);
    }

    #[test]
    fn zoom_factor_is_continuous_and_bounded() {
        let mut prev = zoom_factor(2.);
        for i in 0..2000 {
            let f = 2. - i as f32 * 0.001;
            let z = zoom_factor(f);
            assert!((FIT_MIN..=1.).contains(&z));
            assert!((z - prev).abs() < 0.002);
            prev = z;
        }
        assert_eq!(zoom_factor(0.1), 1.);
    }

    #[test]
    fn single_view_belongs_to_player_one() {
        let v = Views::single(cam(0., 0.));
        assert_eq!(v.view_at(Vec2::new(1270., 10.), VP), 0);
    }
}
