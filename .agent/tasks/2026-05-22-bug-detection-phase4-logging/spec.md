# 2026-05-22 bug-detection-phase4-logging

## Context

Сейчас Rust backend пишет логи через `eprintln!`/`println!` (~48 случаев),
shell main process — через `console.error("[kepler-shell] ...")` (~48 случаев).
Оба идут только в stderr, который пропадает после exit процесса. Реальный
пользователь, который репортит баг, не может прислать логи — их физически нет
на диске после крэша.

`crash_reporter` пишет panic-логи и minidumps в `<data_dir>/crashes/`, но это
только для **panic'ов**. Обычные warning'и / error'ы / диагностические события
теряются.

Phase 4 из bug-detection roadmap.

## Scope

В задаче:

### Rust side

- Добавить `tracing`, `tracing-subscriber`, `tracing-appender` в
  `services/kepler-backend/Cargo.toml`.
- Инициализировать tracing в `kepler-backend/src/main.rs::setup` после
  `crash_reporter::install`: file appender → `<data_dir>/logs/<slot>/kepler-backend.<DATE>`,
  JSON формат, env-filter (default INFO).
- Заменить горячие `eprintln!`/`println!` в `services/kepler-backend/src/*.rs`
  на `tracing::{info,warn,error}!`. Не sweep'ить тестовый код / build scripts.

### TS side (shell)

- Создать `shell/electron/logging.ts` — `keplerLog.{info,warn,error}(scope, msg, meta?)`.
  Пишет JSON-line одновременно в:
  - `<data_dir>/logs/<slot>/kepler-shell-<DATE>.log` (rolling daily).
  - `process.stderr` (для dev visibility).
- Заменить `console.error("[kepler-shell] ...", e)` в горячих местах
  (main.ts, extension-host.ts, settings-window.ts, autoupdater-host.ts).
  Renderer/Vue code НЕ трогаем.

### Bug bundle

- IPC `kepler:diagnostics:bundle` в `shell/electron/main.ts` (или отдельный
  diagnostics модуль): создаёт ZIP с:
  - последние 7 дней `logs/` (per-slot dir),
  - последние 30 дней `crashes/`,
  - `versions.json` (kepler version, electron version, OS info),
  - `installed-extensions.json` (id + version каждого).
  - **Не** включаем ark.db, отдельные объекты, секреты.
- IPC + UI button в Settings → существующая «Диагностика» секция:
  «Создать отчёт» → IPC → showSaveDialog → пользователь сохраняет ZIP.

Не в задаче:

- Telemetry / network upload — лог bundle остаётся local-only, пользователь
  сам решает прислать.
- Sentry / external crash reporter — `crash_reporter` уже пишет local.
- Конверсия console.log в renderer code (extensions, src/views/\*) — отдельная
  гигиена.
- Перевод всех 48 eprintln в backend — только самые горячие (init,
  command_bus, sync handlers); тестовые eprintln оставляем.

## Acceptance Criteria

AC1. `services/kepler-backend/Cargo.toml` содержит `tracing`,
`tracing-subscriber`, `tracing-appender`. `cargo build -p kepler-backend`
зелёный.

AC2. После старта backend (через `bun run --cwd shell build:backend:dev` +
любой spec) в `<data_dir>/logs/<slot>/kepler-backend.<DATE>` появляется
JSON-line файл с не-пустым содержимым.

AC3. `shell/electron/logging.ts` существует, exports `log.{info,warn,error}`.
После старта shell `<data_dir>/logs/<slot>/kepler-shell-<DATE>.log`
создан и наполнен.

AC4. Минимум 10 callsite'ов `console.error("[kepler-shell]...")` заменены
на `log.error("scope", ...)` в main.ts / extension-host.ts /
settings-window.ts. Точное число — по факту, > 10.

AC5. IPC `kepler:diagnostics:bundle` зарегистрирован. Вызов с running
instance создаёт ZIP с правильным набором директорий.

AC6. UI кнопка «Создать отчёт» в Settings → Диагностика работает: click →
showSaveDialog → ZIP сохраняется по выбранному пути.

AC7. `bunx playwright test tests/e2e/eden.spec.ts` зелёный (9/9). Никаких
регрессий e2e.

AC8. `bunx oxlint .`, `bunx oxfmt --check .`, `bun run ark:guard:writes`,
`cargo clippy --workspace --all-targets` — все зелёные.

## Out of scope decisions

- Log rotation policy: дни как partitioning — старые файлы не trim'ятся
  автоматически в Phase 4. Размер per-day rolling appender ограничит
  единичный file size; cleanup oldest — отдельная задача.
- Rust `tracing` инициализируется в `main.rs` — другие crate'ы (ark-core)
  пишут через `tracing::*` если у них есть `_guard`. ark-core может быть
  библиотекой консьюмером — не требует своего init'а.
- Sentry / external aggregation — out.
