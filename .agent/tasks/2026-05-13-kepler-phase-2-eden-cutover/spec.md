# Phase 2: `@kosmos/ark` kepler-mode + hold-and-replay + Eden cutover

**Дата начала**: 2026-05-13
**Worktree**: `D:\Personal\Hobby\Coding\kosmos-kepler`
**Branch**: `kepler/phase-1-scaffold` (Phase 1 не закоммичена; Phase 2 продолжает в той же ветке до коммита).
**Связанный план**: `C:\Users\Kazui\.claude\plans\sharded-waddling-dove.md`
**Memory**: [[project-kepler-planning]]

## Цель

После Phase 2:
- ARK имеет таблицу `sync_pending_objects` для hold-and-replay objects с неизвестным `type_id` (закрывает блайнд-спот #1).
- `@kosmos/ark` имеет третий transport mode: **kepler** — коннектится к Kepler host через WebSocket, использует existing handshake протокола.
- `@kosmos/ark` exports новый helper `ensureKeplerRunning()` — auto-launch flow (см. план Decision #4 + Phase 6 AC5-AC8).
- Eden использует `@kosmos/ark` ArkClient в kepler-mode за env-флагом `KOSMOS_KEPLER_OPTIONAL=1`. Self-managed fallback сохраняется.

## Acceptance criteria

### AC1 — Eden работает в kepler-mode end-to-end
**Утверждение**: Запустить Kepler host + Eden с `KOSMOS_KEPLER_OPTIONAL=1`. Eden создаёт object_type, объект, обновляет, удаляет — все через WS к Kepler. Никакого self-spawn ark-core-rpc внутри Eden.
**Проверка**: вручную (manual smoke) + Playwright e2e после имплементации.

### AC2 — Kill Kepler → reconnect ≤ 5s
**Утверждение**: пока Eden открыт и работает с ARK, kill Kepler из task-manager. Eden показывает «connection lost» tост, Kepler respawn'ится (если watcher есть) или manually запускается. Eden переподключается ≤ 5s, in-flight requests retry'ятся (или fail gracefully), новые requests работают.
**Проверка**: manual smoke + unit test для `KeplerTransport.reconnect` с mock WS.

### AC3 — Schema drift: unknown type_id → sync_pending_objects + sync_error event
**Утверждение**: peer присылает SyncEntity типа "object" с `data.type_id = "X"`, и `X` не существует в `object_types`. ARK кладёт payload в `sync_pending_objects`, эмитит `sync_error` event `{code: "unknown_type_id", entity_id, awaited_type_id}`. Version_vector обновляется (entity «принят»).
**Проверка**: Rust unit test (`db::tests::object_with_unknown_type_goes_to_pending`).

### AC4 — Replay: создание object_type → автоматический replay + sync_replay event
**Утверждение**: после AC3 в pending лежит payload. Создаётся (через любой path: local upsert_object_type или sync) тот самый `object_type`. ARK автоматически replay'ит pending: parses payload, upsert_object, DELETE из pending, эмитит `sync_replay` event `{entity_id, type_id}`. Объект становится доступен через `list_objects`.
**Проверка**: Rust unit test (`db::tests::replay_runs_when_type_appears`).

### AC5 — Version mismatch отклоняется
**Утверждение**: `@kosmos/ark` ArkClient в kepler-mode читает `kepler.lock.json`, парсит `protocol_version`. Если MAJOR не совпадает с поддерживаемым клиентом — refuse connect, throw `IncompatibleProtocolError`. При `KOSMOS_KEPLER_OPTIONAL=1` — fallback на self-managed.
**Проверка**: TS unit test (`ark-client.test.ts::kepler_mode_rejects_major_mismatch`).

### AC6 — Fallback: lock-file отсутствует → self-managed
**Утверждение**: `KOSMOS_KEPLER_OPTIONAL=1`, lock-файл не существует или PID мёртв. ArkClient прозрачно стартует self-managed ark-core-rpc child (как сейчас). Eden работает идентично pre-Phase-2 поведению.
**Проверка**: TS unit test + manual smoke без Kepler.

## Объём работ (компоненты)

### Rust (ARK core)
- `packages/ark-core/rust/src/schema.rs` — новая таблица `sync_pending_objects`
- `packages/ark-core/rust/src/db.rs`:
  - `is_object_type_known(conn, type_id) -> bool`
  - `insert_pending_object(conn, entity) -> Result<(), String>`
  - `replay_pending_for_type(conn, type_id) -> Result<usize, String>` (эмитит `sync_replay`)
  - Modify `apply_entity_blocking` for "object" — pre-flight check
  - Modify `upsert_object_type` — call replay_pending_for_type after upsert
- Emit `sync_error` event из apply_entity_blocking — через `crate::emit_event`
- Unit tests: object → pending; replay; idempotency

### TypeScript (`@kosmos/ark`)
- Новый `ensureKeplerRunning(opts)` helper в `src/ensure-kepler.ts`:
  - Reads `kepler.lock.json`
  - Checks PID alive
  - Auto-launches Kepler exe if not running и path resolvable
  - Returns `{state: "connected" | "not-installed" | "launch-failed" | "incompatible-version"}`
- В `src/ark-client.ts` — third mode `kepler`:
  - new constructor option `keplerLock?: KeplerLockInfo`
  - new private path: ws connect + handshake instead of spawn
  - same public API surface (`request<T>(req)` unchanged)
- Update `src/index.ts` exports

### Eden
- `apps/eden/ts/main/ark.ts` — заменить локальный `ArkClient` class (lines 101-313) на `import { ArkClient } from "@kosmos/ark"` с kepler-mode resolve.
- Опциональный `SyncDriftToast.vue` component (renderer side) — подписка на `sync_error`/`sync_replay` events через preload bridge, ненавязчивый toast.

## Out of scope для Phase 2

- Delphi/Arrancador/Horologion cutover (это Phase 3).
- usage-tracker WS migration (Phase 4).
- LAN sync централизация (Phase 5).
- Launcher UI / hotkey (Phase 6).

## Rollback

- env `KOSMOS_KEPLER_OPTIONAL=0` → Eden строго требует Kepler; для отката убираем флаг → self-managed как раньше.
- Schema migration additive (`CREATE TABLE IF NOT EXISTS sync_pending_objects`) — старые ARK builds игнорируют таблицу.
- Sync_pending_objects entries не блокируют ARK работу — они появляются только при schema drift.

## Спецификация заморожена

После старта реализации — не редактируется.
