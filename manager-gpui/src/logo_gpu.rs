//! Cargo feature `logo-gpu` (default OFF): renders the Paper liquid-metal
//! shader live on the GPU every overlay frame, so the metal never repeats
//! the WebP's 2 s loop. Port of the kos-355 bench
//! (`perf/src/gpu.rs` + `shaders/liquid_metal.wgsl`): wgpu renders into an
//! offscreen Rgba8Unorm target sized to the displayed logo (logical size ×
//! window scale factor, 256:252 aspect), whose fragment shader emits
//! premultiplied BGRA; a non-blocking readback ring turns it into a
//! `RenderImage` that the overlay paints where the baked WebP frame went.
//! The renderer itself lives in `device.rs`.
//!
//! The Poisson edge field (R=edge, G=alpha) is a checked-in PNG
//! (`assets/logo-field-256.png`) baked offline from `icons/mundus.svg` by
//! the same `field.rs` port — no resvg/SOR work at startup. It is sampled
//! through a linear sampler on UV, so it serves every target size.
//!
//! Failure policy: `MUNDUS_LOGO_GPU=off`, no adapter, device-lost, or any
//! per-frame error parks the renderer for the rest of the session (logged
//! once) and the overlay falls back to `LogoFrames` (the WebP ring).

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use gpui::RenderImage;

mod device;

use device::GpuLogo;

pub(crate) const FIELD_PNG: &[u8] = include_bytes!("../assets/logo-field-256.png");
const STATS_WINDOW: Duration = Duration::from_secs(5);

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

#[derive(Default)]
pub(super) struct Stats {
    window_start: Option<Instant>,
    /// `frame()` calls — one per overlay paint while visible.
    ticks: u32,
    /// Submitted render+copy+map jobs.
    renders: u32,
    /// Submit→harvest latency per completed map, ms.
    times_ms: Vec<f64>,
}

impl Stats {
    pub(super) fn tick(&mut self, submitted: bool) {
        self.window_start.get_or_insert_with(Instant::now);
        self.ticks += 1;
        if submitted {
            self.renders += 1;
        }
    }

    pub(super) fn harvested(&mut self, latency_ms: f64) {
        self.times_ms.push(latency_ms);
        let Some(start) = self.window_start else {
            return;
        };
        if start.elapsed() >= STATS_WINDOW {
            self.times_ms
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let secs = start.elapsed().as_secs_f64();
            eprintln!(
                "logo-gpu: {:.1} renders/s over {:.1} paints/s, submit→harvest median {:.2} ms",
                f64::from(self.renders) / secs,
                f64::from(self.ticks) / secs,
                self.times_ms[self.times_ms.len() / 2],
            );
            *self = Stats::default();
        }
    }
}

#[cfg(test)]
#[path = "logo_gpu_tests.rs"]
mod tests;
