//! Deterministic procedural sprite atlas; no runtime image files or Canvas work.
use berlin_map_loader::citycodes::hash01;
pub const CELL: u32 = 64;
pub const WIDTH: u32 = CELL * 4;
pub const HEIGHT: u32 = CELL * 3;
pub fn pixels() -> Vec<u8> {
    let mut out = vec![0; (WIDTH * HEIGHT * 4) as usize];
    for cell in 0..10 {
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
                let ax = (cell % 4) * CELL + x;
                let ay = (cell / 4) * CELL + y;
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
