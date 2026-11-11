//! The voice the traces follow: a fast level follower with an energy
//! reservoir and a kick, plus HUD Lab's synthetic voice for tests.

use std::f32::consts::TAU;

#[cfg(test)]
use std::f32::consts::PI;

/// HUD Lab's synthetic voice: phrases of syllables at about 4.5 a second
/// with breaths between, as a 0–1 level.
#[cfg(test)]
pub fn demo_level(time: f32) -> f32 {
    const PHRASE: f32 = 2.4;
    const SPOKEN: f32 = 1.7;
    let phrase = (time / PHRASE).floor();
    let t = time - phrase * PHRASE;
    if t > SPOKEN {
        return 0.04;
    }
    let shape = (PI * t / SPOKEN).sin().powf(0.4);
    let rate = TAU * (4.3 + 0.6 * (phrase * 1.7).sin());
    let syllable = 0.5 + 0.5 * (rate * t + phrase).sin();
    let stress = 0.75 + 0.25 * (TAU * 1.1 * t + 2.0 * phrase).sin();
    (0.08 + 0.82 * shape * syllable.powf(1.6) * stress).clamp(0.0, 1.0)
}

/// [`demo_level`] as a laptop microphone at talking distance hears it:
/// breaths near −57 dB and stressed syllables near −32 dB, dBFS.
#[cfg(test)]
pub fn demo_loudness(time: f32) -> f32 {
    -58.0 + 28.0 * demo_level(time)
}

#[derive(Clone, Copy, Default)]
pub(super) struct Voice {
    pub(super) level: f32,
    reservoir: f32,
    pub(super) energy: f32,
    pub(super) kick: f32,
    kick_velocity: f32,
    /// The prototype's 16 bands, all the level: Hex meters no spectrum.
    pub(super) band: f32,
}

impl Voice {
    pub(super) fn step(&mut self, raw: f32, h: f32) {
        let previous = self.level;
        let time = if raw > self.level { 0.035 } else { 0.095 };
        self.level += (raw - self.level) * (1.0 - (-h / time).exp());
        let rise = ((self.level - previous) / h).max(0.0);
        self.reservoir += (1.75 * self.level * self.level - self.reservoir / 1.8) * h;
        self.energy = 1.0 - (-self.reservoir * 1.45).exp();
        let (w, z) = (TAU * 1.3, 0.22);
        self.kick_velocity += (-w * w * self.kick - 2.0 * z * w * self.kick_velocity
            + 15.0 * rise
            + 2.2 * self.level)
            * h;
        self.kick += self.kick_velocity * h;
        let time = if raw > self.band { 0.025 } else { 0.14 };
        self.band += (raw - self.band) * (1.0 - (-h / time).exp());
    }
}
