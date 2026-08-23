impl Session {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        let (events_tx, _) = broadcast::channel::<SessionEvent>(128);
        Self {
            clock,
            events_tx,
            phase: Phase::Idle,
            remaining_ms: 0,
            total_ms: 0,
            completed_pomodoros: 0,
            is_running: false,
            is_paused: false,
            phase_ends_at_ms: 0,
            last_config: None,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.events_tx.subscribe()
    }

    /// Snapshot РґР»СЏ persistence РЅР° РґРёСЃРє (СЃРј. `PersistedSession`).
    pub fn snapshot_persisted(&self) -> PersistedSession {
        PersistedSession {
            version: PersistedSession::CURRENT_VERSION,
            phase: self.phase,
            remaining_ms: self.remaining_ms,
            total_ms: self.total_ms,
            completed_pomodoros: self.completed_pomodoros,
            is_running: self.is_running,
            is_paused: self.is_paused,
            phase_ends_at_ms: self.phase_ends_at_ms,
            last_config: self.last_config.clone(),
        }
    }

    /// Р’РѕСЃСЃС‚Р°РЅРѕРІР»РµРЅРёРµ Session РёР· persisted state. Р•СЃР»Рё Р±С‹Р» Р·Р°РїСѓС‰РµРЅ Рё
    /// `phase_ends_at_ms <= now_ms` вЂ” С„Р°Р·Р° РёСЃС‚РµРєР»Р° РІРѕ РІСЂРµРјСЏ downtime;
    /// СЌРјСѓР»РёСЂСѓРµРј `finish_phase()` РІ loop (СЃ limit 10) РґРѕ С‚РµС… РїРѕСЂ, РїРѕРєР°
    /// РЅРµ РѕРєР°Р¶РµРјСЃСЏ РІ non-expired С„Р°Р·Рµ РёР»Рё idle.
    ///
    /// РќР° restore РќР• РґС‘СЂРіР°РµРј `events_tx` РґР»СЏ intermediate advance вЂ” РєР»РёРµРЅС‚С‹
    /// РµС‰С‘ РЅРµ РїРѕРґРєР»СЋС‡РµРЅС‹. Р’РѕР·РІСЂР°С‰Р°РµРј РіРѕС‚РѕРІС‹Р№ Session вЂ” caller (pomodoro_host)
    /// РїРѕСЃР»Рµ connection РјРѕР¶РµС‚ РІСЂСѓС‡РЅСѓСЋ РІС‹СЃР»Р°С‚СЊ РѕРґРЅРѕ phase_changed РґР»СЏ С‚РµРєСѓС‰РµР№
    /// С„Р°Р·С‹.
    pub fn from_persisted(clock: Arc<dyn Clock>, persisted: PersistedSession) -> Self {
        let (events_tx, _) = broadcast::channel::<SessionEvent>(128);
        let mut s = Self {
            clock,
            events_tx,
            phase: persisted.phase,
            remaining_ms: persisted.remaining_ms,
            total_ms: persisted.total_ms,
            completed_pomodoros: persisted.completed_pomodoros,
            is_running: persisted.is_running,
            is_paused: persisted.is_paused,
            phase_ends_at_ms: persisted.phase_ends_at_ms,
            last_config: persisted.last_config,
        };

        // Р•СЃР»Рё paused вЂ” РѕСЃС‚Р°РІР»СЏРµРј РєР°Рє РµСЃС‚СЊ; РЅРёС‡РµРіРѕ РЅРµ advance'РёРј. РќР° resume
        // anchor РїРµСЂРµСЃС‡РёС‚Р°РµС‚СЃСЏ.
        if s.is_paused {
            return s;
        }

        // Р•СЃР»Рё running вЂ” advance С‡РµСЂРµР· РёСЃС‚С‘РєС€РёРµ С„Р°Р·С‹.
        let now = s.clock.now_ms();
        let mut iterations = 0;
        const MAX_ITER: u32 = 10;
        while s.is_running && !s.is_paused && s.phase_ends_at_ms <= now && iterations < MAX_ITER {
            iterations += 1;
            // Р­РјСѓР»РёСЂСѓРµРј `finish_phase()` РЅРѕ Р‘Р•Р— broadcast (РЅРёРєС‚Рѕ РЅРµ СЃР»СѓС€Р°РµС‚).
            let cfg = s.last_config.clone().unwrap_or_default();
            let finished = s.phase;
            s.is_running = false;
            s.is_paused = false;
            s.remaining_ms = 0;
            if finished == Phase::Work {
                s.completed_pomodoros += 1;
            }
            let next = s.next_phase_after(finished, &cfg);
            let should_auto = match next {
                Phase::Work => cfg.auto_start_work,
                Phase::ShortBreak | Phase::LongBreak => cfg.auto_start_break,
                Phase::Idle => false,
            };
            if should_auto {
                // Re-enter target phase silently вЂ” РїРѕРІС‚РѕСЂСЏРµРј body `start_phase`
                // Р±РµР· emit'Р° СЃРѕР±С‹С‚РёР№.
                let total = s.duration_ms_for_phase(next, &cfg);
                s.phase = next;
                s.total_ms = total;
                s.remaining_ms = total;
                s.is_running = true;
                s.is_paused = false;
                s.phase_ends_at_ms = now.saturating_add(total);
                s.last_config = Some(cfg);
                // Loop РµС‰С‘ СЂР°Р· вЂ” РјРѕР¶РµС‚ РЅРѕРІР°СЏ С„Р°Р·Р° С‚РѕР¶Рµ РёСЃС‚РµРєР»Р° (edge: РѕС‡РµРЅСЊ
                // РґР»РёРЅРЅС‹Р№ downtime + auto_start_*).
            } else {
                let total = s.duration_ms_for_phase(next, &cfg);
                s.phase = next;
                s.total_ms = total;
                s.remaining_ms = total;
                // is_running СѓР¶Рµ false вЂ” break.
                break;
            }
        }

        // Р•СЃР»Рё РµС‰С‘ running РїРѕСЃР»Рµ loop вЂ” РїРµСЂРµСЃС‡РёС‚Р°С‚СЊ remaining_ms (РєР°Рє tick()
        // Р±РµР· emit СЃРѕР±С‹С‚РёСЏ).
        if s.is_running && !s.is_paused {
            s.remaining_ms = s.phase_ends_at_ms.saturating_sub(now);
        }

        s
    }

    pub fn snapshot(&self) -> SessionState {
        let (title, tasks) = match &self.last_config {
            Some(c) => (c.title.clone(), c.tasks.clone()),
            None => (String::new(), Vec::new()),
        };
        let phase_ends_at_ms = if self.is_running && !self.is_paused {
            Some(self.phase_ends_at_ms)
        } else {
            None
        };
        SessionState {
            phase: self.phase,
            remaining_ms: self.remaining_ms,
            total_ms: self.total_ms,
            completed_pomodoros: self.completed_pomodoros,
            is_running: self.is_running,
            is_paused: self.is_paused,
            title,
            tasks,
            phase_ends_at_ms,
        }
    }

    fn duration_ms_for_phase(&self, p: Phase, cfg: &SessionConfig) -> u64 {
        match p {
            Phase::Idle => 0,
            Phase::Work => {
                let min = cfg.work_min_override.unwrap_or(cfg.work_min);
                (min as u64) * 60_000
            }
            Phase::ShortBreak => (cfg.short_break_min as u64) * 60_000,
            Phase::LongBreak => (cfg.long_break_min as u64) * 60_000,
        }
    }

    fn next_phase_after(&self, p: Phase, cfg: &SessionConfig) -> Phase {
        match p {
            Phase::Work => {
                if self.completed_pomodoros + 1 >= cfg.pomodoros_until_long_break {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            }
            Phase::ShortBreak | Phase::LongBreak => Phase::Work,
            Phase::Idle => Phase::Work,
        }
    }

    /// Р—Р°РїСѓСЃРє С„Р°Р·С‹ p СЃ config cfg. РРґРµРјРїРѕС‚РµРЅС‚РЅРѕ РїРµСЂРµР·Р°С‚РёСЂР°РµС‚ state.
    fn start_phase(&mut self, p: Phase, cfg: SessionConfig) {
        let from = self.phase;
        let total = self.duration_ms_for_phase(p, &cfg);
        let now = self.clock.now_ms();
        self.phase = p;
        self.total_ms = total;
        self.remaining_ms = total;
        self.is_running = true;
        self.is_paused = false;
        self.phase_ends_at_ms = now.saturating_add(total);
        self.last_config = Some(cfg);
        let _ = self.events_tx.send(SessionEvent::PhaseChanged {
            from,
            to: p,
            state: self.snapshot(),
        });
    }

    /// `start(config)` вЂ” entry point РёР· ARK op `pomodoro.start`. Р•СЃР»Рё phase==idle
    /// в†’ start work. Р•СЃР»Рё paused в†’ resume. Р•СЃР»Рё СѓР¶Рµ running вЂ” re-start СЃ С‚РµРј Р¶Рµ
    /// config (РєР°Рє TS вЂ” `start()` РїРµСЂРµ-РІС…РѕРґРёС‚ РІ startPhase).
    pub fn start(&mut self, config: SessionConfig) {
        let target = if self.phase == Phase::Idle {
            Phase::Work
        } else {
            self.phase
        };
        self.start_phase(target, config);
    }

    // NOTE: emits no events вЂ” host must persist explicitly.
    pub fn pause(&mut self) {
        if !self.is_running || self.is_paused {
            return;
        }
        self.is_paused = true;
    }

    // NOTE: emits no events вЂ” host must persist explicitly.
    pub fn resume(&mut self) {
        if !self.is_paused {
            return;
        }
        self.is_paused = false;
        let now = self.clock.now_ms();
        self.phase_ends_at_ms = now.saturating_add(self.remaining_ms);
    }

    /// Tick вЂ” РїРµСЂРµСЃС‡РёС‚Р°С‚СЊ remaining_ms. Р•СЃР»Рё remaining=0 в†’ finish_phase().
    pub fn tick(&mut self) {
        if !self.is_running || self.is_paused {
            return;
        }
        let now = self.clock.now_ms();
        self.remaining_ms = self.phase_ends_at_ms.saturating_sub(now);
        // Emit Tick event (subscribers РјРѕРіСѓС‚ throttle).
        let _ = self.events_tx.send(SessionEvent::Tick {
            state: self.snapshot(),
        });
        if self.remaining_ms == 0 {
            self.finish_phase();
        }
    }

}

