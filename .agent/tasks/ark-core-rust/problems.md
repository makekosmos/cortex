# Problems: ark-core-rust

## AC12: StorageBackend trait -- SqliteStorageBackend missing

- **Status**: FAIL
- **Why it is not proven**: The `StorageBackend` trait is defined in `sync_server.rs` with the correct signature, but no `SqliteStorageBackend` struct implementing it exists. The spec requires "A SqliteStorageBackend implementation backed by the DB layer (AC1-AC2)."
- **Minimal reproduction steps**: `grep -r SqliteStorageBackend packages/ark-core/rust/src/` returns no matches.
- **Expected**: A struct `SqliteStorageBackend` wrapping a `rusqlite::Connection` (via `Arc<Mutex<Connection>>` for Send+Sync) that implements `StorageBackend` by delegating to `db.rs` functions.
- **Actual**: No implementation exists.
- **Affected files**: `packages/ark-core/rust/src/db.rs` or a new file (e.g. `storage.rs`)
- **Smallest safe fix**: Add `SqliteStorageBackend` struct (~50 lines) implementing `load_entities` (query all entity types, convert to SyncEntity), `apply_entity` (dispatch upsert/delete by entity_type), `get_kv`/`set_kv` (delegate to db::get_sync_kv/set_sync_kv).
- **Corrective hint**: The DB functions already exist. `load_entities` needs to load todos/projects/areas/tags/headings, convert each to `SyncEntity` with the appropriate `entity_type` and `data` JSON map. `apply_entity` dispatches on `entity.entity_type` to call the right upsert or delete function.

## AC13: UniFFI export surface -- not implemented

- **Status**: FAIL
- **Why it is not proven**: No `uniffi` dependency in `Cargo.toml`. No `#[uniffi::export]` annotations or UDL file anywhere in the crate.
- **Minimal reproduction steps**: `grep uniffi packages/ark-core/rust/Cargo.toml` returns nothing.
- **Expected**: `uniffi` in `[dependencies]`, public API functions annotated for Kotlin/Swift export, callback interface for change/peer notifications.
- **Actual**: No UniFFI integration.
- **Affected files**: `Cargo.toml`, `lib.rs`, potentially `uniffi.toml` or a UDL file.
- **Smallest safe fix**: Add `uniffi = { version = "0.28", features = ["build"] }` to deps. Add `#[uniffi::export]` to key functions in `lib.rs`. Define callback interfaces. Approximately 100-150 lines of new code.
- **Corrective hint**: Use proc-macro approach. Export functions listed in spec AC13: open_db, CRUD ops, start/stop server/client, space code functions, broadcast_live_change. Callback interface needs traits for on_change and on_peer_connect/disconnect.

## AC14: Dual build targets -- cdylib missing

- **Status**: FAIL
- **Why it is not proven**: `Cargo.toml` line 10 has `crate-type = ["lib"]` but spec requires `["cdylib", "lib"]` for UniFFI shared library output.
- **Minimal reproduction steps**: Read `packages/ark-core/rust/Cargo.toml` line 10.
- **Expected**: `crate-type = ["cdylib", "lib"]`
- **Actual**: `crate-type = ["lib"]`
- **Affected files**: `packages/ark-core/rust/Cargo.toml`
- **Smallest safe fix**: Change line 10 from `crate-type = ["lib"]` to `crate-type = ["cdylib", "lib"]`. One-line change. Depends on AC13 being implemented first to be useful.
- **Corrective hint**: This is a one-line Cargo.toml edit, but it should be done together with AC13 so the cdylib actually contains UniFFI scaffolding.
