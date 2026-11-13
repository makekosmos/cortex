//! The processing looks: Flow's bands, Pulse's rings, and done clearing.

use super::super::consts::RING_BIRTH;
use super::super::work::Working;
use super::*;

fn processing() -> Frame {
    Frame {
        stage: Stage::Processing,
        ..recording(0.0)
    }
}

fn with(processing: Processing) -> Tuning {
    Tuning {
        processing,
        ..fixed()
    }
}

/// The working light along the plate, one sample a point.
fn light(scope: &Scope) -> Vec<f32> {
    let scan = scope.scan(scope.width.x);
    let half = scope.width.x / 2.0;
    (0..=half as i32 * 2)
        .map(|i| {
            let (amplitude, bright, _) = scan.at(i as f32 - half);
            bright - amplitude
        })
        .collect()
}

/// How many points the light moved between `a` and `b` in `range`:
/// the shift that best lines them up.
fn shift(a: &[f32], b: &[f32], range: std::ops::Range<usize>) -> i32 {
    (-8..=8)
        .max_by(|&x, &y| {
            let score = |k: i32| -> f32 {
                range
                    .clone()
                    .filter_map(|i| Some(a[i] * b.get(i.checked_add_signed(k as isize)?)?))
                    .sum()
            };
            score(x).total_cmp(&score(y))
        })
        .unwrap()
}

#[test]
fn flow_turns_the_braid_with_light_flowing_rightward() {
    let mut scope = Scope::new(with(Processing::Flow));
    run(&mut scope, 1.0, recording(0.6));
    run(&mut scope, 1.2, processing());
    let mut sweep = Scope::new(fixed());
    run(&mut sweep, 1.0, recording(0.6));
    run(&mut sweep, 1.2, processing());
    assert!(
        spread(&scope) > spread(&sweep) * 1.4,
        "Flow keeps a braid: {} vs {}",
        spread(&scope),
        spread(&sweep)
    );
    let mut rightward = 0;
    for _ in 0..12 {
        let before = light(&scope);
        run(&mut scope, 0.1, processing());
        rightward += shift(&before, &light(&scope), 0..before.len()).signum();
    }
    assert!(rightward >= 10, "rightward {rightward} of 12");
    let helix = scope.flow_turn_hz() * scope.width.x / scope.tuning.helix_cycles;
    assert!(
        (helix - scope.flow_speed()).abs() < 0.5,
        "the braid keeps pace"
    );
    assert!(scope.pose().cool > 0.99);
}

#[test]
fn pulse_carries_light_out_from_the_middle() {
    let mut scope = Scope::new(with(Processing::Pulse));
    run(&mut scope, 1.0, recording(0.6));
    run(&mut scope, 1.2, processing());
    let (mut left, mut right) = (0, 0);
    for _ in 0..12 {
        let before = light(&scope);
        let middle = before.len() / 2;
        run(&mut scope, 0.1, processing());
        let after = light(&scope);
        left += shift(&before, &after, 2..middle - 2).signum();
        right += shift(&before, &after, middle + 2..before.len() - 2).signum();
    }
    assert!(left <= -10 && right >= 10, "left {left} right {right}");
}

#[test]
fn a_pulse_ring_swells_in_as_it_leaves_the_middle() {
    let ring = |time| {
        Working::Pulse {
            time,
            period: 1.0,
            speed: 30.0,
        }
        .light(0.0, 40.0)
    };
    // Just after the second ring is born, the middle has barely lit.
    assert!(ring(1.0) < 0.05, "at birth {}", ring(1.0));
    assert!(ring(1.0 + RING_BIRTH / 2.0) < ring(1.0 + RING_BIRTH));
}

#[test]
fn every_processing_look_settles_dark_on_done() {
    for look in Processing::ALL {
        for processed in [0.05, 1.37] {
            let mut scope = Scope::new(with(look));
            run(&mut scope, 1.0, recording(0.5));
            run(&mut scope, processed, processing());
            let done = Frame {
                stage: Stage::Done,
                ..processing()
            };
            run(&mut scope, 0.5, done);
            assert!(
                lit(&scope) < 1.0,
                "{look:?} after {processed}s: lit {}",
                lit(&scope)
            );
            run(
                &mut scope,
                0.6,
                Frame {
                    visible: false,
                    ..done
                },
            );
            assert!(scope.settled(), "{look:?}");
        }
    }
}

#[test]
fn done_clears_from_the_middle_out_for_every_look() {
    for look in Processing::ALL {
        let mut scope = Scope::new(with(look));
        run(&mut scope, 0.5, recording(0.5));
        run(&mut scope, 1.0, processing());
        run(
            &mut scope,
            0.12,
            Frame {
                stage: Stage::Done,
                ..processing()
            },
        );
        let (columns, rows) = (scope.columns(), scope.rows());
        let column = |i: usize| {
            (0..rows)
                .map(|j| scope.dots()[j * columns + i][0])
                .sum::<f32>()
        };
        let middle = column(columns / 2);
        let brightest = (0..columns).map(column).fold(0.0, f32::max);
        assert!(
            middle < 0.5 * brightest,
            "{look:?}: middle {middle} vs brightest {brightest}"
        );
    }
}
