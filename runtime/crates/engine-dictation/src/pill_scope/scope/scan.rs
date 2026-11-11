//! The per-frame scan description and the method that picks it for the
//! current stage.

use super::super::tuning::Processing;
use super::consts::{BEAM_FADE, ENTER, WIPE, WORK_HELIX};
use super::geometry::{bump, hermite, pass_position, smoothstep};
use super::work::Envelope;
use super::{Scope, Stage};

pub(super) struct Scan {
    pub(super) helix: f32,
    pub(super) decay: f32,
    pub(super) tail: f32,
    pub(super) grid: f32,
    pub(super) envelope: Envelope,
}

impl Scan {
    /// (amplitude, brightness, swell) at `x`.
    pub(super) fn at(&self, x: f32) -> (f32, f32, f32) {
        match self.envelope {
            Envelope::Enter {
                reach,
                head,
                unfold,
            } => {
                let d = x.abs();
                let drawn =
                    unfold * (1.0 - smoothstep((reach - 14.0).max(0.0), reach.max(0.001), d));
                let amplitude = drawn
                    * (1.0 - smoothstep((reach - 22.0).max(0.0), (reach - 3.0).max(0.001), d));
                let beam = head * bump(d - reach, 6.0);
                let swell = 1.8 * unfold * (head / 2.8) * bump(d - (reach - 14.0).max(0.0), 14.0);
                (amplitude, amplitude + beam, swell)
            }
            Envelope::Process { iron, front, head } => {
                (iron, iron + head * bump(x - front, 5.0), 0.0)
            }
            Envelope::Work {
                working,
                head,
                swell,
                half_width,
            } => {
                let light = working.light(x, half_width);
                (1.0, 1.0 + head * light, swell * light)
            }
            Envelope::Clear {
                front,
                head,
                beam,
                beam_at,
                field,
                working,
                glow,
                swell,
                half_width,
            } => {
                let d = x.abs();
                let keep = field * smoothstep(front - 12.0, front - 2.0, d);
                let light = working.light(x, half_width);
                (
                    keep,
                    keep * (1.0 + glow * light)
                        + head * bump(d - front, 6.0)
                        + beam * bump(x - beam_at, 5.0),
                    keep * swell * light,
                )
            }
        }
    }
}

impl Scope {
    pub(super) fn scan(&self, width: f32) -> Scan {
        let e = self.elapsed;
        let still = |helix: f32| Scan {
            helix,
            decay: 1.0,
            tail: 1.0,
            grid: 1.0,
            envelope: Envelope::Enter {
                reach: 1e4,
                head: 0.0,
                unfold: 1.0,
            },
        };
        match self.stage {
            Stage::Failed => still(1.0),
            _ if self.reduce_motion => still(1.0),
            Stage::Recording => Scan {
                helix: 1.0,
                decay: 1.0,
                tail: 1.0,
                grid: (e / 0.3).min(1.0),
                envelope: Envelope::Enter {
                    reach: (width / 2.0 + 12.0) * pass_position(e / ENTER),
                    head: 2.8 * (1.0 - smoothstep(0.08, ENTER, e)),
                    unfold: smoothstep(0.0, 0.1, e),
                },
            },
            Stage::Processing if self.tuning.processing != Processing::Sweep => {
                let enter = smoothstep(0.0, 0.35, e);
                Scan {
                    helix: 1.0 + (WORK_HELIX - 1.0) * enter,
                    decay: 1.0,
                    tail: 1.0,
                    grid: 1.0,
                    envelope: Envelope::Work {
                        working: self.working(e),
                        head: 2.5 * smoothstep(0.05, 0.3, e) * self.tuning.shimmer,
                        swell: 0.9 * enter,
                        half_width: width / 2.0,
                    },
                }
            }
            Stage::Done => {
                let target = width / 2.0 + 8.0;
                let front = hermite(0.0, self.speed_at_done, target, e / WIPE);
                // Sweep's ironed field stays flat and dark.
                let sweep = self.tuning.processing == Processing::Sweep;
                Scan {
                    helix: if sweep { 0.0 } else { WORK_HELIX },
                    decay: 1.0 - 0.6 * smoothstep(0.0, 0.12, e),
                    tail: 1.0 - smoothstep(0.0, 0.15, e),
                    grid: 1.0 - smoothstep(0.2, 0.4, e),
                    envelope: Envelope::Clear {
                        front,
                        head: 2.8 * smoothstep(0.0, 0.04, e) * (1.0 - smoothstep(0.22, 0.32, e)),
                        beam: if sweep && self.done_from > 0.0 {
                            self.process_head(self.done_from + e)
                                * (1.0 - smoothstep(0.0, BEAM_FADE, e))
                        } else {
                            0.0
                        },
                        beam_at: self.process_front(self.done_from + e, width),
                        field: if sweep { 0.0 } else { 1.0 },
                        working: self.working(self.done_from + e),
                        glow: if sweep { 0.0 } else { 2.5 }
                            * self.tuning.shimmer
                            * (1.0 - smoothstep(0.0, 0.18, e)),
                        swell: 0.9,
                        half_width: width / 2.0,
                    },
                }
            }
            Stage::Processing => {
                let iron = 1.0 - smoothstep(0.0, 0.28, e);
                let head = self.process_head(e);
                Scan {
                    helix: iron,
                    decay: 1.0,
                    tail: 1.0,
                    grid: 1.0,
                    envelope: Envelope::Process {
                        iron,
                        front: self.process_front(e, width),
                        head,
                    },
                }
            }
        }
    }
}
