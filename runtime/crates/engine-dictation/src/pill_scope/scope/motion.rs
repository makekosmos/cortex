//! The per-step dynamics: the voice, the springs, Flow and Pulse timing,
//! and done's arming.

use std::f32::consts::{PI, TAU};

use super::super::spring::{Spec, REDUCED_FADE};
use super::super::tuning::{fixed_level, Processing};
use super::consts::{CARRIER_REST, FLOW_SPACING, HIDDEN_SCALE, HIDDEN_SOFTNESS, PASS_INSET, STEP};
use super::geometry::smoothstep;
use super::work::Working;
use super::{Frame, Scope, Stage};

impl Scope {
    pub(super) fn step(&mut self, frame: Frame) {
        let h = STEP;
        let tuning = self.tuning;
        let recording = self.stage == Stage::Recording && frame.visible;
        let level = if recording {
            self.level = if tuning.auto_level {
                self.auto
                    .step(frame.loudness, h, tuning.auto_release, tuning.noise_floor)
            } else {
                fixed_level(frame.loudness)
            };
            (self.level * tuning.gain).clamp(0.0, 1.0)
        } else {
            self.level = 0.0;
            0.0
        };
        self.voices[0].step(level, h);
        self.voices[1].step(level * 0.85, h);
        let live_target = if recording { 1.0 } else { 0.0 };
        self.live += (live_target - self.live) * (1.0 - (-h / 0.55).exp());
        let speed = if frame.reduce_motion {
            0.0
        } else {
            tuning.braid_speed
        };
        for (trace, voice) in self.traces.iter_mut().zip(&self.voices) {
            trace.tick(voice, self.live, speed, &tuning, h);
        }
        let energy = self.voices[0].energy.max(self.voices[1].energy);
        let working = matches!(self.stage, Stage::Processing | Stage::Done);
        let flowing = working && tuning.processing == Processing::Flow;
        self.flow_turn +=
            (if flowing { 1.0 } else { 0.0 } - self.flow_turn) * (1.0 - (-h / 0.25).exp());
        if working {
            self.flow_offset += self.flow_speed() * h;
        }
        let rest = CARRIER_REST + 0.22 * energy;
        let turn = rest + (self.flow_turn_hz() - rest) * self.flow_turn;
        self.carrier = (self.carrier + h * TAU * turn * speed) % TAU;

        let width = match self.stage {
            Stage::Recording => tuning.width,
            Stage::Processing | Stage::Done => tuning.processing_width,
            Stage::Failed => self.width.target,
        };
        let cool = if matches!(self.stage, Stage::Processing | Stage::Done) {
            1.0
        } else {
            0.0
        };
        let fail = if self.stage == Stage::Failed {
            1.0
        } else {
            0.0
        };
        let shown = if frame.visible { 1.0 } else { 0.0 };
        let fade = Spec::new(tuning.transition, 0.0);
        let bouncy = Spec::new(tuning.transition, tuning.bounce);
        if frame.reduce_motion {
            self.width.jump(width);
            self.scale.jump(1.0);
            self.softness.jump(0.0);
            self.opacity.step_to(shown, REDUCED_FADE, h);
            self.cool.step_to(cool, REDUCED_FADE, h);
            self.fail.step_to(fail, REDUCED_FADE, h);
        } else {
            self.width.step_to(width, bouncy, h);
            self.opacity.step_to(shown, fade, h);
            self.scale
                .step_to(if frame.visible { 1.0 } else { HIDDEN_SCALE }, bouncy, h);
            self.softness
                .step_to(if frame.visible { 0.0 } else { HIDDEN_SOFTNESS }, fade, h);
            self.cool.step_to(cool, fade, h);
            self.fail.step_to(fail, fade, h);
        }

        self.elapsed += h;
        self.time += h;
        self.steps += 1;
        if self.steps.is_multiple_of(2) {
            self.beam(2.0 * h);
        }
    }

    /// One band or pulse per this many seconds.
    pub(super) fn work_period(&self) -> f32 {
        2.0 * self.tuning.shimmer_pass.max(0.05)
    }

    pub(super) fn flow_spacing(&self) -> f32 {
        FLOW_SPACING * self.tuning.processing_width
    }

    pub(super) fn flow_speed(&self) -> f32 {
        self.flow_spacing() / self.work_period()
    }

    /// The carrier turn that moves the helix at Flow's band speed, Hz.
    pub(super) fn flow_turn_hz(&self) -> f32 {
        self.flow_speed() * self.tuning.helix_cycles / self.width.x.max(1.0)
    }

    pub(super) fn pulse_speed(&self) -> f32 {
        (self.tuning.processing_width / 2.0 + 12.0) / (1.4 * self.work_period())
    }

    pub(super) fn working(&self, time: f32) -> Working {
        match self.tuning.processing {
            Processing::Pulse => Working::Pulse {
                time,
                period: self.work_period(),
                speed: self.pulse_speed(),
            },
            Processing::Sweep | Processing::Flow => Working::Flow {
                offset: self.flow_offset,
                spacing: self.flow_spacing(),
            },
        }
    }

    /// Done clears from the middle out for every look, its front already
    /// moving at the pulses' speed.
    pub(super) fn arm_done(&mut self) {
        self.speed_at_done = 1.5 * self.pulse_speed();
        self.done_from = if self.stage == Stage::Processing {
            self.elapsed
        } else {
            0.0
        };
    }

    /// Sweep's beam strength `elapsed` into processing.
    pub(super) fn process_head(&self, elapsed: f32) -> f32 {
        let pass = self.tuning.shimmer_pass.max(0.05);
        (2.2 + (PI * (elapsed / pass + 0.5)).sin().abs())
            * smoothstep(0.0, 0.14, elapsed)
            * self.tuning.shimmer
    }

    pub(super) fn process_front(&self, elapsed: f32, width: f32) -> f32 {
        let pass = self.tuning.shimmer_pass.max(0.05);
        let position = 0.5 - 0.5 * (PI * (elapsed / pass + 0.5)).cos();
        -width / 2.0 + PASS_INSET + (width - 2.0 * PASS_INSET) * position
    }
}
