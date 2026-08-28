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

        if s.is_paused {
            return s;
        }

        let now = s.clock.now_ms();
        let mut iterations = 0;
        const MAX_ITER: u32 = 10;
        while s.is_running && !s.is_paused && s.phase_ends_at_ms <= now && iterations < MAX_ITER {
            iterations += 1;
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
                let total = s.duration_ms_for_phase(next, &cfg);
                s.phase = next;
                s.total_ms = total;
                s.remaining_ms = total;
                s.is_running = true;
                s.is_paused = false;
                s.phase_ends_at_ms = now.saturating_add(total);
                s.last_config = Some(cfg);
            } else {
                let total = s.duration_ms_for_phase(next, &cfg);
                s.phase = next;
                s.total_ms = total;
                s.remaining_ms = total;
                break;
            }
        }

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
            Phase::Work => (cfg.work_min_override.unwrap_or(cfg.work_min) as u64) * 60_000,
            Phase::ShortBreak => (cfg.short_break_min as u64) * 60_000,
            Phase::LongBreak => (cfg.long_break_min as u64) * 60_000,
        }
    }

    fn next_phase_after(&self, p: Phase, cfg: &SessionConfig) -> Phase {
        match p {
            Phase::Work => {
                // Work completions are recorded before selecting the next phase.
                // Compare the persisted count directly so the just-finished interval
                // is not counted a second time.
                if self.completed_pomodoros >= cfg.pomodoros_until_long_break {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            }
            Phase::ShortBreak | Phase::LongBreak => Phase::Work,
            Phase::Idle => Phase::Work,
        }
    }

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

    pub fn start(&mut self, config: SessionConfig) {
        let target = if self.phase == Phase::Idle {
            Phase::Work
        } else {
            self.phase
        };
        self.start_phase(target, config);
    }

    pub fn pause(&mut self) {
        if !self.is_running || self.is_paused {
            return;
        }
        self.is_paused = true;
    }

    pub fn resume(&mut self) {
        if !self.is_paused {
            return;
        }
        self.is_paused = false;
        self.phase_ends_at_ms = self.clock.now_ms().saturating_add(self.remaining_ms);
    }

    pub fn tick(&mut self) {
        if !self.is_running || self.is_paused {
            return;
        }
        self.remaining_ms = self.phase_ends_at_ms.saturating_sub(self.clock.now_ms());
        let _ = self.events_tx.send(SessionEvent::Tick {
            state: self.snapshot(),
        });
        if self.remaining_ms == 0 {
            self.finish_phase();
        }
    }
}
