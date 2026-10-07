//! Cargo feature `logo-gpu` (default OFF): renders the Paper liquid-metal
//! shader live on the GPU every overlay frame, so the metal never repeats
//! the WebP's 2 s loop. Port of the kos-355 bench
//! (`perf/src/gpu.rs` + `shaders/liquid_metal.wgsl`): wgpu renders into an
//! offscreen 256×252 Rgba8Unorm target whose fragment shader emits
//! premultiplied BGRA, we read it back into a `RenderImage` and the overlay
//! paints it exactly where the baked WebP frame went.
//!
//! The Poisson edge field (R=edge, G=alpha) is a checked-in PNG
//! (`assets/logo-field-256.png`) baked offline from `icons/mundus.svg` by
//! the same `field.rs` port — no resvg/SOR work at startup.
//!
//! Failure policy: `MUNDUS_LOGO_GPU=off`, no adapter, device-lost, or any
//! per-frame error parks the renderer for the rest of the session (logged
//! once) and the overlay falls back to `LogoFrames` (the WebP ring).

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use gpui::RenderImage;
use image::{Frame, ImageBuffer, Rgba};
use smallvec::{smallvec, SmallVec};

const FIELD_PNG: &[u8] = include_bytes!("../assets/logo-field-256.png");
const SHADER: &str = include_str!("../shaders/liquid_metal.wgsl");
const READBACK_TIMEOUT: Duration = Duration::from_secs(2);

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

/// Frame for `elapsed` seconds since the overlay appeared, or `None` when
/// the GPU path is off/dead — the caller paints the WebP ring instead.
/// Blocking readback is ~1 ms on lavapipe; on real GPUs it is sub-ms.
pub fn frame(elapsed: f32) -> Option<Arc<RenderImage>> {
    let mut slot = slot();
    let Slot::Live(g) = &mut *slot else {
        return None;
    };
    match g.frame(elapsed) {
        Ok(img) => Some(img),
        Err(e) => {
            eprintln!("logo-gpu: frame failed ({e}), using baked WebP logo");
            *slot = Slot::Disabled;
            None
        }
    }
}

struct GpuLogo {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    target: wgpu::Texture,
    readback: wgpu::Buffer,
    row_padded: u32,
    w: u32,
    h: u32,
    adapter_name: String,
    init_ms: f64,
    frame_times: Vec<f64>,
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
        let target = device.create_texture(&wgpu::TextureDescriptor {
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
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(row_padded * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group,
            uniform,
            target,
            readback,
            row_padded,
            w,
            h,
            adapter_name,
            init_ms: t0.elapsed().as_secs_f64() * 1e3,
            frame_times: Vec::with_capacity(600),
        })
    }

    /// Render one frame at `t_secs`, submit, and block on readback.
    /// Returns a premultiplied-BGRA `RenderImage`.
    fn frame(&mut self, t_secs: f32) -> Result<Arc<RenderImage>, String> {
        let t0 = Instant::now();
        let (w, h, row_padded) = (self.w, self.h, self.row_padded);
        let mut ubuf = [0u8; 48];
        ubuf[..4].copy_from_slice(&t_secs.to_le_bytes());
        self.queue.write_buffer(&self.uniform, 0, &ubuf);

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("lm") });
        {
            let view = self.target.create_view(&Default::default());
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
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
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

        let slice = self.readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("poll: {e}"))?;
        rx.recv_timeout(READBACK_TIMEOUT)
            .map_err(|_| "readback timed out".to_string())?
            .map_err(|e| format!("map: {e}"))?;
        let data = slice.get_mapped_range();
        let mut out = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            let src = (y * row_padded) as usize;
            let dst = (y * w * 4) as usize;
            out[dst..dst + (w * 4) as usize].copy_from_slice(&data[src..src + (w * 4) as usize]);
        }
        drop(data);
        self.readback.unmap();

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(w, h, out)
            .ok_or_else(|| "image buffer size mismatch".to_string())?;
        let data: SmallVec<[Frame; 1]> = smallvec![Frame::new(img)];

        self.frame_times.push(t0.elapsed().as_secs_f64() * 1e3);
        if self.frame_times.len() == 150 {
            self.frame_times
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            eprintln!(
                "logo-gpu: render+readback median {:.2} ms over {} frames",
                self.frame_times[75],
                self.frame_times.len()
            );
            self.frame_times.clear();
        }
        Ok(Arc::new(RenderImage::new(data)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_off_env_disables_gpu_path() {
        // This test is the only one that touches the process-wide slot;
        // the ignored GPU test below bypasses it via GpuLogo::new.
        std::env::set_var("MUNDUS_LOGO_GPU", "off");
        assert!(frame(0.0).is_none());
    }

    #[test]
    fn field_png_decodes_to_canvas_size() {
        let img = image::load_from_memory(FIELD_PNG)
            .expect("field png must decode")
            .to_rgba8();
        assert_eq!(img.dimensions(), (256, 252));
    }

    #[test]
    #[ignore = "requires a GPU/Vulkan adapter; run with --features logo-gpu -- --ignored"]
    fn gpu_frame_renders_logo_pixels() {
        let mut g = GpuLogo::new().expect("adapter+device on this box");
        let img = g.frame(0.5).expect("frame");
        let size = img.size(0);
        assert_eq!((i32::from(size.width), i32::from(size.height)), (256, 252));
        if let Ok(path) = std::env::var("MUNDUS_LOGO_GPU_DUMP") {
            // Premultiplied BGRA, same bytes the overlay paints.
            std::fs::write(path, img.as_bytes(0).expect("frame bytes")).expect("dump frame");
        }
    }
}
