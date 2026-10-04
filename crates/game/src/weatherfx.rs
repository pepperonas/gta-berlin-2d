//! Wetter im Bild (vereinfachter Port der Ideen aus `wetfx.js`): Regenstriche und Schneeflocken als Instanzen über
//! der Stadt, nach Wind geneigt und aus Hashes und Spielzeit bewegt (keine Partikellisten, kein Zufall der
//! Simulation); Nebelschleier und Blitzlicht legt das HUD über das Bild.
use berlin_engine::Body;
use berlin_engine::hud::Hud;
use berlin_sim::math::hash01;
use berlin_sim::world::World;

const VIEW: f64 = 1500.;

pub fn bodies(w: &World, out: &mut Vec<Body>) {
    let p = w.sky.p;
    let (cx, cy) = (w.camera.x, w.camera.y);
    let half = VIEW / w.camera.zoom.max(0.5);
    let (wx, wy) = w.sky.wind;
    let t = w.time;
    let rain = p.rain.min(1.6);
    if rain > 0.02 {
        // Striche fallen schräg ins Bild: „nach unten“ (Richtung Betrachter) plus Wind
        let n = (rain / 1.6 * 900.) as usize;
        let (dx, dy) = (wx * 0.6, 520. + wy * 0.6);
        let len = (dx.hypot(dy) * 0.045).clamp(8., 34.);
        let angle = dy.atan2(dx) as f32;
        for i in 0..n {
            let fi = i as f64;
            let (h1, h2, h3) = (
                hash01(fi * 3.1 + 1.),
                hash01(fi * 7.7 + 2.),
                hash01(fi * 1.3 + 9.),
            );
            let cycle = 0.55 + h3 * 0.3;
            let u = ((t / cycle + h3) % 1.) - 0.5;
            let x = cx - half + (h1 * 2. * half + dx * u * cycle).rem_euclid(2. * half);
            let y = cy - half + (h2 * 2. * half + dy * u * cycle).rem_euclid(2. * half);
            out.push(Body {
                center: [x as f32, y as f32],
                half: [len as f32 * 0.5, 0.45],
                angle,
                shape: 0.,
                depth: 0.05,
                color: [0.75, 0.82, 0.92, 0.32],
            });
        }
    }
    let snow = p.snow;
    if snow > 0.02 {
        let n = (snow * 700.) as usize;
        for i in 0..n {
            let fi = i as f64;
            let (h1, h2, h3) = (
                hash01(fi * 5.3 + 4.),
                hash01(fi * 2.9 + 8.),
                hash01(fi * 9.1 + 3.),
            );
            let sway = (t * (0.6 + h3) + fi).sin() * 18.;
            let x = cx - half + (h1 * 2. * half + wx * t * 0.5 + sway).rem_euclid(2. * half);
            let y = cy - half
                + (h2 * 2. * half + (60. + wy * 0.5) * t * (0.6 + h3 * 0.6)).rem_euclid(2. * half);
            let r = 1.2 + h3 as f32 * 1.8;
            out.push(Body {
                center: [x as f32, y as f32],
                half: [r, r],
                angle: 0.,
                shape: 1.,
                depth: 0.05,
                color: [1., 1., 1., 0.85],
            });
        }
    }
}

/// Nebelschleier und Blitz über dem Bild.
pub fn overlay(w: &World, h: &mut Hud) {
    let fog = w.sky.p.fog.min(1.7) as f32;
    if fog > 0.02 {
        h.rect(
            0.,
            0.,
            h.width,
            720.,
            [0.78, 0.8, 0.82, (fog * 0.28).min(0.5)],
            0.,
        );
    }
    let flash = berlin_sim::weather::flash_total(w.seed, w.time, w.sky.p.thunder) as f32;
    if flash > 0.01 {
        h.rect(0., 0., h.width, 720., [0.9, 0.92, 1., flash * 0.45], 0.);
    }
}
