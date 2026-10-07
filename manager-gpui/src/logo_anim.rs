//! KOS-355 (round 3): the update-overlay logo is a pre-baked animated WebP
//! of the Paper-matched liquid-metal Mundus mark — 256×252 canvas, 200
//! frames @ 30 fps (6.67 s seamless loop), lossless, ~8 MiB, checked in at
//! `assets/logo-256-200.webp`. The bake is offline (ffmpeg `libwebp_anim
//! -lossless 1 -loop 0` over frames rendered by the CPU shader port); at
//! runtime we only decode once into a ring of premultiplied BGRA
//! `RenderImage`s and index by elapsed time — ~1 ms/frame decode, no CPU
//! shader in the paint loop, and no first-open bake stall.
//!
//! Loop periodicity: in the shader `t = 0.3 * (t_secs + 2.8)` enters twice —
//! `direction -= t` (periodic via `fract` once `0.3 * period` is an integer,
//! so period = 20/3 s advances it by exactly 2) and a diagonal simplex-noise
//! drift, which the bake makes periodic by crossfading the noise sample with
//! itself one drift-period back, renormalized to unit variance. Reproduce:
//! `anim bake 256 200 6.6666667` then
//! `ffmpeg -framerate 30 -i f%03d.png -c:v libwebp_anim -lossless 1 -loop 0`.

use std::sync::{Arc, OnceLock};

use gpui::RenderImage;
use image::{Frame, ImageBuffer, Rgba};
use smallvec::{smallvec, SmallVec};
use webp_animation::{ColorMode, Decoder, DecoderOptions};

static LOGO_WEBP: &[u8] = include_bytes!("../assets/logo-256-200.webp");

/// Baked playback rate (200 frames → 6.67 s loop).
pub const FPS: f32 = 30.0;

/// The decoded frame ring. libwebp composites each animation frame onto the
/// full canvas; with `ColorMode::Bgra` the bytes are already in the
/// premultiplied BGRA order `RenderImage` expects (the bake premultiplies
/// color by the logo alpha).
pub struct LogoFrames {
    frames: Vec<Arc<RenderImage>>,
}

impl LogoFrames {
    /// Decode-once shared ring. `None` if the embedded asset ever fails to
    /// decode — the overlay falls back to the plain icon.
    pub fn shared() -> Option<Arc<Self>> {
        static CACHE: OnceLock<Option<Arc<LogoFrames>>> = OnceLock::new();
        CACHE.get_or_init(|| Self::decode().map(Arc::new)).clone()
    }

    fn decode() -> Option<Self> {
        let dec = Decoder::new_with_options(
            LOGO_WEBP,
            DecoderOptions {
                use_threads: true,
                color_mode: ColorMode::Bgra,
            },
        )
        .ok()?;
        let mut frames = Vec::new();
        for f in dec.into_iter() {
            let (w, h) = f.dimensions();
            let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
                ImageBuffer::from_raw(w, h, f.data().to_vec())?;
            let data: SmallVec<[Frame; 1]> = smallvec![Frame::new(img)];
            frames.push(Arc::new(RenderImage::new(data)));
        }
        if frames.is_empty() {
            None
        } else {
            Some(Self { frames })
        }
    }

    /// Frame for `elapsed` seconds since the overlay appeared; wraps on the
    /// 6.67 s loop.
    pub fn frame(&self, elapsed: f32) -> Arc<RenderImage> {
        let i = (elapsed.max(0.0) * FPS) as usize % self.frames.len();
        self.frames[i].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_webp_decodes_to_full_ring() {
        let frames = LogoFrames::decode().expect("embedded WebP must decode");
        assert_eq!(frames.frames.len(), 200);
        let size = frames.frame(0.0).size(0);
        assert_eq!((i32::from(size.width), i32::from(size.height)), (256, 252));
        // Indexing wraps on the 6.67 s loop.
        assert!(Arc::ptr_eq(&frames.frame(0.0), &frames.frame(200.0 / FPS)));
    }
}
