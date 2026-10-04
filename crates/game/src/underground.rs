//! Unter Tage (Port von `stationview.js` und `tunnelview.js`): Eingänge an der Straße (Treppenschacht mit Geländer
//! und U-/S-Schild), der U-Bahnhof von innen und die Tunnelansicht während einer Fahrt unter Tage. Innen- und
//! Tunnelansicht liegen im Bildraum über der Stadt (nach dem Lichtpass): der Bahnhof ist immer beleuchtet, und von der
//! Stadt bleibt unter Tage nur ein dunkler Schleier. Nur Darstellung.
use berlin_engine::Body;
use berlin_engine::camera::Camera;
use berlin_engine::hud::{Align, Hud};
use berlin_sim::ride::Ref;
use berlin_sim::station::{self as stn, Station};
use berlin_sim::transit::{Mode, point_on_shape, position_at, train_cars};
use berlin_sim::world::World;
use glam::Vec2;

const U_BLUE: [f32; 4] = [0.114, 0.31, 0.569, 1.];
const S_GREEN: [f32; 4] = [0., 0.553, 0.31, 1.];

fn rgb(c: u32, a: f32) -> [f32; 4] {
    [
        ((c >> 16) & 255) as f32 / 255.,
        ((c >> 8) & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
        a,
    ]
}
fn shade(c: [f32; 4], k: f32) -> [f32; 4] {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}
fn train_style(mode: Mode) -> (u32, u32, u32) {
    match mode {
        Mode::Tram => (0xf2c230, 0xe8e4d8, 0x3a3a3a),
        Mode::SBahn => (0x9b2b25, 0x5b5f66, 0xd9a441),
        _ => (0xf0c419, 0x6a6457, 0x3a3a3a),
    }
}

/// Eingänge an der Straße (Welt): Schacht, Stufen, Geländer, Mast mit Schild.
pub fn entrance_bodies(w: &World, out: &mut Vec<Body>) {
    if w.player.inside.is_some() {
        return;
    }
    for st in &w.st_near {
        for ex in &st.exits {
            if (ex.x - w.camera.x).abs() > 2600. || (ex.y - w.camera.y).abs() > 1800. {
                continue;
            }
            let a = (ex.face + std::f64::consts::PI) as f32;
            let (x, y) = (ex.x as f32, ex.y as f32);
            let (fx, fy) = (a.cos(), a.sin());
            let (nx, ny) = (-fy, fx);
            let push = |out: &mut Vec<Body>,
                        cx: f32,
                        cy: f32,
                        hx: f32,
                        hy: f32,
                        ang: f32,
                        d: f32,
                        c: [f32; 4]| {
                out.push(Body {
                    center: [cx, cy],
                    half: [hx, hy],
                    angle: ang,
                    shape: 4.,
                    depth: d,
                    color: c,
                })
            };
            push(out, x, y, 16., 9., a, 0.62, rgb(0x1b1c20, 1.));
            let mut k = -12.;
            while k <= 12. {
                push(
                    out,
                    x + fx * k,
                    y + fy * k,
                    0.5,
                    8.,
                    a,
                    0.6195,
                    rgb(0x5b5d63, 1.),
                );
                k += 4.;
            }
            for s in [-1f32, 1.] {
                push(
                    out,
                    x + nx * 10. * s,
                    y + ny * 10. * s,
                    16.,
                    1.,
                    a,
                    0.619,
                    rgb(0xc9c9c4, 1.),
                );
            }
            push(
                out,
                x + fx * 16.,
                y + fy * 16.,
                1.,
                10.,
                a,
                0.619,
                rgb(0xc9c9c4, 1.),
            );
            // Mast und Schild neben dem Schacht (Schild steht aufrecht, das Zeichen kommt aus dem HUD)
            let (px, py) = (
                x - (ex.face.cos() as f32) * 4.
                    + (ex.face + std::f64::consts::FRAC_PI_2).cos() as f32 * 14.,
                y - (ex.face.sin() as f32) * 4.
                    + (ex.face + std::f64::consts::FRAC_PI_2).sin() as f32 * 14.,
            );
            push(out, px, py - 8., 1., 8., 0., 0.6185, rgb(0x444444, 1.));
            push(
                out,
                px,
                py - 23.,
                7.,
                7.,
                0.,
                0.618,
                if st.sbahn { S_GREEN } else { U_BLUE },
            );
        }
    }
}

/// Weltpunkt → Bildschirmpixel und Maßstab.
struct View<'a> {
    cam: &'a Camera,
    vp: Vec2,
    k: f32,
}
impl View<'_> {
    fn px(&self, x: f64, y: f64) -> Vec2 {
        self.cam
            .world_to_screen(Vec2::new(x as f32, y as f32), 0., self.vp)
    }
}

/// U/S-Zeichen auf den Schildern der Eingänge (aufrecht, im HUD).
pub fn entrance_letters(w: &World, cam: &Camera, vp: Vec2, h: &mut Hud) {
    if w.player.inside.is_some() || w.underground > 0.5 {
        return;
    }
    let v = View {
        cam,
        vp,
        k: cam.zoom * cam.scale,
    };
    for st in &w.st_near {
        for ex in &st.exits {
            let (px, py) = (
                ex.x - ex.face.cos() * 4. + (ex.face + std::f64::consts::FRAC_PI_2).cos() * 14.,
                ex.y - ex.face.sin() * 4. + (ex.face + std::f64::consts::FRAC_PI_2).sin() * 14.,
            );
            let p = v.px(px, py - 23.);
            if p.x < -20. || p.y < -20. || p.x > vp.x + 20. || p.y > vp.y + 20. || v.k < 0.6 {
                continue;
            }
            let size = (11. * v.k / h.scale).clamp(8., 20.);
            h.text(
                if st.sbahn { "S" } else { "U" },
                p.x / h.scale,
                p.y / h.scale + size * 0.45,
                size,
                [1.; 4],
                Align::Center,
                false,
            );
        }
    }
}

/// Wagen (railart.js drawTrainCar) im Bildraum.
#[allow(clippy::too_many_arguments)]
fn car_px(
    h: &mut Hud,
    v: &View,
    x: f64,
    y: f64,
    angle: f32,
    l: f32,
    wd: f32,
    mode: Mode,
    first: bool,
    last: bool,
    lit: bool,
) {
    let (side, roof, line) = train_style(mode);
    let c = v.px(x, y);
    let k = v.k;
    let (fx, fy) = (angle.cos(), angle.sin());
    h.quad_px(
        c.x + 3. * k,
        c.y + 4. * k,
        l / 2. * k,
        wd / 2. * k,
        angle,
        [0., 0., 0., 0.35],
        0.,
    );
    h.quad_px(c.x, c.y, l / 2. * k, wd / 2. * k, angle, rgb(side, 1.), 0.);
    h.quad_px(
        c.x,
        c.y,
        (l / 2. - 2.) * k,
        (wd / 2. - 3.) * k,
        angle,
        rgb(roof, 1.),
        0.,
    );
    let mut u = -l / 2. + 16.;
    while u < l / 2. - 10. {
        h.quad_px(
            c.x + fx * u * k,
            c.y + fy * u * k,
            4. * k,
            (wd / 2. - 5.) * k,
            angle,
            shade(rgb(roof, 1.), 0.85),
            0.,
        );
        u += 22.;
    }
    for s in [-1f32, 1.] {
        let (ox, oy) = (-fy * (wd / 2. - 0.6) * s * k, fx * (wd / 2. - 0.6) * s * k);
        h.quad_px(
            c.x + ox,
            c.y + oy,
            l / 2. * k,
            0.6 * k,
            angle,
            rgb(line, 1.),
            0.,
        );
    }
    if first {
        let glow = if lit { 1. } else { 0.7 };
        for s in [-1f32, 1.] {
            let (ox, oy) = (-fy * (wd / 2. - 3.5) * s * k, fx * (wd / 2. - 3.5) * s * k);
            h.quad_px(
                c.x + fx * (l / 2. - 0.8) * k + ox,
                c.y + fy * (l / 2. - 0.8) * k + oy,
                0.8 * k,
                1.5 * k,
                angle,
                rgb(0xfff6c8, glow),
                0.,
            );
        }
    }
    if last {
        for s in [-1f32, 1.] {
            let (ox, oy) = (-fy * (wd / 2. - 3.5) * s * k, fx * (wd / 2. - 3.5) * s * k);
            h.quad_px(
                c.x - fx * (l / 2. - 0.8) * k + ox,
                c.y - fy * (l / 2. - 0.8) * k + oy,
                0.8 * k,
                1.5 * k,
                angle,
                rgb(0x8a1c1c, 1.),
                0.,
            );
        }
    }
}

/// Bahnhof von innen: Fliesenwände in der Bahnhofsfarbe mit Namensschildern, zwei Gleise, Mittelbahnsteig mit
/// weißer Kante, Säulen, Treppen (Ausgang), Fahrgastinfo, Wartende, Züge, die Figur.
pub fn draw_station(w: &World, st: &Station, cam: &Camera, vp: Vec2, h: &mut Hud) {
    let v = View {
        cam,
        vp,
        k: cam.zoom * cam.scale,
    };
    let k = v.k;
    let a = st.axis as f32;
    // Rahmen (u, v) → Bildschirm, Rechteck mit halber Ausdehnung
    let quad = |h: &mut Hud, u: f64, vv: f64, hu: f64, hv: f64, c: [f32; 4]| {
        let (x, y) = st.to_world(u, vv);
        let p = v.px(x, y);
        h.quad_px(p.x, p.y, hu as f32 * k, hv as f32 * k, a, c, 0.);
    };
    let (hl, half, track, wall) = (st.hl, stn::HALF, stn::TRACK, stn::WALL);
    let end = hl + 30.;
    let tile = rgb(st.color, 1.);
    if st.open_air {
        // Hochbahn, ebenerdig, Einschnitt: Bahnkörper (Viadukt bzw. Schotter) mit Geländer, die Stadt drumherum
        let deck = track + 26.;
        quad(h, 3., 5., end + 60., deck + 2., [0., 0., 0., 0.3]);
        quad(h, 0., 0., end + 60., deck, rgb(0x5a5751, 1.));
        for s in [-1., 1.] {
            quad(h, 0., s * deck, end + 60., 1.6, rgb(0x2e3035, 1.));
            let mut u = -end - 60.;
            while u < end + 60. {
                quad(h, u, s * deck, 1.2, 2.4, rgb(0x2e3035, 1.));
                u += 24.;
            }
        }
    } else {
        h.rect(0., 0., h.width, 720., rgb(0x0e0f12, 1.), 0.);
        quad(h, 0., 0., end + 40., wall + 30., rgb(0x26282d, 1.));
        quad(h, 0., 0., end, wall, tile);
        // Fugen der Fliesen (gröber als im Browser)
        let mut fv = track + 22.;
        while fv <= wall {
            for s in [-1., 1.] {
                quad(h, 0., s * fv, end, 0.3, [0., 0., 0., 0.12]);
            }
            fv += 12.;
        }
    }
    // Gleisbetten mit Schwellen, Schienen und Stromschiene
    for s in [-1., 1.] {
        quad(h, 0., s * track, end, 22., rgb(0x2a2826, 1.));
        let mut u = -end;
        while u < end {
            quad(h, u + 2., s * track, 2., 13., rgb(0x4a4238, 1.));
            u += 11.;
        }
        for r in [-7., 7.] {
            quad(h, 0., s * track + r, end, 1., rgb(0x9a9ea6, 1.));
        }
        quad(h, 0., s * (track + 16.), end, 1.5, rgb(0x6b6f76, 1.));
    }
    // Namensschilder an den Wänden hinter den Gleisen (unter freiem Himmel steht der Name in der Kopfzeile)
    let mut signs = Vec::new();
    if !st.open_air {
        let mut u = -hl + 120.;
        while u <= hl - 120. {
            for s in [-1., 1.] {
                let w_ = (st.name.chars().count() as f64 * 8. + 16.).max(90.);
                quad(h, u, s * (wall - 14.), w_ / 2., 9., rgb(0x0f3b73, 1.));
                signs.push(st.to_world(u, s * (wall - 14.)));
            }
            u += 240.;
        }
    }
    // Züge am Bahnsteig
    let trains = w.trains_at(st);
    for t in &trains {
        for (i, c) in t.cars.iter().enumerate() {
            // unter Tage verschwinden die Wagen in den Tunnelmündern, oben fahren sie über den Bahnsteig hinaus
            let lim = if st.open_air { end + st.l } else { end + c.2 };
            if c.0.abs() > lim {
                continue;
            }
            let (x, y) = st.to_world(c.0, c.1);
            let ang = a + if t.dir < 0 { std::f32::consts::PI } else { 0. };
            car_px(
                h,
                &v,
                x,
                y,
                ang,
                c.2 as f32,
                c.3 as f32,
                t.mode,
                i == 0,
                i + 1 == t.cars.len(),
                !t.dwelling,
            );
        }
    }
    // Tunnelmünder
    if !st.open_air {
        for s in [-1., 1.] {
            for kk in [-1., 1.] {
                let (x, y) = st.to_world(s * (end + 6.), kk * track);
                let p = v.px(x, y);
                h.ellipse_px(p.x, p.y, 10. * k, 26. * k, a, rgb(0x050506, 1.));
            }
        }
    }
    // Bahnsteig: Terrazzo, weiße Kanten, Leitlinien, Treppen
    quad(h, 0., 0., hl, half, rgb(0xb8b3a8, 1.));
    let mut uu = -hl;
    while uu < hl {
        let mut vv = -half;
        while vv < half {
            if (((uu + vv) / 20.).round() as i64).rem_euclid(2) == 0 {
                quad(h, uu + 10., vv + 10., 10., 10., [0., 0., 0., 0.05]);
            }
            vv += 20.;
        }
        uu += 20.;
    }
    for s in [-1., 1.] {
        quad(h, 0., s * (half - 2.), hl, 2., rgb(0xf2efe6, 1.));
        quad(h, 0., s * (half - 12.), hl, 0.5, [0., 0., 0., 0.25]);
    }
    let mut exit_labels = Vec::new();
    for e in [-1., 1.] {
        let u0 = if e > 0. { hl - stn::STAIR_L } else { -hl };
        let uc = u0 + stn::STAIR_L / 2.;
        let sw = stn::STAIR_W;
        quad(h, uc, 0., stn::STAIR_L / 2., sw / 2., rgb(0x6d6a64, 1.));
        let mut su = u0 + 4.;
        while su < u0 + stn::STAIR_L {
            quad(h, su, 0., 0.6, sw / 2. - 2., rgb(0x8d8a83, 1.));
            su += 6.;
        }
        for s in [-1., 1.] {
            quad(h, uc, s * sw / 2., stn::STAIR_L / 2., 1., rgb(0xd8d5cc, 1.));
        }
        let ex = &st.exits[if e < 0. { 0 } else { 1 }];
        let label = format!("{} Ausgang {}", if e > 0. { "→" } else { "←" }, ex.street)
            .trim()
            .to_string();
        let lu = if e > 0. {
            u0 - 8.
        } else {
            u0 + stn::STAIR_L + 8.
        };
        let lw = (label.chars().count() as f64 * 6.4 + 22.).max(80.);
        quad(h, lu, -sw / 2. - 12.5, lw / 2., 7.5, rgb(0x123f7a, 1.));
        exit_labels.push((st.to_world(lu, -sw / 2. - 12.5), label));
    }
    // Umsteigetreppen zu den anderen Bahnsteigen (hinauf bzw. hinunter, mit Ziel)
    for t in &st.transfers {
        let (tl, sw) = (stn::TRANSFER_L, stn::STAIR_W);
        quad(h, t.u, 0., tl / 2., sw / 2., rgb(0x5f6b78, 1.));
        let mut su = t.u - tl / 2. + 4.;
        while su < t.u + tl / 2. {
            quad(h, su, 0., 0.6, sw / 2. - 2., rgb(0x8796a6, 1.));
            su += 6.;
        }
        for s in [-1., 1.] {
            quad(h, t.u, s * sw / 2., tl / 2., 1., rgb(0xe8e4da, 1.));
        }
        let label = t.label(st.level);
        let lw = (label.chars().count() as f64 * 6.4 + 22.).max(80.);
        quad(h, t.u, sw / 2. + 12.5, lw / 2., 7.5, rgb(0x123f7a, 1.));
        exit_labels.push((st.to_world(t.u, sw / 2. + 12.5), label));
    }
    // Säulen mit Lichtschein
    for pu in st.pillars() {
        let (x, y) = st.to_world(pu, 0.);
        let p = v.px(x, y);
        h.blob_px(
            p.x,
            p.y,
            85. * k,
            half as f32 * k,
            a,
            [1., 0.93, 0.78, 0.18],
        );
        h.ellipse_px(
            p.x,
            p.y,
            stn::PILLAR as f32 * k,
            stn::PILLAR as f32 * k,
            0.,
            shade(tile, 0.65),
        );
    }
    // Fahrgastinfo: zwei Tafeln mit den nächsten Abfahrten
    let deps = w.departures(st);
    let mut boards = Vec::new();
    for bu in [-hl / 3., hl / 3.] {
        quad(h, bu, 0., 58., 15., rgb(0x111317, 1.));
        boards.push(st.to_world(bu, 0.));
    }
    // Wartende (steigen in einen haltenden Zug auf ihrer Seite ein – dann sind sie weg)
    let hour = (w.clock.rem_euclid(1440.) / 60.).floor() as u32;
    let dwell_sides: Vec<i8> = trains
        .iter()
        .filter(|t| t.dwelling)
        .map(|t| t.dir)
        .collect();
    for (k2, (u, vv, side)) in st.waiting(hour).into_iter().enumerate() {
        if dwell_sides.contains(&side) {
            continue;
        }
        let (x, y) = st.to_world(u, vv);
        let p = v.px(x, y);
        let shirt = [0x3d5a80, 0xc0392b, 0x27ae60, 0x8e44ad, 0xe67e22, 0x2c3e50][k2 % 6];
        h.ellipse_px(
            p.x,
            p.y,
            6.5 * k,
            4.5 * k,
            a + std::f32::consts::FRAC_PI_2,
            rgb(shirt, 1.),
        );
        h.ellipse_px(p.x, p.y, 3. * k, 3. * k, 0., rgb(0xe0ac69, 1.));
    }
    // die anderen Bahnsteige des Bahnhofs, durchscheinend (darüber bzw. darunter), mit Linie und Ebene
    let mut ghosts = Vec::new();
    for t in &st.transfers {
        let Some(o) = w.station_by_id(&t.to) else {
            continue;
        };
        let oa = o.axis as f32;
        let oq = |h: &mut Hud, u: f64, vv: f64, hu: f64, hv: f64, c: [f32; 4]| {
            let (x, y) = o.to_world(u, vv);
            let p = v.px(x, y);
            h.quad_px(p.x, p.y, hu as f32 * k, hv as f32 * k, oa, c, 0.);
        };
        let below = o.level < st.level;
        let tint = if below {
            [0.35, 0.55, 0.95, 0.16]
        } else {
            [0.95, 0.8, 0.45, 0.16]
        };
        oq(h, 0., 0., o.hl, stn::TRACK + 12., tint);
        for s in [-1., 1.] {
            oq(
                h,
                0.,
                s * stn::HALF,
                o.hl,
                1.2,
                [tint[0], tint[1], tint[2], 0.5],
            );
            oq(
                h,
                0.,
                s * stn::TRACK,
                o.hl,
                0.8,
                [tint[0], tint[1], tint[2], 0.35],
            );
        }
        ghosts.push((
            o.to_world(o.hl * 0.45, 0.),
            format!("{} · {}", o.lines.join(" "), stn::level_name(o.level)),
        ));
    }
    // Figur
    let pl = &w.player;
    let p = v.px(pl.x, pl.y);
    h.ellipse_px(p.x, p.y, 11. * k, 11. * k, 0., [0.25, 0.85, 1., 0.35]);
    h.ellipse_px(
        p.x,
        p.y,
        7. * k,
        5. * k,
        pl.angle as f32 + std::f32::consts::FRAC_PI_2,
        rgb(0x2b2f3a, 1.),
    );
    h.ellipse_px(p.x, p.y, 3.2 * k, 3.2 * k, 0., rgb(0xe0ac69, 1.));
    // Dunkel zu den Tunneln hin
    for s in [-1., 1.].into_iter().filter(|_| !st.open_air) {
        let (x, y) = st.to_world(s * (end + 30.), 0.);
        let q = v.px(x, y);
        h.blob_px(
            q.x,
            q.y,
            160. * k,
            (wall as f32 + 40.) * k,
            a,
            [0., 0., 0., 0.85],
        );
    }
    // Schrift (aufrecht): Namen, Ausgänge, Abfahrten
    let txt = |h: &mut Hud, (x, y): (f64, f64), t: &str, size: f32, c: [f32; 4]| {
        let p = v.px(x, y);
        let s = (size * k / h.scale).clamp(7., 18.);
        h.text(
            t,
            p.x / h.scale,
            p.y / h.scale + s * 0.4,
            s,
            c,
            Align::Center,
            false,
        );
    };
    for &pos in &signs {
        txt(h, pos, &st.name, 12., [1.; 4]);
    }
    for (pos, label) in &exit_labels {
        txt(h, *pos, label, 9., [1.; 4]);
    }
    for (pos, label) in &ghosts {
        txt(h, *pos, label, 9., [1., 1., 1., 0.75]);
    }
    // wo man ist: Bahnhof, Linien, Ebene
    h.text(
        &format!(
            "{} · {} · {}",
            st.name,
            st.lines.join(" "),
            stn::level_name(st.level)
        ),
        h.width / 2.,
        128.,
        16.,
        [1., 1., 1., 0.92],
        Align::Center,
        true,
    );
    for &(bx, by) in &boards {
        let p = v.px(bx, by);
        let size = (8. * k / h.scale).clamp(7., 14.);
        for (i, d) in deps.iter().take(2).enumerate() {
            let min = if d.sec < 45. {
                "sofort".to_string()
            } else {
                format!("{} min", (d.sec / 60.).round())
            };
            let dest: String = d.dest.chars().take(13).collect();
            let y = p.y / h.scale + (i as f32 - 0.5) * size * 1.3 + size * 0.4;
            h.text(
                &format!("{} {} {}", d.line, dest, min),
                p.x / h.scale,
                y,
                size,
                [1., 0.69, 0., 1.],
                Align::Center,
                true,
            );
        }
    }
}

/// Tunnelansicht (Fahrt unter Tage): die Stadt dunkel, U-/S-Bahn-Strecken unter Tage als Betonröhren mit Lichtern,
/// Bahnsteige mit Namen, Züge in der Röhre. `fade` = `World::underground`.
pub fn draw_tunnels(w: &mut World, cam: &Camera, vp: Vec2, h: &mut Hud) {
    let fade = w.underground as f32;
    if fade <= 0.01 {
        return;
    }
    let Some(tr) = w.transit.take() else { return };
    let v = View {
        cam,
        vp,
        k: cam.zoom * cam.scale,
    };
    let k = v.k;
    h.rect(
        0.,
        0.,
        h.width,
        720.,
        [0.027, 0.031, 0.047, 0.72 * fade],
        0.,
    );
    let own = match w.player.ride.as_ref().map(|r| &r.r) {
        Some(Ref::PlayerTrain) => w.player_train.as_ref().map(|t| t.pid),
        Some(Ref::Veh { pid, .. }) => Some(*pid),
        _ => None,
    };
    let in_view = |p: Vec2| p.x > -300. && p.y > -300. && p.x < vp.x + 300. && p.y < vp.y + 300.;
    let mut drawn_shapes = std::collections::HashSet::new();
    let mut labels: Vec<(Vec2, String)> = Vec::new();
    let mut platforms: Vec<(String, Vec2, f32)> = Vec::new();
    let pids: Vec<usize> = w.transit_state.tracked.keys().copied().collect();
    for &pid in &pids {
        let p = &tr.patterns[pid];
        if !p.mode.rail() || !drawn_shapes.insert(p.shape) {
            continue;
        }
        let sh = tr.shape_of(p);
        let (_, _, cw, _) = p.mode.train();
        let tube = (cw + 20.) as f32;
        // deckend (Halbtransparenz ließe an den Stößen Perlen stehen); fremde Röhren etwas dunkler
        let alpha = fade;
        let dim = if Some(pid) == own { 1. } else { 0.75 };
        let mut pts: Vec<Vec2> = Vec::new();
        let mut lamps: Vec<Vec2> = Vec::new();
        let flush = |h: &mut Hud, pts: &mut Vec<Vec2>| {
            if pts.len() >= 2 {
                for (wd, col) in [
                    (tube, [0.227 * dim, 0.239 * dim, 0.267 * dim, alpha]),
                    (tube - 8., [0.125 * dim, 0.165 * dim, 0.188 * dim, alpha]),
                    (16., [0.447 * dim, 0.482 * dim, 0.49 * dim, alpha]),
                    (13., [0.141 * dim, 0.165 * dim, 0.173 * dim, alpha]),
                ] {
                    for s in pts.windows(2) {
                        h.line(
                            s[0].x / h.scale,
                            s[0].y / h.scale,
                            s[1].x / h.scale,
                            s[1].y / h.scale,
                            wd * k / h.scale,
                            col,
                        );
                    }
                }
            }
            pts.clear();
        };
        let mut s = 0.;
        while s <= sh.len {
            let (x, y, _) = point_on_shape(sh, s);
            let q = v.px(x, y);
            if in_view(q) && w.ug.underground_at_s(&mut w.city, p, sh, s) {
                pts.push(q);
                if ((s / 60.).round() as i64) % 4 == 0 {
                    lamps.push(q);
                }
            } else {
                flush(h, &mut pts);
            }
            s += 60.;
        }
        flush(h, &mut pts);
        for q in lamps {
            h.blob_px(
                q.x,
                q.y,
                30. * k,
                30. * k,
                0.,
                [1., 0.93, 0.75, 0.25 * fade],
            );
            h.quad_px(
                q.x,
                q.y,
                1.5 * k,
                1.5 * k,
                0.,
                [1., 0.925, 0.745, 0.85 * fade],
                0.,
            );
        }
        // Bahnsteige unter Tage (ein je Richtung, Name je Bahnhof einmal)
        let len = p.mode.train_len();
        for (i, &sv) in p.stops.iter().enumerate() {
            let (qx, qy, qa) = point_on_shape(sh, sv);
            let q = v.px(qx, qy);
            if !in_view(q) || !w.ug.underground_at_s(&mut w.city, p, sh, sv) {
                continue;
            }
            let name = stn::station_name(p.stop_names.get(i).map_or("", String::as_str));
            if platforms.iter().any(|(n, at, a)| {
                *n == name && at.distance(q) < 400. * k && (a - qa as f32).cos() > 0.87
            }) {
                continue;
            }
            let a0 = (sv - len).max(0.);
            let (cx, cy, ca) = point_on_shape(sh, a0 + len / 2.);
            let c = v.px(cx, cy);
            for side in [-1f32, 1.] {
                let off = (tube / 2. + 22.) * side;
                let (nx, ny) = (-(ca as f32).sin(), (ca as f32).cos());
                h.quad_px(
                    c.x + nx * off * k,
                    c.y + ny * off * k,
                    len as f32 / 2. * k,
                    20. * k,
                    ca as f32,
                    [0.84, 0.84, 0.81, 0.95 * fade],
                    0.,
                );
                let off2 = (tube / 2. + 4.) * side;
                let col = crate::hud::line_color(&p.color).unwrap_or([1., 0.83, 0.24, 1.]);
                h.quad_px(
                    c.x + nx * off2 * k,
                    c.y + ny * off2 * k,
                    len as f32 / 2. * k,
                    2. * k,
                    ca as f32,
                    [col[0], col[1], col[2], fade],
                    0.,
                );
            }
            platforms.push((name.clone(), q, qa as f32));
            if !labels
                .iter()
                .any(|(at, n)| *n == name && at.distance(c) < 400. * k)
            {
                labels.push((Vec2::new(c.x, c.y - (tube / 2. + 54.) * k), name));
            }
        }
    }
    // Züge in der Röhre (Fahrplan und Spielerzug)
    let mut trains: Vec<(usize, f64, bool)> = Vec::new();
    for &pid in &pids {
        let p = &tr.patterns[pid];
        if !p.mode.rail() {
            continue;
        }
        for veh in &w.transit_state.tracked[&pid].veh {
            if !veh.gone {
                let pos = position_at(p, veh.tau);
                trains.push((pid, pos.s, !pos.dwelling));
            }
        }
    }
    if let Some(t) = &w.player_train
        && tr.patterns[t.pid].mode != Mode::Tram
    {
        trains.push((t.pid, t.s, true));
    }
    for (pid, s, lit) in trains {
        let p = &tr.patterns[pid];
        let sh = tr.shape_of(p);
        if !w.ug.underground_at_s(&mut w.city, p, sh, s) {
            continue;
        }
        for c in train_cars(sh, p.mode, s) {
            if in_view(v.px(c.x, c.y)) {
                car_px(
                    h,
                    &v,
                    c.x,
                    c.y,
                    c.angle as f32,
                    c.l as f32,
                    c.w as f32,
                    p.mode,
                    c.first,
                    c.last,
                    lit,
                );
            }
        }
    }
    for (at, name) in labels {
        h.text(
            &name,
            at.x / h.scale,
            at.y / h.scale,
            18.,
            [1., 1., 1., fade],
            Align::Center,
            true,
        );
    }
    w.transit = Some(tr);
}
