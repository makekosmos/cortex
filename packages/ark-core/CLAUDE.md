# ark-core

Unified Rust crate providing DB layer (SQLite) + sync layer (WebSocket P2P) for the Kosmos/Delphi ecosystem.

## Runtime status (2026-04-09)

- **DB layer** — **LIVE**. Electron spawns `ark-core-rpc` as sidecar, all CRUD/KV ops go through it (`apps/delphi/ts/electron/sidecar.ts`).
- **Sync layer (`sync_server.rs`, `sync_client.rs`)** — **LIBRARY-ONLY**. The Rust types exist and are wire-compatible with TS `@arksync/core`, but `main.rs` Request enum does NOT expose `start_sync_server` / `start_sync_client` yet. Electron still imports `SyncServer` / `SyncClient` from TS `packages/arksync` at runtime. Android runs its own Kotlin sync.
- **Beacon discovery** — **NOT IN RUST**. Currently lives in `apps/delphi/ts/electron/broadcast-discovery.ts` and `apps/delphi/kotlin/.../BroadcastDiscovery.kt`. TODO: port to `rust/src/beacon.rs` when the sync layer goes live.

## Architecture

```
rust/src/
  main.rs          -- stdin/stdout JSON-RPC binary (Electron sidecar); DB ops only
  lib.rs           -- library entry point, re-exports
  types.rs         -- shared data types (TodoItem, Project, Area, Tag, Heading, SyncEntity, PeerRecord)
  schema.rs        -- CREATE TABLE statements (identical to delphi-db sidecar)
  db.rs            -- SQLite CRUD operations (rusqlite, WAL mode)
  hlc.rs           -- Hybrid Logical Clock (tick, merge, compare, format)
  net.rs           -- Syncthing-style address filters (link-local / virtual iface rejection)
  protocol.rs      -- sync message types, constants, vector diff, batch splitting
  space.rs         -- space codes (Base32-Crockford), IPv4 encoding, QR payloads
  sync_server.rs   -- tokio + tungstenite WebSocket server [library-only]
  sync_client.rs   -- tokio + tungstenite WebSocket client with address racing [library-only]
```

## Build

```bash
cd rust/
cargo build              # debug binary + lib
cargo build --release    # release
cargo test               # unit tests
```

Binary output: `target/debug/ark-core-rpc` (or `target/release/ark-core-rpc`).

## JSON-RPC Protocol (stdin/stdout)

Backward-compatible with the existing delphi-db sidecar. One JSON object per line.

**Request**: `{"operation": "snake_case", ...params}`
**Response**: `{"ok": true, "data": ...}` or `{"ok": false, "error": "..."}`

### Operations

| Operation | Params | Notes |
|-----------|--------|-------|
| `init` | `dbPath` | Open/create SQLite database |
| `load_all` | -- | Returns all todos, projects, areas, tags, headings |
| `upsert_todo` | `todo` | INSERT OR REPLACE |
| `delete_todo` | `id` | Hard delete |
| `batch_upsert_todos` | `todos` | Transactional batch |
| `upsert_project` | `project` | INSERT OR REPLACE |
| `delete_project` | `id` | Hard delete |
| `upsert_area` | `area` | INSERT OR REPLACE |
| `upsert_tag` | `tag` | INSERT OR REPLACE |
| `upsert_heading` | `heading` | INSERT OR REPLACE |
| `delete_heading` | `id` | Hard delete |
| `get_sync_kv` | `key` | Returns value or null |
| `set_sync_kv` | `key`, `value` | INSERT OR REPLACE |
| `clear_all` | -- | Deletes all rows from all tables |
| `delete_trashed` | -- | Deletes todos where is_trashed=1 |

## Sync Protocol

Wire-compatible with TS arksync (`packages/arksync/`). Snake_case field names in JSON.

- Port: 21531
- Protocol version: 1
- Message types: hello, version_vector, sync_changes, sync_ack, live_change, live_ack, peer_list, ping, pong
- HLC format: `<ISO8601>:<counter:06d>:<device_id>`
- Batch limits: 100 entities or 1MB per batch

## Space Codes

- 12-char Base32-Crockford (alphabet: `0123456789ABCDEFGHJKMNPQRSTVWXYZ`)
- Format: `XXXX-XXXX-XXXX`
- Space ID: SHA-256(code)[:16 hex]
- IPv4 encoding: 7 Base32-Crockford chars
- QR payload: `ark://join?code=...&addrs=...`

## Key Design Decisions

- **camelCase** for JSON-RPC request/response fields (Electron sidecar compat)
- **snake_case** for sync protocol message fields (arksync TS compat)
- `SyncEntity.type` serializes as `"type"` in JSON (via serde rename)
- `SyncEntity.deleted` omitted when None (via skip_serializing_if)
- UUIDs always lowercase
- StorageBackend trait for pluggable persistence

## Sync invariants (mirror of TS arksync)

- **Single session per device_id**: `sync_server::handle_message` на `Hello` выгоняет все прочие authenticated-сессии с тем же `device_id` до принятия новой. Иначе после реконнектов копятся ghost-пиры.
- **Reject self-connect (client + server)**: если входящий `Hello.device_id` равен нашему собственному — закрыть соединение немедленно. Защита от stale phantom peer records, у которых addresses указывают на наш же LAN-IP.
- **Purge self-peers on start**: `SyncServer::purge_self_peers` вычищает persisted `knownPeerRecords` у которых `device_id == our` или `addresses.all(|a| our_addresses.contains(a))`. Вызывается из `start()` после `load_known_peers()`.
- **Filter self in `update_peer_record` + `handle_peer_list`**: никогда не сохраняем запись, которая соответствует нам по `device_id` или по "все адреса — наши".
- **Routable addresses only**: `net::filter_routable_addresses` (и `is_address_routable`) отбрасывают link-local (`169.254/16`, `fe80::/10`), unique-local (`fc00::/7`), loopback, и интерфейсы с префиксами `utun*`, `docker*`, `tailscale*`, `bridge*` и т.д. Вызывающая сторона (главный процесс хоста или будущий `beacon.rs`) **MUST** прогонять собранные адреса через этот фильтр перед тем, как положить их в `own_addresses` для `hello` / `peer_list` / beacon-анонсов.

## TODO для runtime-перехода на Rust sync

1. Добавить в `main.rs` Request enum операции `StartSyncServer { space_id, device_id, device_name, own_addresses }`, `StartSyncClient { peer, ... }`, `BroadcastLiveChange { entity }`, `StopSync`, `GetConnectedPeers`.
2. Создать `rust/src/beacon.rs` с `BroadcastDiscovery` — port из `apps/delphi/ts/electron/broadcast-discovery.ts`: UDP :21532, dedup `HashMap<device_id, SeenPeer>` с TTL 30 с, `net::filter_routable_addresses` в `collect_local_addresses`, фильтрация self в receiver.
3. Добавить `if-addrs = "0.13"` в `Cargo.toml` для `beacon::collect_local_addresses`.
4. Переключить `apps/delphi/ts/electron/main.ts` на JSON-RPC вызовы к sidecar'у вместо прямого импорта `@arksync/core`.
5. Uniffi bindings для Kotlin/Swift: добавить `#[uniffi::export]` на ключевые API в `sync_server`, `sync_client`, `beacon`.
