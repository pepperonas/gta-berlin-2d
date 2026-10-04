//! Ebenen-Ansicht der Befehlszeile `ebenen` (`render.js drawLevelDebug`): Straßen und Wege in der Farbe ihrer Ebene
//! mit der Zahl in der Mitte, Portale (wo Ebenen ineinander übergehen) als gestrichelte Kreise mit „unten..oben“ (die Bitmapschrift kennt kein „…“).
//! Ebene 0 ist halbtransparent weiß; reine Wege auf Ebene 0 entfallen (sie wären nur Rauschen).
use berlin_engine::camera::Camera;
use berlin_engine::hud::{Align, Hud};
use berlin_sim::collision::Rect;
use berlin_sim::world::World;
use glam::Vec2;
use std::collections::HashSet;

/// Farbe je Ebene wie im Browser (−2 lila, −1 blau, 0 weiß, 1 orange, 2 rot, 3 rosa).
pub fn level_color(lvl: i8) -> [f32; 4] {
    match lvl {
        -2 => [0.557, 0.267, 0.678, 1.],
        -1 => [0.161, 0.502, 0.725, 1.],
        0 => [1., 1., 1., 0.5],
        1 => [0.953, 0.612, 0.071, 1.],
        2 => [0.906, 0.298, 0.235, 1.],
        3 => [1., 0.4, 0.8, 1.],
        _ => [1., 1., 1., 1.],
    }
}
const PORTAL: [f32; 4] = [0., 0.898, 1., 1.];

pub fn draw(w: &mut World, cam: &Camera, vp: Vec2, h: &mut Hud) {
    let s = h.scale;
    let px = |x: f64, y: f64| cam.world_to_screen(Vec2::new(x as f32, y as f32), 0., vp) / s;
    // sichtbarer Ausschnitt in Kartenpixeln
    let corners = [Vec2::ZERO, Vec2::new(vp.x, 0.), Vec2::new(0., vp.y), vp]
        .map(|c| cam.screen_to_ground(c, vp));
    let (x0, x1) = corners
        .iter()
        .fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.x), a.1.max(c.x)));
    let (y0, y1) = corners
        .iter()
        .fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.y), a.1.max(c.y)));
    let view = Rect::new(x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64);
    let city = &mut w.city;
    // Straßen (Kanten): einmal je Kante, Zahl in der Mitte
    let mut seen = HashSet::new();
    let mut labels = Vec::new();
    for hd in city.edge_segs.query(&view) {
        let Some(id) = city.edge_segs.get(hd).edge else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        let Some(e) = city.edges.get(&id) else {
            continue;
        };
        let c = level_color(e.lvl);
        for p in e.pts.windows(2) {
            let (a, b) = (px(p[0].0, p[0].1), px(p[1].0, p[1].1));
            h.line(a.x, a.y, b.x, b.y, 2., c);
        }
        let m = e.pts[e.pts.len() / 2];
        labels.push((px(m.0, m.1), e.lvl));
    }
    // Wege (Fuß-, Rad-, Brückenwege) nur abseits von Ebene 0
    for hd in city.paths.query(&view) {
        let p = city.paths.get(hd);
        if p.lvl == 0 || p.pts.len() < 2 {
            continue;
        }
        let c = level_color(p.lvl);
        for q in p.pts.windows(2) {
            let (a, b) = (px(q[0].0, q[0].1), px(q[1].0, q[1].1));
            h.line(a.x, a.y, b.x, b.y, 2., c);
        }
        let m = p.pts[p.pts.len() / 2];
        labels.push((px(m.0, m.1), p.lvl));
    }
    for (at, lvl) in labels {
        h.rect(at.x - 8., at.y - 7., 16., 14., [0., 0., 0., 0.7], 2.);
        h.text(
            &lvl.to_string(),
            at.x,
            at.y + 4.,
            11.,
            level_color(lvl),
            Align::Center,
            false,
        );
    }
    // Portale: gestrichelter Kreis, darüber „unten…oben“
    for hd in city.portals.query(&view) {
        let p = *city.portals.get(hd);
        let c = px(p.x, p.y);
        let r = (cam.world_to_screen(Vec2::new((p.x + p.r) as f32, p.y as f32), 0., vp) / s - c)
            .length();
        let n = 24;
        for i in (0..n).step_by(2) {
            let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
            h.line(
                c.x + a0.cos() * r,
                c.y + a0.sin() * r,
                c.x + a1.cos() * r,
                c.y + a1.sin() * r,
                2.,
                PORTAL,
            );
        }
        h.text(
            &format!("{}..{}", p.lo, p.hi),
            c.x,
            c.y - r - 6.,
            11.,
            PORTAL,
            Align::Center,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_follow_the_browser_palette() {
        assert_eq!(level_color(0)[3], 0.5, "Ebene 0 halbtransparent");
        assert!(
            level_color(1)[0] > 0.9 && level_color(1)[2] < 0.1,
            "Brücke orange"
        );
        assert!(level_color(-1)[2] > level_color(-1)[0], "Unterführung blau");
        assert_ne!(level_color(2), level_color(3));
        assert_eq!(level_color(7), [1.; 4], "unbekannt weiß");
    }
}
