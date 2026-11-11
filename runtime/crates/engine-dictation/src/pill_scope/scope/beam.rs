//! Rasterises both braids into the dot field and decays the phosphor.

use std::f32::consts::TAU;

use super::consts::{BREATH_PERIOD, PHOSPHOR_HEAD, SAMPLES_PER_COLUMN, SPATIAL};
use super::geometry::smoothstep;
use super::Scope;

impl Scope {
    /// Draws both traces into the dot field and decays the phosphor.
    pub(super) fn beam(&mut self, hb: f32) {
        let (columns, rows) = (self.columns, self.rows);
        let sub = SAMPLES_PER_COLUMN;
        let samples = (columns - 1) * sub + 1;
        let pitch = self.tuning.spacing;
        let width = self.width.x.max(self.tuning.height);
        let scan = self.scan(width);
        self.grid = scan.grid;
        let mid = (rows as f32 - 1.0) / 2.0;
        let half = mid - 0.55;
        let breath = if self.reduce_motion {
            0.5
        } else {
            0.5 + 0.5 * (self.time * TAU / BREATH_PERIOD).sin()
        };
        let rest = (0.55 + 0.35 * breath) * (0.4 + 0.6 * self.live) * scan.helix;
        let half_width = width / 2.0;
        for k in 0..2 {
            let trace = &self.traces[k];
            let sign = if k == 0 { 1.0 } else { -1.0 };
            let ys = &mut self.ys[k];
            let brightness = &mut self.brightness[k];
            for s in 0..samples {
                let x = (s as f32 / sub as f32 - (columns as f32 - 1.0) / 2.0) * pitch;
                if x.abs() > half_width {
                    ys[s] = mid;
                    brightness[s] = 0.0;
                    continue;
                }
                let (amplitude, bright, swell) = scan.at(x);
                let own = x.abs();
                let xn = (x + half_width) / width;
                let ends = smoothstep(-half_width + 3.0, -half_width + 17.0, x).min(smoothstep(
                    -half_width + 3.0,
                    -half_width + 17.0,
                    -x,
                ));
                let reach = 1.0 - 0.8 * smoothstep(14.0, half_width, own);
                let delay = own / self.tuning.history_speed;
                let a = trace.past(delay, 0);
                let mut spread = 0.0;
                for (c, (spatial, phase)) in SPATIAL.iter().zip(trace.phases).enumerate() {
                    spread += trace.past(delay, 1 + c) * (TAU * spatial * xn - phase).sin();
                }
                let helix = (rest + swell + half * 0.55 * a * reach)
                    * (TAU * self.tuning.helix_cycles * xn - self.carrier).sin();
                let y =
                    half * ((helix + half * 0.7 * spread * reach) / half).tanh() * ends * amplitude;
                ys[s] = mid - sign * y;
                let spatial = (0.3 + 0.7 * (a * 1.8).min(1.0)) * (0.45 + 0.55 * ends);
                brightness[s] = spatial * amplitude + 0.36 * (bright - amplitude).max(0.0);
            }
            for s in 0..samples {
                let slope =
                    (ys[(s + 1).min(samples - 1)] - ys[s.saturating_sub(1)]) * sub as f32 / 2.0;
                brightness[s] *= 1.0 - 0.55 + 0.55 / (1.0 + 0.22 * slope * slope).sqrt();
            }
        }

        let head_keep = (-hb / (PHOSPHOR_HEAD * scan.decay)).exp();
        let tail_keep = (-hb / (self.tuning.phosphor * scan.decay)).exp();
        let dx = 1.0 / sub as f32;
        for i in 0..columns {
            let s0 = i.saturating_sub(3) * sub;
            let s1 = ((i + 3) * sub).min(samples - 1);
            for j in 0..rows {
                let n = j * columns + i;
                let (mut now, mut total) = (0.0, 0.0);
                let mut lit = [0.0; 2];
                for (k, lit) in lit.iter_mut().enumerate() {
                    let ys = &self.ys[k];
                    let mut best = f32::MAX;
                    let mut nearest = i * sub;
                    for s in s0..s1 {
                        let ay = ys[s];
                        let dy = ys[s + 1] - ay;
                        let px = i as f32 - s as f32 * dx;
                        let py = j as f32 - ay;
                        let t = ((px * dx + py * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                        let ex = px - t * dx;
                        let ey = py - t * dy;
                        let d2 = ex * ex + ey * ey;
                        if d2 < best {
                            best = d2;
                            nearest = s;
                        }
                    }
                    let brightness = self.brightness[k][nearest];
                    let on_beam = (-best / 0.25).exp();
                    let instant =
                        (on_beam + 0.5 * (-best.sqrt() / 0.8).exp()).min(1.2) * brightness;
                    let core = on_beam * brightness;
                    let head = &mut self.head[k][n];
                    *head = (*head * head_keep).max(core);
                    let tail = &mut self.tail[k][n];
                    *tail = (*tail * tail_keep).max(core);
                    let light = instant.max(*head).max(0.55 * scan.tail * *tail);
                    *lit = light;
                    now += instant;
                    total += light;
                }
                let power = 1.0 - (1.0 - lit[0].min(1.0)) * (1.0 - lit[1].min(1.0));
                self.dots[n] = [power.clamp(0.0, 1.0), (now / total.max(1e-4)).min(1.0)];
            }
        }
    }
}
