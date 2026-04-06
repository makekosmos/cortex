# ark-core

Unified Rust crate providing DB layer (SQLite) + sync layer (WebSocket P2P) for the Kosmos/Delphi ecosystem.

## Architecture

```
rust/src/
  main.rs          -- stdin/stdout JSON-RPC binary (Electron sidecar replacement)
  lib.rs           -- library entry point, re-exports
  types.rs         -- shared data types (TodoItem, Project, Area, Tag, Heading, SyncEntity, PeerRecord)
  schema.rs        -- CREATE TABLE statements (identical to delphi-db sidecar)
  db.rs            -- SQLite CRUD operations (rusqlite, WAL mode)
  hlc.rs           -- Hybrid Logical Clock (tick, merge, compare, format)
  protocol.rs      -- sync message types, constants, vector diff, batch splitting
  space.rs         -- space codes (Base32-Crockford), IPv4 encoding, QR payloads
  sync_server.rs   -- tokio + tungstenite WebSocket server
  sync_client.rs   -- tokio + tungstenite WebSocket client with address racing
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
