//! Drives the pill's dot scope and broadcasts each frame, so the plugin only
//! draws. The scope steps on the display's cadence (16 ms) while a session
//! runs and until it settles after one, then the thread ends on its own.
//!
//! Wire format (`dictation_pill_frame`):
//!
//! ```text
//! {"event":"dictation_pill_frame","width":112.0,"height":26.0,
//!  "opacity":1.0,"scale":1.0,"softness":0.0,"glow":1.0,"grid":1.0,
//!  "live":0.5,"cool":0.0,"dark":true,"columns":43,"rows":9,
//!  "base":[...],"core":[...],"plate":[...],"rim":[...],"shadow":[...],
//!  "dots":[brightness,freshness,...]}
//! ```

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::broadcast;

use super::pill_scope::{palette, Frame, Scope, Stage, Tuning};

/// One display frame, the cadence the app's pill ran at.
const TICK: Duration = Duration::from_millis(16);
/// The app passed this floor for a microphone at talking distance.
/// The scope treats everything under this as silence. A voice a hand's width
/// from the microphone sits near -43 dBFS, so a -42 floor swallowed ordinary
/// speech: only a shout moved the dots.
const NOISE_FLOOR: f32 = -55.0;
/// The engine's meter is 0..1; below this the room is silent.
/// A quieter room than the app's -55 dBFS floor.
const QUIET_DBFS: f32 = -120.0;
/// The scope's response to the meter, 1.5× of its own default.
const PILL_GAIN: f32 = 1.5;
/// How long a finished session may keep fading before the pill is hidden.
const SETTLE_MS: u128 = 1200;

#[derive(Default)]
pub struct PillTicker {
    shared: Arc<Shared>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

struct Shared {
    stage: AtomicU8,
    loudness: Mutex<f32>,
    last: Mutex<Option<Value>>,
    active: AtomicBool,
    /// When the last session left the recording, so a fade cannot last.
    done_since: Mutex<Option<Instant>>,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            stage: AtomicU8::new(stage_code(Stage::Recording)),
            loudness: Mutex::new(QUIET_DBFS),
            last: Mutex::new(None),
            active: AtomicBool::new(false),
            done_since: Mutex::new(None),
        }
    }
}

fn stage_code(stage: Stage) -> u8 {
    match stage {
        Stage::Recording => 0,
        Stage::Processing => 1,
        Stage::Done => 2,
        Stage::Failed => 3,
    }
}

fn stage_from(code: u8) -> Stage {
    match code {
        0 => Stage::Recording,
        1 => Stage::Processing,
        2 => Stage::Done,
        _ => Stage::Failed,
    }
}

impl PillTicker {
    pub fn new() -> Self {
        Self::default()
    }

    /// The newest frame, so a host with no event stream can still draw the pill.
    pub fn last_frame(&self) -> Option<Value> {
        self.shared.last.lock().ok().and_then(|slot| slot.clone())
    }

    /// A session is on screen: the host hides the pill when this turns false,
    /// because the last frame stays readable after the fade.
    pub fn is_active(&self) -> bool {
        self.shared.active.load(Ordering::Acquire)
    }

    /// The meter the scope last received, in dBFS. The frame carries the
    /// drawing; this carries the input that produced it.
    pub fn loudness(&self) -> f32 {
        self.shared
            .loudness
            .lock()
            .map(|value| *value)
            .unwrap_or(QUIET_DBFS)
    }

    /// The capture's 0..1 meter, mapped the way the app mapped it: a -55 dBFS
    /// floor, and near-silence under `SILENT`.
    pub fn set_level(&self, level: f32) {
        // The capture helper sends `min(1, rms * 5)`, so `level / 5` is the
        // RMS again. A linear 0..1 meter pins the scope at the bottom for a
        // normal voice — speech sits near 0.02–0.05 RMS — so the meter becomes
        // dBFS, the scale the scope's noise floor is written in.
        let rms = (level / 5.0).max(1e-5);
        let dbfs = (20.0 * rms.log10()).clamp(QUIET_DBFS, -5.0);
        if let Ok(mut current) = self.shared.loudness.lock() {
            *current = dbfs;
        }
    }

    pub fn set_stage(&self, stage: Stage) {
        self.shared
            .stage
            .store(stage_code(stage), Ordering::Release);
    }

    /// Starts the frame thread for a session. A running thread keeps going:
    /// a recording restarted mid-fade reuses it.
    pub fn begin(self: &Arc<Self>, events: broadcast::Sender<Value>) {
        let mut slot = match self.thread.lock() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        if slot.as_ref().is_some_and(|handle| !handle.is_finished()) {
            return;
        }
        self.shared.active.store(true, Ordering::Release);
        let ticker = Arc::clone(self);
        *slot = Some(
            std::thread::Builder::new()
                .name("mundus-dictation-pill".into())
                .spawn(move || ticker.run(events))
                .ok()
                .unwrap_or_else(|| {
                    // No thread, no pill frames: the session still records.
                    std::thread::spawn(|| {})
                }),
        );
    }

    fn run(&self, events: broadcast::Sender<Value>) {
        let mut scope = Scope::new(Tuning {
            noise_floor: NOISE_FLOOR,
            // Speech moves the dots, but the scope's own gain leaves the
            // movement small. A half more reads as «it hears me» without
            // pinning the plate on loud syllables.
            gain: PILL_GAIN,
            ..Tuning::default()
        });
        let mut last = Instant::now();
        loop {
            std::thread::sleep(TICK);
            let now = Instant::now();
            let dt = now.duration_since(last).as_secs_f32();
            last = now;
            let stage = stage_from(self.shared.stage.load(Ordering::Acquire));
            let loudness = self
                .shared
                .loudness
                .lock()
                .map(|value| *value)
                .unwrap_or(QUIET_DBFS);
            scope.advance(
                dt,
                Frame {
                    stage,
                    visible: true,
                    loudness,
                    reduce_motion: false,
                },
            );
            let done = matches!(stage, Stage::Done | Stage::Failed);
            // A session that ended keeps its fade, then goes: the scope's own
            // settle flag is a hint, not a guarantee, so the clock backs it up.
            let settled = done
                && (scope.settled()
                    || self
                        .shared
                        .done_since
                        .lock()
                        .map(|since| since.is_some_and(|at| at.elapsed().as_millis() > SETTLE_MS))
                        .unwrap_or(false));
            let frame = frame_json(&scope);
            if let Ok(mut last) = self.shared.last.lock() {
                *last = Some(frame.clone());
            }
            let _ = events.send(frame);
            if let Ok(mut since) = self.shared.done_since.lock() {
                if done {
                    since.get_or_insert(now);
                } else {
                    *since = None;
                }
            }
            if settled {
                self.shared.active.store(false, Ordering::Release);
                break;
            }
        }
    }
}

/// One frame as the plugin's `PillFrame`: the pose, the palette the app
/// computed for that pose, and the dots row by row.
pub fn frame_json(scope: &Scope) -> Value {
    let pose = scope.pose();
    let colors = palette(true, &pose, 1.0);
    let mut dots = Vec::with_capacity(scope.dots().len() * 2);
    for [brightness, freshness] in scope.dots() {
        dots.push(brightness);
        dots.push(freshness);
    }
    json!({
        "event": "dictation_pill_frame",
        "width": pose.width,
        "height": pose.height,
        "opacity": pose.opacity,
        "scale": pose.scale,
        "softness": pose.softness,
        "glow": pose.glow,
        "grid": pose.grid,
        "live": pose.live,
        "cool": pose.cool,
        "dark": true,
        "columns": scope.columns(),
        "rows": scope.rows(),
        "base": [colors.base[0], colors.base[1], colors.base[2]],
        "core": [colors.core[0], colors.core[1], colors.core[2]],
        "plate": colors.plate,
        "rim": colors.rim,
        "shadow": colors.shadow,
        "dots": dots,
    })
}

/// Stops the frame thread on shutdown, so a test or a restart does not wait
/// for the fade-out.
pub fn stop(ticker: &PillTicker) {
    if let Ok(mut slot) = ticker.thread.lock() {
        if let Some(handle) = slot.take() {
            handle.join().ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_carries_the_pose_palette_and_every_dot() {
        let mut scope = Scope::new(Tuning::default());
        scope.advance(
            1.0 / 60.0,
            Frame {
                stage: Stage::Recording,
                visible: true,
                loudness: -40.0,
                reduce_motion: false,
            },
        );
        let frame = frame_json(&scope);
        assert_eq!(frame["event"], "dictation_pill_frame");
        assert_eq!(frame["columns"], 43);
        assert_eq!(frame["rows"], 9);
        assert_eq!(
            frame["dots"].as_array().map(Vec::len),
            Some(scope.columns() * scope.rows() * 2)
        );
        assert!(frame["base"].as_array().is_some_and(|base| base.len() == 3));
        assert!(frame["plate"]
            .as_array()
            .is_some_and(|plate| plate.len() == 4));
    }

    #[test]
    fn the_meter_maps_like_the_apps_pill() {
        let ticker = PillTicker::new();
        // The capture helper sends `min(1, rms * 5)`, so half its meter is an
        // RMS of 0.1 — a loud voice, -20 dBFS.
        ticker.set_level(0.5);
        assert!((*ticker.shared.loudness.lock().unwrap() + 20.0).abs() < 0.01);
        // Quiet speech is still above the measure: -54 dBFS, not silence.
        ticker.set_level(0.01);
        let quiet = *ticker.shared.loudness.lock().unwrap();
        assert!((quiet + 54.0).abs() < 0.2, "{quiet}");
        assert!(quiet > NOISE_FLOOR);
        // A stopped microphone is the floor, not minus infinity: 1e-5 of RMS
        // is -100 dBFS, under the scope's -55 and above the rail.
        ticker.set_level(0.0);
        assert_eq!(*ticker.shared.loudness.lock().unwrap(), -100.0);
    }

    #[test]
    fn stages_round_trip() {
        for stage in [
            Stage::Recording,
            Stage::Processing,
            Stage::Done,
            Stage::Failed,
        ] {
            assert_eq!(stage_from(stage_code(stage)), stage);
        }
    }
}
