impl Session {
    fn finish_phase(&mut self) {
        let finished = self.phase;
        self.is_running = false;
        self.is_paused = false;
        self.remaining_ms = 0;

        if finished == Phase::Work {
            self.completed_pomodoros += 1;
        }

        let cfg = self.last_config.clone().unwrap_or_default();
        let next = self.next_phase_after(finished, &cfg);
        let _ = self.events_tx.send(SessionEvent::Finished {
            finished,
            state: self.snapshot(),
        });

        let should_auto = match next {
            Phase::Work => cfg.auto_start_work,
            Phase::ShortBreak | Phase::LongBreak => cfg.auto_start_break,
            Phase::Idle => false,
        };

        if should_auto {
            self.start_phase(next, cfg);
        } else {
            let total = self.duration_ms_for_phase(next, &cfg);
            let from = self.phase;
            self.phase = next;
            self.total_ms = total;
            self.remaining_ms = total;
            let _ = self.events_tx.send(SessionEvent::PhaseChanged {
                from,
                to: next,
                state: self.snapshot(),
            });
        }
    }

    pub fn skip(&mut self) {
        if self.phase == Phase::Idle && !self.is_running {
            return;
        }
        if self.is_running {
            self.remaining_ms = 0;
            self.finish_phase();
        } else {
            let cfg = self.last_config.clone().unwrap_or_default();
            let next = self.next_phase_after(self.phase, &cfg);
            let total = self.duration_ms_for_phase(next, &cfg);
            let from = self.phase;
            self.phase = next;
            self.total_ms = total;
            self.remaining_ms = total;
            let _ = self.events_tx.send(SessionEvent::PhaseChanged {
                from,
                to: next,
                state: self.snapshot(),
            });
        }
    }

    pub fn stop(&mut self) {
        let from = self.phase;
        self.is_running = false;
        self.is_paused = false;
        self.phase = Phase::Idle;
        self.remaining_ms = 0;
        self.total_ms = 0;
        self.completed_pomodoros = 0;
        let _ = self.events_tx.send(SessionEvent::PhaseChanged {
            from,
            to: Phase::Idle,
            state: self.snapshot(),
        });
    }
}
