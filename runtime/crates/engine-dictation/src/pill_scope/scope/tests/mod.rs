//! The dot scope's simulation tests: the braid, the poses and the levels.

use super::super::tuning::Processing;
use super::*;

mod looks;

fn run(scope: &mut Scope, seconds: f32, frame: Frame) {
    let frames = (seconds * 60.0).round() as usize;
    for _ in 0..frames {
        scope.advance(1.0 / 60.0, frame);
    }
}

/// A frame at `level` through the prototype's fixed window.
fn recording(level: f32) -> Frame {
    Frame {
        stage: Stage::Recording,
        visible: true,
        loudness: -40.0 + 30.0 * level,
        reduce_motion: false,
    }
}

/// The prototype's response: the fixed window and round 1's sweep.
fn fixed() -> Tuning {
    Tuning {
        auto_level: false,
        processing: Processing::Sweep,
        ..Tuning::default()
    }
}

/// Mean distance of lit dots from the middle row, in rows.
fn spread(scope: &Scope) -> f32 {
    let mid = (scope.rows() as f32 - 1.0) / 2.0;
    let (mut sum, mut weight) = (0.0, 0.0);
    for (n, [power, _]) in scope.dots().iter().enumerate() {
        let row = (n / scope.columns()) as f32;
        sum += power * (row - mid).abs();
        weight += power;
    }
    sum / weight.max(1e-6)
}

fn lit(scope: &Scope) -> f32 {
    scope.dots().iter().map(|[power, _]| power).sum()
}

#[test]
fn silence_draws_a_resting_braid_and_the_voice_widens_it() {
    let mut quiet = Scope::new(fixed());
    run(&mut quiet, 1.5, recording(0.0));
    assert!(lit(&quiet) > 20.0, "the resting braid is visible");
    let mut loud = Scope::new(fixed());
    run(&mut loud, 1.5, recording(0.8));
    assert!(
        spread(&loud) > spread(&quiet) * 1.3,
        "voice {} vs silence {}",
        spread(&loud),
        spread(&quiet)
    );
    let pose = loud.pose();
    assert!((pose.width - 112.0).abs() < 1.0);
    assert!(pose.opacity > 0.99 && pose.glow > 0.2);
}

#[test]
fn processing_irons_the_braid_flat_and_cools_it() {
    let mut scope = Scope::new(fixed());
    run(&mut scope, 1.0, recording(0.8));
    let voice = spread(&scope);
    run(
        &mut scope,
        1.0,
        Frame {
            stage: Stage::Processing,
            ..recording(0.0)
        },
    );
    assert!(spread(&scope) < voice * 0.6);
    let pose = scope.pose();
    assert!(pose.cool > 0.99);
    assert!((pose.width - 76.0).abs() < 1.0);
}

#[test]
fn done_wipes_the_field_dark_and_hiding_settles() {
    let mut scope = Scope::new(fixed());
    run(&mut scope, 1.0, recording(0.5));
    let processing = Frame {
        stage: Stage::Processing,
        ..recording(0.0)
    };
    run(&mut scope, 0.6, processing);
    let done = Frame {
        stage: Stage::Done,
        ..processing
    };
    run(&mut scope, 0.5, done);
    assert!(lit(&scope) < 1.0, "lit {}", lit(&scope));
    assert!(!scope.settled());
    run(
        &mut scope,
        0.6,
        Frame {
            visible: false,
            ..done
        },
    );
    assert!(scope.settled());
}

#[test]
fn auto_level_lights_a_quiet_voice_the_fixed_window_misses() {
    let quiet = |time: f32| Frame {
        loudness: demo_loudness(time),
        ..recording(0.0)
    };
    let mut auto = Scope::new(Tuning::default());
    let mut fixed_window = Scope::new(fixed());
    let (mut a, mut f) = (0.0, 0.0);
    for i in 0..240 {
        let frame = quiet(i as f32 / 60.0);
        auto.advance(1.0 / 60.0, frame);
        fixed_window.advance(1.0 / 60.0, frame);
        if i >= 120 {
            a += spread(&auto);
            f += spread(&fixed_window);
        }
    }
    assert!(a > f * 1.25, "auto {a} vs fixed {f}");
    assert!(auto.level() >= 0.0 && auto.level() <= 1.0);
}

#[test]
fn frame_rate_does_not_change_the_picture() {
    let mut at_60 = Scope::new(fixed());
    let mut at_120 = Scope::new(fixed());
    for _ in 0..90 {
        at_60.advance(1.0 / 60.0, recording(0.6));
        at_120.advance(1.0 / 120.0, recording(0.6));
        at_120.advance(1.0 / 120.0, recording(0.6));
    }
    assert_eq!(at_60.dots(), at_120.dots());
    assert_eq!(at_60.pose(), at_120.pose());
}

#[test]
fn reduce_motion_holds_a_still_braid() {
    let mut scope = Scope::new(fixed());
    let still = Frame {
        reduce_motion: true,
        ..recording(0.0)
    };
    scope.restart(true);
    run(&mut scope, 3.0, still);
    let before = scope.dots().to_vec();
    run(&mut scope, 0.5, still);
    let drift = before
        .iter()
        .zip(scope.dots())
        .map(|(a, b)| (a[0] - b[0]).abs())
        .fold(0.0, f32::max);
    assert!(drift < 1e-3, "drift {drift}");
    assert_eq!(scope.pose().scale, 1.0);
    assert!(lit(&scope) > 20.0);
}

#[test]
fn retuning_the_grid_resizes_the_field() {
    let mut scope = Scope::new(fixed());
    run(&mut scope, 0.2, recording(0.3));
    scope.tune(Tuning {
        columns: 64.0,
        rows: 15.0,
        ..Tuning::default()
    });
    run(&mut scope, 0.2, recording(0.3));
    assert_eq!(scope.dots().len(), 64 * 15);
}

#[test]
fn palette_cools_and_saturates() {
    let pose = Scope::new(fixed()).pose();
    let warm = palette(true, &pose, 1.0);
    assert_eq!(warm.base, [0.86, 0.83, 0.77]);
    let grey = palette(true, &pose, 0.0);
    assert!((grey.base[0] - grey.base[2]).abs() < 1e-6);
    let cool = palette(false, &Pose { cool: 1.0, ..pose }, 1.0);
    assert_eq!(cool.base, [0.40, 0.47, 0.58]);
}
