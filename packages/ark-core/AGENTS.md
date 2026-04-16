# ark-core

Unified Rust crate for Ark storage, sync, and cross-runtime bindings.

## Runtime status (2026-04-16)

- DB layer is live in production-facing apps. Electron sidecars and Rust callers use the same SQLite schema and CRUD helpers.
- Sync layer is live. `main.rs` exposes sync lifecycle RPCs; `ffi.rs` exposes the same runtime to UniFFI callers.
- Usage tracking entities are first-class Ark data. `tracked_apps`, `usage_sessions`, and `usage_events` are stored in the main Ark DB and participate in sync.
- Schema evolution is additive. Existing Ark databases are migrated in place by `init_schema` without replacing the DB file.

## Architecture

```text
rust/src/
  main.rs             stdin/stdout JSON-RPC sidecar for Electron callers
  ffi.rs              UniFFI facade for Android / Swift / embedded callers
  lib.rs              library entry point and public re-exports
  types.rs            shared entities and sync payloads
  schema.rs           CREATE TABLE / index statements
  db.rs               SQLite CRUD, migrations, and sync entity loading
  hlc.rs              Hybrid Logical Clock helpers
  protocol.rs         sync wire messages and batching
  sync_server.rs      WebSocket sync server
  sync_client.rs      WebSocket sync client
  beacon.rs           UDP peer discovery
  relay_transport.rs  outbound relay client
  mesh.rs             LAN + relay coordination
  host.rs / net.rs    hostname + routable address filtering
```

## Primary entities

- Core planning entities: `TodoItem`, `Project`, `Area`, `Tag`, `Heading`
- Usage entities: `TrackedApp`, `UsageSession`, `UsageEvent`
- Sync metadata: `PeerRecord`, `VersionVector`, HLC strings in `sync_kv`

## JSON-RPC surface

The sidecar request envelope stays:

```json
{"operation":"snake_case", "...":"params"}
```

Relevant operations now include:

- DB init/load: `init`, `load_all`, `clear_all`, `delete_trashed`
- Core entity CRUD: `upsert_todo`, `delete_todo`, `batch_upsert_todos`, `upsert_project`, `delete_project`, `upsert_area`, `upsert_tag`, `upsert_heading`, `delete_heading`
- Usage entity CRUD: `upsert_tracked_app`, `delete_tracked_app`, `upsert_usage_session`, `delete_usage_session`, `upsert_usage_event`, `delete_usage_event`
- Sync KV: `get_sync_kv`, `set_sync_kv`
- Sync runtime: `start_sync`, `stop_sync`, `broadcast_change`, `get_connected_peers`, `add_seed_peer`, `leave_space`, `get_own_addresses`, `get_host_device_name`

## Sync rules

- Every persisted syncable entity must round-trip through `db.rs::load_entities` and `db.rs::apply_entity`.
- Direct writers outside the RPC layer, such as `services/usage-tracker`, MUST bump `lan_sync.version_vector` after writing usage entities. If the version vector is stale, CRDT merge correctness is broken.
- Schema additions must be idempotent. Prefer `CREATE TABLE IF NOT EXISTS` / additive migrations over destructive rewrites.
- Sync protocol remains snake_case on the wire; RPC stays camelCase where legacy Electron callers depend on it.
- Self-peer filtering and routable-address filtering remain mandatory invariants. Do not relax them when touching sync startup or peer persistence.

## Verification expectations

- Run `cargo test` in `packages/ark-core/rust`.
- If you touch sync or schema behavior, add or update tests for migration and replication, not only local CRUD tests.
