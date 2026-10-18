# AGENTS.md — ark-core

Compact local boot context for the vendored ARK crate. Detailed topics live in
the repo-root docs; the retired `makekosmos/docs` repo is no longer the source.

## Source docs (repo root `docs/`)

- `docs/ark-core.md` — crate contract: ArkService threading/fault model,
  request dispatch, events.
- `docs/sync.md` — sync protocol: HLC, version vectors, transports.
- `docs/write-boundary.md` — allowed write paths into the user database.
- `docs/brand-legacy-identifiers.md` — persisted legacy-brand identifiers.

## Scope

- `core/crates/ark-core/` is the shared Rust + SQLite runtime; the Engine
  hosts it in-process via `ark_core::service::ArkService`
  (`runtime/src/ark_host.rs`). The old `ark-core-rpc` sidecar is gone.
- Requests are dispatched to a dedicated worker thread in FIFO order under
  `catch_unwind`. A handler panic makes the active request outcome unknown,
  cancels queued requests, stops sync, and terminates the service worker. The
  service does not reopen the DB in place. A new service instance must complete
  normal startup and recovery before it accepts requests (see `src/service.rs`).
- Events go out through the process-wide `crate::events` broadcast bus; the
  Engine re-publishes them on `events_tx`.
- Workspace lints deny `unwrap_used`/`unreachable` and warn on `panic` —
  handlers must return errors, not panic.

## Invariants

- No `Mutex::lock().unwrap()` in production paths; recover poison with
  `unwrap_or_else(|e| e.into_inner())`.
- Schema evolution is additive by default: prefer idempotent
  `CREATE TABLE IF NOT EXISTS` / additive indexes (`../ark-store/src/schema.rs`). A one-off
  rewrite of stored objects is allowed only when it is approved for a specific
  issue, runs once in a transaction, and takes a backup of `ark.db` first
  (KOS-397: `com.kosmos.task` 1.1.0 → `core.task` 0.1.0, approved 2026-10-13).
- Every writer to syncable data must update sync state through
  `ark_core::db::bump_sync_version_vector` (usage entities go through
  `record_usage_sequence`; see `src/db/sync/`).
- Sync wire messages stay `snake_case`; do not weaken self-peer or
  routable-address filtering (`src/sync_server_peers.rs`, `src/net.rs`).
- The Engine-facing request surface (`{"op","params"}` →
  `{"ok","data","error"}`) is kept stable for the ~36 runtime callers and the
  package worker protocol.

## Commands

- `cargo nextest run -p ark-core` — crate tests (needs
  `cargo install cargo-nextest --locked`; the crate builds only through the
  root workspace).
- `pnpm run check:affected` / `pnpm run check` — repo gate; clippy runs
  workspace-wide with `-D warnings`.
