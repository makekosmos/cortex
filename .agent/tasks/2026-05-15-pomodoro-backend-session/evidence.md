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

---

## Wave 3 follow-up: phaseEndsAtMs tech debt closed

Date: 2026-05-15
Branch: main (всё ещё не pushed)

### Контекст

Wave 2 поднял backend ticker до 250 ms потому что renderer'у нужны были
частые snapshots чтобы UI заметно «тикал». Это antipattern — backend
тратил 4× больше CPU/WS frames чем нужно лишь ради smooth UI.

Правильное решение: backend кладёт `phaseEndsAtMs` (Unix ms wallclock)
в snapshot/events; renderer интерполирует
`remainingMs = max(0, phaseEndsAtMs - Date.now())` локально каждые
~33 ms (30 fps); backend тикает 1 Hz keep-alive для consistency
`is_running` / `completed_pomodoros` и доставки `phase_changed` /
`finished` событий.

### Acceptance criteria

| AC  | Описание | Статус |
|-----|----------|--------|
| AC1 | `SessionState.phase_ends_at_ms: Option<u64>` (`Some` когда running && !paused; `None` иначе). `remaining_ms` остаётся в snapshot для paused/idle. | **PASS** — `crates/ark-core/rust/src/pomodoro/session.rs` (см. impl + 4 новых unit-теста). |
| AC2 | Backend ticker → 1 Hz. `horologion-pomodoro.spec.ts` продолжает PASS. | **PASS** — `services/kepler-backend/src/pomodoro_host.rs::ticker_loop` (`Duration::from_secs(1)`); Playwright spec ✓ (10s). |
| AC3 | Renderer `usePomodoroSession.ts` интерполирует remainingMs локально (33 ms timer, server-driven fallback на null anchor, pause→freeze, resume→new anchor). | **PASS** — `extensions/horologion/src/lib/usePomodoroSession.ts` (`applyState` + `recomputeFromAnchor`). |
| AC4 | Rust unit-тесты на `phase_ends_at_ms` Some/None branches. | **PASS** — 4 новых теста: `snapshot_phase_ends_at_ms_some_when_running`, `_none_when_paused`, `_none_when_idle`, `_updates_on_resume`. Всего 16 unit-тестов в `pomodoro::session::tests`. |
| AC5 | TS coverage interpolation поведения. | **PASS** — `extensions/horologion/tests/usePomodoroSession.test.ts` (4 теста). Существующий `usePomodoro.test.ts` (11 тестов) оставлен для deprecated impl. |
| AC6 | `pomodoro-persistence.spec.ts` PASS — backend 1Hz survives renderer close. | **PASS** — 13.5s, time decreases ≥3s между close/reopen. |
| AC7 | `horologion-pomodoro.spec.ts` PASS — UI ticks visibly через local interpolation, не backend tick rate. | **PASS** — 10.0s. |
| AC8 | Bench `Session::snapshot()` отдельно, <100ns. | **PASS** — **13.75 ns** (см. raw ниже). |
| AC9 | Все 14 Playwright PASS. | **PASS** — 14/14 (1.7m). |

**Все 9 AC = PASS.**

### Raw evidence (Wave 3)

#### cargo test (Phase C1)

```
running 16 tests
test pomodoro::session::tests::snapshot_phase_ends_at_ms_some_when_running ... ok
test pomodoro::session::tests::snapshot_phase_ends_at_ms_none_when_paused ... ok
test pomodoro::session::tests::snapshot_phase_ends_at_ms_none_when_idle ... ok
test pomodoro::session::tests::snapshot_phase_ends_at_ms_updates_on_resume ... ok
... (12 baseline tests)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured
```

#### cargo test --lib (Phase C2 — kepler-backend)

```
test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

#### bun test (Phase C4)

```
tests\usePomodoro.test.ts (11 pass)
tests\usePomodoroSession.test.ts (4 pass)
Ran 15 tests across 2 files. [425.00ms]
```

#### Playwright (Phase C4)

```
Running 14 tests using 1 worker

[1/14] tests\e2e\delphi-persistence.spec.ts
[2/14] tests\e2e\delphi-tasks.spec.ts
[3/14] tests\e2e\delphi.spec.ts
[4/14] tests\e2e\extension-ark-bridge.spec.ts
[5/14] tests\e2e\horologion-persistence.spec.ts (pomodoro entry)
[6/14] tests\e2e\horologion-persistence.spec.ts (stopwatch entry)
[7/14] tests\e2e\horologion-persistence.spec.ts (mode hint)
[8/14] tests\e2e\horologion-pomodoro.spec.ts
[9/14] tests\e2e\horologion-stopwatch.spec.ts
[10/14] tests\e2e\horologion-toggle-hide.spec.ts
[11/14] tests\e2e\horologion.spec.ts
[12/14] tests\e2e\launcher.spec.ts (launcher up)
[13/14] tests\e2e\launcher.spec.ts (single window)
[14/14] tests\e2e\pomodoro-persistence.spec.ts

  14 passed (1.7m)
```

#### cargo bench (Phase C5, --quick)

```
pomodoro_session_tick_single
    time:   [15.852 ns 15.906 ns 16.122 ns]
pomodoro_session_full_work_phase_1500_ticks
    time:   [39.744 µs 41.012 µs 41.329 µs]
pomodoro_session_snapshot_only
    time:   [13.646 ns 13.751 ns 14.175 ns]
```

`snapshot()` в isolation — **13.75 ns** (далеко <100 ns target).
`tick() + snapshot()` — **15.9 ns**. На 1Hz ticker'е cost полностью
поглощается одним WS frame'ом — backend overhead negligible.

### Wave 3 commits

```
cc2256e  feat(ark-core): SessionState.phase_ends_at_ms — backend wallclock anchor для renderer interpolation
b999270  feat(kepler-backend): pomodoro ticker 250ms → 1s, events include phaseEndsAtMs
d5c38cc  feat(horologion): usePomodoroSession интерполирует remainingMs локально по phaseEndsAtMs
3fa7811  test(horologion): coverage phaseEndsAtMs interpolation в usePomodoroSession
<this>   docs: phaseEndsAtMs wave 3 follow-up evidence
```

### Wave 3 open blockers

Нет. 5 коммитов локально, не pushed. Никаких `--no-verify` / `--no-edit` /
`push --force`. Pre-Wave 3 решение из Wave 2 (250ms ticker) полностью
отменено — backend снова 1 Hz, UI smoothness через renderer interpolation.
