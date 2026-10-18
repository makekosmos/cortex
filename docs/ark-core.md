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
- Handlers run under `catch_unwind`. A handler panic makes the current request
  outcome unknown, cancels queued requests, stops sync, and terminates the
  service worker. The service does not reopen the database in place. A new
  service instance must pass normal startup and recovery before accepting
  requests. Workspace lints deny `unwrap_used` and warn on `panic`; handlers
  should return errors instead.
- Events flow through the process-wide `crate::events` broadcast bus
  (`ArkService::subscribe`); the Engine re-publishes them via `events_tx` and
  its own `emit_event`s. Exactly one `ArkService` per process.

## Data model and storage

`ark-store` owns the SQLite writer connection, snapshots, and the schema and
storage adapters already moved out of Core. Canonical type migration
coordination and some SQL-backed runtime paths still live in `ark-core`; the
SQLite boundary remains an active extraction.

- Typed objects: `object_types` + `objects` (+ `object_links`), registered
  through `type_registry` / `TypeRegistration`.
- Usage tracking: `tracked_apps`, `usage_sessions`, `usage_events`,
  `usage_days` — sequenced entities with their own sync machinery
  (`usage_sync_versions/heads/log`).
- KV + sync metadata: `sync_kv`, `sync_tombstones`, version vectors.
- Integration replication tables: `authorized_nodes`,
  `integration_node_grants`, `integration_credential_envelopes`, etc.
- Schema evolution is **additive by default**: `CREATE TABLE IF NOT EXISTS`,
  additive columns/indexes. No `DROP TABLE`, no incompatible `ALTER`. A one-off
  object rewrite needs an explicit approval per issue, a transaction and a
  `ark.db` backup taken before it runs (see `core/crates/ark-core/AGENTS.md`).

## Crate ownership

- `ark-domain` contains shared domain values and protocol-independent types.
- `ark-application` defines capabilities used by runtime, sync, and storage.
- `ark-store` owns SQLite opening, the writer actor, snapshots, and extracted
  query and persistence adapters. Some SQL and migration work remains in Core.
- `ark-sync` owns extracted sync policy and session decisions. Core still runs
  parts of the direct client/server loops and applies their effects.
- `ark-net` owns concrete WebSocket, relay, Iroh, and discovery transports.
- `ark-core` exposes `ArkService` and composes these crates. It still owns
  request handlers, migration coordination, and portions of sync orchestration.

## Tests

From the Cortex repository root, run `cargo nextest run -p ark-core`. The crate builds through the root workspace.
`cargo shear`, clippy, and deny run repo-wide.
