//! Bodenmaterialien (Grafik-Überarbeitung, Phase 2): CC0-Texturen aus `data/gfx/materials/` (gebaut von
//! `tools/gfx/build_materials.py`, eingebettet) als zwei Textur-Arrays mit Mip-Stufen – Detail (Farbe/Mittelwert,
//! sRGB-Werte um 0,5) und Normale + Rauheit + Umgebungsverdeckung – und die Zuordnung Kartenfläche → Material aus
//! `data/gfx/material_map.json` als Parameterblock für `scene.wgsl fs`.
use anyhow::{Context, Result, ensure};

/// Manifest der Bodentexturen (Quelle, Lizenz, Urheber) – für die Danksagung in „Über das Spiel“.
pub const MANIFEST: &str = include_str!("../../../data/gfx/materials/manifest.json");
pub(crate) const MAP: &str = include_str!("../../../data/gfx/material_map.json");

macro_rules! material {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!(
                "../../../data/gfx/materials/",
                $name,
                "_detail.png"
            ))
            .as_slice(),
            include_bytes!(concat!("../../../data/gfx/materials/", $name, "_nr.png")).as_slice(),
        )
    };
}
/// Eingebettete Materialien (Reihenfolge = Schicht im Array; muss `materialien` in material_map.json entsprechen).
pub(crate) const FILES: &[(&str, &[u8], &[u8])] = &[
    material!("asphalt"),
    material!("kopfstein"),
    material!("platten"),
    material!("gras"),
    material!("schotter"),
    material!("ziegel"),
    material!("schiefer"),
    material!("blech"),
    material!("kiesdach"),
    material!("putz"),
    material!("klinker"),
    material!("beton"),
];

/// Höchste Material-ID (`mesh.rs`) + 1: Größe des Parameterblocks.
pub(crate) const IDS: usize = 32;

/// Parameter je Material-ID: `a` = (Schicht + 1, 0 = prozedural; Kachel in m; Stärke; Farbanteil),
/// `b` = (Relief, Umgebungsverdeckung, –, –). 2 × 32 × vec4 = 1 KiB.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Params {
    pub a: [[f32; 4]; IDS],
    pub b: [[f32; 4]; IDS],
}

pub(crate) fn params() -> Result<Params> {
    let v: serde_json::Value = serde_json::from_str(MAP).context("material_map.json")?;
    let names: Vec<&str> = v["materialien"]
        .as_array()
        .context("materialien fehlt")?
        .iter()
        .filter_map(|n| n.as_str())
        .collect();
    let mut p = Params {
        a: [[0.; 4]; IDS],
        b: [[0.; 4]; IDS],
    };
    for (id, f) in v["flaechen"].as_object().context("flaechen fehlt")? {
        let id: usize = id.parse().context("Material-ID")?;
        ensure!(id < IDS, "Material-ID {id} zu groß");
        let name = f["material"].as_str().context("material fehlt")?;
        let layer = names
            .iter()
            .position(|n| *n == name)
            .with_context(|| format!("Material {name} nicht in materialien"))?;
        let num = |k: &str| f[k].as_f64().unwrap_or(0.) as f32;
        let tile = num("kachel_m");
        ensure!(tile > 0.05, "kachel_m für {id} fehlt");
        p.a[id] = [layer as f32 + 1., tile, num("staerke"), num("farbe")];
        p.b[id] = [num("relief"), num("ao"), 0., 0.];
    }
    Ok(p)
}

/// Ein Bild als RGBA8 (W × H) aus eingebettetem PNG.
pub(crate) fn decode(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("PNG-Größe")?];
    let info = reader.next_frame(&mut buf)?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        other => anyhow::bail!("PNG-Farbtyp {other:?} nicht unterstützt"),
    };
    Ok((rgba, info.width, info.height))
}

/// Mip-Kette (Kastenfilter 2 × 2); bei der Normalenschicht werden x/y danach wieder auf eine Einheitsnormale
/// gebracht, damit ferne Stufen nicht flacher beleuchtet werden als nötig.
pub(crate) fn mips(rgba: &[u8], w: u32, h: u32, normals: bool) -> Vec<(Vec<u8>, u32, u32)> {
    let mut out = vec![(rgba.to_vec(), w, h)];
    while out.last().is_some_and(|(_, w, h)| *w > 1 && *h > 1) {
        let (cur, cw, ch) = out.last().unwrap();
        let (nw, nh) = (cw / 2, ch / 2);
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0u32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i = (((y * 2 + dy) * cw + x * 2 + dx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += cur[i + c] as u32;
                    }
                }
                let o = ((y * nw + x) * 4) as usize;
                for c in 0..4 {
                    next[o + c] = ((acc[c] + 2) / 4) as u8;
                }
                if normals {
                    let nx = next[o] as f32 / 127.5 - 1.;
                    let ny = next[o + 1] as f32 / 127.5 - 1.;
                    let len = (nx * nx + ny * ny).sqrt();
                    // Neigung in der Ebene behalten (z ergibt sich im Shader), nur zu lange Vektoren kürzen
                    if len > 1. {
                        next[o] = ((nx / len + 1.) * 127.5).round() as u8;
                        next[o + 1] = ((ny / len + 1.) * 127.5).round() as u8;
                    }
                }
            }
        }
        out.push((next, nw, nh));
    }
    out
}

/// Bindungen des Materialblocks (Gruppe 2 der Kachel-Pipelines).
pub(crate) fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let tex = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bodenmaterialien"),
        entries: &[
            tex(0),
            tex(1),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of::<Params>() as u64),
                },
                count: None,
            },
        ],
    })
}

fn array_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    layers: &[Vec<(Vec<u8>, u32, u32)>],
) -> wgpu::TextureView {
    let (w, h) = (layers[0][0].1, layers[0][0].2);
    let levels = layers[0].len() as u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: layers.len() as u32,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, chain) in layers.iter().enumerate() {
        for (level, (px, lw, lh)) in chain.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
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
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

/// Texturen hochladen, Parameter aus der Zuordnung; ergibt die Bindungsgruppe 2.
pub(crate) fn bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> Result<wgpu::BindGroup> {
    let mut detail = Vec::new();
    let mut nr = Vec::new();
    for (name, d, n) in FILES {
        let (px, w, h) = decode(d).with_context(|| format!("{name}_detail.png"))?;
        detail.push(mips(&px, w, h, false));
        let (px, w, h) = decode(n).with_context(|| format!("{name}_nr.png"))?;
        nr.push(mips(&px, w, h, true));
    }
    let params = params()?;
    let buffer = wgpu::util::DeviceExt::create_buffer_init(
        device,
        &wgpu::util::BufferInitDescriptor {
            label: Some("Materialparameter"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        },
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Bodenmaterialien"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        // anisotrop bringt in der Draufsicht nichts Sichtbares (gemessen −0,15 ms ohne)
        anisotropy_clamp: 1,
        ..Default::default()
    });
    let detail = array_texture(device, queue, "Bodendetail", &detail);
    let nr = array_texture(device, queue, "Bodennormalen", &nr);
    Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bodenmaterialien"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&detail),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&nr),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: buffer.as_entire_binding(),
            },
        ],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn manifest_map_and_embedded_files_agree() {
        let man: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
        let in_manifest: BTreeSet<&str> = man["materialien"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let embedded: Vec<&str> = FILES.iter().map(|f| f.0).collect();
        assert_eq!(
            in_manifest,
            embedded.iter().copied().collect(),
            "Manifest und eingebettete Liste"
        );
        let map: serde_json::Value = serde_json::from_str(MAP).unwrap();
        let order: Vec<&str> = map["materialien"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(order, embedded, "Schichtfolge = eingebettete Reihenfolge");
        for (name, m) in man["materialien"].as_object().unwrap() {
            assert_eq!(m["lizenz"], "CC0 1.0", "{name}: nur CC0");
            assert!(
                m["sha256"].as_str().is_some_and(|s| s.len() == 64),
                "{name}: Prüfsumme"
            );
        }
        for (name, d, n) in FILES {
            let (_, w, h) = decode(d).unwrap();
            let (nr, w2, h2) = decode(n).unwrap();
            assert_eq!((w, h, w2, h2), (512, 512, 512, 512), "{name}");
            // Detail ist auf 0,5 gemittelt
            let (px, ..) = decode(d).unwrap();
            let mean = px.chunks(4).map(|c| c[1] as f64).sum::<f64>() / (w * h) as f64 / 255.;
            assert!((mean - 0.5).abs() < 0.03, "{name}: Mittelwert {mean}");
            assert!(
                nr.chunks(4).all(|c| c[2] > 0 || c[3] > 0),
                "{name}: Normalen gepackt"
            );
        }
    }

    #[test]
    fn params_follow_the_map() {
        let p = params().unwrap();
        assert_eq!(p.a[1][0], 1., "Asphalt = Schicht 0 (+1)");
        assert_eq!(p.a[2][1], 2.4, "Kopfstein: 2,4 m je Kachel");
        assert_eq!(p.a[5][0], 0., "Wasser bleibt prozedural");
        assert_eq!(size_of::<Params>(), 1024);
    }

    #[test]
    fn mip_chain_halves_to_one_pixel() {
        let px = vec![200u8; 8 * 4 * 4];
        let chain = mips(&px, 8, 4, false);
        let sizes: Vec<_> = chain.iter().map(|(_, w, h)| (*w, *h)).collect();
        assert_eq!(sizes, [(8, 4), (4, 2), (2, 1)]);
        assert!(chain.iter().all(|(p, ..)| p.iter().all(|&v| v == 200)));
    }

    /// Jede Material-ID, die die Kachel-Meshes tatsächlich erzeugen, ist zugeordnet (Textur oder prozedural).
    #[test]
    fn every_material_id_in_the_tiles_is_mapped() {
        use berlin_map_loader::{
            format::{Index, Tile, TileKey},
            mesh,
        };
        let root = berlin_map_loader::default_data_root();
        let index = Index::read(&root).unwrap();
        let map: serde_json::Value = serde_json::from_str(MAP).unwrap();
        let mut mapped: BTreeSet<i64> = map["flaechen"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.parse().unwrap())
            .collect();
        mapped.extend(
            map["prozedural"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_i64()),
        );
        let mut seen = BTreeSet::new();
        // jede 25. Kachel: Innenstadt, Wald, Wasser, Gleise kommen sicher vor
        for key in index.tiles.iter().step_by(25) {
            let tile = Tile::read(&root, TileKey::parse(key).unwrap(), &index.meta).unwrap();
            for item in &tile.items {
                if let Ok(m) = mesh::prepare(&item.feature, index.meta.scale) {
                    seen.extend(m.vertices.iter().map(|v| v.material as i64));
                }
            }
        }
        assert!(seen.len() >= 10, "zu wenig gesehen: {seen:?}");
        let missing: Vec<_> = seen.difference(&mapped).collect();
        assert!(missing.is_empty(), "nicht zugeordnet: {missing:?}");
    }
}
