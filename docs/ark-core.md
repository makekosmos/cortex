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

`ark-store` owns SQLite opening, the single writer connection, transaction
execution, snapshots and restore, compatibility migrations, and the extracted
query and persistence adapters. Canonical type migration coordination and
some SQL-backed runtime paths still live in `ark-core`; those are explicit
remaining boundaries, not claims that Core is already a storage-free facade.

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

## Durability and recovery

`ark-store::open_db` configures SQLite WAL mode with `synchronous=FULL`, foreign
keys enabled, and a 5-second busy timeout on every open. This is the configured
SQLite durability contract; the project has not measured sudden power loss on
every supported filesystem or device, so a successful commit is not a claim
about hardware that lies about flush completion.

Backups use SQLite's online backup API and are published only after the
snapshot is validated. Restore stages and validates the source, compares its
schema and committed-feed watermark with the live database, preserves the
current device identity, and writes a durable recovery capsule and intent before
applying. A snapshot older than committed sync history is refused before the
live database changes. That refusal is the current recovery boundary: there is
no recovery-epoch/resnapshot protocol yet. After apply, integrity and schema
checks must pass; failures attempt rollback from the capsule. If rollback or
journal completion cannot be proven, the database actor fails closed and
startup recovery must complete before the service accepts requests again.

The current SQLite failure evidence is in
`core/crates/ark-core/src/db/command/tests/production_actor/failures.rs` and
`.../lifecycle.rs`: `SQLITE_FULL`, commit failure, restore failure, and
recovery-required restore all fail closed; panic rolls back and requires
reopen. `core/crates/ark-core/src/service/tests/pairing_iroh/recovery_tests/`
proves a stale restore is refused when an existing peer has observed a change,
pending local usage and origin sequence remain intact, and later changes
resynchronize after reconnect. These tests exercise SQLite failure paths and
peer behavior; they do not simulate physical power loss.

## Crate ownership

- `ark-domain` contains shared domain values and protocol-independent types.
- `ark-application` defines capabilities used by runtime, sync, and storage.
- `ark-store` owns SQLite opening, the writer actor, snapshots, compatibility
  migrations, and extracted query and persistence adapters. Canonical type
  migration coordination and selected SQL-backed runtime paths remain in Core.
- `ark-sync` owns direct client and server session admission, readiness,
  protocol message handling, snapshot/feed/task-merge/live coordination, ACK
  validation, retry state, and session progress. Core supplies storage,
  Engine callbacks, peer-map projection, and service lifecycle.
- `ark-net` owns concrete WebSocket, relay, Iroh, and discovery transports.
- `ark-core` exposes `ArkService`, adapts the Engine JSON contract, and
  composes these crates. It still owns request handlers, migration
  coordination, and selected SQL-backed runtime paths.

## Tests

From the Cortex repository root, run `cargo nextest run -p ark-core`. The crate builds through the root workspace.
`cargo shear`, clippy, and deny run repo-wide.
