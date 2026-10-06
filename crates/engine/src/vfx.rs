//! Flipbooks für Feuer, Explosionen und Rauch (CC0, Unity Labs Paris; gebaut von `tools/gfx/build_vfx.py`).
//!
//! Alle Folgen liegen in einem Atlas `data/gfx/vfx/vfx_atlas.png` (eingebettet). Das Manifest beschreibt das Raster
//! je Folge; daraus entstehen die Shader-Konstanten (`shader_constants`), Atlas und Shader können also nicht
//! auseinanderlaufen. Ein Effekt-Körper zeigt ein Flipbook-Bild, wenn seine Form negativ ist: `shape(folge, bild)`
//! kodiert Folge und (gebrochene) Bildnummer; der Shader blendet zwischen zwei aufeinanderfolgenden Bildern über.
//! Die Farbe des Körpers ist Tönung (rgb, gilt für die nicht leuchtenden Teile, z. B. Rauch im Umgebungslicht) und
//! Deckkraft (a). Gezeichnet werden Flipbooks nur im Effekt-Durchgang (`effect_fs`, vormultiplizierte Mischung).
use anyhow::{Context, Result, ensure};
use std::sync::OnceLock;

/// Manifest (Quelle, Lizenz, Urheber, Raster) – auch für die Danksagung in „Über das Spiel“.
pub const MANIFEST: &str = include_str!("../../../data/gfx/vfx/manifest.json");
pub(crate) const ATLAS_PNG: &[u8] = include_bytes!("../../../data/gfx/vfx/vfx_atlas.png");
/// Abstand zweier Folgen in der Formkodierung (mehr Bilder hat keine Folge)
const SEQ_STRIDE: f32 = 1000.;
/// Mip-Stufen des Atlas (tiefer verwischen benachbarte Bilder ineinander)
pub(crate) const LEVELS: u32 = 5;

/// Eine Folge im Atlas.
#[derive(Debug, Clone, PartialEq)]
pub struct Seq {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub cell: [u32; 2],
    pub cols: u32,
    pub frames: u32,
    /// additiv (Schwarz durchsichtig) statt über den Hintergrund gelegt
    pub add: bool,
    /// Anteil, mit dem helle Bildteile selbst leuchten (0 = Rauch)
    pub glow: f32,
}

fn parse() -> Result<(u32, Vec<Seq>)> {
    let v: serde_json::Value = serde_json::from_str(MANIFEST).context("vfx manifest.json")?;
    let size = v["atlas"].as_u64().context("atlas fehlt")? as u32;
    let mut out = Vec::new();
    for s in v["folgen"].as_array().context("folgen fehlt")? {
        let n = |k: &str| s[k].as_u64().with_context(|| format!("{k} fehlt"));
        let cell = s["zelle"].as_array().context("zelle fehlt")?;
        let seq = Seq {
            name: s["name"].as_str().context("name fehlt")?.to_string(),
            x: n("x")? as u32,
            y: n("y")? as u32,
            cell: [
                cell[0].as_u64().context("zelle")? as u32,
                cell[1].as_u64().context("zelle")? as u32,
            ],
            cols: n("spalten")? as u32,
            frames: n("bilder")? as u32,
            add: s["mischart"].as_str() == Some("add"),
            glow: s["leuchten"].as_f64().unwrap_or(0.) as f32,
        };
        ensure!(
            seq.frames > 0 && (seq.frames as f32) < SEQ_STRIDE,
            "{}: Bilder",
            seq.name
        );
        let rows = seq.frames.div_ceil(seq.cols);
        ensure!(
            seq.x + seq.cols * seq.cell[0] <= size && seq.y + rows * seq.cell[1] <= size,
            "{}: außerhalb des Atlas",
            seq.name
        );
        out.push(seq);
    }
    Ok((size, out))
}

fn table() -> &'static (u32, Vec<Seq>) {
    static T: OnceLock<(u32, Vec<Seq>)> = OnceLock::new();
    T.get_or_init(|| parse().expect("data/gfx/vfx/manifest.json"))
}

/// Alle Folgen in Atlas-Reihenfolge.
pub fn sequences() -> &'static [Seq] {
    &table().1
}

/// Nummer der Folge `name` (für `shape`).
pub fn seq(name: &str) -> Option<u32> {
    sequences()
        .iter()
        .position(|s| s.name == name)
        .map(|i| i as u32)
}

/// Bilder der Folge `seq`.
pub fn frames(seq: u32) -> u32 {
    sequences().get(seq as usize).map_or(1, |s| s.frames)
}

/// Form eines Effekt-Körpers, der Bild `frame` (gebrochen: Überblendung zum nächsten) der Folge `seq` zeigt. Der
/// Shader nimmt das Bild modulo der Bildzahl; wer nicht wiederholen will, klemmt selbst auf `frames − 1`.
pub fn shape(seq: u32, frame: f32) -> f32 {
    let n = frames(seq) as f32;
    let f = frame.rem_euclid(n).min(n - 1e-3);
    -(1. + seq as f32 * SEQ_STRIDE + f)
}

/// WGSL: Atlasgröße, Raster je Folge (`vfx_rect`: x, y, Zellbreite, -höhe) und Eigenschaften (`vfx_meta`: Spalten,
/// Bilder, additiv, Leuchten). Leere Folgenliste ergibt eine gültige, nie getroffene Funktion.
pub fn shader_constants() -> String {
    let (size, seqs) = table();
    let mut rect = String::new();
    let mut meta = String::new();
    for (i, s) in seqs.iter().enumerate() {
        // ganze Zahlen als f32(n): keine Dezimal-Literale, die wie ein fest verdrahtetes Raster aussehen
        rect += &format!(
            "        case {i}u: {{ return vec4(f32({}), f32({}), f32({}), f32({})); }}\n",
            s.x, s.y, s.cell[0], s.cell[1]
        );
        meta += &format!(
            "        case {i}u: {{ return vec4(f32({}), f32({}), f32({}), {:?}); }}\n",
            s.cols,
            s.frames,
            u32::from(s.add),
            s.glow
        );
    }
    format!(
        "const VFX_ATLAS: f32 = f32({});\nconst VFX_STRIDE: f32 = f32({});\n\
         fn vfx_rect(s: u32) -> vec4<f32> {{\n    switch s {{\n{rect}        default: {{ return vec4(0.0); }}\n    }}\n}}\n\
         fn vfx_meta(s: u32) -> vec4<f32> {{\n    switch s {{\n{meta}        default: {{ return vec4(1.0, 1.0, 0.0, 0.0); }}\n    }}\n}}\n",
        size, SEQ_STRIDE as u32
    )
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1. / 2.4) - 0.055
    }
}

/// Atlas als Mip-Kette für `Rgba8UnormSrgb`: Farbe linear mit der Deckkraft vormultipliziert, dann sRGB-kodiert –
/// die Hardware dekodiert vor dem Filtern, gefiltert wird also korrekt vormultipliziert (keine dunklen Säume).
/// Additive Folgen behalten Deckkraft 1 (der Shader setzt sie auf 0).
pub(crate) fn mip_chain() -> Result<Vec<(Vec<u8>, u32, u32)>> {
    let (rgba, w, h) = crate::materials::decode(ATLAS_PNG)?;
    let (size, _) = table();
    ensure!(w == *size && h == *size, "VFX-Atlas {w}×{h} statt {size}²");
    // Stufe 0 linear und vormultipliziert
    let mut lin: Vec<[f32; 4]> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            let a = p[3] as f32 / 255.;
            [
                to_linear(p[0] as f32 / 255.) * a,
                to_linear(p[1] as f32 / 255.) * a,
                to_linear(p[2] as f32 / 255.) * a,
                a,
            ]
        })
        .collect();
    let encode = |px: &[[f32; 4]]| -> Vec<u8> {
        px.iter()
            .flat_map(|c| {
                [
                    (to_srgb(c[0].clamp(0., 1.)) * 255. + 0.5) as u8,
                    (to_srgb(c[1].clamp(0., 1.)) * 255. + 0.5) as u8,
                    (to_srgb(c[2].clamp(0., 1.)) * 255. + 0.5) as u8,
                    (c[3].clamp(0., 1.) * 255. + 0.5) as u8,
                ]
            })
            .collect()
    };
    let mut out = vec![(encode(&lin), w, h)];
    let (mut cw, mut ch) = (w, h);
    for _ in 1..LEVELS {
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let mut next = vec![[0f32; 4]; (nw * nh) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p =
                        lin[((y * 2 + dy).min(ch - 1) * cw + (x * 2 + dx).min(cw - 1)) as usize];
                    for c in 0..4 {
                        acc[c] += p[c] * 0.25;
                    }
                }
                next[(y * nw + x) as usize] = acc;
            }
        }
        out.push((encode(&next), nw, nh));
        lin = next;
        (cw, ch) = (nw, nh);
    }
    Ok(out)
}

/// Atlas-Textur mit allen Mip-Stufen hochladen.
pub(crate) fn texture(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<wgpu::TextureView> {
    let chain = mip_chain()?;
    let (w, h) = (chain[0].1, chain[0].2);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("VFX-Atlas"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: chain.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, (px, lw, lh)) in chain.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            px,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(lw * 4),
                rows_per_image: Some(*lh),
            },
            wgpu::Extent3d {
                width: *lw,
                height: *lh,
                depth_or_array_layers: 1,
            },
        );
    }
    Ok(texture.create_view(&Default::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_lists_every_effect_the_game_uses() {
        for name in [
            "explosion_a",
            "explosion_b",
            "blast",
            "fireball",
            "flame",
            "flame_small",
            "smoke",
        ] {
            let i = seq(name).unwrap_or_else(|| panic!("{name} fehlt"));
            assert!(frames(i) >= 16, "{name}: zu wenig Bilder");
        }
        assert!(sequences().iter().any(|s| s.add), "Feuerkern additiv");
        assert_eq!(
            sequences().iter().find(|s| s.name == "smoke").unwrap().glow,
            0.
        );
    }
    #[test]
    fn shape_codes_round_trip_and_wrap() {
        let s = seq("flame").unwrap();
        let n = frames(s) as f32;
        let dec = |sh: f32| {
            let g = -sh - 1.;
            (
                (g / SEQ_STRIDE).floor() as u32,
                g - (g / SEQ_STRIDE).floor() * SEQ_STRIDE,
            )
        };
        assert!(shape(s, 0.) < 0.);
        assert_eq!(dec(shape(s, 3.25)), (s, 3.25));
        let (seq2, f) = dec(shape(s, n + 2.5));
        assert_eq!(seq2, s);
        assert!((f - 2.5).abs() < 1e-3, "Schleife: {f}");
        assert!(dec(shape(s, n)).1 < n);
    }
    #[test]
    fn shader_constants_name_every_sequence() {
        let w = shader_constants();
        for i in 0..sequences().len() {
            assert!(w.contains(&format!("case {i}u")));
        }
        assert!(w.contains("fn vfx_rect") && w.contains("fn vfx_meta"));
    }
    #[test]
    fn atlas_decodes_premultiplied_with_mips() {
        let chain = mip_chain().unwrap();
        assert_eq!(chain.len() as u32, LEVELS);
        assert_eq!((chain[0].1, chain[0].2), (2048, 2048));
        // vormultipliziert: keine Farbe ohne Deckkraft (außer im additiven Feuerkern, der Deckkraft 1 trägt)
        let px = &chain[0].0;
        let bad = px
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] == 0 && (p[0] | p[1] | p[2]) != 0)
            .count();
        assert_eq!(bad, 0);
        assert!(px.as_chunks::<4>().0.iter().any(|p| p[3] > 200));
    }
}
