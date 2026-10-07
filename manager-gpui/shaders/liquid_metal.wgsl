// WGSL port of the Paper liquid-metal fragment shader (image mode),
// mirroring manager-gpui/src/liquid_metal.rs and perf/src/cpu.rs.
// field_tex: R = edge field (1 at rim), G = shape alpha. Linear sampler.

struct Uniforms {
    t_secs: f32,
    _pad: vec3<f32>,
    _pad2: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var field_tex: texture_2d<f32>;
@group(0) @binding(2) var field_samp: sampler;

const DISTORTION: f32 = 0.07;
const REPETITION: f32 = 2.0;
const SHIFT_RED: f32 = 0.3;
const SHIFT_BLUE: f32 = 0.3;
const CONTOUR: f32 = 0.4;
const SOFTNESS: f32 = 0.1;
const ANGLE_DEG: f32 = 70.0;
const CANVAS_MIN_PX: f32 = 1024.0;

fn ssmoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn permute(x: f32) -> f32 {
    let y = (x * 34.0 + 1.0) * x;
    return y - floor(y / 289.0) * 289.0;
}

fn snoise(v: vec2<f32>) -> f32 {
    let C = vec4<f32>(0.21132487, 0.36602542, -0.57735026, 0.024390243);
    let s = (v.x + v.y) * C.y;
    let i = floor(v + vec2<f32>(s));
    let t = (i.x + i.y) * C.x;
    let x0 = v - i + vec2<f32>(t);
    var i1: vec2<f32>;
    if x0.x > x0.y { i1 = vec2<f32>(1.0, 0.0); } else { i1 = vec2<f32>(0.0, 1.0); }
    let x12 = x0 + vec2<f32>(C.x) - i1;
    let x3 = vec2<f32>(x0.x + C.z, x0.y + C.z);
    let im = i - floor(i / 289.0) * 289.0;
    let p = vec3<f32>(
        permute(permute(im.y) + im.x),
        permute(permute(im.y + i1.y) + im.x + i1.x),
        permute(permute(im.y + 1.0) + im.x + 1.0),
    );
    var m = vec3<f32>(
        max(0.5 - dot(x0, x0), 0.0),
        max(0.5 - dot(x12, x12), 0.0),
        max(0.5 - dot(x3, x3), 0.0),
    );
    m = m * m;
    m = m * m;
    let x = 2.0 * fract3(p * vec3<f32>(C.w)) - vec3<f32>(1.0);
    let h = abs(x) - vec3<f32>(0.5);
    let ox = floor(x + vec3<f32>(0.5));
    let a0 = x - ox;
    m = m * (vec3<f32>(1.7928429) - 0.85373473 * (a0 * a0 + h * h));
    let g = vec3<f32>(a0.x * x0.x + h.x * x0.y, a0.y * x12.x + h.y * x12.y, a0.z * x3.x + h.z * x3.y);
    return 130.0 * dot(m, g);
}

fn fract3(v: vec3<f32>) -> vec3<f32> {
    return v - floor(v);
}

fn color_changes(c1: f32, c2: f32, stripe_p: f32, w: vec3<f32>, blur: f32, bump_in: f32, tint: f32) -> f32 {
    var ch = c2 + (c1 - c2) * ssmoothstep(0.0, 2.0 * blur, stripe_p);
    var border = w.x;
    ch += (c2 - ch) * ssmoothstep(border, border + 2.0 * blur, stripe_p);
    let bump = ssmoothstep(0.2, 0.8, bump_in);
    border = w.x + 0.4 * (1.0 - bump) * w.y;
    ch += (c1 - ch) * ssmoothstep(border, border + 2.0 * blur, stripe_p);
    border = w.x + 0.5 * (1.0 - bump) * w.y;
    ch += (c2 - ch) * ssmoothstep(border, border + 2.0 * blur, stripe_p);
    border = w.x + w.y;
    ch += (c1 - ch) * ssmoothstep(border, border + 2.0 * blur, stripe_p);
    let gradient_t = (stripe_p - w.x - w.y) / w.z;
    let gradient = c1 + (c2 - c1) * ssmoothstep(0.0, 1.0, gradient_t);
    ch += (gradient - ch) * ssmoothstep(border, border + 0.5 * blur, stripe_p);
    ch = 1.0 - min((1.0 - ch) / max(tint, 0.0001), 1.0);
    return clamp(ch, 0.0, 1.0);
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var out: VsOut;
    let xy = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    out.pos = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

fn edge_at(uv: vec2<f32>) -> f32 {
    return textureSample(field_tex, field_samp, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0))).r;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(field_tex));
    let uv = in.pos.xy / dims;
    let texel = 1.0 / dims;
    let t = 0.3 * (u.t_secs + 2.8);
    let cycle_width = REPETITION;

    var edge = edge_at(uv);
    var sum = 4.0 * edge;
    let r = 6.0 * texel;
    sum += 2.0 * (edge_at(uv + vec2<f32>(0.0, -r.y)) + edge_at(uv + vec2<f32>(0.0, r.y))
                + edge_at(uv + vec2<f32>(-r.x, 0.0)) + edge_at(uv + vec2<f32>(r.x, 0.0)));
    sum += edge_at(uv + vec2<f32>(-r.x, -r.y)) + edge_at(uv + vec2<f32>(-r.x, r.y))
         + edge_at(uv + vec2<f32>(r.x, -r.y)) + edge_at(uv + vec2<f32>(r.x, r.y));
    edge = pow(sum / 16.0, 1.6);
    edge *= ssmoothstep(0.0, 0.4, CONTOUR);

    let angle = (-ANGLE_DEG + 70.0) * 3.14159265 / 180.0;
    let ca = cos(angle);
    let sa = sin(angle);
    let rx = uv.x - 0.5;
    let ry = uv.y - 0.5;
    let rotated_uv = vec2<f32>(rx * ca - ry * sa + 0.5, rx * sa + ry * ca + 0.5);
    let diag_bl_tr = rotated_uv.x - rotated_uv.y;

    let grad = vec2<f32>(uv.x - 0.5, uv.y - 0.5);
    let gyd = grad.y + 0.2 * diag_bl_tr;
    let dist = sqrt(grad.x * grad.x + gyd * gyd);
    let rot = (0.25 - 0.2 * diag_bl_tr) * 3.14159265;
    var direction = grad.x * cos(rot) - grad.y * sin(rot);

    var bump = 1.0 - pow(1.8 * dist, 1.2);
    bump *= pow(uv.y, 0.3);

    let thin1_ratio = 0.12 / cycle_width * (1.0 - 0.4 * bump);
    let thin2_ratio = 0.07 / cycle_width * (1.0 + 0.4 * bump);
    var w_vec = vec3<f32>(cycle_width * thin1_ratio, cycle_width * thin2_ratio,
                          1.0 - thin1_ratio - thin2_ratio);

    let noise = snoise(vec2<f32>(uv.x - t, uv.y - t));
    edge += (1.0 - edge) * DISTORTION * noise;

    direction += diag_bl_tr;
    let e_smooth = ssmoothstep(0.0, 1.0, edge);
    direction -= 2.0 * noise * diag_bl_tr * e_smooth * (1.0 - e_smooth);
    direction *= 1.0 - edge * ssmoothstep(0.5, 1.0, CONTOUR);
    direction -= 1.7 * edge * ssmoothstep(0.5, 1.0, CONTOUR);
    direction += 0.2 * pow(CONTOUR, 4.0) * (1.0 - e_smooth);

    bump *= clamp(pow(uv.y, 0.1), 0.3, 1.0);
    direction *= 0.1 + (1.1 - edge) * bump;
    direction *= 0.4 + 0.6 * (1.0 - ssmoothstep(0.5, 1.0, edge));
    direction += 0.18 * (ssmoothstep(0.1, 0.2, uv.y) * (1.0 - ssmoothstep(0.2, 0.4, uv.y)));
    direction += 0.03 * (ssmoothstep(0.1, 0.2, 1.0 - uv.y) * (1.0 - ssmoothstep(0.2, 0.4, 1.0 - uv.y)));
    direction *= 0.5 + 0.5 * uv.y * uv.y;
    direction *= cycle_width;
    direction -= t;

    let color_dispersion = clamp(1.0 - bump, 0.0, 1.0);
    var dispersion_red = color_dispersion;
    dispersion_red += 0.03 * bump * noise;
    dispersion_red += 5.0
        * (ssmoothstep(-0.1, 0.2, uv.y) * (1.0 - ssmoothstep(0.1, 0.5, uv.y)))
        * (ssmoothstep(0.4, 0.6, bump) * (1.0 - ssmoothstep(0.4, 1.0, bump)));
    dispersion_red -= diag_bl_tr;

    var dispersion_blue = color_dispersion * 1.3;
    dispersion_blue += (ssmoothstep(0.0, 0.4, uv.y) * (1.0 - ssmoothstep(0.1, 0.8, uv.y)))
        * (ssmoothstep(0.4, 0.6, bump) * (1.0 - ssmoothstep(0.4, 0.8, bump)));
    dispersion_blue -= 0.2 * edge;
    dispersion_red *= SHIFT_RED / 20.0;
    dispersion_blue *= SHIFT_BLUE / 20.0;

    let softness = 0.05 * SOFTNESS;
    var blur = softness + 0.5 * ssmoothstep(1.0, 10.0, REPETITION) * ssmoothstep(0.0, 1.0, edge);
    let small_canvas_t = 1.0 - ssmoothstep(100.0, 500.0, CANVAS_MIN_PX);
    blur += small_canvas_t * ssmoothstep(0.0, 1.0, edge);
    let r_extra = softness * (0.05 + 0.1 * (SHIFT_RED / 20.0) * bump);
    let g_extra = softness * 0.05 / max(abs(1.0 - diag_bl_tr), 0.001);

    w_vec.y -= 0.02 * ssmoothstep(0.0, 1.0, edge + bump);

    let stripes = vec3<f32>(
        fract(direction + dispersion_red),
        fract(direction),
        fract(direction - dispersion_blue),
    );

    // GPU derivative units give fwidth for free; halve to match the CPU
    // central-difference scale.
    let fw = vec3<f32>(fwidth(stripes.x), fwidth(stripes.y), fwidth(stripes.z)) * 0.5;

    let diag_tl_br = rotated_uv.x + rotated_uv.y;
    let color1 = vec3<f32>(0.98, 0.98, 1.0);
    let color2 = vec3<f32>(0.1, 0.1, 0.1 + 0.1 * ssmoothstep(0.7, 1.3, diag_tl_br));

    let rr = color_changes(color1.x, color2.x, stripes.x, w_vec, blur + fw.x + r_extra, bump, 1.0);
    let gg = color_changes(color1.y, color2.y, stripes.y, w_vec, blur + fw.y + g_extra, bump, 1.0);
    let bb = color_changes(color1.z, color2.z, stripes.z, w_vec, blur + fw.z, bump, 1.0);

    // Banding dither (same hash as the CPU port).
    let ds = sin(in.pos.x * 0.014 * 12.9898 + in.pos.y * 0.014 * 78.233) * 43758.547;
    let d = (fract(ds) - 0.5) / 256.0;

    let opacity = textureSample(field_tex, field_samp, uv).g;
    let rgb = clamp(vec3<f32>(rr + d, gg + d, bb + d), vec3<f32>(0.0), vec3<f32>(1.0));
    // Premultiplied BGRA — the byte order GPUI's RenderImage expects.
    return vec4<f32>(rgb.b * opacity, rgb.g * opacity, rgb.r * opacity, opacity);
}
