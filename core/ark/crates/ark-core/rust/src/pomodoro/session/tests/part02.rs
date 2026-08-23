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

        // Даже если clock уехал далеко в будущее — paused state не advance'ится.
        let later_clock = Arc::new(MockClock::new(10 * 60 * 60_000));
        let restored = Session::from_persisted(later_clock, persisted);
        let st = restored.snapshot();
        assert_eq!(st.phase, Phase::Work);
        assert!(st.is_paused);
        // remaining_ms сохранилось со времени паузы (24 * 60_000).
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
        // auto_start_break + auto_start_work + долгий downtime → advance
        // должен остановиться на MAX_ITER даже если бесконечно бы цеплялся.
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_auto_all());
        let persisted = s.snapshot_persisted();

        // Огромный downtime — 24 часа.
        let later_clock = Arc::new(MockClock::new(24 * 60 * 60_000));
        let restored = Session::from_persisted(later_clock, persisted);
        // Просто проверяем что вернулись — не зациклились.
        let _st = restored.snapshot();
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
