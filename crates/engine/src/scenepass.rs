//! Szenenbild des HD-Pfads (Grafik-Überarbeitung, Phase 1): Karte, Bäume, Bodies, Licht und Effekte werden in ein
//! lineares Float-Ziel (`Rgba16Float`) mit `samples` Abtastungen je Bildpunkt gezeichnet und aufgelöst; die
//! Nachbearbeitung (`post_fs`) bringt es danach ins Ausgabebild, darüber HUD und Minikarte. Pipelines und Ziele hängen
//! an der Abtastzahl und werden beim Wechsel der Qualität neu gebaut.
use crate::{Body, lightpass};
use berlin_map_loader::mesh::{Sprite, Vertex};
use winit::dpi::PhysicalSize;

/// Format des Szenenziels: linear, mit Reserve über 1 (ab Phase 7 HDR).
pub(crate) const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub(crate) const MESH_ATTRS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3,3=>Float32x2,4=>Float32x2,5=>Float32,6=>Float32];
pub(crate) const SPRITE_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32,5=>Float32];
pub(crate) const BODY_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32,3=>Float32,4=>Float32,5=>Float32x4];

pub(crate) fn mesh_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &MESH_ATTRS,
    }
}
pub(crate) fn sprite_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: size_of::<Sprite>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &SPRITE_ATTRS,
    }
}
pub(crate) fn body_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: size_of::<Body>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &BODY_ATTRS,
    }
}

/// Ein Shader, ein Layout: Pipeline mit Tiefe `Depth32Float` für Ziel `format` und `samples` Abtastungen.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pipeline(
    cx: &lightpass::Ctx,
    label: &str,
    vs: &str,
    fs: &str,
    buffers: &[wgpu::VertexBufferLayout],
    format: wgpu::TextureFormat,
    samples: u32,
    blend: Option<wgpu::BlendState>,
    write: bool,
    compare: wgpu::CompareFunction,
) -> wgpu::RenderPipeline {
    cx.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(cx.layout),
            vertex: wgpu::VertexState {
                module: cx.shader,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: cx.shader,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(write),
                depth_compare: Some(compare),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        })
}

/// Pipelines des Szenendurchgangs.
pub(crate) struct ScenePipes {
    pub samples: u32,
    /// Boden, wo keine Fläche liegt (Vollbild nach den Kacheln, ganz hinten: füllt nur, was frei blieb)
    pub ground: wgpu::RenderPipeline,
    pub tiles: wgpu::RenderPipeline,
    pub windows: wgpu::RenderPipeline,
    pub sprites: wgpu::RenderPipeline,
    pub bodies: wgpu::RenderPipeline,
    pub silhouettes: wgpu::RenderPipeline,
    pub effects: wgpu::RenderPipeline,
    pub comp: lightpass::Composites,
}

impl ScenePipes {
    /// `tiles` = Kontext mit Materialgruppe (Gruppe 2) für die Kachel-Pipeline.
    pub fn new(cx: &lightpass::Ctx, tiles: &lightpass::Ctx, samples: u32) -> Self {
        use wgpu::CompareFunction::{Greater, LessEqual};
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let p = |label, vs, fs, layout: wgpu::VertexBufferLayout, blend, write, compare| {
            pipeline(
                cx,
                label,
                vs,
                fs,
                &[layout],
                SCENE_FORMAT,
                samples,
                blend,
                write,
                compare,
            )
        };
        Self {
            samples,
            ground: pipeline(
                tiles,
                "Hintergrundboden",
                "far_vs",
                "ground_fs",
                &[],
                SCENE_FORMAT,
                samples,
                None,
                false,
                wgpu::CompareFunction::Less,
            ),
            tiles: pipeline(
                tiles,
                "Berlin indexed meshes",
                "vs",
                "fs",
                &[mesh_layout()],
                SCENE_FORMAT,
                samples,
                None,
                true,
                LessEqual,
            ),
            // erleuchtete Fenster: dieselben Fassaden nach dem Licht, ohne Tiefe zu schreiben
            windows: p(
                "Berlin lit windows",
                "vs",
                "window_fs",
                mesh_layout(),
                alpha,
                false,
                LessEqual,
            ),
            sprites: p(
                "Berlin instanced atlas",
                "sprite_vs",
                "sprite_fs",
                sprite_layout(),
                alpha,
                true,
                LessEqual,
            ),
            bodies: p(
                "Berlin instanced bodies",
                "body_vs",
                "body_fs",
                body_layout(),
                alpha,
                true,
                LessEqual,
            ),
            // Silhouetten: dieselben Körper, aber nur wo Näheres davor liegt (Tiefe größer als gespeichert)
            silhouettes: p(
                "Berlin silhouettes",
                "body_vs",
                "silhouette_fs",
                body_layout(),
                alpha,
                false,
                Greater,
            ),
            // Durchscheinende Effekte (Qualm, Gischt, Leuchtspuren, Mündungsfeuer): nach Licht und Silhouetten, mit
            // Tiefentest (Dächer bleiben davor), aber ohne Tiefe zu schreiben – sonst zählten sie als Verdeckung und
            // der Silhouetten-Durchgang zeichnete das Auto unter einer Reifenwolke als Umriss.
            effects: p(
                "Berlin effects",
                "body_vs",
                "body_fs",
                body_layout(),
                alpha,
                false,
                LessEqual,
            ),
            comp: lightpass::composites(cx, SCENE_FORMAT, samples),
        }
    }
}

/// Bloom aus dem HDR-Szenenbild (Phase 7): Vorfilter (Schwelle) aufs halbe, Verkleinern aufs Viertel, Vergrößern
/// zurück (additiv). Ohne Tiefe, eine Abtastung – unabhängig von der Kantenglättung.
pub(crate) struct Bloom {
    pub prefilter: wgpu::RenderPipeline,
    pub down: wgpu::RenderPipeline,
    pub up: wgpu::RenderPipeline,
}
impl Bloom {
    pub fn new(cx: &lightpass::Ctx) -> Self {
        let p =
            |label, fs, blend| cx.pipeline(label, "full_vs", fs, &[], SCENE_FORMAT, blend, None, 1);
        Self {
            prefilter: p(
                "Bloom Schwelle ½",
                "bloom_prefilter_fs",
                wgpu::BlendState::REPLACE,
            ),
            down: p("Bloom ¼", "bloom_down_fs", wgpu::BlendState::REPLACE),
            up: p("Bloom ¼ → ½", "bloom_up_fs", lightpass::ADD),
        }
    }
}

/// Ziele des Szenendurchgangs: Farbe (bei Kantenglättung multisampled, sonst direkt das aufgelöste Bild), Tiefe,
/// aufgelöstes Bild als Quelle der Nachbearbeitung, dazu die Bloom-Stufen ½ und ¼.
pub(crate) struct SceneTargets {
    pub color: wgpu::TextureView,
    pub resolve: Option<wgpu::TextureView>,
    pub depth: wgpu::TextureView,
    pub post: wgpu::BindGroup,
    pub half: wgpu::TextureView,
    pub quarter: wgpu::TextureView,
    pub half_bind: wgpu::BindGroup,
    pub quarter_bind: wgpu::BindGroup,
}

impl SceneTargets {
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        size: PhysicalSize<u32>,
        samples: u32,
    ) -> Self {
        let make_sized = |label, format, samples, usage, w: u32, h: u32| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: w.max(1),
                        height: h.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let make = |label, format, samples, usage| {
            make_sized(label, format, samples, usage, size.width, size.height)
        };
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let sampled = attach | wgpu::TextureUsages::TEXTURE_BINDING;
        let image = make("Szenenbild", SCENE_FORMAT, 1, sampled);
        let (color, resolve) = if samples > 1 {
            (
                make("Szenenbild (Abtastungen)", SCENE_FORMAT, samples, attach),
                Some(image.clone()),
            )
        } else {
            (image.clone(), None)
        };
        let bind = |label, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };
        let (w, h) = (size.width.max(2), size.height.max(2));
        let half = make_sized("Bloom ½", SCENE_FORMAT, 1, sampled, w / 2, h / 2);
        let quarter = make_sized("Bloom ¼", SCENE_FORMAT, 1, sampled, w / 4, h / 4);
        Self {
            color,
            resolve,
            depth: make("Szenentiefe", DEPTH_FORMAT, samples, attach),
            post: bind("Nachbearbeitung", &image),
            half_bind: bind("Bloom ½", &half),
            quarter_bind: bind("Bloom ¼", &quarter),
            half,
            quarter,
        }
    }
}
