//! Deterministic procedural sprite atlas; no runtime image files or Canvas work.
//! Zellen: 0 Laubkrone, 1 Gully, 2 Schachtdeckel, 3 Flicken, 4 Riss, 5 Ölfleck, 6 Nadelkrone, 7 weißes Rechteck
//! (Markierungen), 8 Ölband, 9 Kontaktschatten, 10 Laub, 11 Fahrradpiktogramm, 12 Linde, 13 Platane, 14 Kastanie,
//! 15 Kiefer. Raster und Zellgröße gehen als Konstanten in den Shader (`shader_constants`), nichts ist dort fest
//! verdrahtet.
//!
//! Kronenzellen (`CROWNS`) tragen kein eingemaltes Licht: R = Helligkeit ohne Sonne (Blattwerk, Verdeckung in den
//! Senken), G/B = Normale der Krone (x, y; ·0,5 + 0,5), A = Deckung. Der Shader (`sprite_fs`) dreht die Normale mit
//! dem Baum und setzt Licht- und Schattenseite nach dem Sonnenstand.
use berlin_map_loader::citycodes::hash01;
pub const CELL: u32 = 256;
pub const COLS: u32 = 4;
pub const ROWS: u32 = 4;
pub const WIDTH: u32 = CELL * COLS;
pub const HEIGHT: u32 = CELL * ROWS;
/// belegte Zellen
pub const CELLS: u32 = 16;
/// Kronenzellen (Licht im Shader): allgemeiner Laubbaum, allgemeiner Nadelbaum, Linde, Platane, Kastanie, Kiefer
pub const CROWNS: [u32; 6] = [0, 6, 12, 13, 14, 15];
/// Mip-Stufen: bis 16 px je Zelle (darunter bluten Nachbarzellen ineinander)
pub const MIPS: u32 = 5;

/// WGSL-Konstanten des Atlas (dem Shader vorangestellt).
pub fn shader_constants() -> String {
    format!(
        "const ATLAS_CELL: f32 = {CELL}.0;\nconst ATLAS_COLS: f32 = {COLS}.0;\nconst ATLAS_ROWS: f32 = {ROWS}.0;\n{}",
        crown_fn()
    )
}
fn crown_fn() -> String {
    let tests: Vec<String> = CROWNS.iter().map(|c| format!("cell == {c}.0")).collect();
    format!(
        "fn is_crown(cell: f32) -> bool {{ return {}; }}\n",
        tests.join(" || ")
    )
}

/// Gestalt einer Baumkrone von oben: Umriss (Radius, Zacken), Blattballen (Anzahl, Größe, Streuung), Lücken,
/// Blattkörnung und Nadelstreifen.
struct Crown {
    seed: u32,
    radius: f32,
    lobes: u32,
    lobe_amp: f32,
    clumps: u32,
    clump_r: (f32, f32),
    dome: f32,
    gaps: f32,
    grain: f32,
    needles: f32,
}
fn crown_kind(cell: u32) -> Crown {
    match cell {
        // allgemeiner Laubbaum (Ahorn, Eiche, …): mittlere Ballen
        0 => Crown {
            seed: 11,
            radius: 0.8,
            lobes: 7,
            lobe_amp: 0.06,
            clumps: 34,
            clump_r: (0.16, 0.27),
            dome: 0.55,
            gaps: 0.05,
            grain: 22.,
            needles: 0.,
        },
        // allgemeiner Nadelbaum (Fichte, Tanne): sternförmig, spitz, dicht
        6 => Crown {
            seed: 23,
            radius: 0.74,
            lobes: 9,
            lobe_amp: 0.16,
            clumps: 26,
            clump_r: (0.1, 0.18),
            dome: 0.9,
            gaps: 0.0,
            grain: 30.,
            needles: 0.8,
        },
        // Linde: dicht, gleichmäßig rund, viele kleine Ballen (herzförmige Blätter, feine Körnung)
        12 => Crown {
            seed: 37,
            radius: 0.84,
            lobes: 5,
            lobe_amp: 0.035,
            clumps: 52,
            clump_r: (0.13, 0.21),
            dome: 0.6,
            gaps: 0.0,
            grain: 28.,
            needles: 0.,
        },
        // Platane: breit, wenige große Ballen, lockere Krone mit Lücken, grobe Blätter
        13 => Crown {
            seed: 41,
            radius: 0.88,
            lobes: 6,
            lobe_amp: 0.08,
            clumps: 18,
            clump_r: (0.22, 0.34),
            dome: 0.45,
            gaps: 0.16,
            grain: 14.,
            needles: 0.,
        },
        // Kastanie: groß, rund, schwere Ballen, Fingerblätter (sternförmige Körnung)
        14 => Crown {
            seed: 53,
            radius: 0.86,
            lobes: 8,
            lobe_amp: 0.05,
            clumps: 24,
            clump_r: (0.2, 0.3),
            dome: 0.7,
            gaps: 0.03,
            grain: 18.,
            needles: 0.,
        },
        // Kiefer: unregelmäßig, offene Krone aus Nadelbüscheln mit Lücken
        _ => Crown {
            seed: 67,
            radius: 0.82,
            lobes: 5,
            lobe_amp: 0.12,
            clumps: 36,
            clump_r: (0.11, 0.2),
            dome: 0.5,
            gaps: 0.24,
            grain: 34.,
            needles: 1.,
        },
    }
}
/// Weiches Wertrauschen (bilinear mit Glättung), 0…1, deterministisch aus dem Startwert.
fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (sx, sy) = (fx * fx * (3. - 2. * fx), fy * fy * (3. - 2. * fy));
    let at = |dx: f32, dy: f32| {
        let (a, b) = ((ix + dx) as i32 as u32, (iy + dy) as i32 as u32);
        hash01(
            seed.wrapping_mul(2_654_435_761)
                .wrapping_add(a.wrapping_mul(92_821))
                .wrapping_add(b.wrapping_mul(68_917)),
        ) as f32
    };
    let top = at(0., 0.) + (at(1., 0.) - at(0., 0.)) * sx;
    let bot = at(0., 1.) + (at(1., 1.) - at(0., 1.)) * sx;
    top + (bot - top) * sy
}
/// Kronenpixel an (u, v) ∈ [−1, 1]²: (Helligkeit ohne Sonne, Normale x, y, Deckung). Blattballen als Kugeln auf
/// einer flachen Kuppel; wo kein Ballen liegt, füllt die Kuppel tiefer im Schatten (keine Löcher im Kern), der Rand
/// entsteht nur aus Ballen (unregelmäßiger Umriss).
fn crown(c: &Crown, u: f32, v: f32) -> (f32, f32, f32, f32) {
    let mix = |a: u32, b: u32, k: u32| {
        a.wrapping_mul(7_368_787)
            .wrapping_add(b.wrapping_mul(7919))
            .wrapping_add(k.wrapping_mul(104_729))
    };
    let h = |i: u32, k: u32| hash01(mix(c.seed, i, k)) as f32;
    let a = v.atan2(u);
    let r = (u * u + v * v).sqrt();
    let edge = c.radius
        * (1.
            + c.lobe_amp
                * ((a * c.lobes as f32 + h(0, 9) * 6.).sin() * 0.6
                    + (a * (c.lobes + 3) as f32 + 1.3).cos() * 0.4));
    let dome_h = |x: f32, y: f32| (1. - (x * x + y * y) / (edge * edge)).max(0.).sqrt() * c.dome;
    let (mut best, mut n, mut cover) = (f32::MIN, (0f32, 0f32, 1f32), 0f32);
    for i in 0..c.clumps {
        let ang = h(i, 1) * std::f32::consts::TAU;
        let rr = c.clump_r.0 + (c.clump_r.1 - c.clump_r.0) * h(i, 3);
        let dist = (edge - rr * 0.7).max(0.) * h(i, 2).sqrt();
        let (cx, cy) = (ang.cos() * dist, ang.sin() * dist);
        let (dx, dy) = (u - cx, v - cy);
        let d2 = dx * dx + dy * dy;
        if d2 >= rr * rr {
            continue;
        }
        let z = (rr * rr - d2).sqrt();
        let top = dome_h(cx, cy) + z;
        cover = cover.max(((rr - d2.sqrt()) / rr * 6.).min(1.));
        if top > best {
            best = top;
            n = (dx / rr, dy / rr, z / rr);
        }
    }
    // Kern ohne Ballen: Kuppel, tiefer
    let core = (((edge * 0.84) - r) * 20.).clamp(0., 1.);
    let mut depth = 1.;
    if best == f32::MIN {
        if core <= 0. {
            return (0., 0., 0., 0.);
        }
        best = dome_h(u, v) * 0.7;
        n = (0., 0., 1.);
        depth = 0.78;
    }
    let inside = ((edge - r) * 28.).clamp(0., 1.);
    let dn = (u / edge * 0.7, v / edge * 0.7);
    let (nx, ny, nz) = (n.0 + dn.0, n.1 + dn.1, n.2);
    let l = (nx * nx + ny * ny + nz * nz).sqrt();
    let (nx, ny) = (nx / l, ny / l);
    // Blattwerk: zwei Oktaven Rauschen, bei Nadeln entlang des Radius gestreckt (Büschel vom Stamm weg)
    let g = c.grain;
    let (gu, gv) = if c.needles > 0. {
        (r * g * 0.35, a * g * 1.6)
    } else {
        (u * g, v * g)
    };
    let leaf = 0.6 * vnoise(gu, gv, c.seed) + 0.4 * vnoise(gu * 2.3, gv * 2.3, c.seed + 5);
    let needle = if c.needles > 0. {
        1. - c.needles * 0.35 * (1. - vnoise(r * 90., a * 70., c.seed + 9)).powi(3)
    } else {
        1.
    };
    let top = (best / (c.dome + c.clump_r.1)).clamp(0., 1.);
    let ao = (0.6 + 0.4 * top) * (0.86 + 0.14 * n.2) * depth;
    let shade = (ao * (0.8 + 0.22 * leaf) * needle).clamp(0., 1.);
    // Lücken (lockere Kronen): weiche Löcher aus grobem Rauschen, eher am Rand als in der Mitte
    let holes =
        0.65 * vnoise(u * 4.5, v * 4.5, c.seed + 3) + 0.35 * vnoise(u * 9.7, v * 9.7, c.seed + 4);
    let gap = 1. - ((c.gaps * (0.6 + r) - holes) * 6.).clamp(0., 1.) * 0.85;
    let alpha = inside * cover.max(core) * gap;
    (shade, nx, ny, alpha)
}

/// Abstand eines Punktes p zur Strecke a–b.
fn seg(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (px, py, ax, ay, bx, by) = (p.0, p.1, a.0, a.1, b.0, b.1);
    let (dx, dy) = (bx - ax, by - ay);
    let t = (((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0., 1.);
    ((px - ax - t * dx).powi(2) + (py - ay - t * dy).powi(2)).sqrt()
}

/// Laub: verstreute Blätter (Ellipsen) in Herbstfarben, deterministisch aus einem Hash.
fn leaves(u: f32, v: f32) -> ([f32; 3], f32) {
    for i in 0..110u32 {
        let h = |k: u32| hash01(i * 7919 + k * 104729 + 31) as f32;
        let (cx, cy) = (h(1) * 1.7 - 0.85, h(2) * 1.7 - 0.85);
        if (cx * cx + cy * cy) > 0.8 {
            continue;
        }
        let a = h(3) * std::f32::consts::TAU;
        let (dx, dy) = (u - cx, v - cy);
        let (x, y) = (dx * a.cos() + dy * a.sin(), -dx * a.sin() + dy * a.cos());
        let (rx, ry) = (0.08 + h(4) * 0.06, 0.04 + h(5) * 0.025);
        let d = (x / rx).powi(2) + (y / ry).powi(2);
        if d < 1. {
            let c = match (h(6) * 4.) as u32 {
                0 => [0.62, 0.36, 0.12],
                1 => [0.74, 0.55, 0.18],
                2 => [0.45, 0.30, 0.14],
                _ => [0.56, 0.48, 0.20],
            };
            // Mittelrippe etwas dunkler
            let k = if y.abs() < ry * 0.18 { 0.8 } else { 1. };
            return (c.map(|x| x * k), ((1. - d) * 6.).clamp(0., 0.92));
        }
    }
    ([0.; 3], 0.)
}

/// Fahrradpiktogramm (Radfahrstreifen): zwei Räder, Rahmen, Lenker, Sattel – weiße Linien.
fn bike(u: f32, v: f32) -> f32 {
    let p = (u, v);
    let ring = |c: (f32, f32)| (((u - c.0).powi(2) + (v - c.1).powi(2)).sqrt() - 0.27).abs();
    let (rear, front) = ((-0.5, 0.2), (0.5, 0.2));
    let (crank, seat, head) = ((-0.05, 0.2), (-0.2, -0.22), (0.32, -0.22));
    let d = ring(rear)
        .min(ring(front))
        .min(seg(p, rear, crank))
        .min(seg(p, crank, seat))
        .min(seg(p, seat, head))
        .min(seg(p, head, crank))
        .min(seg(p, rear, seat))
        .min(seg(p, head, front))
        .min(seg(p, (0.24, -0.34), (0.42, -0.34)))
        .min(seg(p, (-0.3, -0.3), (-0.1, -0.3)));
    ((0.065 - d) * 60.).clamp(0., 1.)
}

pub fn pixels() -> Vec<u8> {
    let mut out = vec![0; (WIDTH * HEIGHT * 4) as usize];
    for cell in 0..CELLS {
        for y in 0..CELL {
            for x in 0..CELL {
                let u = (x as f32 + 0.5) / CELL as f32 * 2. - 1.;
                let v = (y as f32 + 0.5) / CELL as f32 * 2. - 1.;
                let r = (u * u + v * v).sqrt();
                let noise = hash01(cell * 100003 + x * 137 + y * 7919) as f32;
                let (color, alpha) = match cell {
                    0 | 6 | 12..=15 => {
                        let (shade, nx, ny, a) = crown(&crown_kind(cell), u, v);
                        ([shade, nx * 0.5 + 0.5, ny * 0.5 + 0.5], a)
                    }
                    1 => {
                        let edge = u.abs().max(v.abs());
                        let grate = (u * 12.).sin().abs();
                        let shade = if grate < 0.3 { 0.12 } else { 0.36 };
                        (
                            [shade, shade, shade * 0.9],
                            if edge < 0.82 { 1. } else { 0. },
                        )
                    }
                    2 => {
                        let edge = 0.87;
                        let ring = if r > 0.73 { 0.2 } else { 0.34 };
                        let detail = if (u * 15.).sin().abs() < 0.17 || (v * 15.).sin().abs() < 0.17
                        {
                            0.08
                        } else {
                            0.
                        };
                        ([ring - detail; 3], ((edge - r) * 40.).clamp(0., 1.))
                    }
                    3 => {
                        let edge = (u.abs() / 0.90).max(v.abs() / 0.84);
                        let shade = 0.2 + noise * 0.06;
                        (
                            [shade, shade + 0.025, shade + 0.03],
                            ((1. - edge) * 24.).clamp(0., 1.),
                        )
                    }
                    4 => {
                        let line = 0.28 * (u * 8.).sin() + 0.11 * (u * 19.).sin();
                        (
                            [0.07, 0.08, 0.08],
                            ((0.065 - (v - line).abs()) * 35.).clamp(0., 0.8),
                        )
                    }
                    5 => {
                        let ellipse = (u * u + v * v * 1.4).sqrt();
                        ([0.09, 0.08, 0.065], ((0.8 - ellipse) * 2.5).clamp(0., 0.5))
                    }
                    10 => leaves(u, v),
                    11 => ([1.; 3], bike(u, v)),
                    // weiches Band (Ölband in der Fahrstreifenmitte, Kontaktschatten am Hausfuß): quer
                    // glockenförmig, längs mit weichen Enden, damit überlappende Stempel kaum Stufen bilden
                    8 | 9 => {
                        let across = (1. - v * v).max(0.).powi(2);
                        let along = (1. - u.abs().powi(6)).max(0.);
                        let peak = if cell == 8 { 0.13 } else { 0.32 };
                        ([0.02, 0.02, 0.025], across * along * peak)
                    }
                    _ => (
                        [1.; 3],
                        if u.abs() < 0.94 && v.abs() < 0.90 {
                            1.
                        } else {
                            0.
                        },
                    ),
                };
                let ax = (cell % COLS) * CELL + x;
                let ay = (cell / COLS) * CELL + y;
                let i = ((ay * WIDTH + ax) * 4) as usize;
                for c in 0..3 {
                    out[i + c] = (color[c] * 255.).round() as u8;
                }
                out[i + 3] = (alpha * 255.).round() as u8;
            }
        }
    }
    out
}

/// Mip-Kette des Atlas: 2 × 2 gemittelt, Farbe nach Deckkraft gewichtet (sonst färben leere Ränder dunkel).
pub fn mips(rgba: &[u8], w: u32, h: u32, levels: u32) -> Vec<(Vec<u8>, u32, u32)> {
    let mut out = vec![(rgba.to_vec(), w, h)];
    for _ in 1..levels {
        let (cur, cw, ch) = out.last().unwrap();
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i =
                        (((y * 2 + dy).min(ch - 1) * cw + (x * 2 + dx).min(cw - 1)) * 4) as usize;
                    let a = cur[i + 3] as f32;
                    for c in 0..3 {
                        acc[c] += cur[i + c] as f32 * a;
                    }
                    acc[3] += a;
                }
                let o = ((y * nw + x) * 4) as usize;
                for c in 0..3 {
                    next[o + c] = if acc[3] > 0. {
                        (acc[c] / acc[3]).round() as u8
                    } else {
                        0
                    };
                }
                next[o + 3] = (acc[3] / 4.).round() as u8;
            }
        }
        out.push((next, nw, nh));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn alpha_of(px: &[u8], cell: u32) -> f32 {
        let mut sum = 0u64;
        for y in 0..CELL {
            for x in 0..CELL {
                let i = ((((cell / COLS) * CELL + y) * WIDTH + (cell % COLS) * CELL + x) * 4 + 3)
                    as usize;
                sum += px[i] as u64;
            }
        }
        sum as f32 / (CELL * CELL) as f32 / 255.
    }
    #[test]
    fn every_used_cell_has_content_and_borders_stay_clear() {
        let px = pixels();
        for cell in 0..CELLS {
            assert!(alpha_of(&px, cell) > 0.005, "Zelle {cell} leer");
        }
        // der Atlas ist voll belegt (eine neue Zelle braucht eine weitere Zeile)
        const { assert!(CELLS == COLS * ROWS) };
        // Rand jeder Zelle (2 px) frei: Mip-Stufen bluten sonst in die Nachbarn (außer Rechteck/Band, die bis
        // knapp an den Rand reichen und dort ohnehin auslaufen)
        for cell in [0, 1, 2, 3, 5, 6, 10, 11, 12, 13, 14, 15] {
            for t in 0..CELL {
                for (x, y) in [(t, 0), (t, CELL - 1), (0, t), (CELL - 1, t)] {
                    let i = ((((cell / COLS) * CELL + y) * WIDTH + (cell % COLS) * CELL + x) * 4
                        + 3) as usize;
                    assert!(px[i] < 8, "Zelle {cell} randet bei ({x}, {y})");
                }
            }
        }
    }
    #[test]
    fn constants_and_mips() {
        let c = shader_constants();
        assert!(c.contains(&format!("ATLAS_CELL: f32 = {CELL}.0")));
        assert!(c.contains(&format!("ATLAS_COLS: f32 = {COLS}.0")));
        let m = mips(&pixels(), WIDTH, HEIGHT, MIPS);
        assert_eq!(m.len() as u32, MIPS);
        assert_eq!((m[4].1, m[4].2), (WIDTH / 16, HEIGHT / 16));
    }
}
#[cfg(test)]
mod dump {
    /// Atlas als PPM (RGB über Schachbrett) zum Ansehen: `GTA_DECAL_DUMP=x.ppm cargo test -p berlin-engine dump_decals -- --ignored`
    #[test]
    #[ignore]
    fn dump_decals() {
        let Ok(path) = std::env::var("GTA_DECAL_DUMP") else {
            return;
        };
        let px = super::pixels();
        let mut out = format!("P6 {} {} 255\n", super::WIDTH, super::HEIGHT).into_bytes();
        for (i, p) in px.chunks(4).enumerate() {
            let (x, y) = (i as u32 % super::WIDTH, i as u32 / super::WIDTH);
            let bg = if (x / 16 + y / 16) % 2 == 0 {
                90.
            } else {
                130.
            };
            let a = p[3] as f32 / 255.;
            for &c in &p[..3] {
                out.push((c as f32 * a + bg * (1. - a)) as u8);
            }
        }
        std::fs::write(&path, out).unwrap();
        std::fs::write(format!("{path}.rgba"), &px).unwrap();
    }
}
