//! The curves and bumps the scan, the working light and the beam share.

use std::f32::consts::PI;

use super::consts::WIPE;

pub(super) fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn bump(d: f32, w: f32) -> f32 {
    (-(d / w) * (d / w)).exp()
}

/// From `start` moving at `speed` to rest at `end` as `t` goes 0 to 1 over
/// [`WIPE`]: a cubic Hermite, so done picks up the motion it interrupts.
pub(super) fn hermite(start: f32, speed: f32, end: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * start
        + (t3 - 2.0 * t2 + t) * WIPE * speed
        + (3.0 * t2 - 2.0 * t3) * end
}

pub(super) fn pass_position(p: f32) -> f32 {
    0.5 - 0.5 * (PI * p.clamp(0.0, 1.0)).cos()
}
