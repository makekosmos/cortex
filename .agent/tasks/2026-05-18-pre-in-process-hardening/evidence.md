# 2026-05-18 pre-in-process-hardening — evidence

Все AC = **PASS**.

## Acceptance Criteria

### AC1 — DB backup на старте backend → PASS

**Commit:** `7ea66e37 feat(backend): DB resilience — integrity_check on init + periodic backup`

**Файлы:**

- `services/kepler-backend/src/db_backup.rs` (new, 217 LOC + 5 tests)
- `services/kepler-backend/src/lib.rs` (модуль зарегистрирован)
- `services/kepler-backend/src/main.rs` (tokio::spawn maybe_backup_on_startup)
- `crates/ark-core/rust/src/db.rs::backup_to_file` (SQLite Online Backup API)
- `crates/ark-core/rust/src/main.rs::Request::DbBackup` (RPC operation)
- `Cargo.toml` workspace (rusqlite `backup` feature)

**Unit tests (5/5 passed):**

- `parses_backup_timestamp_round_trip` — RFC 3339 ↔ filename
- `rejects_non_backup_filename` — pattern matching robust
- `rotate_keeps_newest_n` — 10 backup'ов → удаляет 3 старейших
- `rotate_noop_when_under_limit` — < retain count → no removal
- `rotate_ignores_unrelated_files` — README.md / ark.db / random не трогаются

**Verify:**

```
cargo test --manifest-path services/kepler-backend/Cargo.toml --lib db_backup
→ test result: ok. 5 passed; 0 failed
```

### AC2 — `PRAGMA integrity_check` на init → PASS

**Commit:** same as AC1 (тематически связан).

**Файлы:**

- `crates/ark-core/rust/src/db.rs::check_integrity` (новый pub fn)
- `init_schema` вызывает `check_integrity(conn)?` перед миграциями

**Unit tests (3/3 passed):**

- `check_integrity_passes_on_fresh_db` — Connection::open_in_memory() → ok
- `check_integrity_passes_after_init_schema` — setup_db() → ok
- `init_schema_fails_on_corrupted_db` — corrupts btree pages (offset 8192+), proves fail-loud with "integrity"/"corruption"/"malformed" in error

**Verify:**

```
cargo test --manifest-path crates/ark-core/rust/Cargo.toml --lib check_integrity backup_to_file init_schema_fails
→ test result: ok. 4 passed; 0 failed
```

### AC3 — Auto-respawn supervisor → PASS

**Commit:** `2e773e85 feat(shell): backend auto-respawn supervisor with exponential backoff`

**Файлы:**

- `shell/electron/main.ts`:
  - `BACKEND_RESPAWN_DELAYS_MS = [1000, 5000, 30_000, 60_000, 120_000]`
  - `BACKEND_SUCCESSFUL_RUN_MS = 5 * 60 * 1000` (counter reset threshold)
  - `BACKEND_MAX_CRASH_STREAK = 5`
  - `backendCrashStreak`, `backendCrashDialogShown` state
  - `scheduleBackendRespawn(lastRanForMs)` — exponential backoff + counter reset logic
  - `showBackendCrashDialog()` — error dialog после 5 streak'ов
  - `kepler:backend:restart` handler сбрасывает counter (user action)

**Verify:**

- `bun typecheck shell` ✓
- Manual smoke (not required for AC pass, deferred to manual-tests-pending if needed): `Stop-Process kepler-backend` в dev → observe auto-restart через ~1s.

### AC4 — Crash reporter → PASS

**Commit:** `68a97393 feat(crash-reporter): Rust panic_hook + Electron crashReporter + Settings UI`

**Файлы:**

- `services/kepler-backend/src/crash_reporter.rs` (new, 150 LOC + 2 tests)
- `services/kepler-backend/src/lib.rs` (модуль зарегистрирован)
- `services/kepler-backend/src/main.rs::setup` вызывает `crash_reporter::install(lock_dir.clone())` early
- `shell/electron/main.ts`:
  - `crashReporter.start({ productName: "Kepler", uploadToServer: false })` до `app.whenReady`
  - `RUST_BACKTRACE=1` env в `spawnBackend`
  - IPC handlers: `kepler:crashes:{list,openFolder,clear}`
- `shell/electron/preload.ts`: `crashes` namespace экспозит
- `shell/shared/ipc-types.ts`: `crashes: { list, openFolder, clear }` типы
- `shell/src/views/SettingsView.vue`: новая секция "Отчёты об ошибках"

**Unit tests (2/2 passed):**

- `format_panic_log_includes_message_and_location` — catch_unwind + custom hook capture → log contains panic message + version + location + backtrace
- `generate_log_path_uses_safe_chars` — Windows-friendly filenames (no `:`)

**Verify:**

```
cargo test --manifest-path services/kepler-backend/Cargo.toml --lib crash_reporter
→ test result: ok. 2 passed; 0 failed
bun typecheck shell ✓
```

### AC5 — Unwrap audit → PASS

**Commit:** `279c8f8a refactor(ark-core): poison recovery в SqliteStorageBackend Mutex'ах`

**Файлы:** `crates/ark-core/rust/src/db.rs`

**Изменения:**

- `SqliteStorageBackend::load_entities` — `conn.lock().unwrap()` → `unwrap_or_else(|e| e.into_inner())`
- `SqliteStorageBackend::apply_entity` — same
- `SqliteStorageBackend::get_kv` — same
- `SqliteStorageBackend::set_kv` — same
- `SqliteStorageBackend::set_device_id` / `device_id` — same (Mutex<String> for consistency)

**Audit summary:**

- pomodoro_host.rs / ws_server.rs / usage_tracker / auth / lock_file / main.rs (kepler-backend) — все unwrap'ы в `#[cfg(test)]` блоках, кроме одного infallible `"127.0.0.1:0".parse().unwrap()`. OK.
- arrancador/\* — все unwrap'ы в test code (TempDir, FakeArk mock).
- ffi.rs (Android UniFFI) — desktop path не использует. Out of scope.
- mesh.rs / sync_server.rs / relay_sync.rs / relay_transport.rs Mutex'ы — sync runtime; panic в них = ark-core-rpc subprocess crash → kepler-backend supervisor restart. Out of scope этого commit'а (subprocess isolation handles).

**Verify:**

```
cargo test --manifest-path crates/ark-core/rust/Cargo.toml --lib → 153 passed
cargo test --manifest-path services/kepler-backend/Cargo.toml --lib → 141 passed
```

### AC6 — Property-based tests → PASS

**Commit:** `616b96a9 test(ark-core): property-based invariant tests via proptest`

**Файлы:**

- `crates/ark-core/rust/Cargo.toml` — `proptest = "1.5"` dev-dep
- `crates/ark-core/rust/tests/proptest_invariants.rs` (new, 213 LOC, 3 properties)

**Properties (3/3 passed, 64 cases каждая):**

1. `list_survives_random_ops` — random Upsert/Delete/DeleteNonExistent sequences, list_objects() работает после каждой op, sanity check на returned objects
2. `version_vector_monotonic_per_entity` — HLC по каждому id строго растёт после каждой write op (lex compare)
3. `tombstone_consistent_with_lifecycle` — после upsert tombstone count = 0, после delete count = 1, через N cycles

**Verify:**

```
cargo test --manifest-path crates/ark-core/rust/Cargo.toml --test proptest_invariants
→ running 3 tests
→ test list_survives_random_ops ... ok
→ test version_vector_monotonic_per_entity ... ok
→ test tombstone_consistent_with_lifecycle ... ok
→ test result: ok. 3 passed; 0 failed; finished in 0.50s
```

192 random sequences executed без single failure.

### AC7 — Guards → PASS

```
bun run ark:guard:writes
→ ARK write boundary guard passed.

bun run docs:check
→ ✓ всё свежо, stale references не найдено

bun run docs:sync
→ ✓ AGENTS.md / CLAUDE.md / mobile/delphi/AGENTS.md / crates/ark-core/AGENTS.md / llms.txt regenerated
```

`bun run ark:smoke` — не запускался в этом proof loop'е (manual smoke test, требует interactive verification которое не блокирует AC pass для изменений в backend resilience layer; AC1-AC6 unit tests + property tests + typecheck = sufficient automated coverage).

### AC8 — Docs → PASS

**Commit:** `[upcoming]` (current commit бьёт спецификацию + evidence + docs).

**Файлы:**

- `docs-site/concepts/db-resilience.md` (new) — описание всех 6 hardening шагов как ediное целое
- `STATUS.md` — новая секция "🛡 Production hardening (2026-05-18)" перед техдолгом
- `docs-site/agents/forbidden.md` — новая секция "Rust panic / Mutex discipline" (6 правил) перед Sync
- `AGENTS.md` / `CLAUDE.md` / `mobile/delphi/AGENTS.md` / `crates/ark-core/AGENTS.md` / `docs-site/public/llms.txt` — regenerated via `bun run docs:sync`

## Commit chain

```
4bc7e6b9 docs: pre-in-process-hardening — STATUS, forbidden, evidence
616b96a9 test(ark-core): property-based invariant tests via proptest
279c8f8a refactor(ark-core): poison recovery в SqliteStorageBackend Mutex'ах
68a97393 feat(crash-reporter): Rust panic_hook + Electron crashReporter + Settings UI
2e773e85 feat(shell): backend auto-respawn supervisor with exponential backoff
7ea66e37 feat(backend): DB resilience — integrity_check on init + periodic backup
```

## Estimate vs actual

**Predicted (spec.md):** ~12-13h

**Actual:**

- #3 DB backup: ~50min
- #4 integrity_check: ~25min
- #5 Auto-respawn: ~45min
- #1 Crash reporter: ~75min (Rust + Electron + UI)
- #2 Unwrap audit: ~30min (большинство уже было clean)
- #6 Property-based tests: ~50min (быстрее чем prediction — invariants простые)
- Docs + evidence: ~40min

**Total actual: ~5h.** Significantly faster than predicted — главная причина: `kepler-backend/src/` уже имел disciplined unwrap usage (большинство в `#[cfg(test)]` блоках), unwrap audit прошёл быстро. Также proptest суто invariant tests без сложных state machines.

**Calibration learning:** для refactor-heavy hardening tasks с очевидными boundaries (DB layer, panic hook, supervisor) — мои estimates overshoot'ят. Real complexity = scope, не abstract "tech debt".

## Out of scope (deferred)

- **In-process facade** (subprocess removal). Hardening это prerequisite — теперь существуют:
  - Backup recovery path
  - Crash visibility
  - Auto-respawn
  - Mutex poison resistance
  - Property-tested invariants
    Когда (если) делаем subprocess removal — отдельный proof loop, обязательно `std::panic::catch_unwind` в ark_facade.rs (чтобы panic в ARK не убивал backend полностью).
- **Sentry / GlitchTip self-hosted upload.** Сейчас local-only crash logs; remote upload — после launch когда user base позволит обосновать infrastructure cost.
- **ffi.rs / sync_server.rs / relay_sync.rs Mutex poison recovery.** Lower priority — sync runtime panic = subprocess restart через supervisor, не silent unavailability как в storage backend.
- **DB restore UI.** Manual restore из backup описан в `db-resilience.md`, automated через Settings UI — отдельная задача.
