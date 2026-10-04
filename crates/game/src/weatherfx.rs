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
            [0.78, 0.8, 0.82, (fog * 0.16).min(0.3)],
            0.,
        );
    }
    let flash = berlin_sim::weather::flash_total(w.seed, w.time, w.sky.p.thunder) as f32;
    if flash > 0.01 {
        h.rect(0., 0., h.width, 720., [0.9, 0.92, 1., flash * 0.45], 0.);
    }
}

/// `h` aus wetfx.js (Startwert 5).
fn h(n: &[f64]) -> f64 {
    hash01(n.iter().fold(5., |a, &b| a * 31. + (b + 0.5).floor()))
}

/// Tiefen der Bodenschichten (Straßen liegen bei 0,85, Gehwege/Flächen dahinter, Autos/Leute bei 0,62). Innerhalb
/// einer Schicht steigt die Tiefe je Körper um `EPS`: so deckt jede Stelle nur einmal (Tiefentest), Überlappungen
/// an Gelenken und Kreuzungen dunkeln nicht doppelt nach.
const GROUND: f32 = 0.86;
const ROAD: f32 = 0.849;
const TRACK: f32 = 0.8475;
const PUDDLE: f32 = 0.8465;
const SPOT: f32 = 0.8455;
const FOG: f32 = 0.844;
const EPS: f32 = 1.5e-7;

struct Layer<'a> {
    out: &'a mut Vec<Body>,
    depth: f32,
}
impl Layer<'_> {
    fn push(&mut self, center: [f32; 2], half: [f32; 2], angle: f32, shape: f32, color: [f32; 4]) {
        self.out.push(Body {
            center,
            half,
            angle,
            shape,
            depth: self.depth,
            color,
        });
        self.depth += EPS;
    }
    /// Streifen entlang einer Linie (je Stück ein Rechteck), Breite `w`.
    fn strip(&mut self, pts: &[(f64, f64)], w: f32, color: [f32; 4]) {
        for p in pts.windows(2) {
            let ((ax, ay), (bx, by)) = (p[0], p[1]);
            let l = (bx - ax).hypot(by - ay) as f32;
            if l < 0.5 {
                continue;
            }
            self.push(
                [((ax + bx) / 2.) as f32, ((ay + by) / 2.) as f32],
                [l / 2. + w * 0.04, w / 2.],
                (by - ay).atan2(bx - ax) as f32,
                4.,
                color,
            );
        }
    }
}

/// Wetter am Boden: nasser Asphalt und Pfützen (mit Regenringen), Schneedecke, Matsch und festgefahrene Spuren,
/// Reifenspuren im Schnee, Aufschlagringe des Regens und Bodennebel. Liegt über Straßen und Flächen, unter Autos,
/// Leuten und Häusern.
pub fn ground_bodies(w: &World, trails: &crate::snowtracks::Trails, out: &mut Vec<Body>) {
    let p = w.sky.p;
    let gw = w.weather;
    let (cx, cy) = (w.camera.x, w.camera.y);
    let half = VIEW / w.camera.zoom.max(0.5);
    let (x0, y0, x1, y1) = (cx - half, cy - half * 0.7, cx + half, cy + half * 0.7);
    let inside = |x: f64, y: f64, m: f64| x > x0 - m && x < x1 + m && y > y0 - m && y < y1 + m;
    let day = 1. - crate::play::world_light(w).dark;
    let t = w.time;
    let (wet, snow) = (gw.wet.clamp(0., 1.), gw.snow.clamp(0., 1.));
    let ground = |c: [f32; 4]| Body {
        center: [cx as f32, cy as f32],
        half: [half as f32 * 1.4, half as f32 * 1.4],
        angle: 0.,
        shape: 0.,
        depth: GROUND,
        color: c,
    };
    // Schneedecke auf Gehwegen, Höfen und Grün (die Straßen liegen davor und bekommen eigenen Matsch)
    if snow > 0.02 {
        out.push(ground([0.92, 0.94, 0.97, (0.85 * snow) as f32]));
    } else if wet > 0.03 {
        out.push(ground([0.063, 0.086, 0.125, (0.16 * wet) as f32]));
    }
    // Straßen im Bild (Boden, keine Brücken/Tunnel)
    let edges: Vec<&berlin_sim::city::Edge> = w
        .city
        .edges
        .values()
        .filter(|e| e.lvl == 0 && e.cls <= 10 && !e.passage)
        .filter(|e| e.pts.iter().any(|&(x, y)| inside(x, y, 60.)))
        .collect();
    if snow > 0.02 || wet > 0.02 {
        let mut road = Layer { out, depth: ROAD };
        for e in &edges {
            let col = if snow > 0.02 {
                let k = match e.cls {
                    0..=3 => 0.5,
                    4..=5 => 0.62,
                    6..=8 => 0.78,
                    _ => 0.9,
                };
                [0.86, 0.88, 0.91, (snow * k) as f32]
            } else {
                // nasser Asphalt: dunkler und leicht bläulich, tags mit mattem Himmelsglanz
                let g = 0.04 * day;
                [
                    (0.043 + g * 0.5) as f32,
                    (0.07 + g * 0.6) as f32,
                    (0.114 + g * 0.7) as f32,
                    (0.34 * wet) as f32,
                ]
            };
            road.strip(&e.pts, e.w as f32, col);
        }
        for j in w.city.junction_discs() {
            if j.lo != 0 || !inside(j.x, j.y, 40.) {
                continue;
            }
            let col = if snow > 0.02 {
                [0.86, 0.88, 0.91, (snow * 0.62) as f32]
            } else {
                [0.05, 0.08, 0.125, (0.34 * wet) as f32]
            };
            road.push(
                [j.x as f32, j.y as f32],
                [j.r as f32, j.r as f32],
                0.,
                5.,
                col,
            );
        }
    }
    // Schnee: festgefahrene Spuren je Fahrstreifen (grauer Matsch), Wälle am Bordstein
    if snow > 0.05 {
        let mut tracks = Layer { out, depth: TRACK };
        let s = w.city.scale;
        for e in &edges {
            if e.cls > 8 {
                continue;
            }
            let lo = berlin_sim::roadgraph::lane_offsets(&e.cs, s);
            let mut centers: Vec<f64> = lo.fwd.iter().chain(&lo.bwd).copied().collect();
            centers.sort_by(f64::total_cmp);
            centers.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            let a = (snow * 0.55) as f32;
            for c in centers {
                for k in [-1., 1.] {
                    let line = berlin_sim::city::offset_polyline(&e.pts, c + k * 0.85 * s);
                    tracks.strip(&line, (0.55 * s) as f32, [0.42, 0.43, 0.45, a]);
                }
            }
        }
    }
    // Reifenspuren im Schnee
    if snow > 0.05 {
        let mut tr = Layer {
            out,
            depth: TRACK - 0.0004,
        };
        for pc in &trails.pieces {
            let a = crate::snowtracks::alpha(t - pc.t, snow, p.snow);
            if a <= 0.01 || !inside(pc.a.0, pc.a.1, 0.) {
                continue;
            }
            tr.strip(&[pc.a, pc.b], 1.6, [0.36, 0.38, 0.42, a as f32]);
        }
    }
    // Pfützen: dunkler Rand, darin der Himmel (tags hell-graublau, nachts dunkel), bei Regen Ringe
    if wet > berlin_sim::traction::PUDDLE_WET {
        let mut pd = Layer { out, depth: PUDDLE };
        let mut rings = Vec::new();
        for e in &edges {
            let Some(list) = w.puddles.get(&e.id) else {
                continue;
            };
            for pu in list {
                if !inside(pu.x, pu.y, 0.) {
                    continue;
                }
                let (rx, ry, a) = ((pu.rx * wet) as f32, (pu.ry * wet) as f32, pu.a as f32);
                pd.push(
                    [pu.x as f32, pu.y as f32],
                    [rx * 1.15, ry * 1.2],
                    a,
                    1.,
                    [0.03, 0.047, 0.07, (0.35 * wet) as f32],
                );
                let k = day;
                pd.push(
                    [pu.x as f32, pu.y as f32],
                    [rx, ry],
                    a,
                    1.,
                    [
                        ((55. + 115. * k) / 255.) as f32,
                        ((64. + 124. * k) / 255.) as f32,
                        ((82. + 130. * k) / 255.) as f32,
                        (0.62 * wet) as f32,
                    ],
                );
                rings.push(*pu);
            }
        }
        if p.rain > 0.05 {
            let per = (1. + (p.rain * 2.5).round()).min(4.) as usize;
            for pu in rings {
                for k in 0..per {
                    let kf = k as f64;
                    let life = 0.7 + h(&[pu.x, pu.y, kf]) * 0.5;
                    let ph0 = t / life + h(&[pu.y, pu.x, kf]);
                    let (ph, cyc) = (ph0 % 1., ph0.floor());
                    let u = h(&[pu.x, kf, cyc]) * 2. - 1.;
                    let v = h(&[pu.y, kf, cyc]) * 2. - 1.;
                    if u * u + v * v > 0.7 {
                        continue;
                    }
                    let (c, s) = (pu.a.cos(), pu.a.sin());
                    let (ox, oy) = (u * pu.rx * wet * 0.8, v * pu.ry * wet * 0.8);
                    let (x, y) = (pu.x + ox * c - oy * s, pu.y + ox * s + oy * c);
                    let r = (0.6 + ph * 4.5) as f32;
                    pd.push(
                        [x as f32, y as f32],
                        [r, r * 0.8],
                        0.,
                        2.,
                        [0.88, 0.91, 0.95, (0.55 * (1. - ph) * p.rain.min(1.)) as f32],
                    );
                }
            }
        }
    }
    // Aufschlagringe des Regens am Boden
    if p.rain > 0.05 {
        let mut sp = Layer { out, depth: SPOT };
        let n = (220. * p.rain.min(1.6)).round() as usize;
        let (vw, vh) = (x1 - x0, y1 - y0);
        for i in 0..n {
            let fi = i as f64;
            let life = 0.35 + h(&[fi, 61.]) * 0.3;
            let ph0 = t / life + h(&[fi, 62.]);
            let (ph, cyc) = (ph0 % 1., ph0.floor());
            let (x, y) = (x0 + h(&[fi, cyc, 63.]) * vw, y0 + h(&[fi, cyc, 64.]) * vh);
            let r = (0.8 + ph * (3. + h(&[fi, 65.]) * 3.)) as f32;
            sp.push(
                [x as f32, y as f32],
                [r, r * 0.8],
                0.,
                2.,
                [0.85, 0.89, 0.95, (0.4 * (1. - ph)) as f32],
            );
        }
    }
    // Bodennebel: Dunst plus Schwaden, die mit dem Wind ziehen (Häuser ragen heraus)
    let fog = p.fog;
    if fog > 0.03 {
        let k = fog.min(1.);
        let dense = ((fog - 1.) / 0.7).clamp(0., 1.);
        out.push(Body {
            center: [cx as f32, cy as f32],
            half: [half as f32 * 1.4, half as f32 * 1.4],
            angle: 0.,
            shape: 0.,
            depth: FOG,
            color: [0.82, 0.84, 0.86, (0.22 * k + 0.18 * dense) as f32],
        });
        let mut fl = Layer {
            out,
            depth: FOG - 0.0005,
        };
        let (wx, wy) = w.sky.wind;
        const CELL: f64 = 520.;
        let (ox, oy) = (wx * t * 0.4, wy * t * 0.4);
        let (i0, i1) = (
            ((x0 - ox) / CELL).floor() as i64 - 1,
            ((x1 - ox) / CELL).floor() as i64 + 1,
        );
        let (j0, j1) = (
            ((y0 - oy) / CELL).floor() as i64 - 1,
            ((y1 - oy) / CELL).floor() as i64 + 1,
        );
        for i in i0..=i1 {
            for j in j0..=j1 {
                let (fi, fj) = (i as f64, j as f64);
                if h(&[fi, fj, 71.]) > 0.35 + 0.4 * k {
                    continue;
                }
                let x = (fi + h(&[fi, fj, 72.])) * CELL + ox;
                let y = (fj + h(&[fj, fi, 73.])) * CELL + oy;
                let r = (CELL * (0.5 + h(&[fj, fi, 74.]) * 0.6)) as f32;
                fl.depth = FOG - 0.0005; // Schwaden dürfen sich überlagern (weich, gewollt dichter)
                fl.push(
                    [x as f32, y as f32],
                    [r, r * 0.6],
                    (h(&[fi, fj, 75.]) * 3.) as f32,
                    3.,
                    [0.86, 0.87, 0.9, (0.35 * k + 0.25 * dense) as f32],
                );
            }
        }
    }
}

/// Wolkenschatten (über Dächern und Straßen, im Bildraum gezeichnet) und grauer Himmel bei Bedeckung.
pub fn sky_overlay(
    w: &World,
    camera: &berlin_engine::camera::Camera,
    viewport: glam::Vec2,
    h_: &mut Hud,
) {
    let p = w.sky.p;
    let light = crate::play::world_light(w);
    let dark = light.dark;
    let a = 0.14 * p.cloud * (1. - dark) * (1. - 0.6 * w.weather.snow);
    if a >= 0.01 {
        h_.rect(0., 0., h_.width, 720., [0.27, 0.31, 0.38, a as f32], 0.);
    }
    let closed = ((0.97 - p.cloud) / 0.4).clamp(0., 1.);
    let sun = berlin_sim::daylight::light_at(w.clock).sun.strength;
    let alpha = 0.55
        * sun
        * (p.cloud * 1.4).min(1.)
        * (1. - p.rain.min(1.) * 0.6)
        * closed
        * (1. - p.snow.min(1.));
    if alpha < 0.02 {
        return;
    }
    const CELL: f64 = 2600.;
    let (wx, wy) = w.sky.wind;
    let (ox, oy) = (wx * w.time, wy * w.time);
    let half = 2200. / camera.zoom.max(0.5) as f64;
    let (cx, cy) = (w.camera.x, w.camera.y);
    let (i0, i1) = (
        ((cx - half - ox) / CELL).floor() as i64 - 1,
        ((cx + half - ox) / CELL).floor() as i64 + 1,
    );
    let (j0, j1) = (
        ((cy - half - oy) / CELL).floor() as i64 - 1,
        ((cy + half - oy) / CELL).floor() as i64 + 1,
    );
    let px = camera.zoom * camera.scale;
    for i in i0..=i1 {
        for j in j0..=j1 {
            for k in 0..2 {
                let (fi, fj, fk) = (i as f64, j as f64, k as f64);
                if h(&[fi, fj, fk, 1.]) > p.cloud * 0.9 {
                    continue;
                }
                let r = CELL * (0.35 + h(&[fj, fi, fk]) * 0.45);
                let x = (fi + h(&[fi, fk, fj, 2.])) * CELL + ox;
                let y = (fj + h(&[fk, fj, fi, 3.])) * CELL + oy;
                let s = camera.world_to_screen(glam::Vec2::new(x as f32, y as f32), 0., viewport);
                let rr = r as f32 * px;
                if s.x + rr < 0. || s.y + rr < 0. || s.x - rr > viewport.x || s.y - rr > viewport.y
                {
                    continue;
                }
                // vier Klumpen wie das Wolken-Sprite in wetfx.js (128er-Raster, Mitte 64/64)
                let rot = (h(&[fi, fj, fk, 9.]) * std::f64::consts::TAU) as f32;
                let (c, sn) = (rot.cos(), rot.sin());
                for (lx, ly, lr) in [
                    (0., 0., 52.),
                    (-24., -6., 34.),
                    (24., 6., 36.),
                    (-2., -20., 30.),
                ] {
                    let (ux, uy) = (lx / 64. * rr, ly / 64. * rr);
                    h_.blob_px(
                        s.x + ux * c - uy * sn,
                        s.y + ux * sn + uy * c,
                        lr / 64. * rr,
                        lr / 64. * rr * 0.85,
                        rot,
                        [0.08, 0.11, 0.18, alpha as f32],
                    );
                }
            }
        }
    }
}
