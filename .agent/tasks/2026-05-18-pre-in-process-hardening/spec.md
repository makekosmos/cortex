# 2026-05-18 pre-in-process-hardening

## Context

Local-first продукт с real users (не только developer): каждый user работает на своей машине, данные — локальный ARK SQLite. Сейчас отсутствуют production-критичные safety nets:

1. **Нет автобэкапов ARK DB.** Single corrupted file → user теряет все заметки/задачи/pomodoro данные навсегда.
2. **Нет integrity check.** SQLite corruption (WAL crash, диск сбойнул) проявляется silent — ошибки разлетаются по run-time, root cause непонятен.
3. **Нет supervisor для backend.** Если `kepler-backend.exe` crash'нется (например, panic в FTS5 rebuild), Electron shell остаётся жив но без backend — все UI окна в loading state навечно.
4. **Нет crash reporter.** Когда у Васи Пупкина (real user) что-то падает, нет artifact'а который он может прислать.
5. **Pervasive `unwrap()` / `expect()`** в Rust путях — каждый из них = potential silent panic от corruption / sync drift / FTS5 errors. Mutex poison propagation `db.rs:2898/2920/2932` (HIGH-2 finding из 2026-05-18 audit).
6. **Нет property-based тестов** — текущее unit-test coverage ловит regressions, но не находит invariant violations при random Upsert/Delete sequences.

Этот proof loop закрывает все 6 пунктов **до** обсуждения subprocess removal (отдельный proof loop, если делаем). Subprocess сейчас — единственный crash isolation layer; убирать его без hardening = trade одного риска на другой.

## Scope

### #3 — DB backup на старте backend

`services/kepler-backend/src/db_backup.rs` (новый модуль).

На startup в `main.rs` (после `db::init_schema`, до spawn'а WS server):
- Прочитать `last_backup_timestamp` из `sync_kv` (или из файла-marker в data-dir).
- Если timestamp отсутствует ИЛИ `(now - last_backup) > 24h` — скопировать `<data_dir>/ark.db` в `<data_dir>/backups/ark.db.backup-YYYY-MM-DD-HHMMSS`.
- Создать `<data_dir>/backups/` если не существует.
- Запустить SQLite Online Backup API (а не raw copy) — это safe для live DB; `rusqlite::Connection::backup`.
- Записать новый timestamp в `sync_kv`.
- Rotation: после успешного backup'а, удалить все кроме последних **7** в `backups/` (sort by mtime).
- Логировать через `eprintln!` с префиксом `[db-backup]`: success / skip (recent) / failure (с reason).
- Failure НЕ блокирует startup — backend продолжает работу (но user видит warning в логах).

### #4 — PRAGMA integrity_check на init

В `crates/ark-core/rust/src/db.rs` функция `init_schema(conn)`:
- После открытия DB и до миграций — `conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))`.
- Если результат != "ok" — `return Err(format!("ARK DB corruption detected: {result}"))`.
- В backend `main.rs` Init handler: если `init_schema` вернул Err — fail loud (eprintln + не записывать lock-файл + return Err через WS).
- Shell main.ts видит non-connected state → resetArkClient уже срабатывает → user видит error в UI.

### #5 — Auto-respawn supervisor (Electron main)

`shell/electron/main.ts`:
- Сохранять `let backendRestartAttempts = 0` и `let lastBackendRestartAt = 0`.
- В `backendProc.on('exit', code)`:
  - Если `isQuiting === true` — exit, no respawn.
  - Если `code === 0` — clean exit, no respawn.
  - Иначе — exponential backoff: 1s → 5s → 30s → 60s → 120s. После 5 неудачных попыток подряд (без 5min "успешного" работающего периода) — error dialog "kepler-backend keeps crashing. Check %APPDATA%\Kosmos\crashes\". Не respawn пока user не нажмёт OK + ручной restart.
  - Reset attempts counter если backend проработал > 5 минут без exit.
- `resetArkClient(reason)` — уже сделан в предыдущей сессии (commit `a989adb2`). Дополнить cascade: `await resetArkClient(...) → setTimeout(spawnBackend, delay) → setTimeout(initArkClient, delay+500ms)`.

### #1 — Crash reporter MVP

**Rust side** (`services/kepler-backend/src/crash_reporter.rs`):
- В `main()` early init: `std::panic::set_hook(Box::new(panic_handler))`.
- `panic_handler(info: &PanicHookInfo)`:
  - Resolve crash dir: `<data_dir>/crashes/`.
  - Файл `panic-<ISO timestamp>.log` содержит:
    - kepler-backend version (из `env!("CARGO_PKG_VERSION")`)
    - timestamp
    - panic message (`info.payload()` cast to `&str`)
    - location (file:line)
    - `std::backtrace::Backtrace::capture()` (требует `RUST_BACKTRACE=1` — set'им через env при spawn'е)
  - Записать sync (не tokio — panic в shutdown path).
  - Re-raise default handler чтобы process всё-таки exit'нулся.

**Electron side** (`shell/electron/main.ts`):
- `crashReporter.start({ uploadToServer: false, productName: "Kepler", companyName: "Kosmos", submitURL: "" })` ДО `app.whenReady()`.
- Electron сам пишет minidump'ы в `app.getPath("crashDumps")` (по умолчанию `%APPDATA%\Kepler\Crashpad\`). После init move/symlink — оставляем default.
- В spawnBackend `env: { RUST_BACKTRACE: "1", ... }` чтобы backtraces были захвачены.

**Settings UI** (`shell/src/views/SettingsView.vue`):
- Новая секция "Диагностика" в Settings.
- Показывает: путь к `<data_dir>/crashes/`, count crash файлов за last 30 days.
- Кнопка "Открыть папку отчётов" — `kepler:crashes:openFolder` IPC → shell main вызывает `shell.openPath(crashesDir)`.
- Кнопка "Очистить отчёты" — `kepler:crashes:clear` IPC → удаляет всё в `crashes/`.
- Email кнопка пока не добавляем (нет SMTP). User вручную может прикрепить файлы.

### #2 — Unwrap audit

Цели в порядке приоритета:

**Critical** (`crates/ark-core/rust/src/db.rs`):
- HIGH-2 finding: `SqliteStorageBackend::load_entities` (line 2898), `get_kv` (2920), `set_kv` (2932) делают `conn.lock().unwrap()`. Mutex poison propagation = panic в spawn_blocking → silent unavailability. Заменить на `lock().unwrap_or_else(|e| e.into_inner())` (recovery path: poisoned lock содержит valid data).
- `bump_sync_version_vector` — проверить unwrap chain.
- Все `let row = stmt.query_row(...).unwrap()` → `?`.

**Important** (`services/kepler-backend/src/`):
- `ws_server.rs`: handler-handler propagation. Замена `.unwrap()` на `?` и proper error response.
- `pomodoro_host.rs`: `Mutex<Session>` lock + `.unwrap()` — heart of pomodoro. Замена на `unwrap_or_else(|e| e.into_inner())`.
- `usage_tracker/mod.rs`: foreground polling loop — unwrap'ы здесь убивают трекер, теряя текущую сессию.
- `auth.rs`, `lock_file.rs`: точечные unwrap'ы.

**Acceptable to leave** (контекст требует panic-on-failure):
- `static` initialization (e.g., `STDOUT_LOCK`, `DB`) — Mutex never poisoned at init.
- Tests / `#[cfg(test)]` — fine.
- `main.rs` startup (если init fail — panic OK, process exit).

Process:
1. Grep `unwrap()` + `expect(` + `panic!(` в targets (исключая `#[cfg(test)]` блоки).
2. Для каждого callsite — оценить: error path возможен? recovery возможен?
3. Заменить с appropriate error propagation.
4. Все Rust tests + cargo check passes.

### #6 — Property-based tests (proptest)

`crates/ark-core/rust/Cargo.toml` dev-dep:
```toml
proptest = "1.5"
proptest-derive = "0.5"
```

`crates/ark-core/rust/tests/proptest_invariants.rs` (новый файл):

**Strategy:** генерируем `Vec<Op>` где `Op = Insert(id, props) | Update(id, props) | Delete(id) | Read(id) | List`.

**Invariants checked:**
1. **Read survives random ops:** после любой sequence ops, `list_objects()` не падает; каждый Read возвращает либо Some(obj) либо None (не панического).
2. **Version vector monotonic:** после каждой Upsert/Delete operation, version vector содержит HLC для затронутого entity. HLC по каждому device-id строго увеличивается.
3. **Tombstones consistent:** после Delete(id), `sync_tombstones` содержит row для id; повторный Insert(id) удаляет tombstone (см. `delete_sync_tombstone`).
4. **Soft-delete idempotent:** Delete несуществующего id — no-op без panic.

**Scope:** только `ArkObject` через `upsert_object` / `delete_object` / `list_objects` (наибольшая поверхность). Расширить на other entity types — позже.

**Test config:** 256 iterations, max sequence length 50, runtime budget 30s per test.

## Acceptance Criteria

**AC1 — DB backup.** `services/kepler-backend/src/db_backup.rs` существует и регистрируется в `main.rs` после init_schema. Unit test `backup_runs_when_no_previous_backup` + `backup_rotates_keeping_last_7` зелёные. Manual smoke: запустить backend → проверить `<data_dir>/backups/ark.db.backup-*` создан → запустить второй раз < 24h — backup пропущен.

**AC2 — integrity_check.** `db::init_schema(conn)` вызывает `PRAGMA integrity_check` и возвращает Err при corruption. Unit test `init_schema_fails_on_corrupted_db` зелёный (создаём DB с битыми bytes, проверяем что init возвращает Err containing "corruption").

**AC3 — Auto-respawn.** `shell/electron/main.ts` имеет supervisor logic в `backendProc.on('exit')`. Опционально (manual smoke): запустить Kepler в dev, `Stop-Process -Name kepler-backend` — backend respawned автоматически через 1s, ArkClient переподключился, dashboard работает. Counter увеличивается до 5 → dialog. После 5min успешного работы — counter сбрасывается.

**AC4 — Crash reporter.** Rust `panic_hook` пишет в `<data_dir>/crashes/panic-*.log`. Unit test `panic_handler_writes_log` (форсит panic через `panic!` в test и проверяет файл создан) — невозможно полностью из unit-test'а потому что test runner ловит panic, но можно тестировать `format_panic_log(info)` функцию отдельно. Electron `crashReporter.start()` зарегистрирован. Settings UI имеет секцию "Диагностика" с кнопкой "Открыть папку отчётов" → открывает explorer на `crashes/`.

**AC5 — Unwrap audit.** Все `unwrap()` / `expect(` в `crates/ark-core/rust/src/db.rs` SqliteStorageBackend методах + critical `kepler-backend/src/{pomodoro_host,ws_server,usage_tracker}.rs` пути заменены на error propagation или explicit poison recovery. `cargo clippy --all-targets -- -D warnings` (новых warnings нет). Все existing rust tests passing.

**AC6 — Proptest.** `crates/ark-core/rust/tests/proptest_invariants.rs` существует. `cargo test --test proptest_invariants` зелёный (256 iterations × N invariants). При intentional regression (например, удалить tombstone insert в delete_object) — proptest падает с reproducible seed.

**AC7 — Guards.** `bun run ark:guard:writes` clean, `bun run ark:smoke` зелёный, `bun run docs:sync && bun run docs:check` clean.

**AC8 — Docs.** `docs-site/concepts/db-resilience.md` (новая страница) описывает: backup rotation, integrity check on init, crash reporter location, supervisor backoff schedule. Обновлены `STATUS.md` (новый раздел "Production hardening"), `docs-site/agents/forbidden.md` (новые правила про unwrap discipline).

## Verification commands

```powershell
# Rust side
cargo test --manifest-path crates/ark-core/rust/Cargo.toml --lib
cargo test --manifest-path crates/ark-core/rust/Cargo.toml --test proptest_invariants
cargo test --manifest-path services/kepler-backend/Cargo.toml
cargo clippy --manifest-path crates/ark-core/rust/Cargo.toml --all-targets

# Shell
bun run --cwd shell typecheck

# Guards
bun run ark:guard:writes
bun run ark:smoke
bun run docs:sync; bun run docs:check
```

## Out of scope

- **Subprocess removal (in-process facade).** Отдельный proof loop после стабилизации этого. Без crash isolation в-process facade слишком risky.
- **Sentry / GlitchTip self-hosted.** Сейчас MVP: local crash log файлы. Internet upload — отдельная задача после launch (когда будет ясно сколько users → нужна ли централизация).
- **Encrypted crash reports.** Crash logs содержат backtraces + filenames, не personal data. Plaintext OK.
- **Cross-device backup sync.** Backup'ы локальные, в `<data_dir>/backups/`. Cloud sync — отдельная задача (если/когда).
- **DB encryption at rest** (SQLCipher) — не в scope. Если будет нужно — отдельный loop.
- **Mac/Linux paths.** Сейчас Kepler Win-only.

## Estimate (предварительный, до начала)

Per skill `estimate-calibration`: записываю prediction в `log.jsonl` после старта.

- #3 DB backup: 1h (+ test)
- #4 integrity_check: 30min (+ test)
- #5 Auto-respawn: 1h
- #1 Crash reporter: 2h (Rust + Electron + UI)
- #2 Unwrap audit: 3h (careful per-site review)
- #6 Property-based tests: 4h (proptest setup + invariant design + debugging)
- Docs + evidence: 1h

**Total estimate: ~12-13h.** Single session — может потребоваться разбить на 2-3 sittings.
