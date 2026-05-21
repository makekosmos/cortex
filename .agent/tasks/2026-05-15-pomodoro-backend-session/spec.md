# Wave 2 — Pomodoro session в backend

## Цель (НЕ скорость)

Pomodoro state machine сейчас живёт в renderer'е (`extensions/horologion/
src/lib/usePomodoro.ts`, 320 LOC). Это значит:

- **Renderer crash → state потерян.** Юзер тикает 24-ю минуту фокуса, окно
  крешнулось → таймер исчезает.
- **Reload Horologion окна → state потерян.** F5 / closeshow → начинать
  заново.
- **Несколько окон → дублирование state.** Если бы юзер открыл Horologion
  дважды — два таймера. (Сейчас singleton в одном iframe, но архитектурно
  hardcoded для single-renderer случая.)
- **Timer тики на 250ms в renderer.** Если renderer thread занят (heavy
  Vue re-render, animation) — тики дрифтят.

**Backend persistence — реальная нужда. Не perf-tweak, а correctness.**

## Архитектура (Wave 2)

```
┌──────────────────────────────────────────────────────────────┐
│ Renderer (Horologion / Дashboard / любой)                    │
│  PomodoroView.vue                                            │
│   ↓ subscribe                                                │
│  usePomodoroSession.ts (thin)                                │
│   • кэш state (phase, remainingMs, totalMs, ...)             │
│   • подписка на ARK events                                   │
│   • вызовы pomodoro.{start,pause,resume,stop,skip}           │
└──────┬────────────────────────────────────────────▲──────────┘
       │ kepler.ark.request                          │ events
       │   "pomodoro.start" {workMin, tasks}         │ "pomodoro_tick"
       │                                             │ "pomodoro_phase_changed"
       ↓                                             │ "pomodoro_finished"
┌──────────────────────────────────────────────────────────────┐
│ kepler-backend (Rust)                                         │
│  pomodoro module:                                             │
│   • Session struct (singleton state в Arc<Mutex<...>>)        │
│   • tokio::spawn taskticker — tick каждую секунду            │
│   • broadcast events через ws_server'овский subscribe         │
│  ARK ops:                                                     │
│   • pomodoro.get_state → текущий snapshot                     │
│   • pomodoro.start(config) → starts work phase                │
│   • pomodoro.pause / resume / skip / stop                     │
│                                                               │
│  Time entries создаются ARK ops'ами как и раньше — subscriber│
│  в Horologion (TS) ловит phase_changed event и делает         │
│  upsert_object для time_entry_obj. Side effect не в Rust.     │
└──────────────────────────────────────────────────────────────┘
```

## Acceptance criteria

### Baseline (TS golden, capture текущего поведения)

- **AC1**: TS unit tests на текущий `usePomodoro.ts` — золотые сценарии
  state machine: idle→work→shortBreak→work, completion counter,
  pause/resume/skip/stop. Mock'аем `window.horologion.timeEntries.*`
  и Date.now() для детерминистики. → PASS.

### Rust port

- **AC2**: Rust `pomodoro::Session` модуль в `crates/ark-core/rust/src/
pomodoro/{mod,session,events}.rs`. Public API:
  - `Session::start(SessionConfig)` → emit phase_changed(idle→work)
  - `Session::pause()` / `resume()` / `skip()` / `stop()`
  - `Session::tick()` — внутренний, вызывается tokio таймером
  - `Session::snapshot()` → `SessionState { phase, remaining_ms, ... }`
- **AC3**: Unit tests того же state machine с теми же сценариями.
  Деterministic clock через trait `Clock` (production uses
  `tokio::time::Instant::now`, tests use `MockClock` с manual advance).
  → PASS.
- **AC4**: parity — output state machine identical TS-impl на одинаковых
  input'ах. Verified через `cargo test`.

### Integration

- **AC5**: ARK ops exposed в `ws_server.rs`:
  - `pomodoro.start` / `pause` / `resume` / `skip` / `stop` / `get_state`
- **AC6**: Backend emits events `pomodoro_tick`, `pomodoro_phase_changed`,
  `pomodoro_finished` через ws_server'овский broadcast.
- **AC7**: Renderer `usePomodoroSession.ts` thin wrapper — calls + subs.
  Заменяет `usePomodoro.ts` для PomodoroView. Сохраняем ту же reactive
  shape (phase, remainingMs, isRunning refs).
- **AC8**: Time entries создаются renderer'ом на phase_changed events
  (как было). Side-effect side-of-the-wall не меняется в Wave 2.

### Survival

- **AC9 (PERSISTENCE CORE)**: timer переживает renderer crash. Reproduce:
  start работу через WS, симулировать close-and-reopen renderer окна,
  state не сбрасывается (phase, remainingMs сохраняются).
- **AC10**: timer переживает renderer reload. F5 в окне Horologion →
  через WS event renderer получает actual state, не starts заново.

### Tests

- **AC11**: Playwright e2e: open Horologion → start pomodoro → close window
  → wait 2s → reopen Horologion → timer на 2s продвинулся (НЕ reset).
- **AC12**: Все existing 22 e2e PASS.

## Out of scope

- **Не переносим time_entry CRUD в Rust.** Это side-effect. Renderer
  слушает phase_changed event, делает upsert через текущую горологионовскую
  shim. Wave 2 фокус — на state machine.
- **Не делаем notifications в Rust** (Notification API живёт в renderer).
  Backend шлёт event, renderer показывает notification.
- **Не делаем sound в Rust.** Renderer audio API.
- **Multi-renderer subscription** — может работать «бесплатно» (любой
  renderer subscribed к WS получает events), но dedicated test не пишем.

## Workflow (TDD, по proof loop)

1. Write TS golden tests для текущего usePomodoro behaviour. Run, PASS.
2. Write Rust Session struct + tests. Run cargo test, PASS.
3. Expose ARK ops + events в ws_server.
4. Write renderer thin wrapper.
5. Write Playwright integration test (AC11).
6. Switch PomodoroView (and any other consumer) с `usePomodoro` на
   `usePomodoroSession`. Existing horologion-pomodoro.spec.ts должен
   PASS без изменений.
7. evidence.md с PASS per AC.

## Migration plan / порядок коммитов

```
1. test(horologion): golden tests for usePomodoro state machine
2. feat(ark-core): pomodoro::Session struct + unit tests
3. feat(kepler-backend): expose pomodoro.* ARK ops + events
4. feat(horologion): usePomodoroSession thin wrapper subscribed to backend
5. test(horologion): e2e — session переживает renderer reload
6. refactor(horologion): switch PomodoroView с usePomodoro → usePomodoroSession
7. docs: .agent/tasks/2026-05-15-pomodoro-backend-session/evidence.md
```

Каждый коммит сохраняет existing 22/22 e2e PASS.
