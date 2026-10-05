//! Licht und Schatten als GPU-Pässe (Port von `lighting.js`):
//! - Schattenmaske (R8, Offscreen): Hauswände werden im Vertex-Shader entlang der Sonne extrudiert, Baumkronen
//!   als gestreckte Kronenbilder versetzt; überlappende Schatten addieren sich nicht (Max-Mischung).
//! - Lichtkarte (RGBA16F, halbe Auflösung): mit dem Umgebungslicht gefüllt, Lichtquellen additiv.
//! - Auftragen über die vorhandene Tiefe: ein Vollbild-Dreieck in Tiefe 0,5 trifft nur den Boden (Straßen,
//!   Flächen, Autos, Personen liegen dahinter); Dächer und Kronen liegen davor und bekommen nur das
//!   Umgebungslicht – kein Laternenschein auf Dächern (lightOccluders der JS-Fassung).
use crate::LightSource;

pub(crate) const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
pub(crate) const LIGHT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub(crate) struct Target {
    pub view: wgpu::TextureView,
    pub bind: wgpu::BindGroup,
}

pub(crate) struct LightPass {
    pub shadow: wgpu::RenderPipeline,
    pub tree_shadow: wgpu::RenderPipeline,
    pub light: wgpu::RenderPipeline,
    pub mask: Target,
    pub lightmap: Target,
    sampler: wgpu::Sampler,
    pub lights: Option<wgpu::Buffer>,
    capacity: usize,
    pub light_count: u32,
}

/// Auftragen auf das Szenenbild (Vollbild-Dreiecke im Hauptdurchgang): hängen am Format und an der Abtastzahl
/// des Szenenziels und werden mit ihm neu gebaut.
pub(crate) struct Composites {
    pub shadow: wgpu::RenderPipeline,
    pub light: wgpu::RenderPipeline,
    pub ambient: wgpu::RenderPipeline,
    pub bloom: wgpu::RenderPipeline,
}

pub(crate) struct Ctx<'a> {
    pub device: &'a wgpu::Device,
    pub layout: &'a wgpu::PipelineLayout,
    pub atlas_layout: &'a wgpu::BindGroupLayout,
    pub shader: &'a wgpu::ShaderModule,
}

const MAX: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Max,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Max,
    },
};
const ADD: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};
/// Quelle · (1 − Ziel) + Ziel („screen“, Bloom).
const SCREEN: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::OneMinusDst,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};
/// Ziel × Quelle (Lichtkarte über das fertige Bild, „multiply“).
const MULTIPLY: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Dst,
        dst_factor: wgpu::BlendFactor::Zero,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

fn depth_test(compare: wgpu::CompareFunction) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth32Float,
        depth_write_enabled: Some(false),
        depth_compare: Some(compare),
        stencil: Default::default(),
        bias: Default::default(),
    }
}

impl Ctx<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn pipeline(
        &self,
        label: &str,
        vs: &str,
        fs: &str,
        buffers: &[wgpu::VertexBufferLayout],
        format: wgpu::TextureFormat,
        blend: wgpu::BlendState,
        depth: Option<wgpu::DepthStencilState>,
        samples: u32,
    ) -> wgpu::RenderPipeline {
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(self.layout),
                vertex: wgpu::VertexState {
                    module: self.shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: self.shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: depth,
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
    }
    fn target(
        &self,
        sampler: &wgpu::Sampler,
        label: &str,
        format: wgpu::TextureFormat,
        w: u32,
        h: u32,
    ) -> Target {
        let view = self
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w.max(1),
                    height: h.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: self.atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        Target { view, bind }
    }
}

/// Auftragen über die vorhandene Tiefe (s. Moduldoku) ins Szenenziel `format` mit `samples` Abtastungen.
pub(crate) fn composites(cx: &Ctx, format: wgpu::TextureFormat, samples: u32) -> Composites {
    use wgpu::CompareFunction::{Always, GreaterEqual, Less};
    let full = |label, fs, blend, compare| {
        cx.pipeline(
            label,
            "full_vs",
            fs,
            &[],
            format,
            blend,
            Some(depth_test(compare)),
            samples,
        )
    };
    Composites {
        shadow: full(
            "Schatten auftragen",
            "shadow_composite_fs",
            wgpu::BlendState::ALPHA_BLENDING,
            Less,
        ),
        light: full("Lichtkarte auftragen", "light_composite_fs", MULTIPLY, Less),
        ambient: full(
            "Umgebungslicht auf Dächern",
            "ambient_composite_fs",
            MULTIPLY,
            GreaterEqual,
        ),
        bloom: full("Bloom", "bloom_fs", SCREEN, Always),
    }
}

impl LightPass {
    pub fn new(cx: &Ctx, sprite_layout: wgpu::VertexBufferLayout, w: u32, h: u32) -> Self {
        let shadow_attrs = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32,2=>Float32];
        let shadow_layout = wgpu::VertexBufferLayout {
            array_stride: size_of::<berlin_map_loader::mesh::ShadowVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &shadow_attrs,
        };
        let light_attrs = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32,2=>Float32,3=>Float32x3,4=>Float32,5=>Float32];
        let light_layout = wgpu::VertexBufferLayout {
            array_stride: size_of::<LightSource>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &light_attrs,
        };
        let sampler = cx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Licht/Schatten"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Self {
            shadow: cx.pipeline(
                "Schattenmaske Häuser",
                "shadow_vs",
                "shadow_fs",
                &[shadow_layout],
                MASK_FORMAT,
                MAX,
                None,
                1,
            ),
            tree_shadow: cx.pipeline(
                "Schattenmaske Bäume",
                "tree_shadow_vs",
                "tree_shadow_fs",
                &[sprite_layout],
                MASK_FORMAT,
                MAX,
                None,
                1,
            ),
            light: cx.pipeline(
                "Lichtquellen",
                "light_vs",
                "light_fs",
                &[light_layout],
                LIGHT_FORMAT,
                ADD,
                None,
                1,
            ),
            mask: cx.target(&sampler, "Schattenmaske", MASK_FORMAT, w, h),
            lightmap: cx.target(
                &sampler,
                "Lichtkarte",
                LIGHT_FORMAT,
                w.div_ceil(2),
                h.div_ceil(2),
            ),
            sampler,
            lights: None,
            capacity: 0,
            light_count: 0,
        }
    }
    pub fn resize(&mut self, cx: &Ctx, w: u32, h: u32) {
        self.mask = cx.target(&self.sampler, "Schattenmaske", MASK_FORMAT, w, h);
        self.lightmap = cx.target(
            &self.sampler,
            "Lichtkarte",
            LIGHT_FORMAT,
            w.div_ceil(2),
            h.div_ceil(2),
        );
    }
    pub fn set_lights(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lights: &[LightSource],
    ) {
        self.light_count = lights.len() as u32;
        if lights.is_empty() {
            return;
        }
        if self.lights.is_none() || self.capacity < lights.len() {
            self.capacity = lights.len().next_power_of_two().max(256);
            self.lights = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Lichtquellen"),
                size: (self.capacity * size_of::<LightSource>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(b) = &self.lights {
            queue.write_buffer(b, 0, bytemuck::cast_slice(lights));
        }
    }
}
