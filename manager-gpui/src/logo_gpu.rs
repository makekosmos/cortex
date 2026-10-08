//! Cargo feature `logo-gpu` (default OFF): renders the Paper liquid-metal
//! shader live on the GPU every overlay frame, so the metal never repeats
//! the WebP's 2 s loop. Port of the kos-355 bench
//! (`perf/src/gpu.rs` + `shaders/liquid_metal.wgsl`): wgpu renders into an
//! offscreen Rgba8Unorm target sized to the displayed logo (logical size ×
//! window scale factor, 256:252 aspect), whose fragment shader emits
//! premultiplied BGRA; a double-buffered non-blocking readback turns it into
//! a `RenderImage` that the overlay paints where the baked WebP frame went.
//!
//! The Poisson edge field (R=edge, G=alpha) is a checked-in PNG
//! (`assets/logo-field-256.png`) baked offline from `icons/mundus.svg` by
//! the same `field.rs` port — no resvg/SOR work at startup. It is sampled
//! through a linear sampler on UV, so it serves every target size.
//!
//! Readback never blocks the UI thread: each tick submits render+copy into
//! one of two MAP_READ buffers and `map_async`s it; the previous buffer's
//! map callback is harvested via a `PollType::Poll` device pump and a
//! channel. Until the first map lands the caller sees `Ok(None)` and paints
//! a WebP frame, so there is no blank flash. A map pending longer than
//! `READBACK_TIMEOUT` is a hard failure.
//!
//! Failure policy: `MUNDUS_LOGO_GPU=off`, no adapter, device-lost, or any
//! per-frame error parks the renderer for the rest of the session (logged
//! once) and the overlay falls back to `LogoFrames` (the WebP ring).

use std::sync::{mpsc, Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use gpui::RenderImage;
use image::{Frame, ImageBuffer, Rgba};
use smallvec::{smallvec, SmallVec};

const FIELD_PNG: &[u8] = include_bytes!("../assets/logo-field-256.png");
const SHADER: &str = include_str!("../shaders/liquid_metal.wgsl");
const READBACK_TIMEOUT: Duration = Duration::from_secs(2);
const STATS_WINDOW: Duration = Duration::from_secs(5);
/// The field PNG is 256×252; targets keep that aspect.
const ASPECT: f32 = 252.0 / 256.0;

/// Lazy GPU renderer state: one init attempt per session.
enum Slot {
    Disabled,
    Live(Box<GpuLogo>),
}

fn slot() -> MutexGuard<'static, Slot> {
    static SLOT: OnceLock<Mutex<Slot>> = OnceLock::new();
    let m = SLOT.get_or_init(|| {
        Mutex::new(match std::env::var("MUNDUS_LOGO_GPU").as_deref() {
            Ok("off") | Ok("0") => Slot::Disabled,
            _ => match GpuLogo::new() {
                Ok(g) => {
                    eprintln!(
                        "logo-gpu: live liquid-metal on '{}' (init {:.1} ms)",
                        g.adapter_name, g.init_ms
                    );
                    Slot::Live(Box::new(g))
                }
                Err(e) => {
                    eprintln!("logo-gpu: init failed ({e}), using baked WebP logo");
                    Slot::Disabled
                }
            },
        })
    });
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Latest rendered frame for `elapsed` seconds since the overlay appeared,
/// rendered into a `w`-wide physical-px target (height follows the 256:252
/// aspect). `None` while the first async readback is in flight or when the
/// GPU path is off/dead — the caller paints the WebP ring instead.
pub fn frame(elapsed: f32, w: u32) -> Option<Arc<RenderImage>> {
    let mut slot = slot();
    let Slot::Live(g) = &mut *slot else {
        return None;
    };
    match g.frame(elapsed, w) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("logo-gpu: frame failed ({e}), using baked WebP logo");
            *slot = Slot::Disabled;
            None
        }
    }
}

/// Offscreen render target + its pair of readback buffers, recreated when
/// the requested size (scale factor) changes.
struct Target {
    texture: wgpu::Texture,
    readbacks: [wgpu::Buffer; 2],
    row_padded: u32,
    w: u32,
    h: u32,
}

type MapDone = (u64, usize, Result<(), wgpu::BufferAsyncError>);

struct GpuLogo {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    target: Option<Target>,
    /// Bumped on every target recreate; stale map callbacks are dropped.
    target_gen: u64,
    map_tx: mpsc::Sender<MapDone>,
    map_rx: mpsc::Receiver<MapDone>,
    /// (buffer index, submitted at) for the in-flight map, if any.
    pending: Option<(usize, Instant)>,
    next_buf: usize,
    last_image: Option<Arc<RenderImage>>,
    adapter_name: String,
    init_ms: f64,
    stats: Stats,
}

#[derive(Default)]
struct Stats {
    window_start: Option<Instant>,
    renders: u32,
    /// Submit→readback latency per harvested frame, ms.
    times_ms: Vec<f64>,
}

impl Stats {
    fn frame(&mut self, latency_ms: f64) {
        let start = self.window_start.get_or_insert_with(Instant::now);
        self.renders += 1;
        self.times_ms.push(latency_ms);
        if start.elapsed() >= STATS_WINDOW {
            self.times_ms
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            eprintln!(
                "logo-gpu: {:.1} renders/s, render+readback median {:.2} ms",
                f64::from(self.renders) / start.elapsed().as_secs_f64(),
                self.times_ms[self.times_ms.len() / 2],
            );
            *self = Stats::default();
        }
    }
}

impl GpuLogo {
    fn new() -> Result<Self, String> {
        if std::env::var("MUNDUS_LOGO_GPU").as_deref() == Ok("fail-init") {
            return Err("forced failure via MUNDUS_LOGO_GPU=fail-init".into());
        }
        let t0 = Instant::now();
        let field = image::load_from_memory(FIELD_PNG)
            .map_err(|e| format!("field png: {e}"))?
            .to_rgba8();
        let (w, h) = field.dimensions();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: true,
        }))
        .map_err(|e| format!("no adapter: {e}"))?;
        let adapter_name = adapter.get_info().name;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("logo-gpu"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("device: {e}"))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("liquid-metal"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let field_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("field"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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
                texture: &field_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &field,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("field-samp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lm-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lm-bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &field_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lm-pipe"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("lm-layout"),
                    bind_group_layouts: &[Some(&bgl)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let (map_tx, map_rx) = mpsc::channel();
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group,
            uniform,
            target: None,
            target_gen: 0,
            map_tx,
            map_rx,
            pending: None,
            next_buf: 0,
            last_image: None,
            adapter_name,
            init_ms: t0.elapsed().as_secs_f64() * 1e3,
            stats: Stats::default(),
        })
    }

    /// (Re)create the render target and readback buffers for `w`×`h`.
    fn ensure_target(&mut self, w: u32, h: u32) {
        if self.target.as_ref().is_some_and(|t| t.w == w && t.h == h) {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let row_padded = (w * 4).div_ceil(256) * 256;
        let readbacks = [(); 2].map(|()| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: u64::from(row_padded * h),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        });
        self.target = Some(Target {
            texture,
            readbacks,
            row_padded,
            w,
            h,
        });
        self.target_gen += 1;
        self.pending = None;
        self.next_buf = 0;
        // The old image is the wrong size; a fresh one is one tick away.
        self.last_image = None;
    }

    /// Harvest any completed readback into `last_image`. Never blocks.
    fn harvest(&mut self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Poll)
            .map_err(|e| format!("poll: {e}"))?;
        while let Ok((gen, idx, res)) = self.map_rx.try_recv() {
            let Some(target) = &self.target else { continue };
            if gen != self.target_gen {
                continue; // stale buffer from a recreated target
            }
            res.map_err(|e| format!("map: {e}"))?;
            let slice = target.readbacks[idx].slice(..);
            let data = slice.get_mapped_range();
            let (w, h, row_padded) = (target.w, target.h, target.row_padded);
            let mut out = vec![0u8; (w * h * 4) as usize];
            for y in 0..h {
                let src = (y * row_padded) as usize;
                let dst = (y * w * 4) as usize;
                out[dst..dst + (w * 4) as usize]
                    .copy_from_slice(&data[src..src + (w * 4) as usize]);
            }
            drop(data);
            target.readbacks[idx].unmap();
            let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(w, h, out)
                .ok_or_else(|| "image buffer size mismatch".to_string())?;
            let frames: SmallVec<[Frame; 1]> = smallvec![Frame::new(img)];
            self.last_image = Some(Arc::new(RenderImage::new(frames)));
            if let Some((_, at)) = self.pending.take() {
                self.stats.frame(at.elapsed().as_secs_f64() * 1e3);
            }
        }
        Ok(())
    }

    /// Submit render+copy for `t_secs` into the next readback buffer and
    /// kick off `map_async`; returns the newest frame we already hold.
    fn frame(&mut self, t_secs: f32, w: u32) -> Result<Option<Arc<RenderImage>>, String> {
        let w = w.max(1);
        let h = (w as f32 * ASPECT).round().max(1.0) as u32;
        self.ensure_target(w, h);
        self.harvest()?;

        if let Some((_, at)) = self.pending {
            if at.elapsed() > READBACK_TIMEOUT {
                return Err("readback timed out".into());
            }
            // A map is still in flight — keep showing the previous frame.
            return Ok(self.last_image.clone());
        }

        let Some(target) = &self.target else {
            return Err("no target".into());
        };
        let (w, h, row_padded) = (target.w, target.h, target.row_padded);
        let mut ubuf = [0u8; 48];
        ubuf[..4].copy_from_slice(&t_secs.to_le_bytes());
        ubuf[8..12].copy_from_slice(&(w as f32).to_le_bytes());
        ubuf[12..16].copy_from_slice(&(h as f32).to_le_bytes());
        self.queue.write_buffer(&self.uniform, 0, &ubuf);

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("lm") });
        {
            let view = target.texture.create_view(&Default::default());
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lm-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        let idx = self.next_buf;
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &target.readbacks[idx],
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);

        let gen = self.target_gen;
        let tx = self.map_tx.clone();
        target.readbacks[idx]
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send((gen, idx, res));
            });
        self.pending = Some((idx, Instant::now()));
        self.next_buf = idx ^ 1;
        Ok(self.last_image.clone())
    }
}

#[cfg(test)]
#[path = "logo_gpu_tests.rs"]
mod tests;
