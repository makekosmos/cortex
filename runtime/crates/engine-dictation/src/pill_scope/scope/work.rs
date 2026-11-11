//! The light the processing looks lay along the plate — Flow's bands and
//! Pulse's rings — and the envelope that tells the scan how to draw it.

use super::consts::RING_BIRTH;
use super::geometry::{bump, smoothstep};

/// Flow's bands or Pulse's rings, as light along the plate.
#[derive(Clone, Copy)]
pub(super) enum Working {
    /// Bands `spacing` apart, one at `offset`.
    Flow { offset: f32, spacing: f32 },
    /// Rings leaving the middle every `period` at `speed`, `time` in.
    Pulse { time: f32, period: f32, speed: f32 },
}

impl Working {
    /// 0–1 light at `x`, and how far toward the plate's ends it fades.
    pub(super) fn light(self, x: f32, half_width: f32) -> f32 {
        match self {
            Working::Flow { offset, spacing } => {
                let d = (x - offset + spacing / 2.0).rem_euclid(spacing) - spacing / 2.0;
                bump(d, 7.0)
            }
            Working::Pulse {
                time,
                period,
                speed,
            } => {
                let d = x.abs();
                let mut light = 0.0;
                let mut age = time.rem_euclid(period);
                while age * speed < half_width + 12.0 && age <= time + 1e-4 {
                    let radius = age * speed;
                    // A ring swells in as it leaves the middle, never pops.
                    let fade = smoothstep(0.0, RING_BIRTH, age)
                        * (1.0 - 0.6 * smoothstep(0.3 * half_width, half_width, radius));
                    light += bump(d - radius, 6.0) * fade;
                    age += period;
                }
                light.min(1.0)
            }
        }
    }
}

/// How the trace is drawn this instant: the scanner that unfolds it, the
/// processing beam or the working light, or the done wipe.
pub(super) enum Envelope {
    Enter {
        reach: f32,
        head: f32,
        unfold: f32,
    },
    /// Sweep: the beam over the ironed trace.
    Process {
        iron: f32,
        front: f32,
        head: f32,
    },
    /// Flow and Pulse: light travels through the turning braid.
    Work {
        working: Working,
        head: f32,
        swell: f32,
        half_width: f32,
    },
    /// Done, for every look: a front clears the field from the middle
    /// out as it collapses, while the light ahead of it fades. Sweep's
    /// ironed field is already dark (`field` 0), so only the fronts show,
    /// and its beam fades out where it was (`beam` at `beam_at`).
    Clear {
        front: f32,
        head: f32,
        beam: f32,
        beam_at: f32,
        field: f32,
        working: Working,
        glow: f32,
        swell: f32,
        half_width: f32,
    },
}
