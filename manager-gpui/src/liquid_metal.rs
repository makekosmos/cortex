//! CPU port of Paper's `liquid-metal` fragment shader
//! (`@paper-design/shaders`, `packages/shaders/src/shaders/liquid-metal.ts`),
//! restricted to the `u_isImage = true` path. Field preprocessing lives in
//! `liquid_metal_field.rs`; this file evaluates the fragment math per pixel
//! each frame and paints it as a `RenderImage` clipped to the logo alpha.
//!
//! Uniforms follow the upstream `defaultPreset` for image mode:
//! colorBack transparent, colorTint #ffffff, distortion 0.07, repetition 2.0,
//! shiftRed 0.3, shiftBlue 0.3, contour 0.4, softness 0.1, angle 70, speed 1.

use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, ImageBuffer, Rgba};
use smallvec::{smallvec, SmallVec};

#[path = "liquid_metal_field.rs"]
mod field;

use field::bilinear;
pub use field::LogoField;

/// Default preset uniforms (image mode).
const DISTORTION: f32 = 0.07;
const REPETITION: f32 = 2.0;
const SHIFT_RED: f32 = 0.3;
const SHIFT_BLUE: f32 = 0.3;
const CONTOUR: f32 = 0.4;
const SOFTNESS: f32 = 0.1;
const ANGLE_DEG: f32 = 70.0;
/// `u_resolution` drives only `smallCanvasT`, which fades to zero above a
/// 500px canvas. The Paper demo renders on a full-size canvas, so the term
/// vanishes; the overlay window is likewise always > 500px.
const CANVAS_MIN_PX: f32 = 1024.0;

// --- GLSL helpers ------------------------------------------------------------

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// 2D simplex noise — direct port of `simplexNoise` in shader-utils.ts.
fn snoise(v: [f32; 2]) -> f32 {
    const C: [f32; 4] = [0.211_324_87, 0.366_025_42, -0.577_350_26, 0.024_390_243];
    fn permute(x: f32) -> f32 {
        let x = (x * 34.0 + 1.0) * x;
        x - (x / 289.0).floor() * 289.0
    }
    let s = (v[0] + v[1]) * C[1];
    let i = [(v[0] + s).floor(), (v[1] + s).floor()];
    let t = (i[0] + i[1]) * C[0];
    let x0 = [v[0] - i[0] + t, v[1] - i[1] + t];
    let i1 = if x0[0] > x0[1] {
        [1.0, 0.0]
    } else {
        [0.0, 1.0]
    };
    let x12 = [x0[0] + C[0] - i1[0], x0[1] + C[0] - i1[1]];
    let x3 = [x0[0] + C[2], x0[1] + C[2]];
    let im = [
        i[0] - (i[0] / 289.0).floor() * 289.0,
        i[1] - (i[1] / 289.0).floor() * 289.0,
    ];
    let p = [
        permute(permute(im[1]) + im[0]),
        permute(permute(im[1] + i1[1]) + im[0] + i1[0]),
        permute(permute(im[1] + 1.0) + im[0] + 1.0),
    ];
    let mut m = [
        (0.5 - (x0[0] * x0[0] + x0[1] * x0[1])).max(0.0),
        (0.5 - (x12[0] * x12[0] + x12[1] * x12[1])).max(0.0),
        (0.5 - (x3[0] * x3[0] + x3[1] * x3[1])).max(0.0),
    ];
    for v in &mut m {
        *v *= *v;
        *v *= *v;
    }
    let x = [
        2.0 * fract(p[0] * C[3]) - 1.0,
        2.0 * fract(p[1] * C[3]) - 1.0,
        2.0 * fract(p[2] * C[3]) - 1.0,
    ];
    let h = [x[0].abs() - 0.5, x[1].abs() - 0.5, x[2].abs() - 0.5];
    let ox = [
        (x[0] + 0.5).floor(),
        (x[1] + 0.5).floor(),
        (x[2] + 0.5).floor(),
    ];
    let a0 = [x[0] - ox[0], x[1] - ox[1], x[2] - ox[2]];
    for k in 0..3 {
        m[k] *= 1.792_842_9 - 0.853_734_73 * (a0[k] * a0[k] + h[k] * h[k]);
    }
    let g = [
        a0[0] * x0[0] + h[0] * x0[1],
        a0[1] * x12[0] + h[1] * x12[1],
        a0[2] * x3[0] + h[2] * x3[1],
    ];
    130.0 * (m[0] * g[0] + m[1] * g[1] + m[2] * g[2])
}

/// `getColorChanges`: stripe coloring with borders, bump modulation and
/// color-burn tint (u_colorTint = white → identity).
#[allow(clippy::too_many_arguments)]
fn color_changes(
    c1: f32,
    c2: f32,
    stripe_p: f32,
    w: [f32; 3],
    blur: f32,
    bump: f32,
    tint: f32,
) -> f32 {
    let mut ch = c2 + (c1 - c2) * smoothstep(0.0, 2.0 * blur, stripe_p);
    let mut border = w[0];
    ch += (c2 - ch) * smoothstep(border, border + 2.0 * blur, stripe_p);
    let bump = smoothstep(0.2, 0.8, bump);
    border = w[0] + 0.4 * (1.0 - bump) * w[1];
    ch += (c1 - ch) * smoothstep(border, border + 2.0 * blur, stripe_p);
    border = w[0] + 0.5 * (1.0 - bump) * w[1];
    ch += (c2 - ch) * smoothstep(border, border + 2.0 * blur, stripe_p);
    border = w[0] + w[1];
    ch += (c1 - ch) * smoothstep(border, border + 2.0 * blur, stripe_p);
    let gradient_t = (stripe_p - w[0] - w[1]) / w[2];
    let gradient = c1 + (c2 - c1) * smoothstep(0.0, 1.0, gradient_t);
    ch += (gradient - ch) * smoothstep(border, border + 0.5 * blur, stripe_p);
    // u_colorTint.a = 1 → color burn with the tint channel.
    ch = 1.0 - ((1.0 - ch) / tint.max(0.0001)).min(1.0);
    ch.clamp(0.0, 1.0)
}

/// `blurEdge3x3`: tent filter over the edge field at ±`radius` texels,
/// bilinear-sampled like `textureGrad`.
fn blur_edge3x3(field: &LogoField, uv: [f32; 2], radius: f32, center: f32) -> f32 {
    let (w, h) = (field.width, field.height);
    let texel = [1.0 / w as f32, 1.0 / h as f32];
    let s = |du: f32, dv: f32| {
        bilinear(
            &field.edge,
            w,
            h,
            (uv[0] + du * texel[0]).clamp(0.0, 1.0),
            (uv[1] + dv * texel[1]).clamp(0.0, 1.0),
        )
    };
    let mut sum = 4.0 * center;
    sum += 2.0 * (s(0.0, -radius) + s(0.0, radius) + s(-radius, 0.0) + s(radius, 0.0));
    sum += s(-radius, -radius) + s(-radius, radius) + s(radius, -radius) + s(radius, radius);
    sum / 16.0
}

/// Per-pixel intermediates computed in pass 1 of `frame`: the three stripe
/// phases plus everything `getColorChanges` still needs. Storing them lets
/// pass 2 derive a real `fwidth(stripe)` from neighbor differences — the
/// term upstream gets for free from the GPU derivative units, and the term
/// that turns stripe edges into the wide soft ramps of the demo.
#[derive(Clone, Copy, Default)]
struct StripeParams {
    stripes: [f32; 3],
    w: [f32; 3],
    blur: f32,
    r_extra: f32,
    g_extra: f32,
    bump: f32,
    diag_tl_br: f32,
}

/// Pass 1 of the ported fragment shader: everything up to the per-channel
/// `getColorChanges` calls. Evaluated for every texel, not just opaque ones —
/// the shader runs on the whole canvas and neighbors feed `fwidth`.
fn stripe_params(field: &LogoField, x: usize, y: usize, t_secs: f32) -> StripeParams {
    let (w, h) = (field.width, field.height);

    let t = 0.3 * (t_secs + 2.8);
    let uv = [(x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32];
    let cycle_width = REPETITION;

    let edge_raw = bilinear(&field.edge, w, h, uv[0], uv[1]);
    let mut edge = blur_edge3x3(field, uv, 6.0, edge_raw).powf(1.6);
    edge *= smoothstep(0.0, 0.4, CONTOUR); // contour ≤ .4 range: edge hardness

    let angle = (-ANGLE_DEG + 70.0) * std::f32::consts::PI / 180.0;
    let (cos_a, sin_a) = (angle.cos(), angle.sin());
    let rx = uv[0] - 0.5;
    let ry = uv[1] - 0.5;
    let rotated_uv = [rx * cos_a - ry * sin_a + 0.5, rx * sin_a + ry * cos_a + 0.5];

    let diag_bl_tr = rotated_uv[0] - rotated_uv[1];

    let grad = [uv[0] - 0.5, uv[1] - 0.5];
    let dist = ((grad[0]) * (grad[0])
        + (grad[1] + 0.2 * diag_bl_tr) * (grad[1] + 0.2 * diag_bl_tr))
        .sqrt();
    // rotate(grad_uv, rot).x = cos·x − sin·y (GLSL mat2 is column-major).
    let rot = (0.25 - 0.2 * diag_bl_tr) * std::f32::consts::PI;
    let (rc, rs) = (rot.cos(), rot.sin());
    let mut direction = grad[0] * rc - grad[1] * rs;

    let mut bump = 1.0 - (1.8 * dist).powf(1.2);
    bump *= uv[1].powf(0.3);

    let thin1_ratio = 0.12 / cycle_width * (1.0 - 0.4 * bump);
    let thin2_ratio = 0.07 / cycle_width * (1.0 + 0.4 * bump);
    let wide_ratio = 1.0 - thin1_ratio - thin2_ratio;
    let mut w_vec = [
        cycle_width * thin1_ratio,
        cycle_width * thin2_ratio,
        wide_ratio,
    ];

    let noise = snoise([uv[0] - t, uv[1] - t]);
    edge += (1.0 - edge) * DISTORTION * noise;

    direction += diag_bl_tr;
    let e_smooth = smoothstep(0.0, 1.0, edge);
    direction -= 2.0 * noise * diag_bl_tr * e_smooth * (1.0 - e_smooth);
    direction *= 1.0 - edge * smoothstep(0.5, 1.0, CONTOUR);
    direction -= 1.7 * edge * smoothstep(0.5, 1.0, CONTOUR);
    direction += 0.2 * CONTOUR.powi(4) * (1.0 - e_smooth);

    bump *= uv[1].powf(0.1).clamp(0.3, 1.0);
    direction *= 0.1 + (1.1 - edge) * bump;
    direction *= 0.4 + 0.6 * (1.0 - smoothstep(0.5, 1.0, edge));
    direction += 0.18 * (smoothstep(0.1, 0.2, uv[1]) * (1.0 - smoothstep(0.2, 0.4, uv[1])));
    direction +=
        0.03 * (smoothstep(0.1, 0.2, 1.0 - uv[1]) * (1.0 - smoothstep(0.2, 0.4, 1.0 - uv[1])));
    direction *= 0.5 + 0.5 * uv[1] * uv[1];
    direction *= cycle_width;
    direction -= t;

    let color_dispersion = (1.0 - bump).clamp(0.0, 1.0);
    let mut dispersion_red = color_dispersion;
    dispersion_red += 0.03 * bump * noise;
    dispersion_red += 5.0
        * (smoothstep(-0.1, 0.2, uv[1]) * (1.0 - smoothstep(0.1, 0.5, uv[1])))
        * (smoothstep(0.4, 0.6, bump) * (1.0 - smoothstep(0.4, 1.0, bump)));
    dispersion_red -= diag_bl_tr;

    let mut dispersion_blue = color_dispersion * 1.3;
    dispersion_blue += (smoothstep(0.0, 0.4, uv[1]) * (1.0 - smoothstep(0.1, 0.8, uv[1])))
        * (smoothstep(0.4, 0.6, bump) * (1.0 - smoothstep(0.4, 0.8, bump)));
    dispersion_blue -= 0.2 * edge;
    dispersion_red *= SHIFT_RED / 20.0;
    dispersion_blue *= SHIFT_BLUE / 20.0;

    let softness = 0.05 * SOFTNESS;
    let mut blur = softness + 0.5 * smoothstep(1.0, 10.0, REPETITION) * smoothstep(0.0, 1.0, edge);
    let small_canvas_t = 1.0 - smoothstep(100.0, 500.0, CANVAS_MIN_PX);
    blur += small_canvas_t * smoothstep(0.0, 1.0, edge);
    let r_extra_blur = softness * (0.05 + 0.1 * (SHIFT_RED / 20.0) * bump);
    let g_extra_blur = softness * 0.05 / (1.0 - diag_bl_tr).abs().max(0.001);

    w_vec[1] -= 0.02 * smoothstep(0.0, 1.0, edge + bump);

    StripeParams {
        stripes: [
            fract(direction + dispersion_red),
            fract(direction),
            fract(direction - dispersion_blue),
        ],
        w: w_vec,
        blur,
        r_extra: r_extra_blur,
        g_extra: g_extra_blur,
        bump,
        diag_tl_br: rotated_uv[0] + rotated_uv[1],
    }
}

/// GLSL `fwidth(v)` = |∂v/∂x| + |∂v/∂y|, central differences on the fract'ed
/// stripe phase — crucially ~1 at wrap lines, which is what softens them.
fn stripe_fwidth(
    stripes: &[StripeParams],
    w: usize,
    h: usize,
    x: usize,
    y: usize,
    ch: usize,
) -> f32 {
    let xm = x.saturating_sub(1);
    let xp = (x + 1).min(w - 1);
    let ym = y.saturating_sub(1);
    let yp = (y + 1).min(h - 1);
    let dx = (stripes[y * w + xp].stripes[ch] - stripes[y * w + xm].stripes[ch]).abs()
        / (xp - xm) as f32;
    let dy = (stripes[yp * w + x].stripes[ch] - stripes[ym * w + x].stripes[ch]).abs()
        / (yp - ym) as f32;
    dx + dy
}

/// `colorBandingFix` dither — ±1/512 hash noise before premultiplying.
fn banding_dither(x: usize, y: usize) -> f32 {
    let s = (x as f32 * 0.014 * 12.9898 + y as f32 * 0.014 * 78.233).sin() * 43_758.547;
    (fract(s) - 0.5) / 256.0
}

/// Render one animation frame of the liquid-metal logo as a GPUI image.
/// `t_secs` is seconds since the overlay appeared.
pub fn frame(field: &LogoField, t_secs: f32) -> Arc<RenderImage> {
    let (w, h) = (field.width, field.height);
    let mut params = vec![StripeParams::default(); w * h];
    for y in 0..h {
        for x in 0..w {
            params[y * w + x] = stripe_params(field, x, y, t_secs);
        }
    }

    let color1 = [0.98f32, 0.98, 1.0];
    let mut buf = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let opacity = field.alpha[i];
            if opacity <= 0.0 {
                continue;
            }
            let p = params[i];
            let color2 = [0.1, 0.1, 0.1 + 0.1 * smoothstep(0.7, 1.3, p.diag_tl_br)];

            let fw_r = stripe_fwidth(&params, w, h, x, y, 0);
            let fw_g = stripe_fwidth(&params, w, h, x, y, 1);
            let fw_b = stripe_fwidth(&params, w, h, x, y, 2);
            let r = color_changes(
                color1[0],
                color2[0],
                p.stripes[0],
                p.w,
                p.blur + fw_r + p.r_extra,
                p.bump,
                1.0,
            );
            let g = color_changes(
                color1[1],
                color2[1],
                p.stripes[1],
                p.w,
                p.blur + fw_g + p.g_extra,
                p.bump,
                1.0,
            );
            let b = color_changes(
                color1[2],
                color2[2],
                p.stripes[2],
                p.w,
                p.blur + fw_b,
                p.bump,
                1.0,
            );
            let d = banding_dither(x, y);
            let (r, g, b) = (
                (r + d).clamp(0.0, 1.0),
                (g + d).clamp(0.0, 1.0),
                (b + d).clamp(0.0, 1.0),
            );
            // Premultiplied output, BGRA byte order for RenderImage.
            buf[i * 4..i * 4 + 4].copy_from_slice(&[
                (b * opacity * 255.0) as u8,
                (g * opacity * 255.0) as u8,
                (r * opacity * 255.0) as u8,
                (opacity * 255.0) as u8,
            ]);
        }
    }
    let image: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(w as u32, h as u32, buf).expect("liquid-metal frame buffer size");
    let data: SmallVec<[Frame; 1]> = smallvec![Frame::new(image)];
    Arc::new(RenderImage::new(data))
}
