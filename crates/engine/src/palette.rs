//! Palette des Pixel-Modus (Grafik-Überarbeitung Phase 8): Farben aus `data/gfx/palette.json`, Abstand in OKLab,
//! 3D-Farbtabelle (LUT) für den Shader, Bayer-4×4-Streuung und die Größe eines Bildpunkts. Alles reine Funktionen;
//! der Renderer lädt nur die fertige Tabelle hoch, der Shader rechnet nichts davon nach.

/// Kantenlänge der Farbtabelle (32³ Einträge, sRGB-Eingang).
pub const LUT: usize = 32;

pub struct Palette {
    pub colors: Vec<[u8; 3]>,
    pub dither: f32,
}

/// Palette aus JSON (`farben[].hex` = "#rrggbb", `dither`).
pub fn parse(json: &str) -> anyhow::Result<Palette> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let dither = v["dither"].as_f64().unwrap_or(0.5) as f32;
    let mut colors = Vec::new();
    for f in v["farben"].as_array().into_iter().flatten() {
        let hex = f["hex"]
            .as_str()
            .unwrap_or_default()
            .trim_start_matches('#');
        anyhow::ensure!(hex.len() == 6, "Farbe {hex:?} ist kein #rrggbb");
        let n = u32::from_str_radix(hex, 16)?;
        colors.push([(n >> 16) as u8, (n >> 8) as u8, n as u8]);
    }
    anyhow::ensure!(
        (32..=48).contains(&colors.len()),
        "Palette braucht 32–48 Farben, hat {}",
        colors.len()
    );
    anyhow::ensure!(
        (0. ..=1.).contains(&dither),
        "dither {dither} außerhalb 0…1"
    );
    Ok(Palette { colors, dither })
}

/// Die mitgelieferte Palette.
pub fn shipped() -> Palette {
    parse(include_str!("../../../data/gfx/palette.json")).expect("data/gfx/palette.json")
}

fn linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
/// sRGB (0…1) → OKLab (Björn Ottosson).
pub fn oklab(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(linear);
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// Nächste Palettenfarbe (Index) zu einer sRGB-Farbe, gemessen in OKLab.
pub fn nearest(p: &Palette, rgb: [f32; 3]) -> usize {
    let q = oklab(rgb);
    let mut best = (f32::MAX, 0);
    for (i, c) in p.colors.iter().enumerate() {
        let o = oklab(c.map(|v| v as f32 / 255.));
        let d = (q[0] - o[0]).powi(2) + (q[1] - o[1]).powi(2) + (q[2] - o[2]).powi(2);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// Farbtabelle LUT³ als RGBA8 (Index = r + g·LUT + b·LUT², Eingang sRGB in Zellmitten): je Eintrag die nächste
/// Palettenfarbe (sRGB).
pub fn build_lut(p: &Palette) -> Vec<u8> {
    let mut out = Vec::with_capacity(LUT * LUT * LUT * 4);
    let at = |i: usize| (i as f32 + 0.5) / LUT as f32;
    for b in 0..LUT {
        for g in 0..LUT {
            for r in 0..LUT {
                let c = p.colors[nearest(p, [at(r), at(g), at(b)])];
                out.extend([c[0], c[1], c[2], 255]);
            }
        }
    }
    out
}

/// Bayer-Matrix 4 × 4, Werte (0…15 + 0,5) / 16 − 0,5 ∈ (−0,5; 0,5).
pub fn bayer4(x: u32, y: u32) -> f32 {
    const M: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[(y % 4) as usize][(x % 4) as usize] as f32 + 0.5) / 16. - 0.5
}

/// WGSL-Konstanten (Streuung, Tabellengröße, Bayer-Matrix zeilenweise) – dem Shader vorangestellt.
pub fn shader_constants() -> String {
    let bayer: Vec<String> = (0..16)
        .map(|i| format!("{:?}", bayer4(i % 4, i / 4)))
        .collect();
    format!(
        "const PIXEL_DITHER: f32 = {:?};\nconst PIXEL_LUT: f32 = {:?};\nconst BAYER4 = array<f32, 16>({});\n",
        shipped().dither,
        LUT as f32,
        bayer.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_palette_is_valid_and_distinct() {
        let p = shipped();
        assert!((32..=48).contains(&p.colors.len()));
        for (i, a) in p.colors.iter().enumerate() {
            for b in &p.colors[i + 1..] {
                assert_ne!(a, b, "doppelte Farbe");
            }
        }
        assert!(
            parse(r##"{"farben":[{"hex":"#000000"}]}"##).is_err(),
            "zu wenige Farben"
        );
    }
    #[test]
    fn oklab_and_nearest() {
        let w = oklab([1., 1., 1.]);
        assert!((w[0] - 1.).abs() < 1e-3 && w[1].abs() < 1e-3 && w[2].abs() < 1e-3);
        assert!(oklab([0., 0., 0.])[0].abs() < 1e-6);
        let p = shipped();
        // jede Palettenfarbe findet sich selbst
        for (i, c) in p.colors.iter().enumerate() {
            assert_eq!(nearest(&p, c.map(|v| v as f32 / 255.)), i);
        }
    }
    #[test]
    fn lut_maps_into_the_palette() {
        let p = shipped();
        let lut = build_lut(&p);
        assert_eq!(lut.len(), LUT * LUT * LUT * 4);
        for e in lut.chunks(4) {
            assert!(p.colors.contains(&[e[0], e[1], e[2]]));
        }
        // Schwarz bleibt das dunkelste, Weiß das hellste
        assert_eq!(&lut[..3], &p.colors[0]);
        let last = lut.len() - 4;
        assert_eq!(&lut[last..last + 3], &[0xf7, 0xf5, 0xee]);
    }
    #[test]
    fn bayer_is_balanced() {
        let mut v: Vec<f32> = (0..16).map(|i| bayer4(i % 4, i / 4)).collect();
        assert!(v.iter().sum::<f32>().abs() < 1e-5);
        v.sort_by(f32::total_cmp);
        for w in v.windows(2) {
            assert!((w[1] - w[0] - 1. / 16.).abs() < 1e-6);
        }
        assert_eq!(bayer4(5, 6), bayer4(1, 2));
        // eine Quelle: der Shader bekommt die Matrix aus Rust
        let src = crate::renderer::shader_source();
        assert!(src.contains("const BAYER4 = array<f32, 16>(-0.46875, 0.03125,"));
        assert!(!src.contains("array<f32, 16>(0.0, 8.0"));
    }
}
