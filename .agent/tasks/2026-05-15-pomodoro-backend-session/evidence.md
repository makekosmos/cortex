# Evidence — Wave 2: Pomodoro backend session

Date: 2026-05-15
Branch: main (ahead of origin; **не пушено**, оставлено user'у)

## Summary

State machine pomodoro мигрирован renderer → backend. Renderer теперь
thin subscriber, state живёт в `kepler-backend` (Rust singleton
`PomodoroHost`). Это даёт **persistence**: окно закрыть → backend всё
тикает → окно открыть → текущее состояние возвращается через
`pomodoro.get_state`.

Не perf-история (хотя bench показывает, что Rust state machine — 15ns
на tick, 40µs на full work-phase). Cost оплачивается ws-транспортом.

## Acceptance criteria

| AC | Описание | Статус |
|----|----------|--------|
| AC1 | TS golden tests на текущий usePomodoro state machine | **PASS** — 11 bun-test сценариев (`extensions/horologion/tests/usePomodoro.test.ts`) |
| AC2 | Rust `pomodoro::Session` модуль с public API | **PASS** — `crates/ark-core/rust/src/pomodoro/{mod,session,events,clock}.rs` |
| AC3 | Unit tests Rust state machine с теми же сценариями + MockClock | **PASS** — 12 unit tests в `pomodoro::session::tests` |
| AC4 | Parity — Rust output совпадает с TS на одинаковых input'ах | **PASS** — same scenarios assert'ятся в обоих наборах тестов (start/tick/pause/resume/finish-work/4-cycle-long-break/skip/stop/auto-start/workMinOverride) |
| AC5 | ARK ops exposed: `pomodoro.{start,pause,resume,skip,stop,get_state}` | **PASS** — `services/kepler-backend/src/pomodoro_host.rs::handle_pomodoro_op`, intercepted в `ws_server.rs` (strip prefix `pomodoro.`) |
| AC6 | Backend emits flat events `pomodoro_{tick,phase_changed,finished}` через ws_server broadcast | **PASS** — `forward_session_events` task + per-connection select на `pomo_rx.recv()` в `handle_connection` |
| AC7 | Renderer `usePomodoroSession.ts` thin wrapper, та же reactive shape | **PASS** — `extensions/horologion/src/lib/usePomodoroSession.ts` (340 LOC); PomodoroView / HomeView / main.ts переключены |
| AC8 | Time entries создаются renderer'ом на phase_changed events | **PASS** — `createArkEntry` / `closeArkEntry` вызываются в `pomodoro_phase_changed` subscribe-handler'е в usePomodoroSession |
| AC9 | Timer переживает renderer crash (close window, reopen) | **PASS** — `pomodoro-persistence.spec.ts` Playwright e2e ✓ |
| AC10 | Timer переживает renderer reload (state восстанавливается из get_state) | **PASS** — covered by AC9 e2e (close-and-reopen = reload в этом контексте) |
| AC11 | Playwright e2e: open → start → close window → wait → reopen → time продвинулся | **PASS** — `tests/e2e/pomodoro-persistence.spec.ts:1:1` (13.5s) |
| AC12 | Все existing e2e PASS | **PASS** — 14/14 (был 13 + новый persistence; spec'овский счёт «22+» оказался завышенным — фактический baseline 13) |

**Все 12 AC = PASS.**

## Raw evidence

### bun test (Phase 1)

```
bun test v1.3.5 (1e86cebd)
tests\usePomodoro.test.ts:
 11 pass
 0 fail
 35 expect() calls
Ran 11 tests across 1 file. [422.00ms]
```

### cargo test (Phase 2)

```
running 12 tests
test pomodoro::session::tests::four_pomodoros_then_long_break ... ok
test pomodoro::session::tests::tick_advances_remaining_ms ... ok
test pomodoro::session::tests::stop_resets_completed_counter ... ok
test pomodoro::session::tests::pause_freezes_remaining ... ok
test pomodoro::session::tests::subscribe_receives_phase_changed_on_start ... ok
test pomodoro::session::tests::work_finishes_and_enters_short_break ... ok
test pomodoro::session::tests::auto_start_break_when_enabled ... ok
test pomodoro::session::tests::skip_on_idle_is_noop ... ok
test pomodoro::session::tests::start_from_idle_enters_work ... ok
test pomodoro::session::tests::resume_continues_from_paused ... ok
test pomodoro::session::tests::skip_during_work_finishes_immediately ... ok
test pomodoro::session::tests::work_min_override_applied ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured
```

### cargo test --lib (Phase 3 — kepler-backend full)

```
running 55 tests
... (50 pre-existing tests pass)
test pomodoro_host::tests::get_state_idle_initially ... ok
test pomodoro_host::tests::unknown_subop_errors ... ok
test pomodoro_host::tests::start_returns_running_work_state ... ok
test pomodoro_host::tests::stop_resets_to_idle ... ok
test pomodoro_host::tests::start_emits_phase_changed_event ... ok
test result: ok. 55 passed; 0 failed
```

### Playwright (Phase 5 + Phase 6 + AC12)

```
Running 14 tests using 1 worker

  ok  1 tests\e2e\delphi-persistence.spec.ts (13.8s)
  ok  2 tests\e2e\delphi-tasks.spec.ts (8.5s)
  ok  3 tests\e2e\delphi.spec.ts (4.5s)
  ok  4 tests\e2e\extension-ark-bridge.spec.ts (1.9s)
  ok  5 tests\e2e\horologion-persistence.spec.ts (pomodoro entry) (8.2s)
  ok  6 tests\e2e\horologion-persistence.spec.ts (stopwatch entry) (8.5s)
  ok  7 tests\e2e\horologion-persistence.spec.ts (mode hint) (7.6s)
  ok  8 tests\e2e\horologion-pomodoro.spec.ts (10.0s)
  ok  9 tests\e2e\horologion-stopwatch.spec.ts (8.5s)
  ok 10 tests\e2e\horologion-toggle-hide.spec.ts (7.5s)
  ok 11 tests\e2e\horologion.spec.ts (4.6s)
  ok 12 tests\e2e\launcher.spec.ts (launcher up) (821ms)
  ok 13 tests\e2e\launcher.spec.ts (single window) (2.4s)
  ok 14 tests\e2e\pomodoro-persistence.spec.ts (13.5s)

  14 passed (1.7m)
```

### cargo bench (Phase 7)

```
pomodoro_session_tick_single
    time:   [15.476 ns 15.589 ns 15.705 ns]
pomodoro_session_full_work_phase_1500_ticks
    time:   [39.207 µs 39.603 µs 40.019 µs]
```

Single `tick()` — **15.6 ns**. Full work phase (1500 ticks across 25
simulated minutes) — **39.6 µs**. State machine compute cost — negligible
относительно WS round-trip (~ms range). Это confirm'ит: wave 2 — про
persistence, не про speed.

## Commits

```
420b596  test(horologion): golden tests for usePomodoro state machine
6dba5a8  feat(ark-core): pomodoro::Session state machine + Clock + tests
752a69a  feat(kepler-backend): expose pomodoro.* ARK ops + WS events
64ce452  feat(horologion): usePomodoroSession thin wrapper subscribed to backend
b0052a8  refactor(horologion): switch PomodoroView с usePomodoro → usePomodoroSession
ef9b7cb  test(e2e): pomodoro session переживает renderer reload + ticker 1s→250ms
<this>   docs(.agent): evidence.md для wave 2 — pomodoro backend session
```

## Decisions / открытые

- **Ticker 250ms (не 1000ms).** 1s frequency давал first-tick latency
  ~700-1700ms post-start, что валил `horologion-pomodoro.spec.ts`
  (тот ждёт 1500ms после клика «Начать сессию» и ожидает что
  `time != initial`). 250ms — parity с оригинальным TS `setInterval`,
  ws-overhead minimal (4 events/sec, маленькие JSON). Альтернатива:
  включить `phase_ends_at_ms` в snapshot и вычислять remainingMs в
  renderer'е по wallclock — wave 3.

- **`usePomodoro.ts` оставлен как `@deprecated`, не удалён.**
  Spec'овское ограничение явное; rollback path сохраняем. После двух
  недель stable production'а можно удалить.

- **Pomodoro settings (workMin/shortBreakMin/...) живут в renderer'е
  (localStorage)**, передаются в backend каждым `pomodoro.start`-вызовом.
  Backend stateless по settings. Если когда-нибудь захотим server-side
  settings (multi-renderer consistency, autostart после shell restart)
  — это отдельная история.

- **Multi-renderer не tested explicitly.** Контракт — backend broadcasts
  всем subscribed клиентам, так что open двух Horologion-окон оба
  получат one и тот же state «бесплатно». Spec не требует dedicated
  теста; skipped.

## Open blockers

Нет. Все 7 коммитов локально, не pushed. Не использованы `--no-verify` /
`--no-edit` / push --force.
