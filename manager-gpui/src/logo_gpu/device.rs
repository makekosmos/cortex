//! The wgpu renderer behind `crate::logo_gpu::frame`: `GpuLogo` owns the
//! adapter/device/pipeline, the offscreen target, and the readback ring.
//!
//! Readback never blocks the UI thread: each tick submits render+copy into
//! the next free slot of a `READBACK_RING`-deep ring of MAP_READ buffers and
//! `map_async`s it; completed maps are harvested via a `PollType::Poll`
//! device pump (run before AND after each submit) and a channel, keeping the
//! newest and recycling the rest — a fresh submit never waits on a map in
//! flight, so renders can run at the full paint rate. Until the first map
//! lands the caller sees `Ok(None)` and paints a WebP frame, so there is no
//! blank flash. A map pending longer than `READBACK_TIMEOUT` is a hard
//! failure.

use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use gpui::RenderImage;
use image::{Frame, ImageBuffer, Rgba};
use smallvec::{smallvec, SmallVec};

use super::{Stats, FIELD_PNG};

const SHADER: &str = include_str!("../../shaders/liquid_metal.wgsl");
const READBACK_TIMEOUT: Duration = Duration::from_secs(2);
/// Readback ring depth: enough that a new submit never has to wait for the
/// previous map to land, even with a couple of frames in flight.
const READBACK_RING: usize = 3;
/// The field PNG is 256×252; targets keep that aspect.
const ASPECT: f32 = 252.0 / 256.0;

/// Offscreen render target + its ring of readback buffers, recreated when
/// the requested size (scale factor) changes.
struct Target {
    texture: wgpu::Texture,
    readbacks: [wgpu::Buffer; READBACK_RING],
    row_padded: u32,
    w: u32,
    h: u32,
}

/// (target generation, submit sequence, ring index, result).
type MapDone = (u64, u64, usize, Result<(), wgpu::BufferAsyncError>);

pub(super) struct GpuLogo {
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
    /// Submit sequence — identifies the newest completed map on harvest.
    seq: u64,
    /// (seq, submitted at) per ring slot while its map is in flight.
    pending: [Option<(u64, Instant)>; READBACK_RING],
    next_buf: usize,
    last_image: Option<Arc<RenderImage>>,
    pub(super) adapter_name: String,
    pub(super) init_ms: f64,
    stats: Stats,
}

impl GpuLogo {
    pub(super) fn new() -> Result<Self, String> {
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
            seq: 0,
            pending: [None; READBACK_RING],
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
        let readbacks = [(); READBACK_RING].map(|()| {
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
        self.pending = [None; READBACK_RING];
        self.next_buf = 0;
        // The old image is the wrong size; a fresh one is one tick away.
        self.last_image = None;
    }

    /// Pump the device and harvest completed readbacks into `last_image`,
    /// keeping the newest and recycling the rest. Never blocks.
    fn pump(&mut self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Poll)
            .map_err(|e| format!("poll: {e}"))?;
        let mut newest: Option<(u64, Arc<RenderImage>)> = None;
        while let Ok((gen, seq, idx, res)) = self.map_rx.try_recv() {
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
            if let Some((_, at)) = self.pending[idx].take() {
                self.stats.harvested(at.elapsed().as_secs_f64() * 1e3);
            }
            if newest.as_ref().is_none_or(|(s, _)| seq > *s) {
                let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(w, h, out)
                    .ok_or_else(|| "image buffer size mismatch".to_string())?;
                let frames: SmallVec<[Frame; 1]> = smallvec![Frame::new(img)];
                newest = Some((seq, Arc::new(RenderImage::new(frames))));
            }
        }
        if let Some((_, img)) = newest {
            self.last_image = Some(img);
        }
        Ok(())
    }

    /// Submit render+copy for `t_secs` into the next free ring buffer and
    /// kick off `map_async`; returns the newest frame we already hold.
    pub(super) fn frame(
        &mut self,
        t_secs: f32,
        w: u32,
    ) -> Result<Option<Arc<RenderImage>>, String> {
        let w = w.max(1);
        let h = (w as f32 * ASPECT).round().max(1.0) as u32;
        self.ensure_target(w, h);
        self.pump()?;

        // A map stuck past the timeout is a wedged readback — hard fail.
        if self
            .pending
            .iter()
            .flatten()
            .any(|(_, at)| at.elapsed() > READBACK_TIMEOUT)
        {
            return Err("readback timed out".into());
        }
        // Next free ring slot; if every buffer has a map in flight we just
        // keep showing the last frame this tick — still non-blocking.
        let idx = (0..READBACK_RING)
            .map(|i| (self.next_buf + i) % READBACK_RING)
            .find(|&i| self.pending[i].is_none());
        let Some(idx) = idx else {
            self.stats.tick(false);
            return Ok(self.last_image.clone());
        };

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
        let seq = self.seq;
        self.seq += 1;
        let tx = self.map_tx.clone();
        target.readbacks[idx]
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send((gen, seq, idx, res));
            });
        self.pending[idx] = Some((seq, Instant::now()));
        self.next_buf = (idx + 1) % READBACK_RING;
        self.stats.tick(true);
        // Pump once more after submitting: a fast adapter may have the map
        // callback ready already, so it lands this same tick.
        self.pump()?;
        Ok(self.last_image.clone())
    }
}
