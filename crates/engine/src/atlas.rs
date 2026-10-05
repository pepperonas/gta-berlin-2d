//! Deterministic procedural sprite atlas; no runtime image files or Canvas work.
//! Zellen: 0 Laubkrone, 1 Gully, 2 Schachtdeckel, 3 Flicken, 4 Riss, 5 Ölfleck, 6 Nadelkrone, 7 weißes Rechteck
//! (Markierungen), 8 Ölband, 9 Kontaktschatten, 10 Laub, 11 Fahrradpiktogramm. Raster und Zellgröße gehen als
//! Konstanten in den Shader (`shader_constants`), nichts ist dort fest verdrahtet.
use berlin_map_loader::citycodes::hash01;
pub const CELL: u32 = 256;
pub const COLS: u32 = 4;
pub const ROWS: u32 = 4;
pub const WIDTH: u32 = CELL * COLS;
pub const HEIGHT: u32 = CELL * ROWS;
/// belegte Zellen
pub const CELLS: u32 = 12;
/// Mip-Stufen: bis 16 px je Zelle (darunter bluten Nachbarzellen ineinander)
pub const MIPS: u32 = 5;

/// WGSL-Konstanten des Atlas (dem Shader vorangestellt).
pub fn shader_constants() -> String {
    format!(
        "const ATLAS_CELL: f32 = {CELL}.0;\nconst ATLAS_COLS: f32 = {COLS}.0;\nconst ATLAS_ROWS: f32 = {ROWS}.0;\n"
    )
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
                    0 => {
                        let a = v.atan2(u);
                        let edge = 0.79 + 0.065 * (a * 7.).sin() + 0.045 * (a * 11. + 0.7).cos();
                        let shade = (0.58 + 0.30 * (1. - r) + 0.10 * noise - 0.10 * u + 0.08 * v)
                            .clamp(0., 1.);
                        ([shade; 3], ((edge - r) * 32.).clamp(0., 1.))
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
                    6 => {
                        let a = v.atan2(u);
                        let edge = 0.65 + 0.19 * (a * 9.).cos().abs();
                        let shade = (0.53 + 0.32 * (1. - r) + 0.11 * noise).clamp(0., 1.);
                        ([shade; 3], ((edge - r) * 32.).clamp(0., 1.))
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
        for cell in CELLS..COLS * ROWS {
            assert_eq!(alpha_of(&px, cell), 0., "Zelle {cell} sollte frei sein");
        }
        // Rand jeder Zelle (2 px) frei: Mip-Stufen bluten sonst in die Nachbarn (außer Rechteck/Band, die bis
        // knapp an den Rand reichen und dort ohnehin auslaufen)
        for cell in [0, 1, 2, 3, 5, 6, 10, 11] {
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
        std::fs::write(path, out).unwrap();
    }
}
