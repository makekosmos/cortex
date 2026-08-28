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
fn persisted_restore_auto_chain_terminates_under_max_iter() {
    let clock = Arc::new(MockClock::new(0));
    let mut s = Session::new(clock.clone());
    s.start(cfg_auto_all());
    let persisted = s.snapshot_persisted();

    let later_clock = Arc::new(MockClock::new(24 * 60 * 60_000));
    let restored = Session::from_persisted(later_clock, persisted);
    let _ = restored.snapshot();
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


fn cfg_with_threshold(pomodoros_until_long_break: u32) -> SessionConfig {
    SessionConfig {
        pomodoros_until_long_break,
        ..cfg_default()
    }
}

#[test]
fn long_break_threshold_one_uses_first_completed_work_interval() {
    let clock = Arc::new(MockClock::new(0));
    let mut session = Session::new(clock);
    session.start(cfg_with_threshold(1));

    session.skip();

    let state = session.snapshot();
    assert_eq!(state.completed_pomodoros, 1);
    assert_eq!(state.phase, Phase::LongBreak);
}

#[test]
fn long_break_threshold_two_uses_second_completed_work_interval() {
    let clock = Arc::new(MockClock::new(0));
    let mut session = Session::new(clock);
    session.start(cfg_with_threshold(2));

    session.skip();
    assert_eq!(session.snapshot().completed_pomodoros, 1);
    assert_eq!(session.snapshot().phase, Phase::ShortBreak);

    session.skip();
    assert_eq!(session.snapshot().phase, Phase::Work);

    session.skip();

    let state = session.snapshot();
    assert_eq!(state.completed_pomodoros, 2);
    assert_eq!(state.phase, Phase::LongBreak);
}

#[test]
fn long_break_threshold_four_keeps_first_three_intervals_short() {
    let clock = Arc::new(MockClock::new(0));
    let mut session = Session::new(clock);
    session.start(cfg_with_threshold(4));

    for completed in 1..=3 {
        session.skip();
        let state = session.snapshot();
        assert_eq!(state.completed_pomodoros, completed);
        assert_eq!(state.phase, Phase::ShortBreak);
        session.skip();
        assert_eq!(session.snapshot().phase, Phase::Work);
    }

    session.skip();

    let state = session.snapshot();
    assert_eq!(state.completed_pomodoros, 4);
    assert_eq!(state.phase, Phase::LongBreak);
}

#[test]
fn restored_overdue_work_uses_same_long_break_boundary() {
    let clock = Arc::new(MockClock::new(0));
    let persisted = PersistedSession {
        version: PersistedSession::CURRENT_VERSION,
        phase: Phase::Work,
        remaining_ms: 0,
        total_ms: 25 * 60_000,
        completed_pomodoros: 3,
        is_running: true,
        is_paused: false,
        phase_ends_at_ms: 0,
        last_config: Some(cfg_with_threshold(4)),
    };

    let restored = Session::from_persisted(clock, persisted);

    let state = restored.snapshot();
    assert_eq!(state.completed_pomodoros, 4);
    assert_eq!(state.phase, Phase::LongBreak);
}
