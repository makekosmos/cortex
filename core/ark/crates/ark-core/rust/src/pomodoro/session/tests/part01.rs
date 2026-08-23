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
        let after = s.snapshot().remaining_ms;
        assert!(after < before);
        assert_eq!(after, before - 5 * 60_000);
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
        s.tick(); // РґРѕР»Р¶РµРЅ Р±С‹С‚СЊ no-op (paused)
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
        clock.advance(30_000); // РІ РїР°СѓР·Рµ
        s.resume();
        clock.advance(60_000);
        s.tick();
        assert_eq!(s.snapshot().remaining_ms, frozen - 60_000);
    }

    #[test]
    fn work_finishes_and_enters_short_break() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default()); // auto_start_break = false
        clock.advance(25 * 60_000);
        s.tick();
        let st = s.snapshot();
        assert_eq!(st.completed_pomodoros, 1);
        // auto_start_break=false в†’ phase РїРµСЂРµРєР»СЋС‡РёР»СЃСЏ РЅРѕ РЅРµ Р·Р°РїСѓС‰РµРЅ
        assert_eq!(st.phase, Phase::ShortBreak);
        assert!(!st.is_running);
    }

    #[test]
    fn four_pomodoros_then_long_break() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock);
        s.start(cfg_auto_all());
        for _ in 0..4 {
            // skip work в†’ autoStartBreak в†’ break
            s.skip();
            // skip break в†’ autoStartWork в†’ next work (РїРѕСЃР»Рµ 4-РіРѕ вЂ” РЅРµС‚, longBreak СѓР¶Рµ)
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
        s.skip(); // в†’ break (completed=1)
        s.skip(); // в†’ work
        s.skip(); // в†’ break (completed=2)
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
        let cfg = SessionConfig {
            work_min_override: Some(50),
            ..SessionConfig::default()
        };
        s.start(cfg);
        let st = s.snapshot();
        assert_eq!(st.total_ms, 50 * 60_000);
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
        let st = s.snapshot();
        assert_eq!(
            st.phase_ends_at_ms,
            Some(1_000_000 + 25 * 60_000),
            "running session must expose wallclock phase_ends_at_ms"
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
        assert!(
            st.phase_ends_at_ms.is_none(),
            "paused session must hide phase_ends_at_ms вЂ” renderer falls back РЅР° remaining_ms"
        );
        // remaining_ms РІСЃС‘ РµС‰С‘ РґРѕСЃС‚СѓРїРµРЅ.
        assert!(st.remaining_ms > 0);
    }

    #[test]
    fn snapshot_phase_ends_at_ms_none_when_idle() {
        let clock = Arc::new(MockClock::new(1_000_000));
        let s = Session::new(clock);
        let st = s.snapshot();
        assert!(st.phase_ends_at_ms.is_none());
    }

    #[test]
    fn snapshot_phase_ends_at_ms_updates_on_resume() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        clock.advance(60_000);
        s.tick();
        s.pause();
        // РІ РїР°СѓР·Рµ clock СѓС…РѕРґРёС‚ РІРїРµСЂС‘Рґ, anchor РґРѕР»Р¶РµРЅ РїРµСЂРµСЃС‡РёС‚Р°С‚СЊСЃСЏ РЅР° resume
        clock.advance(30_000);
        s.resume();
        let st = s.snapshot();
        // remaining_ms Р±С‹Р» 24*60_000, anchor = now (90_000) + remaining_ms
        assert_eq!(st.phase_ends_at_ms, Some(90_000 + 24 * 60_000));
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

        // Reconstruct РїРѕРґ С‚РµРј Р¶Рµ clock вЂ” phase_ends_at_ms РµС‰С‘ РІ Р±СѓРґСѓС‰РµРј.
        let restored = Session::from_persisted(clock.clone(), persisted);
        let st = restored.snapshot();
        assert_eq!(st.phase, Phase::Work);
        assert!(st.is_running);
        assert!(!st.is_paused);
        assert_eq!(st.total_ms, 25 * 60_000);
        // remaining_ms РјРѕРіР»Рѕ РёР·РјРµРЅРёС‚СЊСЃСЏ РѕС‚ tick'Р° РЅР° restore вЂ” РЅРѕ СЂР°Р·
        // clock РЅРµ РґРІРёРіР°Р»СЃСЏ, РѕРЅРѕ в‰€ total_ms.
        assert_eq!(st.remaining_ms, 25 * 60_000);
    }

    #[test]
    fn persisted_restore_advances_through_expired_work_phase() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default()); // work, ends at 25*60_000
        let persisted = s.snapshot_persisted();

        // Р­РјСѓР»РёСЂСѓРµРј downtime: 30 РјРёРЅСѓС‚ РїСЂРѕС€Р»Рѕ, work-С„Р°Р·Р° РёСЃС‚РµРєР»Р°.
        let later_clock = Arc::new(MockClock::new(30 * 60_000));
        let restored = Session::from_persisted(later_clock, persisted);
        let st = restored.snapshot();
        // auto_start_break=false в†’ advance РѕСЃС‚Р°РЅРѕРІРёР»СЃСЏ РЅР° ShortBreak idle.
        assert_eq!(st.phase, Phase::ShortBreak);
        assert!(!st.is_running);
        assert_eq!(st.completed_pomodoros, 1);
    }
