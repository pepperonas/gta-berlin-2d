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

/// Linienzug in Welt-px
type Line = Vec<(f64, f64)>;

/// Blitzstrahl (`wetfx.js boltPath`): Zickzack vom Himmel (oberhalb im Bild) zum Einschlag, mit Ästen.
pub fn bolt_path(x: f64, y: f64, seed: f64, height: f64) -> (Line, Vec<Line>) {
    let (mut main, mut branches) = (Vec::new(), Vec::new());
    let mut px = x + (hash01(seed * 3. + 1.) - 0.5) * 500.;
    main.push((px, y - height));
    let n = 22;
    for k in 1..=n {
        let kf = k as f64;
        let u = kf / n as f64;
        let tx = px + (x - px) / (n - k + 1) as f64;
        let ty = y - height * (1. - u);
        let j = (hash01(seed * 17. + kf) - 0.5) * 90. * (1. - u * 0.6);
        let (nx, ny) = (tx + j, ty);
        main.push((nx, ny));
        if k > 3 && k < n - 2 && hash01(seed * 29. + kf) < 0.22 {
            let mut b = vec![(nx, ny)];
            let (mut bx, mut by) = (nx, ny);
            let dir = if hash01(seed * 37. + kf) < 0.5 {
                -1.
            } else {
                1.
            };
            let m = 4 + (hash01(seed * 41. + kf) * 5.) as usize;
            for q in 1..=m {
                let qf = q as f64;
                bx += dir * (15. + hash01(seed * 43. + kf * 7. + qf) * 45.);
                by += 30. + hash01(seed * 47. + kf + qf) * 50.;
                b.push((bx, by));
            }
            branches.push(b);
        }
        px = nx;
    }
    if let Some(last) = main.last_mut() {
        *last = (x, y);
    }
    (main, branches)
}

/// Sturmtrümmer (`wetfx.js stormDebris`): Laub und Papier, das mit dem Wind übers Bild fegt.
pub fn storm_debris(
    v: (f64, f64, f64, f64),
    storm: f64,
    wind: (f64, f64),
    t: f64,
    gust: f64,
) -> Vec<(f64, f64, f64, bool, f64)> {
    let mut out = Vec::new();
    if storm < 0.2 {
        return out;
    }
    let n = (60. * storm).round() as usize;
    let wl = wind.0.hypot(wind.1).max(1e-9);
    let (ux, uy) = (wind.0 / wl, wind.1 / wl);
    for i in 0..n {
        let fi = i as f64;
        let life = 1.6 + h(&[fi, 51.]) * 1.4;
        let ph = (t / life + h(&[fi, 52.])).rem_euclid(1.);
        let cyc = (t / life + h(&[fi, 52.])).floor();
        let sp = (380. + h(&[fi, 53.]) * 420.) * gust;
        let bx = v.0 + h(&[fi, cyc, 54.]) * v.2;
        let by = v.1 + h(&[fi, cyc, 55.]) * v.3;
        let d = (ph - 0.5) * life * sp;
        let wob = (t * 5. + fi).sin() * 10.;
        out.push((
            bx + ux * d - uy * wob,
            by + uy * d + ux * wob,
            t * (6. + h(&[fi, 56.]) * 8.) + fi,
            h(&[fi, 57.]) < 0.8,
            h(&[fi, 58.]),
        ));
    }
    out
}

/// Regen und Sturm im Bildraum (nach dem Licht): grauer Regenschleier, Regenwände, die mit dem Wind durchs Bild
/// ziehen, fliegendes Laub und Papier, Blitzstrahl mit Ästen und hellem Fleck am Einschlag (`wetfx.js
/// drawRainLayers`, `drawStormDebris`, `drawLightning`).
pub fn storm_overlay(
    w: &World,
    camera: &berlin_engine::camera::Camera,
    viewport: glam::Vec2,
    hud: &mut Hud,
) {
    use glam::Vec2;
    let p = w.sky.p;
    let t = w.time;
    let s = hud.scale;
    let to = |x: f64, y: f64| camera.world_to_screen(Vec2::new(x as f32, y as f32), 0., viewport);
    // Bildschirm-Pixel je Welt-px
    let k = (to(w.camera.x + 100., w.camera.y) - to(w.camera.x, w.camera.y)).length() / 100.;
    let (hw, hh) = (
        viewport.x as f64 / 2. / k.max(1e-3) as f64,
        viewport.y as f64 / 2. / k.max(1e-3) as f64,
    );
    let view = (w.camera.x - hw, w.camera.y - hh, 2. * hw, 2. * hh);
    let rain = p.rain.min(1.6);
    let heavy = (rain - 1.).clamp(0., 1.);
    let (wx, wy) = w.sky.wind;
    let gust = berlin_sim::weather::gust_at(p.storm, t);
    if rain >= 0.03 {
        let a = 0.12 * rain.min(1.) + 0.14 * heavy;
        hud.rect(0., 0., hud.width, 720., [0.275, 0.333, 0.412, a as f32], 0.);
    }
    if rain > 0.5 || p.storm > 0.2 {
        // Regenwände: lange, weiche Bänder quer zum Wind, die mit ihm durchs Bild ziehen
        let a = (0.1 * rain.min(1.) + 0.22 * heavy + 0.14 * p.storm).min(0.5) as f32;
        let wl = wx.hypot(wy).max(1.);
        let (ux, uy) = (wx / wl, wy / wl);
        let sp = (140. + 320. * p.storm) * gust;
        const GAP: f64 = 520.;
        let off = (sp * t).rem_euclid(GAP);
        let along = ux * w.camera.x + uy * w.camera.y;
        let reach = hw.hypot(hh);
        let n0 = ((along - reach - off) / GAP).floor() as i64;
        let n1 = ((along + reach - off) / GAP).ceil() as i64;
        let angle = (uy.atan2(ux) + std::f64::consts::FRAC_PI_2) as f32;
        for n in n0..=n1 {
            let d = n as f64 * GAP + off - along;
            let jitter = (h(&[n as f64, 61.]) - 0.5) * GAP * 0.5;
            let (x, y) = (
                w.camera.x + ux * (d + jitter),
                w.camera.y + uy * (d + jitter),
            );
            let c = to(x, y);
            let thick = (60. + h(&[n as f64, 62.]) * 90.) as f32 * k;
            let dens = 0.6 + 0.4 * h(&[n as f64, 63.]) as f32;
            hud.blob_px(
                c.x,
                c.y,
                reach as f32 * k * 1.2,
                thick,
                angle,
                [0.62, 0.68, 0.76, a * dens],
            );
        }
    }
    // Sturmtrümmer
    for (x, y, rot, leaf, c) in storm_debris(view, p.storm, (wx, wy), t, gust) {
        let q = to(x, y);
        if leaf {
            let col = if c < 0.4 {
                [0.541, 0.416, 0.173, 1.]
            } else if c < 0.7 {
                [0.627, 0.467, 0.165, 1.]
            } else {
                [0.42, 0.478, 0.173, 1.]
            };
            hud.ellipse_px(q.x, q.y, 3.2 * k, 1.6 * k, rot as f32, col);
        } else {
            hud.quad_px(
                q.x,
                q.y,
                3. * k,
                2. * k,
                rot as f32,
                [0.92, 0.91, 0.87, 0.9],
                0.,
            );
        }
    }
    // Blitzstrahl: nahe Einschläge landen im Bild oder knapp daneben
    if p.thunder > 0.02 {
        use berlin_sim::weather::{STRIKE_SLOT, flash_at, strike_in_slot};
        let i1 = (t / STRIKE_SLOT).floor() as i64;
        for i in i1 - 1..=i1 {
            let Some(st) = strike_in_slot(w.seed, i, p.thunder).filter(|st| st.near) else {
                continue;
            };
            let a = (flash_at(t - st.t0) * 1.3).clamp(0., 1.) as f32;
            if a < 0.02 {
                continue;
            }
            let (x, y) = (w.camera.x + st.dx * 0.14, w.camera.y + st.dy * 0.08);
            let (main, branches) = bolt_path(x, y, i as f64, 2400.);
            let line = |hud: &mut Hud, pts: &[(f64, f64)], width: f32, color: [f32; 4]| {
                for seg in pts.windows(2) {
                    let (a, b) = (to(seg[0].0, seg[0].1), to(seg[1].0, seg[1].1));
                    hud.line(a.x / s, a.y / s, b.x / s, b.y / s, width * k / s, color);
                }
            };
            line(hud, &main, 44., [0.55, 0.63, 1., 0.3 * a]);
            line(hud, &main, 12., [0.75, 0.8, 1., 0.6 * a]);
            line(hud, &main, 4., [1., 1., 1., a]);
            for b in &branches {
                line(hud, b, 2., [0.9, 0.925, 1., 0.8 * a]);
            }
            let q = to(x, y);
            hud.blob_px(q.x, q.y, 260. * k, 260. * k, 0., [0.92, 0.94, 1., 0.7 * a]);
        }
    }
}

/// Gischt hinter schnellen Autos auf nasser Straße bzw. Schneestaub auf Schnee (`wetfx.js drawSpray`).
pub fn spray_bodies(w: &World, out: &mut Vec<Body>) {
    let (wet, snow) = (w.weather.wet, w.weather.snow);
    if wet <= 0.3 && snow <= 0.2 {
        return;
    }
    let half = VIEW / w.camera.zoom.max(0.5);
    for c in &w.cars {
        if (c.x - w.camera.x).abs() > half || (c.y - w.camera.y).abs() > half || c.lvl() != 0 {
            continue;
        }
        let sp = c.speed();
        let k = ((sp - 120.) / 400.).clamp(0., 1.) * wet.max(snow);
        if k < 0.03 {
            continue;
        }
        let (ca, sa) = (c.angle.cos(), c.angle.sin());
        let (bx, by) = (c.x - ca * (c.hw + 4.), c.y - sa * (c.hw + 4.));
        let color = if snow > wet {
            [0.94, 0.957, 0.98, (0.4 * k) as f32]
        } else {
            [0.784, 0.816, 0.855, (0.35 * k) as f32]
        };
        for i in 0..3 {
            let d = 6. + i as f64 * 9.;
            let r = (c.hh * (0.8 + i as f64 * 0.35)) as f32;
            out.push(Body {
                center: [(bx - ca * d) as f32, (by - sa * d) as f32],
                half: [r * 0.9, r],
                angle: c.angle as f32,
                shape: 3.,
                depth: 0.6195 - i as f32 * 0.00002,
                color,
            });
        }
    }
}

#[cfg(test)]
mod storm_tests {
    use super::*;

    #[test]
    fn bolt_ends_at_the_strike_and_branches() {
        let (main, _) = bolt_path(100., 200., 7., 2400.);
        assert_eq!(main.len(), 23);
        assert_eq!(*main.last().unwrap(), (100., 200.));
        assert!((main[0].1 - (200. - 2400.)).abs() < 1e-9, "oben im Himmel");
        let total: usize = (0..40)
            .map(|s| bolt_path(0., 0., s as f64, 2400.).1.len())
            .sum();
        assert!(total > 20, "Äste: {total}");
    }

    #[test]
    fn debris_only_in_a_storm_and_moves_with_the_wind() {
        let v = (0., 0., 1000., 600.);
        assert!(storm_debris(v, 0.1, (300., 0.), 1., 1.).is_empty());
        let a = storm_debris(v, 1., (300., 0.), 1., 1.);
        let b = storm_debris(v, 1., (300., 0.), 1.05, 1.);
        assert_eq!(a.len(), 60);
        // die meisten Blätter wandern in Windrichtung (gleicher Zyklus)
        let moved = a.iter().zip(&b).filter(|(p, q)| q.0 > p.0).count();
        assert!(moved > 40, "{moved}");
    }
}
