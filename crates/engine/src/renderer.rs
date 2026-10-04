use crate::{
    Body, LightSource, Lighting, atlas,
    camera::Camera,
    hud::{self, HudItem, MapInset},
    lightpass,
};
use anyhow::{Context, Result};
use berlin_map_loader::{
    format::TileKey,
    geom::Bounds,
    mesh::{Mesh, ShadowVertex, Sprite, Vertex},
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
    pipeline: wgpu::RenderPipeline,
    window_pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    body_pipeline: wgpu::RenderPipeline,
    bodies: Option<wgpu::Buffer>,
    body_capacity: usize,
    body_count: u32,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    atlas_bind: wgpu::BindGroup,
    depth: wgpu::TextureView,
    tiles: BTreeMap<TileKey, GpuTile>,
    size: PhysicalSize<u32>,
    scale: f32,
    lighting: Lighting,
    light: lightpass::LightPass,
    layout: wgpu::PipelineLayout,
    atlas_layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    hud_pipeline: wgpu::RenderPipeline,
    hud_font: wgpu::BindGroup,
    hud: Option<wgpu::Buffer>,
    hud_capacity: usize,
    hud_count: u32,
    map: Option<MapInset>,
    map_uniform: wgpu::Buffer,
    map_bind: wgpu::BindGroup,
    overlay_pipeline: wgpu::RenderPipeline,
    overview: Option<(wgpu::Buffer, wgpu::Buffer, u32)>,
}
impl Renderer {
    pub async fn new(window: Arc<Window>, scale: f32, lighting: Lighting) -> Result<Self> {
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
                ..Default::default()
            })
            .await?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
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
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("scene.wgsl"),
                    include_str!("lighting.wgsl"),
                    include_str!("hud.wgsl"),
                    include_str!("overlay.wgsl")
                )
                .into(),
            ),
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
        let attrs = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3,3=>Float32x2,4=>Float32x2,5=>Float32,6=>Float32];
        let sprite_attrs = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32,5=>Float32];
        let body_attrs = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32,3=>Float32,4=>Float32,5=>Float32x4];
        let depth_state = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        };
        let make_pipeline = |label,
                             vs,
                             fs,
                             stride,
                             step_mode,
                             attributes: &[wgpu::VertexAttribute],
                             blend,
                             write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: stride,
                        step_mode,
                        attributes,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    depth_write_enabled: Some(write),
                    ..depth_state.clone()
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline = make_pipeline(
            "Berlin indexed meshes",
            "vs",
            "fs",
            size_of::<Vertex>() as u64,
            wgpu::VertexStepMode::Vertex,
            &attrs,
            None,
            true,
        );
        // erleuchtete Fenster: dieselben Fassaden nach dem Licht, ohne Tiefe zu schreiben
        let window_pipeline = make_pipeline(
            "Berlin lit windows",
            "vs",
            "window_fs",
            size_of::<Vertex>() as u64,
            wgpu::VertexStepMode::Vertex,
            &attrs,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            false,
        );
        let sprite_pipeline = make_pipeline(
            "Berlin instanced atlas",
            "sprite_vs",
            "sprite_fs",
            size_of::<Sprite>() as u64,
            wgpu::VertexStepMode::Instance,
            &sprite_attrs,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            true,
        );
        let body_pipeline = make_pipeline(
            "Berlin instanced bodies",
            "body_vs",
            "body_fs",
            size_of::<Body>() as u64,
            wgpu::VertexStepMode::Instance,
            &body_attrs,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            true,
        );
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
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas::pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas::WIDTH * 4),
                rows_per_image: Some(atlas::HEIGHT),
            },
            texture.size(),
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
        let depth = depth_view(&device, &config);
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
        let font = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HUD-Schrift"),
            size: wgpu::Extent3d {
                width: hud::ATLAS,
                height: hud::ATLAS,
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
            &hud::atlas(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(hud::ATLAS),
                rows_per_image: Some(hud::ATLAS),
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
        let light = lightpass::LightPass::new(
            &lightpass::Ctx {
                device: &device,
                layout: &layout,
                atlas_layout: &atlas_layout,
                shader: &shader,
                surface: config.format,
            },
            wgpu::VertexBufferLayout {
                array_stride: size_of::<Sprite>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &sprite_attrs,
            },
            config.width,
            config.height,
        );
        Ok(Self {
            window,
            instance,
            surface,
            device,
            queue,
            config,
            pipeline,
            window_pipeline,
            sprite_pipeline,
            body_pipeline,
            bodies: None,
            body_capacity: 0,
            body_count: 0,
            uniform,
            bind,
            atlas_bind,
            depth,
            tiles: BTreeMap::new(),
            size,
            scale,
            lighting,
            light,
            layout,
            atlas_layout,
            shader,
            hud_pipeline,
            hud_font,
            hud: None,
            hud_capacity: 0,
            hud_count: 0,
            map: None,
            map_uniform,
            map_bind,
            overlay_pipeline,
            overview: None,
        })
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
    pub fn set_hud(&mut self, items: &[HudItem], map: Option<MapInset>) {
        self.hud_count = items.len() as u32;
        self.map = map.filter(|m| m.rect[2] >= 4. && m.rect[3] >= 4. && m.span > 0.);
        if let Some(m) = self.map {
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
            u.extend([0.; 8]);
            self.queue
                .write_buffer(&self.map_uniform, 0, bytemuck::cast_slice(&u));
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
    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = size;
        if self.drawable() {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth = depth_view(&self.device, &self.config);
            let cx = lightpass::Ctx {
                device: &self.device,
                layout: &self.layout,
                atlas_layout: &self.atlas_layout,
                shader: &self.shader,
                surface: self.config.format,
            };
            self.light.resize(&cx, size.width, size.height);
        }
    }
    fn draw_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        camera: &Camera,
    ) {
        let visible = self.view_bounds(camera).expand(512.);
        let l = self.lighting;
        let shadows = l.shadow_strength > 0.02;
        let night = l.dark > 0.;
        // 1) Schattenmaske: auch Häuser außerhalb des Bildes können hineinwerfen (bis 900 px)
        if shadows {
            let casters = visible.expand(900.);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Schattenmaske"),
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
            pass.set_bind_group(0, &self.bind, &[]);
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
                pass.set_bind_group(0, &self.bind, &[]);
                pass.set_bind_group(1, &self.atlas_bind, &[]);
                pass.set_pipeline(&self.light.light);
                pass.set_vertex_buffer(0, lights.slice(..));
                pass.draw(0..6, 0..self.light.light_count);
            }
        }
        // 3) Bild: Karte, Schatten auf den Boden, Bäume/Decals, bewegte Objekte, dann das Licht
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Berlin frame"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.14,
                        g: 0.20,
                        b: 0.12,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_bind_group(1, &self.atlas_bind, &[]);
        pass.set_pipeline(&self.pipeline);
        for tile in self.tiles.values().filter(|t| t.bounds.intersects(visible)) {
            if let (Some(vertices), Some(indices)) = (&tile.vertices, &tile.indices) {
                pass.set_vertex_buffer(0, vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..tile.source.indices.len() as u32, 0, 0..1);
            }
        }
        if shadows {
            pass.set_pipeline(&self.light.shadow_composite);
            pass.set_bind_group(1, &self.light.mask.bind, &[]);
            pass.draw(0..3, 0..1);
            pass.set_bind_group(1, &self.atlas_bind, &[]);
        }
        pass.set_pipeline(&self.sprite_pipeline);
        for tile in self.tiles.values().filter(|t| t.bounds.intersects(visible)) {
            if let Some(sprites) = &tile.sprites {
                pass.set_vertex_buffer(0, sprites.slice(..));
                pass.draw(0..6, 0..tile.source.sprites.len() as u32);
            }
        }
        if let Some(bodies) = self.bodies.as_ref().filter(|_| self.body_count > 0) {
            pass.set_pipeline(&self.body_pipeline);
            pass.set_vertex_buffer(0, bodies.slice(..));
            pass.draw(0..6, 0..self.body_count);
        }
        if night {
            pass.set_pipeline(&self.light.light_composite);
            pass.set_bind_group(1, &self.light.lightmap.bind, &[]);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.light.ambient_composite);
            pass.draw(0..3, 0..1);
        }
        let m = self.lighting.minutes.rem_euclid(1440.);
        if self.lighting.windows > 0.001 || !(330. ..=1380.).contains(&m) {
            pass.set_pipeline(&self.window_pipeline);
            pass.set_bind_group(1, &self.atlas_bind, &[]);
            for tile in self.tiles.values().filter(|t| t.bounds.intersects(visible)) {
                if let (Some(vertices), Some(indices)) = (&tile.vertices, &tile.indices) {
                    pass.set_vertex_buffer(0, vertices.slice(..));
                    pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..tile.source.indices.len() as u32, 0, 0..1);
                }
            }
        }
        pass.set_pipeline(&self.light.grade);
        pass.draw(0..3, 0..1);
        drop(pass);
        // 4) HUD über allem (ohne Tiefentest), dazwischen die Minikarte in ihrem Rechteck
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("HUD"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        let Some(hud) = self.hud.as_ref().filter(|_| self.hud_count > 0) else {
            return;
        };
        let split = self
            .map
            .map_or(self.hud_count, |m| m.split.min(self.hud_count));
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_pipeline(&self.hud_pipeline);
        pass.set_bind_group(1, &self.hud_font, &[]);
        pass.set_vertex_buffer(0, hud.slice(..));
        pass.draw(0..6, 0..split);
        if let Some(m) = self.map {
            let (w, h) = (self.size.width as f32, self.size.height as f32);
            let x0 = m.rect[0].clamp(0., w);
            let y0 = m.rect[1].clamp(0., h);
            let x1 = (m.rect[0] + m.rect[2]).clamp(0., w);
            let y1 = (m.rect[1] + m.rect[3]).clamp(0., h);
            if x1 - x0 >= 1. && y1 - y0 >= 1. {
                let k = m.span / m.rect[2] / 2.;
                let (c, r) = (Vec2::from(m.center), Vec2::new(m.rect[2], m.rect[3]) * k);
                let area = Bounds {
                    min: c - r,
                    max: c + r,
                }
                .expand(256.);
                pass.set_viewport(m.rect[0], m.rect[1], m.rect[2], m.rect[3], 0., 1.);
                pass.set_scissor_rect(x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32);
                pass.set_bind_group(0, &self.map_bind, &[]);
                pass.set_bind_group(1, &self.atlas_bind, &[]);
                if m.overview {
                    if let Some((v, i, n)) = &self.overview {
                        pass.set_pipeline(&self.overlay_pipeline);
                        pass.set_vertex_buffer(0, v.slice(..));
                        pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..*n, 0, 0..1);
                    }
                } else {
                    pass.set_pipeline(&self.pipeline);
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
                pass.draw(0..6, split..self.hud_count);
            }
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
        self.draw_scene(
            &mut encoder,
            &texture.create_view(&Default::default()),
            camera,
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
    pub fn render(&mut self, camera: &Camera) -> Result<bool> {
        if !self.drawable() {
            return Ok(false);
        }
        let (frame, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.size);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(self.window.clone())?;
                self.resize(self.size);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => anyhow::bail!("Surface-Validierungsfehler"),
        };
        let l = self.lighting;
        let mut uniform = camera.uniform(self.viewport()).to_vec();
        uniform.extend([l.sun[0], l.sun[1], l.sun[2].max(0.05), l.minutes]);
        uniform.extend([self.scale, 0., l.windows, l.warmth]);
        uniform.extend([l.shadow[0], l.shadow[1], l.shadow_len, l.shadow_strength]);
        uniform.extend([l.ambient[0], l.ambient[1], l.ambient[2], l.dark]);
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.draw_scene(&mut encoder, &view, camera);
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        frame.present();
        if suboptimal {
            self.resize(self.size);
        }
        Ok(true)
    }
}
/// Kamera (32 B) + Sonne + Parameter + Schatten + Umgebungslicht (je 16 B).
const UNIFORM_BYTES: u64 = 96;
fn depth_view(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("2D layer / building depth"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
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
