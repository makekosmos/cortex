#![allow(clippy::unwrap_used)]

//! Bench для `pomodoro::Session::tick()` — оцениваем cost одного tick'а
//! plus full lifecycle (start → много ticks → finish).
//!
//! Pure compute bench (без WS, без serialization). Цель — показать, что
//! state machine latency negligible vs. сетевой overhead WS-транспорта,
//! значит миграция в backend стоит persistence, а не perf.
//!
//! Запуск: `cargo bench --manifest-path core/ark/crates/ark-core/rust/Cargo.toml \
//!   --bench pomodoro_session`.

use std::sync::Arc;

use ark_core::pomodoro::{MockClock, Session, SessionConfig};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_single_tick(c: &mut Criterion) {
    c.bench_function("pomodoro_session_tick_single", |b| {
        let clock = Arc::new(MockClock::new(0));
        let mut session = Session::new(clock.clone());
        session.start(SessionConfig::default());
        b.iter(|| {
            clock.advance(1000);
            session.tick();
            black_box(session.snapshot());
        });
    });
}

/// Snapshot-only bench — отдельно от tick(). Wave 3 переносит UI smoothness
/// на renderer-side interpolation, backend тикает 1 Hz и в основном
/// нагрузка приходится на `snapshot()` под broadcast'ом. Должен быть
/// заведомо <100ns (никакой алокации в hot path кроме clone'ов
/// `last_config.title`/`tasks` — обычно пустые).
fn bench_snapshot_only(c: &mut Criterion) {
    c.bench_function("pomodoro_session_snapshot_only", |b| {
        let clock = Arc::new(MockClock::new(0));
        let mut session = Session::new(clock);
        session.start(SessionConfig::default());
        b.iter(|| {
            black_box(session.snapshot());
        });
    });
}

fn bench_lifecycle(c: &mut Criterion) {
    c.bench_function("pomodoro_session_full_work_phase_1500_ticks", |b| {
        b.iter(|| {
            let clock = Arc::new(MockClock::new(0));
            let mut session = Session::new(clock.clone());
            session.start(SessionConfig::default());
            // 1500 ticks по секунде = 25 минут — целая work-фаза.
            for _ in 0..1500 {
                clock.advance(1000);
                session.tick();
            }
            black_box(session.snapshot());
        });
    });
}

criterion_group!(
    benches,
    bench_single_tick,
    bench_lifecycle,
    bench_snapshot_only
);
criterion_main!(benches);
