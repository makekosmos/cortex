#[test]
fn persisted_restore_preserves_paused_state() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_default());
    clock.advance(60_000);
    s.tick();
    s.pause();
    let persisted = s.snapshot_persisted();
    assert!(persisted.is_paused);

    let later_clock = Arc::new(MockClock::new(10 * 60 * 60_000));
    let restored = Session::from_persisted(later_clock, persisted);
    let st = restored.snapshot();
    assert_eq!(st.phase, Phase::Work);
    assert!(st.is_paused);
    assert!(st.remaining_ms > 0);
}

#[test]
fn persisted_restore_idle_remains_idle() {
    let clock = Arc::new(MockClock::new(0));
    let s = Session::new(clock.clone());
    let persisted = s.snapshot_persisted();
    let restored = Session::from_persisted(clock, persisted);
    let st = restored.snapshot();
    assert_eq!(st.phase, Phase::Idle);
    assert!(!st.is_running);
}

#[test]
fn persisted_restore_auto_chain_catches_up_past_ten_phases() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    let cfg = SessionConfig {
        auto_start_break: true,
        auto_start_work: true,
        short_break_min: 1,
        ..cfg_threshold(4)
    };
    s.start(cfg);
    let persisted = s.snapshot_persisted();

    let later_clock = Arc::new(MockClock::new(25 * 60_000 + 15_000));
    let restored = Session::from_persisted(later_clock, persisted);
    let state = restored.snapshot();
    assert_eq!(state.phase, Phase::ShortBreak);
    assert_eq!(state.completed_pomodoros, 6);
    assert!(state.is_running);
    assert_eq!(state.remaining_ms, 45_000);
}

#[test]
fn subscribe_receives_phase_changed_on_start() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock);
    let mut rx = s.subscribe();
    s.start(cfg_default());
    let evt = rx.try_recv().expect("should receive event");
    match evt {
        SessionEvent::PhaseChanged { from, to, .. } => {
            assert_eq!(from, Phase::Idle);
            assert_eq!(to, Phase::Work);
        }
        other => unreachable!("expected PhaseChanged, got {other:?}"),
    }
}

#[test]
fn overdue_restore_matches_live_thresholds_and_counters() {
    for threshold in [1, 2, 4] {
        let clock = Arc::new(MockClock::new(0));
        let cfg = cfg_threshold(threshold);
        let mut live = Session::new(clock.clone());
        live.start(cfg.clone());
        for _ in 1..threshold {
            live.skip();
            live.skip();
            live.start(cfg.clone());
        }
        let persisted = live.snapshot_persisted();
        clock.advance(60_000);
        live.tick();
        let restored = Session::from_persisted(Arc::new(MockClock::new(60_000)), persisted);
        let live = live.snapshot();
        let restored = restored.snapshot();
        assert_eq!(live.phase, expected_break(threshold, threshold));
        assert_eq!(restored.phase, live.phase);
        assert_eq!(live.completed_pomodoros, threshold);
        assert_eq!(restored.completed_pomodoros, threshold);
    }
}

#[test]
fn overdue_restore_auto_starts_multiple_phases_with_exact_state() {
    let clock = Arc::new(MockClock::new(0));
    let cfg = SessionConfig {
        auto_start_break: true,
        auto_start_work: true,
        short_break_min: 1,
        ..cfg_threshold(4)
    };
    let mut session = Session::new(clock);
    session.start(cfg.clone());

    let restored = Session::from_persisted(Arc::new(MockClock::new(3 * 60_000 + 15_000)), session.snapshot_persisted());
    let state = restored.snapshot();
    assert_eq!(state.phase, Phase::ShortBreak);
    assert_eq!(state.completed_pomodoros, 2);
    assert!(state.is_running);
    assert_eq!(state.remaining_ms, 45_000);
}

#[test]
fn boundary_phase_changed_event_has_corrected_state() {
    let clock = Arc::new(MockClock::new(0));
    let mut session = Session::new(clock.clone());
    let mut events = session.subscribe();
    session.start(cfg_default());
    let _ = events.try_recv();

    clock.advance(25 * 60_000);
    session.tick();
    let _ = events.try_recv();
    let _ = events.try_recv();
    match events.try_recv().expect("phase change at boundary") {
        SessionEvent::PhaseChanged { from, to, state } => {
            assert_eq!(from, Phase::Work);
            assert_eq!(to, Phase::ShortBreak);
            assert_eq!(state.phase, Phase::ShortBreak);
            assert_eq!(state.completed_pomodoros, 1);
        }
        other => unreachable!("expected PhaseChanged, got {other:?}"),
    }
}

#[test]
fn paused_restore_does_not_increment_counter() {
    let clock = Arc::new(MockClock::new(0));
    let mut session = Session::new(clock.clone());
    session.start(cfg_threshold(1));
    clock.advance(1_000);
    session.tick();
    session.pause();
    let restored = Session::from_persisted(Arc::new(MockClock::new(60_000)), session.snapshot_persisted());
    let state = restored.snapshot();
    assert!(state.is_paused);
    assert_eq!(state.phase, Phase::Work);
    assert_eq!(state.completed_pomodoros, 0);
}

#[test]
fn changing_threshold_after_progress_updates_next_completion() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock);
    s.start(cfg_threshold(4));
    s.skip();
    s.start(cfg_threshold(2));
    s.skip();
    s.start(cfg_threshold(2));
    s.skip();
    let state = s.snapshot();
    assert_eq!(state.completed_pomodoros, 2);
    assert_eq!(state.phase, Phase::LongBreak);
}

#[test]
fn pause_resume_then_completion_increments_counter_once() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_threshold(2));
    clock.advance(30_000);
    s.tick();
    s.pause();
    clock.advance(5 * 60_000);
    s.resume();
    clock.advance(30_000);
    s.tick();
    let state = s.snapshot();
    assert_eq!(state.completed_pomodoros, 1);
    assert_eq!(state.phase, Phase::ShortBreak);
    assert!(!state.is_running);
}
