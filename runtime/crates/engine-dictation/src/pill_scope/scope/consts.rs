//! The dot scope's fixed numbers: the 240 Hz step, the braid history width,
//! and the timings and geometry the simulation's parts share.

pub(super) const STEP: f32 = 1.0 / 240.0;
pub(super) const HISTORY: usize = 512;
pub(super) const SAMPLES_PER_COLUMN: usize = 3;
pub(super) const SPEEDS: [f32; 4] = [0.8, 1.25, 1.85, 2.6];
pub(super) const BASE: [f32; 4] = [0.55, 0.42, 0.3, 0.2];
pub(super) const SPATIAL: [f32; 4] = [1.4, 2.4, 3.8, 5.6];
pub(super) const START_WIDTH: f32 = 30.0;
pub(super) const ENTER: f32 = 0.42;
pub(super) const WIPE: f32 = 0.3;
pub(super) const PASS_INSET: f32 = 9.0;
pub(super) const PHOSPHOR_HEAD: f32 = 0.15;
pub(super) const BREATH_PERIOD: f32 = 5.5;
pub(super) const HIDDEN_SCALE: f32 = 0.9;
pub(super) const HIDDEN_SOFTNESS: f32 = 3.0;
/// How much the resting helix swells while Flow and Pulse work.
pub(super) const WORK_HELIX: f32 = 2.6;
/// Flow's bands sit this far apart, as a share of the processing width.
pub(super) const FLOW_SPACING: f32 = 0.7;
/// The recording carrier's slowest turn, Hz.
pub(super) const CARRIER_REST: f32 = 0.14;
/// How long a Pulse ring takes to swell in, s.
pub(super) const RING_BIRTH: f32 = 0.15;
/// How long Sweep's beam takes to fade where it is once done begins, s.
pub(super) const BEAM_FADE: f32 = 0.14;
