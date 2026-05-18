# ark-core

::: tip Источник правды
`crates/ark-core/README.md`, `crates/ark-core/AGENTS.md`
:::

Канонический local-first data runtime для Kosmos. Rust crate + sidecar бинарь `ark-core-rpc` поверх SQLite. Один и тот же runtime используется Electron-приложениями через JSON-RPC и Android/Swift через UniFFI.

## Статус (2026-04-16)

- **DB layer** в продакшен-приложениях. Electron sidecars и Rust callers используют одну SQLite-схему и CRUD-хелперы.
- **Sync layer** в продакшене. `main.rs` экспонирует sync lifecycle RPCs; `ffi.rs` — те же операции для UniFFI.
- **Usage tracking entities** — first-class ARK data. `tracked_apps`, `usage_sessions`, `usage_events` лежат в основной ARK DB и участвуют в sync.
- **Schema evolution — additive.** Существующие ARK DBs мигрируются на месте через `init_schema` без замены файла.

## Архитектура

```text
packages/ark-core/rust/src/
  main.rs              # stdin/stdout JSON-RPC sidecar для Electron callers
  ffi.rs               # UniFFI facade для Android / Swift / embedded callers
  lib.rs               # library entry point, public re-exports
  types.rs             # shared entities и sync payloads
  schema.rs            # CREATE TABLE / index statements
  db.rs                # SQLite CRUD, миграции, sync entity loading
  hlc.rs               # Hybrid Logical Clock helpers
  protocol.rs          # sync wire messages и батчинг
  sync_server.rs       # WebSocket sync server
  sync_client.rs       # WebSocket sync client
  beacon.rs            # UDP peer discovery
  relay_transport.rs   # outbound relay client
  mesh.rs              # LAN + relay coordination
  host.rs / net.rs     # hostname + routable address filtering
  space.rs             # пространство данных
```

## Сборка и тесты

```powershell
cargo build --manifest-path packages/ark-core/rust/Cargo.toml --bin ark-core-rpc
cargo test  --manifest-path packages/ark-core/rust/Cargo.toml
bun run --cwd packages/kosmos-ark typecheck
bun run --cwd packages/kosmos-ark build
```

## Sidecar контракт

`ark-core-rpc` читает newline-delimited JSON со stdin и пишет ответы / events в stdout. Запросная envelope:

```json
{ "operation": "snake_case_name", "...params": "..." }
```

Modern callers могут включать `id`; ответы echo'ят его. Legacy callers без `id` работают.

Лайфцикл:

1. `init` с `dbPath`.
2. Object / usage CRUD и query через RPC.
3. Опционально `start_sync`.
4. `stop_sync` перед shutdown.

Когда `relay_url` передан в `start_sync`, sidecar поднимает relay-bridge рядом с LAN sync. Bridge обменивается `hello`, `version_vector`, `sync_changes`, `live_change` фреймами через relay сервер.

`start_sync` принимает опциональный `auth_secret`. Когда настроен, LAN/P2P пиры обязаны доказать знание секрета через `auth_nonce` + `auth_hmac` в `hello`. Это аутентификация, **не** шифрование.

## Полный список RPC

### DB lifecycle

- `init` — инициализация runtime с путём к БД.
- `load_all` — загрузить все сущности (для bootstrap renderer).
- `clear_all` — очистить (для тестов).
- `delete_trashed` — почистить мусор.

### Core entity CRUD (legacy планнинг)

- `upsert_todo`, `delete_todo`, `batch_upsert_todos`
- `upsert_project`, `delete_project`
- `upsert_area`
- `upsert_tag`
- `upsert_heading`, `delete_heading`

### Object model

- `upsert_object_type`, `delete_object_type`
- `upsert_object`, `delete_object`
- `upsert_object_link`, `delete_object_link`
- `list_objects_by_type`
- `get_objects_by_ids`
- `search_objects`

### Usage entity CRUD

- `upsert_tracked_app`, `delete_tracked_app`
- `upsert_usage_session`, `delete_usage_session`
- `upsert_usage_event`, `delete_usage_event`
- `list_recent_usage_processes`
- `search_usage_processes`
- `get_usage_game_playtime_summary`

### Sync KV

- `get_sync_kv`, `set_sync_kv`

### Sync runtime

- `start_sync`, `stop_sync`
- `broadcast_change`
- `get_connected_peers`
- `add_seed_peer`
- `leave_space`
- `get_own_addresses`
- `get_host_device_name`

## Первичные сущности

- **Legacy planning**: `TodoItem`, `Project`, `Area`, `Tag`, `Heading`.
- **Usage**: `TrackedApp`, `UsageSession`, `UsageEvent`.
- **Sync metadata**: `PeerRecord`, `VersionVector`, HLC strings в `sync_kv`.
- **Object model**: `ObjectType`, `Object`, `ObjectLink`.

## Sync rules

::: warning Жёстко
- Каждая persisted syncable сущность обязана round-trip'иться через `db.rs::load_entities` и `db.rs::apply_entity`.
- Direct writers вне RPC layer (`services/kepler-backend/src/usage_tracker`) **обязаны** bump'ать `lan_sync.version_vector` после прямых писей. Stale version vector ломает CRDT-merge.
- Schema-добавления **идемпотентны**. Используй `CREATE TABLE IF NOT EXISTS` / additive миграции, не destructive rewrites.
- Wire-протокол sync остаётся `snake_case`. RPC может быть `camelCase` где зависят legacy Electron callers.
- **Self-peer filtering** и **routable-address filtering** — обязательные инварианты. Не ослабляй при изменениях в sync startup или peer persistence.
:::

## Verification expectations

- Запускай `cargo test` в `crates/ark-core/rust`.
- Если трогаешь sync или schema — добавь/обнови тесты миграции и репликации, а не только локальный CRUD.

## Текущие ограничения

- Relay sync подключён в `ark-core-rpc` и в UniFFI `ArkCore::start_sync`.
- LAN sync поддерживает опциональную HMAC peer auth, но трафик не шифруется.
- Generic object search использует SQLite FTS5 если доступен; fallback на in-memory matching когда FTS не работает.
- Часть исторических docs/скриптов остаётся как migration aids, не текущий integration contract.

## Связанные документы

- [@kosmos/ark](/packages/ark) — TS SDK.
- [Модель данных ARK](/concepts/ark-objects).
- [Синхронизация](/concepts/sync).
- [Граница записи в ARK](/concepts/write-boundary).
