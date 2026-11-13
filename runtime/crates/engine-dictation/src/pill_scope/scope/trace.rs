//! One braid: the amplitude spring, the glow terms and the phase history
//! the beam reads back in time.

use std::f32::consts::TAU;

use super::super::tuning::Tuning;
use super::consts::{BASE, HISTORY, SPEEDS, STEP};
use super::voice::Voice;

pub(super) struct Trace {
    envelope: f32,
    amplitude: f32,
    amplitude_velocity: f32,
    glow_fast: f32,
    glow_spring: f32,
    glow_spring_velocity: f32,
    glow_slow: f32,
    pub(super) glow: f32,
    pub(super) phases: [f32; 4],
    history: Vec<f32>,
    head: usize,
}

impl Trace {
    pub(super) fn new(phases: [f32; 4]) -> Self {
        Self {
            envelope: 0.0,
            amplitude: 0.0,
            amplitude_velocity: 0.0,
            glow_fast: 0.0,
            glow_spring: 0.0,
            glow_spring_velocity: 0.0,
            glow_slow: 0.0,
            glow: 0.0,
            phases,
            history: vec![0.0; HISTORY * 5],
            head: 0,
        }
    }

    pub(super) fn tick(&mut self, voice: &Voice, live: f32, speed: f32, tuning: &Tuning, h: f32) {
        let follow = |value: &mut f32, target: f32, rise: f32, fall: f32| {
            let time = if target > *value { rise } else { fall };
            *value += (target - *value) * (1.0 - (-h / time).exp());
        };
        follow(&mut self.envelope, voice.level, 0.022, tuning.release);
        let (w, z) = (TAU * tuning.response, 0.52);
        self.amplitude_velocity +=
            (-w * w * (self.amplitude - self.envelope) - 2.0 * z * w * self.amplitude_velocity) * h;
        self.amplitude += self.amplitude_velocity * h;
        follow(&mut self.glow_fast, voice.level, 0.045, 0.13);
        let (gw, gz) = (TAU * 3.6, 0.5);
        self.glow_spring_velocity += (-gw * gw * (self.glow_spring - self.glow_fast)
            - 2.0 * gz * gw * self.glow_spring_velocity)
            * h;
        self.glow_spring += self.glow_spring_velocity * h;
        follow(&mut self.glow_slow, voice.level, 0.15, 0.7);
        let swell = 0.8 * (self.glow_spring - 0.25).max(0.0) + 0.04 * voice.kick.max(-0.5);
        self.glow = (swell.max(0.0) + 0.42 * self.glow_slow + 0.2 * voice.energy).min(1.15) * live;
        let go = 0.6 + 1.1 * (self.envelope * 1.6).min(1.0);
        for (phase, speed_hz) in self.phases.iter_mut().zip(SPEEDS) {
            *phase = (*phase + h * TAU * speed_hz * go * speed) % TAU;
        }
        // With flat bands each group is the mean, so the prototype's
        // `g / (mean + 0.06)` emphasis is the same for every component.
        let emphasis = 0.35 + 0.65 * (voice.band / (voice.band + 0.06)).min(2.0);
        let offset = self.head * 5;
        let amplitude = self.amplitude.max(0.0) * live;
        self.history[offset] = amplitude;
        for (c, base) in BASE.into_iter().enumerate() {
            self.history[offset + 1 + c] = amplitude * base * emphasis;
        }
        self.head = (self.head + 1) % HISTORY;
    }

    pub(super) fn past(&self, delay: f32, channel: usize) -> f32 {
        let d = (delay / STEP).min((HISTORY - 2) as f32);
        let i = d.floor() as usize;
        let k = d - i as f32;
        let a = (self.head + 2 * HISTORY - 1 - i) % HISTORY;
        let b = (a + HISTORY - 1) % HISTORY;
        self.history[a * 5 + channel] * (1.0 - k) + self.history[b * 5 + channel] * k
    }
}
