# Phase 2 partial evidence — lib portion (Rust ARK + TS kosmos-ark)

**Дата**: 2026-05-13
**Worktree**: `D:\Personal\Hobby\Coding\kosmos-kepler`
**Branch**: `kepler/phase-1-scaffold` (Phase 1+2 пока в одной ветке до коммита)

## Сводка

Phase 2 разделён на **lib portion** (этот файл) и **Eden cutover** (отдельный шаг, требует manual smoke).

| AC | Утверждение | Уровень | Статус |
|----|-------------|---------|--------|
| AC1 | Eden работает в kepler-mode end-to-end | **Eden cutover** | Pending |
| AC2 | Kill Kepler → reconnect ≤ 5s | **Eden cutover + integration** | Pending |
| AC3 | unknown type_id → sync_pending_objects + sync_error | **Lib (Rust)** | **PASS** |
| AC4 | object_type creation → auto-replay + sync_replay | **Lib (Rust)** | **PASS** |
| AC5 | MAJOR mismatch → incompatible-version reject | **Lib (TS)** | **PASS** |
| AC6 | Lock missing + fallback option → not-installed | **Lib (TS)** | **PASS** |

## Доказательства

### Rust changes (ARK core)

**Module refactor**: `emit_event`/`set_event_sender` перенесены из `bin/main.rs` в новый `lib/events.rs` чтобы `db.rs` мог эмитить события (для AC3/AC4).

**Schema migration** (additive): `sync_pending_objects` table + index в `schema.rs`.

**New db.rs functions:**
- `is_object_type_known(conn, type_id) -> Result<bool, String>`
- `insert_pending_object(conn, entity, awaited_type_id) -> Result<(), String>`
- `replay_pending_for_type(conn, type_id) -> Result<usize, String>` — эмитит `sync_replay` event per object
- `count_pending_for_type(conn, type_id) -> Result<i64, String>` (observability)

**Modified:**
- `apply_entity_blocking` "object" branch — pre-flight `is_object_type_known`; если false → `insert_pending_object` + emit `sync_error` event + return Ok (version_vector advances)
- `upsert_object_type` — вызывает `replay_pending_for_type` после INSERT

### Тесты Rust

```
cargo test --manifest-path packages/ark-core/rust/Cargo.toml --lib

test result: ok. 125 passed; 0 failed; 0 ignored
```

В том числе 7 новых phase2 тестов:
- `phase2_is_object_type_known_false_when_missing`
- `phase2_is_object_type_known_true_after_upsert`
- `phase2_insert_pending_object_persists_payload`
- `phase2_insert_pending_overwrites_same_id`
- `phase2_replay_runs_when_type_appears`
- `phase2_replay_handles_multiple_pending_same_type`
- `phase2_replay_only_targets_matching_type`

### TS changes (`@kosmos/ark`)

**Новые файлы:**
- `src/ensure-kepler.ts` — `ensureKeplerRunning(opts)` + `readLockIfAlive` + `isPidAlive` + `resolveKeplerExe`. Cross-platform (Win/Mac/Linux) conventional paths.
- `tests/ensure-kepler.test.ts` — 12 Bun tests.

**Modified:**
- `src/index.ts` — exports `ensureKeplerRunning` + типы.
- `src/ark-client.ts`:
  - Add `keplerLock?: KeplerLockInfo` + `keplerPidForHandshake?: number` в `ArkClientOptions`
  - Add `keplerWs` + `keplerHandshakeDone` + `keplerConnectPromise` state
  - `request<T>` третья ветка: если `keplerLock` → `requestViaKepler`
  - `ensureInitialized` — skip ark-core-rpc init в kepler mode (Kepler host уже init'нул)
  - `start()` — skip `start_sync` в kepler mode (Kepler владеет LAN sync, согласовано с Phase 5)
  - `stop()` — close kepler WS вместо kill child
  - New `ensureKeplerConnection` / `openKeplerConnection` / `handleKeplerFrame` / `requestViaKepler` / `closeKeplerConnection`
  - Hello-handshake (kind=hello → kind=hello_ok|hello_error)

**Package.json:** добавлен `"test": "bun test"` script.

### Тесты TS

```
bun test  (в packages/kosmos-ark/)

12 pass
 0 fail
19 expect() calls
Ran 12 tests across 1 file. [223.00ms]
```

Покрывают AC5 (MAJOR mismatch reject), AC6 (no lock → not-installed), + edge cases (stale PID cleanup, malformed JSON, malformed fields, isPidAlive boundary cases).

### Регрессия Phase 1

Re-run Kepler integration tests с обновлённым ark-core-rpc release:
```
cargo test --manifest-path apps/kepler/Cargo.toml --test handshake -- --test-threads=1

test result: ok. 4 passed; 0 failed
```

Phase 1 не сломан. Schema migration `sync_pending_objects` корректно additive — старые e2e сценарии работают как раньше.

## Команды для верификации

```powershell
# Rust ARK
cargo test --manifest-path D:\Personal\Hobby\Coding\kosmos-kepler\packages\ark-core\rust\Cargo.toml --lib

# Kepler integration (не должно регрессировать)
cargo test --manifest-path D:\Personal\Hobby\Coding\kosmos-kepler\apps\kepler\Cargo.toml --test handshake -- --test-threads=1

# TS @kosmos/ark
cd D:\Personal\Hobby\Coding\kosmos-kepler\packages\kosmos-ark; bun test
bun run --cwd packages/kosmos-ark typecheck
```

## Что осталось для полного Phase 2 PASS

1. **Eden cutover** (`apps/eden/ts/main/ark.ts`):
   - Заменить локальный `ArkClient` class (lines 101-313, ~210 строк) на `import { ArkClient, ensureKeplerRunning } from "@kosmos/ark"`.
   - Добавить env-флаг `KOSMOS_KEPLER_OPTIONAL=1` с fallback на self-managed.
   - Опционально — Vue компонент `SyncDriftToast.vue` для отображения `sync_error`/`sync_replay` events.
2. **Manual smoke** (AC1+AC2):
   - Запустить Kepler host вручную (или dev cargo run)
   - Запустить Eden с `KOSMOS_KEPLER_OPTIONAL=1`
   - Verify через task-manager: ровно 1 ark-core-rpc на машине (внутри Kepler), Eden не спавнит свой
   - CRUD через Eden UI — работает
   - Kill Kepler → Eden показывает error toast → spawn Kepler again → Eden reconnects

Эти два шага требуют **manual testing** пользователя — поэтому остановка здесь.
