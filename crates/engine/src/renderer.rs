use crate::{
    Body, LightSource, Lighting, atlas,
    camera::Camera,
    gputime::GpuTimer,
    graphics::{self, GraphicsMode, GraphicsSettings},
    hud::{self, HudItem, MapInset},
    lightpass, materials,
    scenepass::{self, ScenePipes, SceneTargets},
};
use anyhow::{Context, Result};
use berlin_map_loader::{
    format::TileKey,
    geom::Bounds,
    mesh::{Mesh, ShadowVertex},
    overview::{OverlayMesh, OverlayVertex},
    stream::Snapshot,
};
use glam::Vec2;
use std::{collections::BTreeMap, sync::Arc};
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};
struct GpuTile {
    source: Arc<Mesh>,
    bounds: Bounds,
    vertices: Option<wgpu::Buffer>,
    indices: Option<wgpu::Buffer>,
    sprites: Option<wgpu::Buffer>,
    shadows: Option<wgpu::Buffer>,
    shadow_indices: Option<wgpu::Buffer>,
}
pub(crate) struct Renderer {
    pub window: Arc<Window>,
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// Szenendurchgang (HD-Pfad): Pipelines und Ziele, je nach Abtastzahl der Qualitätsstufe
    pipes: ScenePipes,
    targets: SceneTargets,
    /// Layout und Gruppe 2 der Körper-Pipelines (Schriftatlas für Schilder in der Welt)
    body_layout: wgpu::PipelineLayout,
    world_font: wgpu::BindGroup,
    graphics: GraphicsSettings,
    /// Minikarte: Kacheln direkt ins Ausgabebild (Ausgabeformat, ohne Kantenglättung)
    map_pipeline: wgpu::RenderPipeline,
    /// Nachbearbeitung: Szenenbild → Ausgabebild (Farbabstimmung, Vignette)
    post_pipeline: wgpu::RenderPipeline,
    bloom: scenepass::Bloom,
    /// Pixel-Modus: Nachbearbeitung und ihre Gruppe 3 (Tiefe + Farbtabelle; nur bei einer Abtastung)
    pixel: scenepass::PixelPass,
    pixel_bind: Option<wgpu::BindGroup>,
    /// Kamera der Szene: im Pixel-Modus kleines Ziel, gröberer Maßstab, Lage auf das Bildpunktraster gerastet;
    /// Nachbearbeitung und HUD behalten `uniform`/`bind` (volle Größe)
    scene_uniform: wgpu::Buffer,
    scene_bind: wgpu::BindGroup,
    /// Splitscreen: Kamera der zweiten Ansicht (Szene und volle Größe)
    scene_uniform2: wgpu::Buffer,
    scene_bind2: wgpu::BindGroup,
    uniform2: wgpu::Buffer,
    bind2: wgpu::BindGroup,
    /// Ansichten des letzten Bildes (Aufnahme zeichnet sie nach)
    last_cams: Vec<Camera>,
    /// Tiefe des HUD-Durchgangs (Minikarte), eine Abtastung
    hud_depth: wgpu::TextureView,
    silhouettes: Option<wgpu::Buffer>,
    silhouette_count: u32,
    effects: Option<wgpu::Buffer>,
    effect_count: u32,
    bodies: Option<wgpu::Buffer>,
    body_capacity: usize,
    body_count: u32,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    atlas_bind: wgpu::BindGroup,
    /// Fahrzeugbilder (vom Spiel einmal geliefert; bis dahin leer)
    vehicle_bind: wgpu::BindGroup,
    atlas_sampler: wgpu::Sampler,
    tiles: BTreeMap<TileKey, GpuTile>,
    /// Zeichengröße (Fenster oder fest, `--fenster`)
    size: PhysicalSize<u32>,
    /// feste Zeichengröße: das Bild entsteht abseits des Fensters (Messung, Aufnahmen größer als der Bildschirm)
    fixed: Option<PhysicalSize<u32>>,
    offscreen: Option<wgpu::TextureView>,
    /// Fenstervorschau des abseits gezeichneten Bildes (nur bei fester Zeichengröße)
    preview: Option<(wgpu::RenderPipeline, wgpu::BindGroup)>,
    scale: f32,
    lighting: Lighting,
    light: lightpass::LightPass,
    layout: wgpu::PipelineLayout,
    /// Layout der Kachel-Pipelines: Kamera, Atlas, Bodenmaterialien
    tile_layout: wgpu::PipelineLayout,
    materials: wgpu::BindGroup,
    atlas_layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    hud_pipeline: wgpu::RenderPipeline,
    hud_font: wgpu::BindGroup,
    hud: Option<wgpu::Buffer>,
    hud_capacity: usize,
    hud_count: u32,
    /// Minikarten (Koop: zwei), je mit eigener Kamera
    maps: [Option<MapInset>; 2],
    map_uniform: wgpu::Buffer,
    map_bind: wgpu::BindGroup,
    map_uniform2: wgpu::Buffer,
    map_bind2: wgpu::BindGroup,
    overlay_pipeline: wgpu::RenderPipeline,
    overview: Option<(wgpu::Buffer, wgpu::Buffer, u32)>,
    /// GPU-Zeit je Bild (nur mit `--messung` und wenn der Adapter Zeitstempel kann)
    gpu: Option<GpuTimer>,
    /// Wartezeit auf das nächste Swapchain-Bild im letzten `render` (ms; Bildtakt, keine Arbeit)
    pub acquire_ms: f32,
}
impl Renderer {
    pub async fn new(
        window: Arc<Window>,
        scale: f32,
        lighting: Lighting,
        measure: bool,
        fixed: Option<(u32, u32)>,
    ) -> Result<Self> {
        let backends = if cfg!(target_os = "macos") {
            wgpu::Backends::METAL
        } else if cfg!(target_os = "windows") {
            wgpu::Backends::DX12
        } else {
            wgpu::Backends::VULKAN
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await?;
        eprintln!("GPU: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Berlin device"),
                // Zeitstempel nur zum Messen anfordern (optionales Merkmal; ohne läuft alles wie bisher)
                required_features: if measure {
                    adapter.features() & wgpu::Features::TIMESTAMP_QUERY
                } else {
                    wgpu::Features::empty()
                },
                ..Default::default()
            })
            .await?;
        let fixed = fixed.map(|(w, h)| PhysicalSize::new(w, h));
        let win = window.inner_size();
        let size = fixed.unwrap_or(win);
        let mut config = surface
            .get_default_config(&adapter, win.width.max(1), win.height.max(1))
            .context("Keine Surface-Konfiguration")?;
        let caps = surface.get_capabilities(&adapter);
        config.format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 1;
        surface.configure(&device, &config);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Berlin mesh / atlas shaders"),
            source: wgpu::ShaderSource::Wgsl(shader_source().into()),
        });
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
                },
                count: None,
            }],
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Atlas layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Berlin pipeline layout"),
            bind_group_layouts: &[Some(&camera_layout), Some(&atlas_layout)],
            immediate_size: 0,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera / sun"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let scene_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Kamera der Szene"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Kamera der Szene"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scene_uniform.as_entire_binding(),
            }],
        });
        let camera_buffer = |label| {
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: UNIFORM_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &camera_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            });
            (buf, bind)
        };
        let (scene_uniform2, scene_bind2) = camera_buffer("Kamera der Szene (Ansicht 2)");
        let (uniform2, bind2) = camera_buffer("Kamera (Ansicht 2)");
        let (map_uniform2, map_bind2) = camera_buffer("Minikarte 2");
        let map_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Minikarte"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let map_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Minikarte"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: map_uniform.as_entire_binding(),
            }],
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Procedural decals / trees atlas"),
            size: wgpu::Extent3d {
                width: atlas::WIDTH,
                height: atlas::HEIGHT,
                depth_or_array_layers: 1,
            },
            // Mip-Stufen: Kronen und Decals flimmern beim Herauszoomen sonst
            mip_level_count: atlas::MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, (px, w, h)) in
            atlas::mips(&atlas::pixels(), atlas::WIDTH, atlas::HEIGHT, atlas::MIPS)
                .iter()
                .enumerate()
        {
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
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(*h),
                },
                wgpu::Extent3d {
                    width: *w,
                    height: *h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let atlas_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Atlas"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let vehicle_bind =
            vehicle_atlas_bind(&device, &queue, &atlas_layout, &sampler, &[0; 4], 1, 1);
        let hud_attrs = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32,3=>Float32,4=>Float32x4,5=>Float32x4];
        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HUD"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("hud_vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<HudItem>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &hud_attrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("hud_fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            // im Hauptpass gezeichnet (mit Tiefenpuffer), aber ohne Tiefentest
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let overlay_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x4];
        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Stadtplan"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("overlay_vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<OverlayVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &overlay_attrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("overlay_fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let (font_px, font_w, font_h) = hud::atlas();
        let font = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HUD-Schrift"),
            size: wgpu::Extent3d {
                width: font_w,
                height: font_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &font,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &font_px,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(font_w),
                rows_per_image: Some(font_h),
            },
            font.size(),
        );
        let font_view = font.create_view(&Default::default());
        let font_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HUD-Schrift"),
            ..Default::default()
        });
        let hud_font = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HUD-Schrift"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&font_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&font_sampler),
                },
            ],
        });
        let mat_layout = materials::layout(&device);
        let materials = materials::bind_group(&device, &queue, &mat_layout)?;
        let tile_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Kacheln mit Bodenmaterialien"),
            bind_group_layouts: &[Some(&camera_layout), Some(&atlas_layout), Some(&mat_layout)],
            immediate_size: 0,
        });
        let cx = lightpass::Ctx {
            device: &device,
            layout: &layout,
            atlas_layout: &atlas_layout,
            shader: &shader,
        };
        let tile_cx = lightpass::Ctx {
            layout: &tile_layout,
            ..cx
        };
        let light =
            lightpass::LightPass::new(&cx, scenepass::sprite_layout(), size.width, size.height);
        let graphics = GraphicsSettings::default();
        // Körper (Autos, Personen, Schilder): Gruppe 2 = Schriftatlas (Schildtext in der Welt), Gruppe 3 = Lichtkarte
        // (Laternen spiegeln sich nachts im Lack)
        let world_font_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Schrift in der Welt"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let world_font = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Schrift in der Welt"),
            layout: &world_font_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&font_view),
            }],
        });
        let body_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Körper mit Schrift und Lichtkarte"),
            bind_group_layouts: &[
                Some(&camera_layout),
                Some(&atlas_layout),
                Some(&world_font_layout),
                Some(&atlas_layout),
            ],
            immediate_size: 0,
        });
        let body_cx = lightpass::Ctx {
            layout: &body_layout,
            ..cx
        };
        let pipes = ScenePipes::new(&cx, &tile_cx, &body_cx, graphics.msaa());
        let targets = SceneTargets::new(&device, &atlas_layout, &sampler, size, graphics.msaa());
        let map_pipeline = scenepass::pipeline(
            &tile_cx,
            "Minikarte",
            "vs",
            "fs",
            &[scenepass::mesh_layout()],
            config.format,
            1,
            None,
            true,
            wgpu::CompareFunction::LessEqual,
        );
        // Nachbearbeitung: Szenenbild in Gruppe 1, Bloom ½ in Gruppe 3 (Gruppe 2 = Bodenmaterialien bleibt frei)
        let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Nachbearbeitung mit Bloom"),
            bind_group_layouts: &[
                Some(&camera_layout),
                Some(&atlas_layout),
                None,
                Some(&atlas_layout),
            ],
            immediate_size: 0,
        });
        let bloom = scenepass::Bloom::new(&cx);
        let pixel = scenepass::PixelPass::new(&device, &queue, &cx, &camera_layout, config.format);
        let post_pipeline = scenepass::pipeline(
            &lightpass::Ctx {
                layout: &post_layout,
                ..cx
            },
            "Nachbearbeitung",
            "full_vs",
            "post_fs",
            &[],
            config.format,
            1,
            // Splitscreen: die zweite Ansicht mischt sich mit Deckkraft ein (sonst Deckkraft 1 = unverändert)
            Some(wgpu::BlendState::ALPHA_BLENDING),
            false,
            wgpu::CompareFunction::Always,
        );
        let hud_depth = depth_view(&device, size);
        let offscreen = fixed.map(|f| offscreen_view(&device, config.format, f));
        let preview = offscreen.as_ref().map(|view| {
            let cx = lightpass::Ctx {
                device: &device,
                layout: &layout,
                atlas_layout: &atlas_layout,
                shader: &shader,
            };
            (
                cx.pipeline(
                    "Fenstervorschau",
                    "full_vs",
                    "preview_fs",
                    &[],
                    config.format,
                    wgpu::BlendState::REPLACE,
                    None,
                    1,
                ),
                preview_bind(&device, &atlas_layout, &sampler, view),
            )
        });
        let gpu = if measure {
            GpuTimer::new(&device, &queue)
        } else {
            None
        };
        Ok(Self {
            window,
            instance,
            surface,
            device,
            queue,
            config,
            pipes,
            targets,
            body_layout,
            world_font,
            graphics,
            map_pipeline,
            post_pipeline,
            bloom,
            pixel_bind: None,
            pixel,
            scene_uniform,
            scene_uniform2,
            scene_bind2,
            uniform2,
            bind2,
            last_cams: Vec::new(),
            scene_bind,
            hud_depth,
            silhouettes: None,
            silhouette_count: 0,
            effects: None,
            effect_count: 0,
            bodies: None,
            body_capacity: 0,
            body_count: 0,
            uniform,
            bind,
            atlas_bind,
            vehicle_bind,
            atlas_sampler: sampler,
            tiles: BTreeMap::new(),
            size,
            fixed,
            offscreen,
            preview,
            scale,
            lighting,
            light,
            layout,
            tile_layout,
            materials,
            atlas_layout,
            shader,
            hud_pipeline,
            hud_font,
            hud: None,
            hud_capacity: 0,
            hud_count: 0,
            maps: [None; 2],
            map_uniform,
            map_bind,
            map_uniform2,
            map_bind2,
            gpu,
            acquire_ms: 0.,
            overlay_pipeline,
            overview: None,
        })
    }
    /// Gemessene GPU-Zeiten (ms) seit dem letzten Leeren; `None` = keine Zeitstempel verfügbar.
    pub fn gpu_samples(&mut self) -> Option<&mut Vec<f32>> {
        self.gpu.as_mut().map(|g| &mut g.samples)
    }
    pub fn viewport(&self) -> Vec2 {
        Vec2::new(self.size.width as f32, self.size.height as f32)
    }
    pub fn view_bounds(&self, camera: &Camera) -> Bounds {
        let half = self.viewport() * 0.5 / (camera.zoom * camera.scale);
        Bounds {
            min: camera.position - half,
            max: camera.position + half,
        }
    }
    pub fn sync(&mut self, snapshot: &Snapshot) {
        self.tiles.retain(|key, _| snapshot.tiles.contains_key(key));
        for (&key, mesh) in &snapshot.tiles {
            if self
                .tiles
                .get(&key)
                .is_some_and(|t| Arc::ptr_eq(&t.source, mesh))
            {
                continue;
            }
            let upload = |label, bytes: &[u8], usage| {
                if bytes.is_empty() {
                    None
                } else {
                    Some(
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some(label),
                                contents: bytes,
                                usage,
                            }),
                    )
                }
            };
            self.tiles.insert(
                key,
                GpuTile {
                    source: mesh.clone(),
                    bounds: snapshot.bounds[&key],
                    vertices: upload(
                        "Tile vertices",
                        bytemuck::cast_slice(&mesh.vertices),
                        wgpu::BufferUsages::VERTEX,
                    ),
                    indices: upload(
                        "Tile indices",
                        bytemuck::cast_slice(&mesh.indices),
                        wgpu::BufferUsages::INDEX,
                    ),
                    sprites: upload(
                        "Tile sprite instances",
                        bytemuck::cast_slice(&mesh.sprites),
                        wgpu::BufferUsages::VERTEX,
                    ),
                    shadows: upload(
                        "Tile shadow walls",
                        bytemuck::cast_slice::<ShadowVertex, u8>(&mesh.shadows),
                        wgpu::BufferUsages::VERTEX,
                    ),
                    shadow_indices: upload(
                        "Tile shadow indices",
                        bytemuck::cast_slice(&mesh.shadow_indices),
                        wgpu::BufferUsages::INDEX,
                    ),
                },
            );
        }
    }
    /// Bewegte Objekte des nächsten Bildes (Autos, Personen, Marker) übernehmen.
    pub fn set_bodies(&mut self, bodies: &[Body]) {
        self.body_count = bodies.len() as u32;
        if bodies.is_empty() {
            return;
        }
        if self.bodies.is_none() || self.body_capacity < bodies.len() {
            self.body_capacity = bodies.len().next_power_of_two().max(256);
            self.bodies = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Body instances"),
                size: (self.body_capacity * size_of::<Body>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = &self.bodies {
            self.queue
                .write_buffer(buffer, 0, bytemuck::cast_slice(bodies));
        }
    }
    /// Fahrzeugbilder hochladen (RGBA8, w × h).
    pub fn set_vehicle_atlas(&mut self, pixels: &[u8], w: u32, h: u32) {
        self.vehicle_bind = vehicle_atlas_bind(
            &self.device,
            &self.queue,
            &self.atlas_layout,
            &self.atlas_sampler,
            pixels,
            w,
            h,
        );
    }
    /// Umrisse verdeckter Figuren (wenige; Puffer wächst bei Bedarf).
    pub fn set_silhouettes(&mut self, bodies: &[Body]) {
        self.silhouette_count = bodies.len() as u32;
        upload_bodies(
            &self.device,
            &self.queue,
            &mut self.silhouettes,
            bodies,
            "Silhouette instances",
        );
    }
    /// Durchscheinende Effekte (eigener Durchgang ohne Tiefenschreiben, s. `effect_pipeline`).
    pub fn set_effects(&mut self, bodies: &[Body]) {
        self.effect_count = bodies.len() as u32;
        upload_bodies(
            &self.device,
            &self.queue,
            &mut self.effects,
            bodies,
            "Effect instances",
        );
    }
    /// Grafikmodus und Qualität; Pipelines und Szenenziele nur neu, wenn sich die Abtastzahl ändert.
    pub fn set_graphics(&mut self, graphics: GraphicsSettings) {
        if graphics == self.graphics {
            return;
        }
        let samples = graphics.msaa();
        let mode_changed = graphics.mode != self.graphics.mode;
        // vor dem Neubau der Pipelines merken: danach stimmt `pipes.samples` schon (Niedrig → Hoch baute sonst die
        // Ziele nicht neu und die Abtastzahlen passten nicht zusammen)
        let samples_changed = samples != self.pipes.samples;
        self.graphics = graphics;
        if samples_changed {
            let cx = self.ctx();
            let tile_cx = lightpass::Ctx {
                layout: &self.tile_layout,
                ..cx
            };
            let body_cx = lightpass::Ctx {
                layout: &self.body_layout,
                ..cx
            };
            self.pipes = ScenePipes::new(&cx, &tile_cx, &body_cx, samples);
        }
        if samples_changed || mode_changed || self.pixel_bind.is_none() {
            self.rebuild_targets();
        }
    }
    /// Größe des Szenenziels: volle Zeichengröße, im Pixel-Modus geteilt durch die Bildpunktgröße (abgerundet, der
    /// Rest wird Rand).
    fn scene_size(&self) -> PhysicalSize<u32> {
        if self.graphics.mode != GraphicsMode::Pixel {
            return self.size;
        }
        let k = graphics::pixel_factor(self.size.height);
        PhysicalSize::new((self.size.width / k).max(1), (self.size.height / k).max(1))
    }
    fn rebuild_targets(&mut self) {
        self.targets = SceneTargets::new(
            &self.device,
            &self.atlas_layout,
            &self.atlas_sampler,
            self.scene_size(),
            self.pipes.samples,
        );
        self.pixel_bind =
            (self.pipes.samples == 1).then(|| self.pixel.bind(&self.device, &self.targets.depth));
        // Schattenmaske und Lichtkarte: im HD-Pfad volle bzw. halbe Zeichengröße; im Pixel-Modus reicht die
        // doppelte bzw. einfache Größe des kleinen Szenenziels (spart ~95 % der Bildpunkte)
        let (w, h) = if self.graphics.mode == GraphicsMode::Pixel {
            let s = self.scene_size();
            (s.width * 2, s.height * 2)
        } else {
            (self.size.width, self.size.height)
        };
        let cx = lightpass::Ctx {
            device: &self.device,
            layout: &self.layout,
            atlas_layout: &self.atlas_layout,
            shader: &self.shader,
        };
        self.light.resize(&cx, w, h);
    }
    fn ctx(&self) -> lightpass::Ctx<'_> {
        lightpass::Ctx {
            device: &self.device,
            layout: &self.layout,
            atlas_layout: &self.atlas_layout,
            shader: &self.shader,
        }
    }
    pub fn set_lighting(&mut self, lighting: Lighting) {
        self.lighting = lighting;
    }
    pub fn set_lights(&mut self, lights: &[LightSource]) {
        self.light.set_lights(&self.device, &self.queue, lights);
    }
    /// HUD-Elemente des nächsten Bildes (Bildschirm-Pixel).
    /// Stadtplan einmalig hochladen (große Karte).
    pub fn set_overview(&mut self, mesh: &OverlayMesh) {
        if mesh.indices.is_empty() {
            return;
        }
        let vertices = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Stadtplan"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let indices = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Stadtplan-Indizes"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        self.overview = Some((vertices, indices, mesh.indices.len() as u32));
    }
    pub fn set_hud(&mut self, items: &[HudItem], map: Option<MapInset>, map2: Option<MapInset>) {
        self.hud_count = items.len() as u32;
        let ok =
            |m: Option<MapInset>| m.filter(|m| m.rect[2] >= 4. && m.rect[3] >= 4. && m.span > 0.);
        self.maps = [ok(map), ok(map2)];
        for (k, m) in self.maps.into_iter().enumerate() {
            let Some(m) = m else { continue };
            // eigene Kamera: Mitte, Maßstab Pixel je Welt-px, Ausschnittgröße; params.y = schematisch
            let mut u = vec![
                m.center[0],
                m.center[1],
                m.rect[2] / m.span,
                0.,
                m.rect[2],
                m.rect[3],
                0.,
                0.,
            ];
            u.extend([0.3, -0.5, 0.8, 0.]);
            u.extend([self.scale, 1., m.px, if m.detail { 1. } else { 0. }]);
            u.extend([0.; 12]);
            let buf = if k == 0 {
                &self.map_uniform
            } else {
                &self.map_uniform2
            };
            self.queue.write_buffer(buf, 0, bytemuck::cast_slice(&u));
        }
        if items.is_empty() {
            return;
        }
        if self.hud.is_none() || self.hud_capacity < items.len() {
            self.hud_capacity = items.len().next_power_of_two().max(1024);
            self.hud = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("HUD"),
                size: (self.hud_capacity * size_of::<HudItem>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(b) = &self.hud {
            self.queue.write_buffer(b, 0, bytemuck::cast_slice(items));
        }
    }
    pub fn drawable(&self) -> bool {
        self.size.width > 0 && self.size.height > 0
    }
    /// Fenstergröße geändert (bei fester Zeichengröße ändert sich nur die Fensterfläche).
    pub fn resize(&mut self, window: PhysicalSize<u32>) {
        if window.width > 0 && window.height > 0 {
            self.config.width = window.width;
            self.config.height = window.height;
            self.surface.configure(&self.device, &self.config);
        }
        let size = self.fixed.unwrap_or(window);
        self.size = size;
        if self.drawable() {
            self.hud_depth = depth_view(&self.device, size);
            self.rebuild_targets();
            if let Some(f) = self.fixed {
                let view = offscreen_view(&self.device, self.config.format, f);
                if let Some((_, bind)) = &mut self.preview {
                    *bind =
                        preview_bind(&self.device, &self.atlas_layout, &self.atlas_sampler, &view);
                }
                self.offscreen = Some(view);
            }
        }
    }
    fn draw_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        cams: &[Camera],
        timestamps: Option<u32>,
    ) {
        // Messung: Anfang im ersten Durchgang des Bildes, Ende im HUD-Durchgang (dem letzten)
        let stamp = |begin: bool| {
            let (set, q) = (self.gpu.as_ref()?, timestamps?);
            Some(wgpu::RenderPassTimestampWrites {
                query_set: &set.set,
                beginning_of_pass_write_index: begin.then_some(q),
                end_of_pass_write_index: (!begin).then_some(q + 1),
            })
        };
        // je Ansicht (Splitscreen: zwei) dieselben Durchgänge in dieselben Ziele; die zweite legt sich in der
        // Nachbearbeitung nur auf ihre Bildhälfte
        for (k, camera) in cams.iter().enumerate() {
            let (first, last) = (k == 0, k + 1 == cams.len());
            let (sb, b) = if first {
                (&self.scene_bind, &self.bind)
            } else {
                (&self.scene_bind2, &self.bind2)
            };
            let begin = || if first { stamp(true) } else { None };
            let visible = self.view_bounds(camera).expand(512.);
            let l = self.lighting;
            let shadows = l.shadow_strength > 0.02;
            let night = l.dark > 0.;
            // 1) Schattenmaske: auch Häuser außerhalb des Bildes können hineinwerfen (bis 900 px)
            if shadows {
                let casters = visible.expand(900.);
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Schattenmaske"),
                    timestamp_writes: begin(),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.light.mask.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_bind_group(0, sb, &[]);
                pass.set_bind_group(1, &self.atlas_bind, &[]);
                pass.set_pipeline(&self.light.shadow);
                for tile in self.tiles.values().filter(|t| t.bounds.intersects(casters)) {
                    if let (Some(v), Some(i)) = (&tile.shadows, &tile.shadow_indices) {
                        pass.set_vertex_buffer(0, v.slice(..));
                        pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..tile.source.shadow_indices.len() as u32, 0, 0..1);
                    }
                }
                pass.set_pipeline(&self.light.tree_shadow);
                for tile in self.tiles.values().filter(|t| t.bounds.intersects(visible)) {
                    if let Some(sprites) = &tile.sprites {
                        pass.set_vertex_buffer(0, sprites.slice(..));
                        pass.draw(0..6, 0..tile.source.sprites.len() as u32);
                    }
                }
            }
            // 2) Lichtkarte: Umgebungslicht plus Lichtquellen
            if night {
                let lin = |c: f32| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) } as f64;
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Lichtkarte"),
                    timestamp_writes: if shadows { None } else { begin() },
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.light.lightmap.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: lin(l.ambient[0]),
                                g: lin(l.ambient[1]),
                                b: lin(l.ambient[2]),
                                a: 1.,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                if let Some(lights) = self
                    .light
                    .lights
                    .as_ref()
                    .filter(|_| self.light.light_count > 0)
                {
                    pass.set_bind_group(0, sb, &[]);
                    pass.set_bind_group(1, &self.atlas_bind, &[]);
                    pass.set_pipeline(&self.light.light);
                    pass.set_vertex_buffer(0, lights.slice(..));
                    pass.draw(0..6, 0..self.light.light_count);
                }
            }
            let pixel_post = self.graphics.mode == GraphicsMode::Pixel && self.pixel_bind.is_some();
            // 3) Bild: Karte, Schatten auf den Boden, Bäume/Decals, bewegte Objekte, dann das Licht
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Berlin frame"),
                timestamp_writes: if shadows || night { None } else { begin() },
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.color,
                    depth_slice: None,
                    resolve_target: self.targets.resolve.as_ref(),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.14,
                            g: 0.20,
                            b: 0.12,
                            a: 1.,
                        }),
                        // mit Kantenglättung zählt nur das aufgelöste Bild (auf Kachel-GPUs bleibt die Abtastung so im
                        // Kachelspeicher)
                        store: if self.targets.resolve.is_some() {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        // der Pixel-Modus liest die Tiefe für die Konturen
                        store: if pixel_post {
                            wgpu::StoreOp::Store
                        } else {
                            wgpu::StoreOp::Discard
                        },
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, sb, &[]);
            pass.set_bind_group(1, &self.atlas_bind, &[]);
            pass.set_bind_group(2, &self.materials, &[]);
            pass.set_pipeline(&self.pipes.tiles);
            for tile in self.tiles.values().filter(|t| t.bounds.intersects(visible)) {
                if let (Some(vertices), Some(indices)) = (&tile.vertices, &tile.indices) {
                    pass.set_vertex_buffer(0, vertices.slice(..));
                    pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..tile.source.indices.len() as u32, 0, 0..1);
                }
            }
            // Hintergrundboden: nur wo keine Fläche liegt (Tiefe noch leer)
            pass.set_pipeline(&self.pipes.ground);
            pass.draw(0..3, 0..1);
            if shadows {
                pass.set_pipeline(&self.pipes.comp.shadow);
                pass.set_bind_group(1, &self.light.mask.bind, &[]);
                pass.draw(0..3, 0..1);
                pass.set_bind_group(1, &self.atlas_bind, &[]);
            }
            pass.set_pipeline(&self.pipes.sprites);
            for tile in self.tiles.values().filter(|t| t.bounds.intersects(visible)) {
                if let Some(sprites) = &tile.sprites {
                    pass.set_vertex_buffer(0, sprites.slice(..));
                    pass.draw(0..6, 0..tile.source.sprites.len() as u32);
                }
            }
            // Körper: Schriftatlas (Gruppe 2), Lichtkarte (Gruppe 3; ohne Nacht ungenutzt, der Shader fragt die Dunkelheit)
            let body_groups = |pass: &mut wgpu::RenderPass| {
                pass.set_bind_group(1, &self.vehicle_bind, &[]);
                pass.set_bind_group(2, &self.world_font, &[]);
                pass.set_bind_group(3, &self.light.lightmap.bind, &[]);
            };
            if let Some(bodies) = self.bodies.as_ref().filter(|_| self.body_count > 0) {
                body_groups(&mut pass);
                pass.set_pipeline(&self.pipes.bodies);
                pass.set_vertex_buffer(0, bodies.slice(..));
                pass.draw(0..6, 0..self.body_count);
            }
            if night {
                pass.set_pipeline(&self.pipes.comp.light);
                pass.set_bind_group(1, &self.light.lightmap.bind, &[]);
                pass.draw(0..3, 0..1);
                pass.set_pipeline(&self.pipes.comp.ambient);
                pass.draw(0..3, 0..1);
                if self.lighting.dark > 0.3 && self.graphics.post_level() == 0 {
                    pass.set_pipeline(&self.pipes.comp.bloom);
                    pass.draw(0..3, 0..1);
                }
            }
            let m = self.lighting.minutes.rem_euclid(1440.);
            if self.lighting.windows > 0.001 || !(330. ..=1380.).contains(&m) {
                pass.set_pipeline(&self.pipes.windows);
                pass.set_bind_group(1, &self.atlas_bind, &[]);
                for tile in self.tiles.values().filter(|t| t.bounds.intersects(visible)) {
                    if let (Some(vertices), Some(indices)) = (&tile.vertices, &tile.indices) {
                        pass.set_vertex_buffer(0, vertices.slice(..));
                        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..tile.source.indices.len() as u32, 0, 0..1);
                    }
                }
            }
            if let Some(buffer) = self
                .silhouettes
                .as_ref()
                .filter(|_| self.silhouette_count > 0)
            {
                body_groups(&mut pass);
                pass.set_pipeline(&self.pipes.silhouettes);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..self.silhouette_count);
            }
            if let Some(buffer) = self.effects.as_ref().filter(|_| self.effect_count > 0) {
                body_groups(&mut pass);
                pass.set_pipeline(&self.pipes.effects);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..self.effect_count);
            }
            drop(pass);
            // 3b) Bloom aus dem HDR-Bild (ab Mittel): Schwelle → ½, bei Hoch zusätzlich ½ → ¼ → zurück auf ½
            let level = self.graphics.post_level();
            if level >= 1 {
                let mut step =
                    |label, view: &wgpu::TextureView, src: &wgpu::BindGroup, pipe, load| {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some(label),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            ..Default::default()
                        });
                        pass.set_bind_group(0, sb, &[]);
                        pass.set_bind_group(1, src, &[]);
                        pass.set_pipeline(pipe);
                        pass.draw(0..3, 0..1);
                    };
                let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
                let t = &self.targets;
                step(
                    "Bloom Schwelle",
                    &t.half,
                    &t.post,
                    &self.bloom.prefilter,
                    clear,
                );
                if level >= 2 {
                    step("Bloom ¼", &t.quarter, &t.half_bind, &self.bloom.down, clear);
                    step(
                        "Bloom ¼ → ½",
                        &t.half,
                        &t.quarter_bind,
                        &self.bloom.up,
                        wgpu::LoadOp::Load,
                    );
                }
            }
            // 3c) Pixel-Modus: Palette je kleinem Bildpunkt
            if let Some(bind) = self.pixel_bind.as_ref().filter(|_| pixel_post) {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Pixel: Palette"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.targets.pix,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_bind_group(0, b, &[]);
                pass.set_bind_group(1, &self.targets.post, &[]);
                pass.set_bind_group(3, bind, &[]);
                pass.set_pipeline(&self.pixel.quant);
                pass.draw(0..3, 0..1);
            }
            // 4) HUD über allem (ohne Tiefentest), dazwischen die Minikarte in ihrem Rechteck
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Nachbearbeitung und HUD"),
                timestamp_writes: if last { stamp(false) } else { None },
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // zweite Ansicht: über die erste, nur auf ihrer Seite der Trennlinie
                        load: if first {
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.hud_depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            // 4a) Szenenbild ins Ausgabebild (Farbabstimmung, Vignette)
            pass.set_bind_group(0, b, &[]);
            match self.pixel_bind.as_ref().filter(|_| pixel_post) {
                Some(bind) => {
                    pass.set_bind_group(1, &self.targets.pix_bind, &[]);
                    pass.set_bind_group(3, bind, &[]);
                    pass.set_pipeline(&self.pixel.pipeline);
                }
                None => {
                    pass.set_bind_group(1, &self.targets.post, &[]);
                    pass.set_bind_group(3, &self.targets.half_bind, &[]);
                    pass.set_pipeline(&self.post_pipeline);
                }
            }
            pass.draw(0..3, 0..1);
            if !last {
                continue;
            }
            let Some(hud) = self.hud.as_ref().filter(|_| self.hud_count > 0) else {
                return;
            };
            // HUD in Abschnitten, dazwischen die Minikarten (in der Reihenfolge, in der sie entstanden)
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_pipeline(&self.hud_pipeline);
            pass.set_bind_group(1, &self.hud_font, &[]);
            pass.set_vertex_buffer(0, hud.slice(..));
            let mut from = 0;
            for (k, m) in self.maps.iter().enumerate() {
                let Some(m) = *m else { continue };
                let split = m.split.clamp(from, self.hud_count);
                pass.draw(0..6, from..split);
                from = split;
                let (w, h) = (self.size.width as f32, self.size.height as f32);
                let x0 = m.rect[0].clamp(0., w);
                let y0 = m.rect[1].clamp(0., h);
                let x1 = (m.rect[0] + m.rect[2]).clamp(0., w);
                let y1 = (m.rect[1] + m.rect[3]).clamp(0., h);
                if x1 - x0 < 1. || y1 - y0 < 1. {
                    continue;
                }
                let kk = m.span / m.rect[2] / 2.;
                let (c, r) = (Vec2::from(m.center), Vec2::new(m.rect[2], m.rect[3]) * kk);
                let area = Bounds {
                    min: c - r,
                    max: c + r,
                }
                .expand(256.);
                pass.set_viewport(m.rect[0], m.rect[1], m.rect[2], m.rect[3], 0., 1.);
                pass.set_scissor_rect(x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32);
                let mb = if k == 0 {
                    &self.map_bind
                } else {
                    &self.map_bind2
                };
                pass.set_bind_group(0, mb, &[]);
                pass.set_bind_group(1, &self.atlas_bind, &[]);
                if m.overview {
                    if let Some((v, i, n)) = &self.overview {
                        pass.set_pipeline(&self.overlay_pipeline);
                        pass.set_vertex_buffer(0, v.slice(..));
                        pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..*n, 0, 0..1);
                    }
                } else {
                    pass.set_bind_group(2, &self.materials, &[]);
                    pass.set_pipeline(&self.map_pipeline);
                    for tile in self.tiles.values().filter(|t| t.bounds.intersects(area)) {
                        if let (Some(v), Some(i)) = (&tile.vertices, &tile.indices) {
                            pass.set_vertex_buffer(0, v.slice(..));
                            pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                            pass.draw_indexed(0..tile.source.indices.len() as u32, 0, 0..1);
                        }
                    }
                }
                pass.set_viewport(0., 0., w, h, 0., 1.);
                pass.set_scissor_rect(0, 0, self.size.width, self.size.height);
                pass.set_bind_group(0, &self.bind, &[]);
                pass.set_pipeline(&self.hud_pipeline);
                pass.set_bind_group(1, &self.hud_font, &[]);
                pass.set_vertex_buffer(0, hud.slice(..));
            }
            pass.draw(0..6, from..self.hud_count);
        }
    }
    /// Read back our own render target for repeatable visual QA, independent of desktop capture.
    pub fn capture(&self, camera: &Camera, path: &std::path::Path) -> Result<()> {
        anyhow::ensure!(self.drawable(), "Fenster hat keine renderbare Größe");
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("QA render target"),
            size: wgpu::Extent3d {
                width: self.size.width,
                height: self.size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let row = (self.size.width * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("QA readback"),
            size: row as u64 * self.size.height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        // Splitscreen: beide Ansichten des letzten Bildes (deren Uniforms stehen noch)
        let cams = if self.last_cams.len() == 2 {
            self.last_cams.clone()
        } else {
            vec![camera.clone()]
        };
        self.draw_scene(
            &mut encoder,
            &texture.create_view(&Default::default()),
            &cams,
            None,
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(self.size.height),
                },
            },
            texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })?;
        receiver.recv_timeout(std::time::Duration::from_secs(10))??;
        let mapped = buffer.slice(..).get_mapped_range();
        let mut rgba = Vec::with_capacity((self.size.width * self.size.height * 4) as usize);
        for line in mapped.chunks(row as usize) {
            rgba.extend_from_slice(&line[..self.size.width as usize * 4]);
        }
        if matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut encoder = png::Encoder::new(
            std::io::BufWriter::new(std::fs::File::create(path)?),
            self.size.width,
            self.size.height,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&rgba)?;
        drop(mapped);
        buffer.unmap();
        eprintln!("GPU-Aufnahme: {}", path.display());
        Ok(())
    }
    /// Kamera-Uniforms einer Ansicht schreiben: volle Größe (Nachbearbeitung, HUD) und Szene (im Pixel-Modus klein).
    fn write_camera(
        &self,
        camera: &Camera,
        split: [f32; 4],
        full: &wgpu::Buffer,
        scene: &wgpu::Buffer,
    ) {
        let l = self.lighting;
        let mut uniform = camera.uniform(self.viewport()).to_vec();
        // freier Platz hinter `scale`: Nebel für das Fensterlicht
        uniform[3] = l.fog;
        // freier Platz hinter dem Bildausschnitt: Nässe der Straßen (Glanz der Bodenmaterialien)
        uniform[6] = l.wet;
        // dahinter: Stufe der Nachbearbeitung (HDR-Bloom, weiche Schatten), GraphicsSettings::post_level
        // Pixel-Modus: −1 (Szenen-Shader dämpfen Texturdetail, `surface_detail`); die HD-Abfragen (≥ 1) bleiben aus
        uniform[7] = if self.graphics.mode == GraphicsMode::Pixel {
            -1.
        } else {
            self.graphics.post_level() as f32
        };
        uniform.extend([l.sun[0], l.sun[1], l.sun[2].max(0.05), l.minutes]);
        uniform.extend([self.scale, 0., l.windows, l.warmth]);
        uniform.extend([l.shadow[0], l.shadow[1], l.shadow_len, l.shadow_strength]);
        uniform.extend([l.ambient[0], l.ambient[1], l.ambient[2], l.dark]);
        uniform.extend(split);
        self.queue
            .write_buffer(full, 0, bytemuck::cast_slice(&uniform));
        // Szene: im Pixel-Modus kleines Ziel, Maßstab / k, Mitte auf das Bildpunktraster gerastet (sonst flimmern
        // Kanten beim Fahren um einen Bildpunkt hin und her)
        if self.graphics.mode == GraphicsMode::Pixel {
            let small = self.scene_size();
            let k = self.size.width as f32 / small.width as f32;
            let k = k
                .min(self.size.height as f32 / small.height as f32)
                .floor()
                .max(1.);
            uniform[2] /= k;
            let s = uniform[2];
            uniform[0] = (uniform[0] * s).round() / s;
            uniform[1] = (uniform[1] * s).round() / s;
            uniform[4] = small.width as f32;
            uniform[5] = small.height as f32;
        }
        self.queue
            .write_buffer(scene, 0, bytemuck::cast_slice(&uniform));
    }
    /// Ein Bild mit einer oder zwei Ansichten (Splitscreen).
    pub fn render_views(&mut self, views: &crate::split::Views) -> Result<bool> {
        if !self.drawable() {
            return Ok(false);
        }
        let t0 = std::time::Instant::now();
        // Mit fester Zeichengröße entsteht das Bild abseits des Fensters: ein verdecktes oder minimiertes Fenster
        // (kein Swapchain-Bild) hält Messung und Aufnahme dann nicht auf.
        let fixed = self.offscreen.is_some();
        let (frame, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (Some(frame), false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (Some(frame), true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                (None, false)
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.window.inner_size());
                (None, false)
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(self.window.clone())?;
                self.resize(self.window.inner_size());
                (None, false)
            }
            wgpu::CurrentSurfaceTexture::Validation => anyhow::bail!("Surface-Validierungsfehler"),
        };
        if frame.is_none() && !fixed {
            return Ok(false);
        }
        self.acquire_ms = t0.elapsed().as_secs_f32() * 1000.;
        let cams: Vec<Camera> = views.cams[..views.count.clamp(1, 2)].to_vec();
        for (k, cam) in cams.iter().enumerate() {
            // Splitscreen für die Nachbearbeitung: Normale, Deckkraft der Linie, Seite (0 = ganzes Bild)
            let side = if views.count == 2 { k as f32 + 1. } else { 0. };
            let split = [views.normal.x, views.normal.y, views.line, side];
            let (u, su) = if k == 0 {
                (&self.uniform, &self.scene_uniform)
            } else {
                (&self.uniform2, &self.scene_uniform2)
            };
            self.write_camera(cam, split, u, su);
        }
        self.last_cams.clone_from(&cams);
        let view = frame
            .as_ref()
            .map(|f| f.texture.create_view(&Default::default()));
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let timestamps = self.gpu.as_mut().and_then(GpuTimer::begin);
        match (&self.offscreen, &view) {
            (Some(target), view) => {
                // feste Zeichengröße: Bild abseits, im Fenster eine verkleinerte Vorschau (Seitenverhältnis bleibt)
                self.draw_scene(&mut encoder, target, &cams, timestamps);
                if let Some(view) = view {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Fenster (feste Zeichengröße)"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..Default::default()
                    });
                    if let Some((pipeline, bind)) = &self.preview {
                        let (ww, wh) = (self.config.width as f32, self.config.height as f32);
                        let (iw, ih) = (self.size.width as f32, self.size.height as f32);
                        let k = (ww / iw).min(wh / ih);
                        let (vw, vh) = (iw * k, ih * k);
                        pass.set_viewport((ww - vw) * 0.5, (wh - vh) * 0.5, vw, vh, 0., 1.);
                        pass.set_bind_group(0, &self.bind, &[]);
                        pass.set_bind_group(1, bind, &[]);
                        pass.set_pipeline(pipeline);
                        pass.draw(0..3, 0..1);
                    }
                }
            }
            (None, Some(view)) => self.draw_scene(&mut encoder, view, &cams, timestamps),
            (None, None) => unreachable!("ohne Swapchain-Bild oben schon beendet"),
        }
        if let Some(g) = &self.gpu {
            g.resolve(&mut encoder);
        }
        self.queue.submit([encoder.finish()]);
        if let Some(g) = &mut self.gpu {
            g.after_submit(&self.device);
        }
        if let Some(frame) = frame {
            self.window.pre_present_notify();
            frame.present();
        }
        if suboptimal {
            self.resize(self.window.inner_size());
        }
        Ok(true)
    }
}
/// WGSL aller Durchgänge; davor die Konstanten aus Rust (Atlasraster), damit Shader und CPU nie auseinanderlaufen.
pub(crate) fn shader_source() -> String {
    [
        atlas::shader_constants().as_str(),
        crate::facade::shader_constants().as_str(),
        crate::vehatlas::shader_constants().as_str(),
        crate::palette::shader_constants().as_str(),
        hud::shader_constants().as_str(),
        include_str!("scene.wgsl"),
        include_str!("lighting.wgsl"),
        include_str!("hud.wgsl"),
        include_str!("overlay.wgsl"),
    ]
    .concat()
}
/// Kamera (32 B) + Sonne + Parameter + Schatten + Umgebungslicht + Splitscreen (je 16 B).
const UNIFORM_BYTES: u64 = 112;
fn offscreen_view(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    size: PhysicalSize<u32>,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Feste Zeichengröße"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
fn preview_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Fenstervorschau"),
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
}
fn depth_view(device: &wgpu::Device, size: PhysicalSize<u32>) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("2D layer / building depth"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// Fahrzeug-Atlas mit Mip-Stufen (Kastenfilter auf der CPU): die Bilder sind bis 256 px breit und werden stark
/// verkleinert gezeichnet, ohne Mips flimmerten Türfugen und Leuchten.
fn vehicle_atlas_bind(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    pixels: &[u8],
    w: u32,
    h: u32,
) -> wgpu::BindGroup {
    let levels = if w >= 64 && h >= 64 { 4 } else { 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Vehicle atlas"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let (mut cur, mut cw, mut ch) = (pixels.to_vec(), w, h);
    for level in 0..levels {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &cur,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(cw * 4),
                rows_per_image: Some(ch),
            },
            wgpu::Extent3d {
                width: cw,
                height: ch,
                depth_or_array_layers: 1,
            },
        );
        if level + 1 == levels {
            break;
        }
        // nächste Stufe: 2×2 mitteln, Farbe nach Deckkraft gewichtet (sonst färben leere Ränder dunkel)
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
        (cur, cw, ch) = (next, nw, nh);
    }
    let view = texture.create_view(&Default::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Vehicle atlas"),
        layout,
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
    })
}

/// Instanzpuffer füllen, bei Bedarf vergrößern (Zweierpotenz, mindestens 1 KiB).
fn upload_bodies(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    slot: &mut Option<wgpu::Buffer>,
    bodies: &[Body],
    label: &'static str,
) {
    if bodies.is_empty() {
        return;
    }
    let need = std::mem::size_of_val(bodies) as u64;
    if slot.as_ref().is_none_or(|b| b.size() < need) {
        *slot = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: need.next_power_of_two().max(1024),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    if let Some(buffer) = slot {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(bodies));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pixel_mode_calms_surfaces_and_gates_dither() {
        let src = super::shader_source();
        // Texturdetail, Relief, Verdeckung, Schmutz und Moos hängen am Pixel-Faktor; HD bleibt bei 1
        assert!(src.contains("select(1.0, PIXEL_DETAIL, camera.padding2.y < -0.5)"));
        assert!(
            src.matches("surface_detail()").count() >= 4,
            "finish_sample, Schmutz, Ausbleichen, Moos"
        );
        // Streuung nur in echten Verläufen, Kontur in Eigenfarbe
        assert!(src.contains("* gradient;") && src.contains("if edge { s = s * 0.22; }"));
    }
    #[test]
    fn atlas_grid_comes_from_rust_not_from_literals() {
        let src = super::shader_source();
        assert!(src.starts_with(&crate::atlas::shader_constants()));
        for bad in ["64.0", "vec2(4.0,3.0)", "vec2(4.0, 3.0)", "cell % 4.0"] {
            assert!(!src.contains(bad), "fest verdrahtetes Atlasraster: {bad}");
        }
        assert!(src.contains("fn atlas_uv"));
    }
}
