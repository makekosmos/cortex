//! Dot scope's simulation: steps the voice, the braided traces, and the
//! phosphor at a fixed 240 Hz and yields one (brightness, freshness) pair
//! per dot.

use self::consts::{HIDDEN_SCALE, HIDDEN_SOFTNESS, SAMPLES_PER_COLUMN, START_WIDTH, STEP};
use self::trace::Trace;
use self::voice::Voice;
use super::spring::Spring;
use super::tuning::{AutoLevel, Tuning};

mod beam;
mod colors;
mod consts;
mod geometry;
mod motion;
mod scan;
mod trace;
mod voice;
mod work;

#[cfg(test)]
mod tests;

pub use colors::{palette, Palette};
#[cfg(test)]
pub use voice::demo_loudness;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Recording,
    Processing,
    Done,
    Failed,
}

/// What the HUD wants this frame.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub stage: Stage,
    pub visible: bool,
    /// The microphone's loudness, dBFS ([`super::loudness`]).
    pub loudness: f32,
    pub reduce_motion: bool,
}

/// The plate's pose and the field's global terms, for the shader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub width: f32,
    pub height: f32,
    pub opacity: f32,
    pub scale: f32,
    pub softness: f32,
    pub cool: f32,
    pub fail: f32,
    pub glow: f32,
    pub grid: f32,
    pub live: f32,
    /// How far the plate has grown in, 0 hidden to 1 shown, past 1 as it
    /// bounces: what an island grows by.
    pub reveal: f32,
}

/// The Dot scope simulation.
pub struct Scope {
    tuning: Tuning,
    stage: Stage,
    elapsed: f32,
    /// How fast the done front was moving when done arrived, pt/s.
    speed_at_done: f32,
    /// How long it had processed when done arrived, s.
    done_from: f32,
    /// Flow's band offset, pt, travelling right while it works.
    flow_offset: f32,
    /// How far the carrier has handed its turn over to Flow's, 0–1.
    flow_turn: f32,
    auto: AutoLevel,
    /// The level the voice follows, 0–1, before the gain.
    level: f32,
    voices: [Voice; 2],
    traces: [Trace; 2],
    live: f32,
    carrier: f32,
    time: f32,
    steps: u64,
    pending: f32,
    reduce_motion: bool,
    columns: usize,
    rows: usize,
    head: [Vec<f32>; 2],
    tail: [Vec<f32>; 2],
    dots: Vec<[f32; 2]>,
    ys: [Vec<f32>; 2],
    brightness: [Vec<f32>; 2],
    grid: f32,
    width: Spring,
    opacity: Spring,
    scale: Spring,
    softness: Spring,
    cool: Spring,
    fail: Spring,
}

impl Scope {
    pub fn new(tuning: Tuning) -> Self {
        let mut scope = Self {
            tuning: tuning.clamped(),
            stage: Stage::Recording,
            elapsed: 0.0,
            speed_at_done: 0.0,
            done_from: 0.0,
            flow_offset: 0.0,
            flow_turn: 0.0,
            auto: AutoLevel::default(),
            level: 0.0,
            voices: [Voice::default(); 2],
            traces: [
                Trace::new([0.1, 1.7, 3.3, 4.6]),
                Trace::new([2.4, 0.6, 5.1, 1.9]),
            ],
            live: 0.0,
            carrier: 0.0,
            time: 0.0,
            steps: 0,
            pending: 0.0,
            reduce_motion: false,
            columns: 0,
            rows: 0,
            head: [Vec::new(), Vec::new()],
            tail: [Vec::new(), Vec::new()],
            dots: Vec::new(),
            ys: [Vec::new(), Vec::new()],
            brightness: [Vec::new(), Vec::new()],
            grid: 0.0,
            width: Spring::at(START_WIDTH),
            opacity: Spring::at(0.0),
            scale: Spring::at(HIDDEN_SCALE),
            softness: Spring::at(HIDDEN_SOFTNESS),
            cool: Spring::at(0.0),
            fail: Spring::at(0.0),
        };
        scope.resize();
        scope
    }

    /// Starts a recording from nothing: the scanner unfolds the trace again.
    /// The auto level remembers the speaker.
    #[cfg(test)]
    pub fn restart(&mut self, reduce_motion: bool) {
        let auto = self.auto;
        *self = Self::new(self.tuning);
        self.auto = auto;
        if reduce_motion {
            self.scale.jump(1.0);
            self.softness.jump(0.0);
            self.width.jump(self.tuning.width);
        }
    }

    #[cfg(test)]
    pub fn tune(&mut self, tuning: Tuning) {
        self.tuning = tuning.clamped();
        self.resize();
    }

    /// The 0–1 level the voice follows now, before the gain.
    #[cfg(test)]
    pub fn level(&self) -> f32 {
        self.level
    }

    fn resize(&mut self) {
        let columns = self.tuning.columns as usize;
        let rows = self.tuning.rows as usize;
        if columns == self.columns && rows == self.rows {
            return;
        }
        self.columns = columns;
        self.rows = rows;
        let dots = columns * rows;
        let samples = (columns - 1) * SAMPLES_PER_COLUMN + 1;
        self.head = [vec![0.0; dots], vec![0.0; dots]];
        self.tail = [vec![0.0; dots], vec![0.0; dots]];
        self.dots = vec![[0.0; 2]; dots];
        self.ys = [vec![0.0; samples], vec![0.0; samples]];
        self.brightness = [vec![0.0; samples], vec![0.0; samples]];
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Each dot's (brightness, freshness), row by row.
    pub fn dots(&self) -> &[[f32; 2]] {
        &self.dots
    }

    pub fn pose(&self) -> Pose {
        Pose {
            width: self.width.x.max(self.tuning.height),
            height: self.tuning.height,
            opacity: self.opacity.x.clamp(0.0, 1.0),
            scale: self.scale.x.max(0.01),
            softness: self.softness.x.clamp(0.0, HIDDEN_SOFTNESS),
            cool: self.cool.x.clamp(0.0, 1.0),
            fail: self.fail.x.clamp(0.0, 1.0),
            glow: (self.traces[0].glow + self.traces[1].glow) / 2.0 * self.tuning.glow,
            grid: self.grid,
            live: self.live,
            reveal: ((self.scale.x - HIDDEN_SCALE) / (1.0 - HIDDEN_SCALE)).max(0.0),
        }
    }

    /// Whether the scope has faded out after being hidden.
    pub fn settled(&self) -> bool {
        self.opacity.target == 0.0 && self.opacity.x < 0.002
    }

    /// Advances by `dt` seconds in fixed 240 Hz steps.
    pub fn advance(&mut self, dt: f32, frame: Frame) {
        if frame.stage != self.stage {
            if frame.stage == Stage::Done {
                self.arm_done();
            }
            if frame.stage == Stage::Processing {
                self.flow_offset = -self.width.x / 2.0 - 4.0;
            }
            self.stage = frame.stage;
            self.elapsed = 0.0;
        }
        self.reduce_motion = frame.reduce_motion;
        self.pending += dt.clamp(0.0, 0.25);
        while self.pending >= STEP {
            self.pending -= STEP;
            self.step(frame);
        }
    }
}
