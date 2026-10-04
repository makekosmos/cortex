# ark-core — the ARK runtime crate

`core/crates/ark-core` is the shared Rust + SQLite runtime ("ARK") vendored
into cortex (subtree merge, KOS-129). The Engine hosts it **in-process** via
`ark_core::service::ArkService` — the old `ark-core-rpc` sidecar binary no
longer exists (`runtime/src/ark_host.rs` is the host wrapper).

## Request/response contract

- Engine calls `ArkService::call(op, params)` and receives the same wire
  shape the sidecar returned: `{"ok","data","error"}`. The op surface is kept
  stable for the ~36 runtime callers and the package worker protocol —
  evolve it additively.
- Dispatch is sequential FIFO on a **dedicated worker thread** with a private
  multi-thread tokio runtime (4 workers). SQLite never runs on Engine
  threads; ordering equals the sidecar's serial stdin loop.
- Handlers run under `catch_unwind`: a panic becomes a request error and the
  database is reopened (sync runtime stopped, shared connection dropped,
  in-flight transaction rolled back). Workspace lints deny `unwrap_used` and
  warn on `panic` — return errors instead.
- Events flow through the process-wide `crate::events` broadcast bus
  (`ArkService::subscribe`); the Engine re-publishes them via `events_tx` and
  its own `emit_event`s. Exactly one `ArkService` per process.

## Data model (schema.rs)

- Typed objects: `object_types` + `objects` (+ `object_links`), registered
  through `type_registry` / `TypeRegistration`.
- Usage tracking: `tracked_apps`, `usage_sessions`, `usage_events`,
  `usage_days` — sequenced entities with their own sync machinery
  (`usage_sync_versions/heads/log`).
- KV + sync metadata: `sync_kv`, `sync_tombstones`, version vectors.
- Integration replication tables: `authorized_nodes`,
  `integration_node_grants`, `integration_credential_envelopes`, etc.
- Schema evolution is **additive only**: `CREATE TABLE IF NOT EXISTS`,
  additive columns/indexes. No `DROP TABLE`, no incompatible `ALTER`.

## Layout

- `src/db/` — storage backend (`SqliteStorageBackend`), loaders, `sync/` for
  version-vector + sequencing logic.
- `src/service/` + `src/service.rs` — `ArkService`, request dispatcher
  (`runtime::handle_request`), op handlers.
- `src/sync_server*.rs`, `src/sync_client*.rs` — sync peers; transports in
  `src/iroh_*` (p2p QUIC), `src/relay_*` (WebSocket relay), `src/beacon_*`
  (LAN broadcast discovery). Selection: `src/transport_select.rs`.
- `src/hlc.rs`, `src/net.rs`, `src/protocol/` — hybrid logical clock,
  routable-address rules, wire protocol.
- `src/canonical_types/`, `src/type_registry/` — built-in object types.

## Tests

`cargo nextest run -p ark-core` (from repo root; the crate builds only via
the workspace). `cargo shear`/`clippy`/`deny` run repo-wide.
