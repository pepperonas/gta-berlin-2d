//! Machbarkeits- und Lastprobe für die Xbox (Grafik-Überarbeitung, Xbox-Teststrategie Schritt 3).
//!
//! Dieselbe Last läuft auf dem Mac (`cargo run --release -p berlin-probe --example mac`) und auf der Konsole (UWP-Hülle
//! `xbox/RustProbe`, die Funktionen aus `ffi.rs` ruft). Die Last ist dem Szenendurchgang des Spiels nachgebildet: acht
//! halbtransparente Vollbild-Schichten mit Texturabtastung aus einem Array und Rauschen in ein 2560 × 1440 großes
//! Float-Ziel, danach die verkleinerte Ausgabe ins Fenster. Gemessen wird je Betriebsart (mit/ohne Kantenglättung, Float
//! oder 8 Bit) die GPU-Zeit per Zeitstempel. Das Verhältnis Xbox/Mac überträgt die Spielmessungen auf die Konsole.
use anyhow::{Context, Result};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};

/// Feste Größe des Lastziels (unabhängig vom Fenster, damit Mac und Xbox dasselbe rechnen).
pub const WIDTH: u32 = 2560;
pub const HEIGHT: u32 = 1440;
/// Schichten je Bild.
pub const LAYERS: u32 = 8;
/// Bilder je Betriebsart, bevor zur nächsten gewechselt wird (die ersten 60 zählen nicht).
pub const FRAMES_PER_MODE: u32 = 360;
const WARMUP: u32 = 60;
const SLOT: u64 = 256;

/// Schritt, in dem `Probe::frame` gerade steckt (Index in `STAGES`). Die Konsolen-Hülle liest ihn aus einem anderen
/// Thread: hängt das Zeichnen, zeigt der Bericht die Stelle.
pub static STAGE: AtomicU8 = AtomicU8::new(0);
pub const STAGES: [&str; 6] = [
    "zwischen Bildern",
    "Bild der Oberfläche holen (get_current_texture)",
    "Last aufzeichnen",
    "abschicken (submit)",
    "anzeigen (present)",
    "Zeitstempel abholen",
];

#[derive(Debug, Clone, Copy)]
pub struct Mode {
    pub name: &'static str,
    pub format: wgpu::TextureFormat,
    pub samples: u32,
}
pub const MODES: [Mode; 3] = [
    Mode {
        name: "Float, 4x MSAA (HD Hoch)",
        format: wgpu::TextureFormat::Rgba16Float,
        samples: 4,
    },
    Mode {
        name: "Float, ohne MSAA (HD Niedrig)",
        format: wgpu::TextureFormat::Rgba16Float,
        samples: 1,
    },
    Mode {
        name: "8 Bit, ohne MSAA (alter Renderer)",
        format: wgpu::TextureFormat::Rgba8Unorm,
        samples: 1,
    },
];

/// Grund, aus dem DX12 das Gerät entfernt hat (`ID3D12Device::GetDeviceRemovedReason`); `None`, solange es lebt
/// oder das Gerät nicht von DX12 stammt.
#[cfg(windows)]
fn removed_reason(device: &wgpu::Device) -> Option<String> {
    let hal = unsafe { device.as_hal::<wgpu::hal::api::Dx12>() }?;
    match unsafe { hal.raw_device().GetDeviceRemovedReason() } {
        Ok(()) => None,
        Err(e) => Some(format!("0x{:08X} {}", e.code().0 as u32, e.message())),
    }
}
#[cfg(not(windows))]
fn removed_reason(_: &wgpu::Device) -> Option<String> {
    None
}

/// Prüfpunkt im Aufbau: alles Bisherige abschicken, bis 5 s auf die GPU warten, dann melden, ob das Gerät noch lebt.
fn alive(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    lost: &Mutex<Option<String>>,
    steps: &mut Vec<String>,
    what: &str,
) {
    queue.submit([]);
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(5)),
    });
    let gone = lost.lock().ok().and_then(|l| l.clone());
    let reason = removed_reason(device);
    steps.push(match (gone, reason) {
        (None, None) => format!("Aufbau {what}: Gerät ok"),
        (g, r) => format!(
            "Aufbau {what}: GERÄT VERLOREN ({}; DX12-Grund {})",
            g.unwrap_or_else(|| "kein Rückruf".into()),
            r.unwrap_or_else(|| "S_OK".into())
        ),
    });
}

/// Median und 95. Perzentil (nächster Rang).
pub fn median_p95(samples: &[f32]) -> Option<(f32, f32)> {
    if samples.is_empty() {
        return None;
    }
    let mut v = samples.to_vec();
    v.sort_by(f32::total_cmp);
    let rank = |p: f64| v[((p * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1];
    Some((rank(0.5), rank(0.95)))
}

struct Target {
    color: wgpu::TextureView,
    resolve: Option<wgpu::TextureView>,
    post: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

/// Zeitstempel über einen Ring von Lesepuffern (wartet nie auf die GPU).
struct Timer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: Vec<wgpu::Buffer>,
    state: Vec<Arc<AtomicU8>>,
    tag: Vec<usize>,
    period: f32,
    next: usize,
    current: Option<usize>,
}
const SLOTS: usize = 4;

impl Timer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        Some(Self {
            set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("Probe-Zeit"),
                ty: wgpu::QueryType::Timestamp,
                count: (SLOTS * 2) as u32,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT * SLOTS as u64,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: (0..SLOTS)
                .map(|_| {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: None,
                        size: 16,
                        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    })
                })
                .collect(),
            state: (0..SLOTS).map(|_| Arc::new(AtomicU8::new(0))).collect(),
            tag: vec![0; SLOTS],
            period: queue.get_timestamp_period(),
            next: 0,
            current: None,
        })
    }
    fn begin(&mut self, mode: usize) -> Option<u32> {
        let s = self.next;
        if self.state[s].load(Ordering::Acquire) != 0 {
            self.current = None;
            return None;
        }
        self.next = (s + 1) % SLOTS;
        self.current = Some(s);
        self.tag[s] = mode;
        Some(s as u32 * 2)
    }
    fn resolve(&self, enc: &mut wgpu::CommandEncoder) {
        if let Some(s) = self.current {
            let at = s as u64 * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
            enc.resolve_query_set(&self.set, s as u32 * 2..s as u32 * 2 + 2, &self.resolve, at);
            enc.copy_buffer_to_buffer(&self.resolve, at, &self.read[s], 0, 16);
        }
    }
    /// Nach dem Abschicken; liefert fertige Messungen (Betriebsart, ms).
    fn collect(&mut self, device: &wgpu::Device) -> Vec<(usize, f32)> {
        if let Some(s) = self.current.take() {
            let st = self.state[s].clone();
            st.store(1, Ordering::Release);
            self.read[s]
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| {
                    st.store(if r.is_ok() { 2 } else { 0 }, Ordering::Release)
                });
        }
        let _ = device.poll(wgpu::PollType::Poll);
        let mut out = Vec::new();
        for s in 0..SLOTS {
            if self.state[s].load(Ordering::Acquire) != 2 {
                continue;
            }
            let ticks = {
                let d = self.read[s].slice(..).get_mapped_range();
                let t0 = u64::from_le_bytes(d[0..8].try_into().unwrap_or_default());
                let t1 = u64::from_le_bytes(d[8..16].try_into().unwrap_or_default());
                t1.saturating_sub(t0)
            };
            self.read[s].unmap();
            self.state[s].store(0, Ordering::Release);
            let ms = ticks as f64 * self.period as f64 / 1e6;
            if ms > 0. && ms < 1000. {
                out.push((self.tag[s], ms as f32));
            }
        }
        out
    }
}

/// Die Probe: Gerät, Oberfläche, Lastziele je Betriebsart, Messwerte.
pub struct Probe {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    layer_bind: wgpu::BindGroup,
    params: wgpu::Buffer,
    targets: Vec<Target>,
    post_pipeline: wgpu::RenderPipeline,
    timer: Option<Timer>,
    frame: u32,
    samples: Vec<Vec<f32>>,
    cpu: Vec<f32>,
    last: std::time::Instant,
    /// Aufrufe, in denen die Oberfläche kein Bild lieferte, und der letzte Grund
    skipped: u32,
    skip_reason: &'static str,
    /// Kopfzeilen: Adapter, Backend, Merkmale, Schritte bis hierher
    pub info: Vec<String>,
    errors: Arc<Mutex<Vec<String>>>,
    /// Meldung des Device-Lost-Rückrufs
    lost: Arc<Mutex<Option<String>>>,
}

impl Probe {
    /// Gerät für eine schon angelegte Oberfläche; `steps` = bisherige Prüfschritte (für die Anzeige), `timestamps` =
    /// GPU-Zeitstempel anfordern, falls das Gerät sie kann (die Konsole verlor das Gerät einmal genau dabei).
    pub fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        mut steps: Vec<String>,
        timestamps: bool,
    ) -> Result<Self> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .context("kein Adapter für die Oberfläche")?;
        let ai = adapter.get_info();
        steps.push(format!(
            "Adapter: {} ({:?}, {:?}, Treiber {})",
            ai.name, ai.backend, ai.device_type, ai.driver
        ));
        let limits = adapter.limits();
        steps.push(format!(
            "Grenzen: Textur {} px, Array {} Schichten",
            limits.max_texture_dimension_2d, limits.max_texture_array_layers
        ));
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Probe"),
            required_features: if timestamps {
                adapter.features() & wgpu::Features::TIMESTAMP_QUERY
            } else {
                wgpu::Features::empty()
            },
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .context("Gerät")?;
        let errors: Arc<Mutex<Vec<String>>> = Arc::default();
        let e2 = errors.clone();
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
            if let Ok(mut v) = e2.lock() {
                v.push(e.to_string());
            }
        }));
        // Ein verlorenes Gerät meldet wgpu nur hier, nicht über on_uncaptured_error
        let lost: Arc<Mutex<Option<String>>> = Arc::default();
        let l2 = lost.clone();
        device.set_device_lost_callback(move |reason, msg| {
            if let Ok(mut l) = l2.lock() {
                *l = Some(format!("{reason:?}: {msg}"));
            }
        });
        alive(&device, &queue, &lost, &mut steps, "Gerät angelegt");
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .or_else(|| caps.formats.first().copied())
            .context("Oberfläche ohne Format")?;
        steps.push(format!(
            "Oberfläche: {:?} (angeboten {:?}), Zeitstempel {}",
            format,
            caps.formats,
            if device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
                "ja"
            } else {
                "nein"
            }
        ));
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        alive(
            &device,
            &queue,
            &lost,
            &mut steps,
            "Oberfläche konfiguriert",
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Probe-Last"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bench.wgsl").into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let float_tex = |dim| wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: dim,
            multisampled: false,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                entry(0, float_tex(wgpu::TextureViewDimension::D2Array)),
                entry(
                    1,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
                entry(
                    2,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                ),
                entry(3, float_tex(wgpu::TextureViewDimension::D2)),
            ],
        });
        let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let array = noise_array(&device, &queue);
        alive(
            &device,
            &queue,
            &lost,
            &mut steps,
            "Rauschtextur hochgeladen",
        );
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: SLOT * (LAYERS as u64 + 1),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Platzhalter für Bindung 3 im Schicht-Durchgang (dort ungenutzt)
        let dummy = device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let bind = |scene: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&array),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &params,
                            offset: 0,
                            size: wgpu::BufferSize::new(16),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(scene),
                    },
                ],
            })
        };
        let layer_bind = bind(&dummy);
        let pipeline = |fs: &str, format, samples, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(&pipe_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        };
        let targets = MODES
            .iter()
            .map(|m| {
                let tex = |samples, usage| {
                    device
                        .create_texture(&wgpu::TextureDescriptor {
                            label: Some(m.name),
                            size: wgpu::Extent3d {
                                width: WIDTH,
                                height: HEIGHT,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: samples,
                            dimension: wgpu::TextureDimension::D2,
                            format: m.format,
                            usage,
                            view_formats: &[],
                        })
                        .create_view(&Default::default())
                };
                let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
                let image = tex(1, attach | wgpu::TextureUsages::TEXTURE_BINDING);
                let (color, resolve) = if m.samples > 1 {
                    (tex(m.samples, attach), Some(image.clone()))
                } else {
                    (image.clone(), None)
                };
                Target {
                    color,
                    resolve,
                    post: bind(&image),
                    pipeline: pipeline(
                        "layer_fs",
                        m.format,
                        m.samples,
                        Some(wgpu::BlendState::ALPHA_BLENDING),
                    ),
                }
            })
            .collect();
        alive(
            &device,
            &queue,
            &lost,
            &mut steps,
            "Lastziele und Pipelines",
        );
        let post_pipeline = pipeline("post_fs", format, 1, None);
        alive(&device, &queue, &lost, &mut steps, "Ausgabe-Pipeline");
        let timer = Timer::new(&device, &queue);
        if timer.is_some() {
            alive(&device, &queue, &lost, &mut steps, "Zeitstempel-Abfragen");
        }
        steps.push("Bereit: Last läuft".into());
        Ok(Self {
            device,
            queue,
            surface,
            config,
            layer_bind,
            params,
            targets,
            post_pipeline,
            timer,
            frame: 0,
            samples: vec![Vec::new(); MODES.len()],
            cpu: Vec::new(),
            last: std::time::Instant::now(),
            skipped: 0,
            skip_reason: "",
            info: steps,
            errors,
            lost,
        })
    }
    /// Gerät verloren (Rückruf von wgpu oder von DX12 entfernt)?
    pub fn lost(&self) -> bool {
        self.lost.lock().map(|l| l.is_some()).unwrap_or(false)
            || removed_reason(&self.device).is_some()
    }
    /// Größe der Swapchain (wie zuletzt konfiguriert).
    pub fn surface_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }
    /// Aktuelle Betriebsart (wechselt alle `FRAMES_PER_MODE` Bilder).
    pub fn mode(&self) -> usize {
        (self.frame / FRAMES_PER_MODE) as usize % MODES.len()
    }
    /// Ein Bild: Last ins Ziel der aktuellen Betriebsart, Ausgabe ins Fenster.
    pub fn frame(&mut self) -> Result<()> {
        let now = std::time::Instant::now();
        self.cpu
            .push(now.duration_since(self.last).as_secs_f32() * 1000.);
        if self.cpu.len() > 600 {
            self.cpu.remove(0);
        }
        self.last = now;
        STAGE.store(1, Ordering::Relaxed);
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            other => {
                self.skipped += 1;
                self.skip_reason = match other {
                    wgpu::CurrentSurfaceTexture::Outdated => "Outdated",
                    wgpu::CurrentSurfaceTexture::Lost => "Lost",
                    wgpu::CurrentSurfaceTexture::Timeout => "Timeout",
                    wgpu::CurrentSurfaceTexture::Occluded => "Occluded",
                    // ohne Eintrag unter wgpu-Fehler: Gerät verloren (wgpu meldet das nur per Rückruf)
                    wgpu::CurrentSurfaceTexture::Validation => "Validation",
                    _ => "anderer",
                };
                if matches!(
                    other,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost
                ) {
                    self.surface.configure(&self.device, &self.config);
                }
                STAGE.store(0, Ordering::Relaxed);
                return Ok(());
            }
        };
        STAGE.store(2, Ordering::Relaxed);
        let mode = self.mode();
        let t = self.frame as f32 / 60.;
        let mut bytes = vec![0u8; (SLOT * (LAYERS as u64 + 1)) as usize];
        for i in 0..=LAYERS {
            let srgb = if self.config.format.is_srgb() { 0. } else { 1. };
            let v: [f32; 4] = [t, i as f32, srgb, 0.];
            let o = (SLOT * i as u64) as usize;
            bytes[o..o + 16].copy_from_slice(bytemuck::cast_slice(&v));
        }
        self.queue.write_buffer(&self.params, 0, &bytes);
        let in_window = self.frame % FRAMES_PER_MODE >= WARMUP;
        let stamps = if in_window {
            self.timer.as_mut().and_then(|tm| tm.begin(mode))
        } else {
            None
        };
        // Ohne Zeitstempel (auf der Xbox verlor der Treiber beim Anlegen der Zeitstempel-Abfragen das Gerät): die Last
        // einzeln abschicken und die Zeit bis zum Fence messen (GPU-Zeit plus Bruchteile einer Millisekunde)
        let fence = self.timer.is_none() && in_window;
        if fence {
            let _ = self.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(3)),
            });
        }
        let mut enc = self.device.create_command_encoder(&Default::default());
        let target = &self.targets[mode];
        {
            let tw =
                stamps
                    .zip(self.timer.as_ref())
                    .map(|(q, tm)| wgpu::RenderPassTimestampWrites {
                        query_set: &tm.set,
                        beginning_of_pass_write_index: Some(q),
                        end_of_pass_write_index: Some(q + 1),
                    });
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Last"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.color,
                    depth_slice: None,
                    resolve_target: target.resolve.as_ref(),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.14,
                            g: 0.2,
                            b: 0.12,
                            a: 1.,
                        }),
                        store: if target.resolve.is_some() {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                timestamp_writes: tw,
                ..Default::default()
            });
            pass.set_pipeline(&target.pipeline);
            for i in 0..LAYERS {
                pass.set_bind_group(0, &self.layer_bind, &[(SLOT * i as u64) as u32]);
                pass.draw(0..3, 0..1);
            }
        }
        if fence {
            let t0 = std::time::Instant::now();
            let idx = self.queue.submit([enc.finish()]);
            let done = self.device.poll(wgpu::PollType::Wait {
                submission_index: Some(idx),
                timeout: Some(std::time::Duration::from_secs(3)),
            });
            if done.is_ok() {
                let v = &mut self.samples[mode];
                v.push(t0.elapsed().as_secs_f32() * 1000.);
                if v.len() > 600 {
                    v.remove(0);
                }
            }
            enc = self.device.create_command_encoder(&Default::default());
        }
        {
            let view = frame.texture.create_view(&Default::default());
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Ausgabe"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.post_pipeline);
            pass.set_bind_group(0, &target.post, &[(SLOT * LAYERS as u64) as u32]);
            pass.draw(0..3, 0..1);
        }
        if let Some(tm) = &self.timer {
            tm.resolve(&mut enc);
        }
        STAGE.store(3, Ordering::Relaxed);
        self.queue.submit([enc.finish()]);
        STAGE.store(4, Ordering::Relaxed);
        frame.present();
        STAGE.store(5, Ordering::Relaxed);
        if let Some(tm) = &mut self.timer {
            for (m, ms) in tm.collect(&self.device) {
                let v = &mut self.samples[m];
                v.push(ms);
                if v.len() > 600 {
                    v.remove(0);
                }
            }
        }
        self.frame += 1;
        STAGE.store(0, Ordering::Relaxed);
        Ok(())
    }
    /// Mehrzeiliger Bericht: Prüfschritte, Messwerte je Betriebsart, Fehler.
    pub fn status(&self) -> String {
        let mut out = self.info.join("\n");
        out.push_str(&format!(
            "\n\nLast: {} Schichten in {}×{}, aktuell: {}\n",
            LAYERS,
            WIDTH,
            HEIGHT,
            MODES[self.mode()].name
        ));
        let kind = if self.timer.is_some() { "GPU" } else { "Fence" };
        for (i, m) in MODES.iter().enumerate() {
            match median_p95(&self.samples[i]) {
                Some((med, p95)) => out.push_str(&format!(
                    "  {:<34} {kind} Median {:6.2} ms  P95 {:6.2} ms  ({} Bilder)\n",
                    m.name,
                    med,
                    p95,
                    self.samples[i].len()
                )),
                None => out.push_str(&format!("  {:<34} (noch keine Messung)\n", m.name)),
            }
        }
        if let Some((med, p95)) = median_p95(&self.cpu) {
            out.push_str(&format!(
                "  Bildabstand Median {med:.2} ms, P95 {p95:.2} ms (Fenster {}×{})\n",
                self.config.width, self.config.height
            ));
        }
        if self.timer.is_none() {
            out.push_str(
                "  ohne Zeitstempel: Fence = Last einzeln abgeschickt, Zeit bis die GPU fertig ist\n",
            );
        }
        if self.skipped > 0 {
            out.push_str(&format!(
                "  Oberfläche ohne Bild: {}× (zuletzt {}), gezeichnet: {}\n",
                self.skipped, self.skip_reason, self.frame
            ));
        }
        let gone = self.lost.lock().ok().and_then(|l| l.clone());
        let reason = removed_reason(&self.device);
        if gone.is_some() || reason.is_some() {
            out.push_str(&format!(
                "\nGERÄT VERLOREN: {}\n  DX12-Grund: {}\n",
                gone.unwrap_or_else(|| "kein Rückruf".into()),
                reason.unwrap_or_else(|| "S_OK".into())
            ));
        }
        if let Ok(e) = self.errors.lock()
            && !e.is_empty()
        {
            out.push_str("\nwgpu-Fehler:\n");
            for line in e.iter().take(5) {
                out.push_str(&format!("  {line}\n"));
            }
        }
        out
    }
    /// Alle Betriebsarten mindestens einmal gemessen?
    pub fn complete(&self) -> bool {
        self.samples.iter().all(|s| s.len() >= 100)
    }
    pub fn frame_count(&self) -> u32 {
        self.frame
    }
}

/// Rauschtextur 512² × 4 Schichten mit Mip-Kette (Hash, deterministisch).
fn noise_array(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    const S: u32 = 512;
    const LEVELS: u32 = 10;
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Probe-Textur"),
        size: wgpu::Extent3d {
            width: S,
            height: S,
            depth_or_array_layers: 4,
        },
        mip_level_count: LEVELS,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for layer in 0..4u32 {
        let mut px = vec![0u8; (S * S * 4) as usize];
        for (i, p) in px.chunks_mut(4).enumerate() {
            let mut h = (i as u32).wrapping_mul(2654435761) ^ layer.wrapping_mul(97);
            h ^= h >> 13;
            h = h.wrapping_mul(0x5bd1e995);
            p.copy_from_slice(&[
                (h & 255) as u8,
                ((h >> 8) & 255) as u8,
                ((h >> 16) & 255) as u8,
                255,
            ]);
        }
        let (mut cur, mut w) = (px, S);
        for level in 0..LEVELS {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: level,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &cur,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(w),
                },
                wgpu::Extent3d {
                    width: w,
                    height: w,
                    depth_or_array_layers: 1,
                },
            );
            if w == 1 {
                break;
            }
            let nw = w / 2;
            let mut next = vec![0u8; (nw * nw * 4) as usize];
            for y in 0..nw {
                for x in 0..nw {
                    for c in 0..4 {
                        let at =
                            |dx, dy| cur[(((y * 2 + dy) * w + x * 2 + dx) * 4 + c) as usize] as u32;
                        next[((y * nw + x) * 4 + c) as usize] =
                            ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
                    }
                }
            }
            (cur, w) = (next, nw);
        }
    }
    tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

#[cfg(windows)]
mod ffi;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modes_cycle_and_stats() {
        assert_eq!(median_p95(&[]), None);
        assert_eq!(median_p95(&[2., 1., 3.]), Some((2., 3.)));
        assert_eq!(MODES.len(), 3);
        const { assert!(FRAMES_PER_MODE > WARMUP) };
    }
}
