//! Preprocessing half of the `liquid-metal` port (see `liquid_metal.rs`):
//! builds the virtual `u_image` texture exactly like upstream
//! `toProcessedLiquidMetal` — rasterize the alpha mask, run the deliberately
//! under-converged Poisson solve at the 512px working size, then upscale
//! bilinearly (R = 1 − normalized edge height, so the rim reads 1 and the
//! interior 0; background texels are R=1, G=0).

use std::sync::Arc;

use gpui::SvgRenderer;

const SVG: &[u8] = include_bytes!("../assets/icons/mundus.svg");
/// Texture resolution: width in device pixels of the virtual `u_image`
/// texture. The logo is displayed at 200px, so the GPU-side image element
/// downscales slightly — matching the site's smooth sampling.
const TEX_W: f32 = 256.0;
/// `render_single_frame` internally multiplies by SMOOTH_SVG_SCALE_FACTOR(2)
/// over the 854×841 viewBox, so scale = desired_px / 854 / 2.
fn svg_scale(px_width: f32) -> f32 {
    px_width / 854.0 / 2.0
}
/// `toProcessedLiquidMetal` solves the Poisson field on a working grid whose
/// smaller side is this many px, then upscales with smoothing.
const WORK_MIN: f32 = 512.0;
/// Upstream solver parameters (`POISSON_CONFIG_OPTIMIZED`): the solve is
/// intentionally stopped early — iteration count controls how far the edge
/// gradient penetrates the shape. Running to convergence (as r2 did) pushes
/// the rim band deep into the petals and reads as blotchy interior spots.
const POISSON_ITERS: usize = 40;
const SOR_OMEGA: f32 = 1.9;
const POISSON_C: f32 = 0.01;

/// Preprocessed shader input: per-pixel R (edge field, 1 at the rim) and
/// G (opacity) channels of the virtual `u_image` texture for the logo shape.
pub struct LogoField {
    pub width: usize,
    pub height: usize,
    /// 1 - normalized Poisson edge-height field (1 at the boundary, 0 deep
    /// inside, 1 on background texels — matching the processed PNG).
    pub edge: Vec<f32>,
    /// Shape opacity 0..1 (antialiased mask edges).
    pub alpha: Vec<f32>,
}

fn rasterize_alpha(renderer: &SvgRenderer, px_width: f32) -> Option<(Vec<f32>, usize, usize)> {
    let image = renderer
        .render_single_frame(SVG, svg_scale(px_width))
        .ok()?;
    let size = image.size(0);
    let (w, h) = (size.width.0.max(0) as usize, size.height.0.max(0) as usize);
    if w < 8 || h < 8 {
        return None;
    }
    let bytes = image.as_bytes(0)?;
    // BGRA premultiplied: byte 3 is the coverage alpha.
    let alpha: Vec<f32> = (0..w * h)
        .map(|i| bytes[i * 4 + 3] as f32 / 255.0)
        .collect();
    Some((alpha, w, h))
}

/// Bilinear sample of `src` at normalized uv (textureGrad-style filtering).
pub(crate) fn bilinear(src: &[f32], w: usize, h: usize, u: f32, v: f32) -> f32 {
    let x = (u * w as f32 - 0.5).max(0.0);
    let y = (v * h as f32 - 0.5).max(0.0);
    let x0 = (x.floor() as usize).min(w - 1);
    let y0 = (y.floor() as usize).min(h - 1);
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = (x - x0 as f32).min(1.0);
    let fy = (y - y0 as f32).min(1.0);
    let a = src[y0 * w + x0] * (1.0 - fx) + src[y0 * w + x1] * fx;
    let b = src[y1 * w + x0] * (1.0 - fx) + src[y1 * w + x1] * fx;
    a * (1.0 - fy) + b * fy
}

impl LogoField {
    /// One-time preprocessing matching `toProcessedLiquidMetal`: mask +
    /// Poisson solve at the 512px working size, bilinear upscale to the
    /// texture grid, alpha re-applied at full resolution.
    pub fn build(renderer: &SvgRenderer) -> Option<Arc<Self>> {
        let (alpha, w, h) = rasterize_alpha(renderer, TEX_W)?;

        // Working grid: rasterize the SVG again with the smaller side at
        // WORK_MIN px (viewBox is 854×841, so width = WORK_MIN·854/841).
        let (work_alpha, ww, wh) = rasterize_alpha(renderer, WORK_MIN * 854.0 / 841.0)?;
        let edge_work = poisson_edge_field(&work_alpha, ww, wh);

        // Upscale the field onto the texture grid (ctx.drawImage smoothing).
        let mut edge = vec![1.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let u = (x as f32 + 0.5) / w as f32;
                let v = (y as f32 + 0.5) / h as f32;
                edge[y * w + x] = bilinear(&edge_work, ww, wh, u, v);
            }
        }
        Some(Arc::new(Self {
            width: w,
            height: h,
            edge,
            alpha,
        }))
    }
}

/// Port of the preprocessing solve in `toProcessedLiquidMetal`: any nonzero
/// alpha is a shape pixel; shape pixels touching a non-shape 8-neighbor (or
/// the grid border) are boundary and pinned to u=0; the interior relaxes
/// ∇²u = −C for only `POISSON_ITERS` SOR sweeps (ω=1.9). Output is
/// `1 − u/max` inside the shape and 1.0 on background — the R channel of
/// the processed PNG.
fn poisson_edge_field(alpha: &[f32], w: usize, h: usize) -> Vec<f32> {
    let shape: Vec<bool> = alpha.iter().map(|&a| a > 0.002).collect();
    let mut u = vec![0.0f32; w * h];
    let mut interior = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !shape[i] {
                continue;
            }
            let boundary = x == 0
                || x == w - 1
                || y == 0
                || y == h - 1
                || !shape[i - 1]
                || !shape[i + 1]
                || !shape[i - w]
                || !shape[i + w]
                || !shape[i - w - 1]
                || !shape[i - w + 1]
                || !shape[i + w - 1]
                || !shape[i + w + 1];
            if !boundary {
                interior.push(i);
            }
        }
    }
    for _ in 0..POISSON_ITERS {
        for &i in &interior {
            let n = u[i - w] + u[i + w] + u[i - 1] + u[i + 1];
            let gs = (POISSON_C + n) * 0.25;
            u[i] = SOR_OMEGA * gs + (1.0 - SOR_OMEGA) * u[i];
        }
    }
    let max = interior.iter().map(|&i| u[i]).fold(0.0f32, f32::max);
    let inv = if max > 0.0 { 1.0 / max } else { 0.0 };
    let mut edge = vec![1.0f32; w * h];
    for (i, &s) in shape.iter().enumerate() {
        if s {
            edge[i] = 1.0 - u[i] * inv;
        }
    }
    edge
}
