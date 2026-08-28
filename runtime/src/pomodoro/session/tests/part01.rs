fn cfg_default() -> SessionConfig {
    SessionConfig::default()
}

fn cfg_auto_break() -> SessionConfig {
    SessionConfig {
        auto_start_break: true,
        ..SessionConfig::default()
    }
}

fn cfg_auto_all() -> SessionConfig {
    SessionConfig {
        auto_start_break: true,
        auto_start_work: true,
        ..SessionConfig::default()
    }
}

fn cfg_threshold(threshold: u32) -> SessionConfig {
    SessionConfig {
        pomodoros_until_long_break: threshold,
        work_min_override: Some(1),
        ..cfg_default()
    }
}

fn expected_break(completed: u32, threshold: u32) -> Phase { if completed % threshold == 0 { Phase::LongBreak } else { Phase::ShortBreak } }

#[test]
fn start_from_idle_enters_work() {
    let clock = Arc::new(MockClock::new(1_000_000));
    let mut s = Session::new(clock);
    s.start(cfg_default());
    let st = s.snapshot();
    assert_eq!(st.phase, Phase::Work);
    assert!(st.is_running);
    assert!(!st.is_paused);
    assert_eq!(st.total_ms, 25 * 60_000);
    assert_eq!(st.remaining_ms, 25 * 60_000);
}

#[test]
fn tick_advances_remaining_ms() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    let before = s.snapshot().remaining_ms;
    clock.advance(5 * 60_000);
    s.tick();
    assert_eq!(s.snapshot().remaining_ms, before - 5 * 60_000);
}

#[test]
fn pause_freezes_remaining() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    clock.advance(60_000);
    s.tick();
    let frozen = s.snapshot().remaining_ms;
    s.pause();
    clock.advance(30_000);
    s.tick();
    assert_eq!(s.snapshot().remaining_ms, frozen);
}

#[test]
fn resume_continues_from_paused() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    clock.advance(60_000);
    s.tick();
    let frozen = s.snapshot().remaining_ms;
    s.pause();
    clock.advance(30_000);
    s.resume();
    clock.advance(60_000);
    s.tick();
    assert_eq!(s.snapshot().remaining_ms, frozen - 60_000);
}

#[test]
fn work_finishes_and_enters_short_break() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    clock.advance(25 * 60_000);
    s.tick();
    let st = s.snapshot();
    assert_eq!(st.completed_pomodoros, 1);
    assert_eq!(st.phase, Phase::ShortBreak);
    assert!(!st.is_running);
}

fn assert_thresholds(manual: bool) {
    for threshold in [1, 2, 4] {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        let cfg = cfg_threshold(threshold);
        s.start(cfg.clone());

        for completed in 1..=threshold * 2 {
            if manual {
                s.skip();
            } else {
                clock.advance(60_000);
                s.tick();
            }
            let st = s.snapshot();
            assert_eq!(st.phase, expected_break(completed, threshold));
            assert_eq!(st.completed_pomodoros, completed);

            if completed < threshold * 2 {
                s.skip();
                s.start(cfg.clone());
            }
        }
    }
}

#[test]
fn live_timer_respects_long_break_thresholds_and_exact_counters() {
    assert_thresholds(false);
}

#[test]
fn manual_skip_respects_long_break_thresholds_and_exact_counters() {
    assert_thresholds(true);
}

#[test]
fn four_pomodoros_then_long_break() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock);
    s.start(cfg_auto_all());
    for _ in 0..4 {
        s.skip();
        if s.snapshot().phase != Phase::LongBreak {
            s.skip();
        }
    }
    let st = s.snapshot();
    assert_eq!(st.completed_pomodoros, 4);
    assert_eq!(st.phase, Phase::LongBreak);
}

#[test]
fn skip_during_work_finishes_immediately() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    clock.advance(60_000);
    s.skip();
    let st = s.snapshot();
    assert_eq!(st.completed_pomodoros, 1);
    assert_eq!(st.phase, Phase::ShortBreak);
}

#[test]
fn stop_resets_completed_counter() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_auto_all());
    s.skip();
    s.skip();
    s.skip();
    assert_eq!(s.snapshot().completed_pomodoros, 2);
    s.stop();
    let st = s.snapshot();
    assert_eq!(st.phase, Phase::Idle);
    assert_eq!(st.completed_pomodoros, 0);
    assert!(!st.is_running);
    assert_eq!(st.remaining_ms, 0);
}

#[test]
fn auto_start_break_when_enabled() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_auto_break());
    clock.advance(25 * 60_000);
    s.tick();
    let st = s.snapshot();
    assert_eq!(st.phase, Phase::ShortBreak);
    assert!(st.is_running);
    assert_eq!(st.total_ms, 5 * 60_000);
}

#[test]
fn work_min_override_applied() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock);
    s.start(SessionConfig {
        work_min_override: Some(50),
        ..SessionConfig::default()
    });
    assert_eq!(s.snapshot().total_ms, 50 * 60_000);
}

#[test]
fn skip_on_idle_is_noop() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock);
    s.skip();
    let st = s.snapshot();
    assert_eq!(st.phase, Phase::Idle);
    assert!(!st.is_running);
}

#[test]
fn snapshot_phase_ends_at_ms_some_when_running() {
    let clock = Arc::new(MockClock::new(1_000_000));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    assert_eq!(
        s.snapshot().phase_ends_at_ms,
        Some(1_000_000 + 25 * 60_000)
    );
}

#[test]
fn snapshot_phase_ends_at_ms_none_when_paused() {
    let clock = Arc::new(MockClock::new(1_000_000));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    clock.advance(60_000);
    s.tick();
    s.pause();
    let st = s.snapshot();
    assert!(st.phase_ends_at_ms.is_none());
    assert!(st.remaining_ms > 0);
}

#[test]
fn snapshot_phase_ends_at_ms_none_when_idle() {
    let clock = Arc::new(MockClock::new(1_000_000));
    let s = Session::new(clock);
    assert!(s.snapshot().phase_ends_at_ms.is_none());
}

#[test]
fn snapshot_phase_ends_at_ms_updates_on_resume() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    clock.advance(60_000);
    s.tick();
    s.pause();
    clock.advance(30_000);
    s.resume();
    assert_eq!(s.snapshot().phase_ends_at_ms, Some(90_000 + 24 * 60_000));
}

#[test]
fn persisted_roundtrip_preserves_running_state() {
    let clock = Arc::new(MockClock::new(1_000_000));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    let persisted = s.snapshot_persisted();
    assert_eq!(persisted.version, PersistedSession::CURRENT_VERSION);
    assert!(persisted.is_running);
    assert_eq!(persisted.phase, Phase::Work);

    let restored = Session::from_persisted(clock, persisted);
    let st = restored.snapshot();
    assert_eq!(st.phase, Phase::Work);
    assert!(st.is_running);
    assert!(!st.is_paused);
    assert_eq!(st.total_ms, 25 * 60_000);
    assert_eq!(st.remaining_ms, 25 * 60_000);
}

#[test]
fn persisted_restore_advances_through_expired_work_phase() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    let persisted = s.snapshot_persisted();

    let later_clock = Arc::new(MockClock::new(30 * 60_000));
    let restored = Session::from_persisted(later_clock, persisted);
    let st = restored.snapshot();
    assert_eq!(st.phase, Phase::ShortBreak);
    assert!(!st.is_running);
    assert_eq!(st.completed_pomodoros, 1);
}
