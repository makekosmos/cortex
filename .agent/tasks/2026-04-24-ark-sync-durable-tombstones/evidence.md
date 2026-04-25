# Evidence

Task: `2026-04-24-ark-sync-durable-tombstones`

Verification date: 2026-04-24

## Acceptance Criteria

### AC1

PASS. Schema initialization now creates `sync_tombstones` with `id`, `entity_type`, `hlc`, and `deleted_at`, plus an HLC index.

Raw evidence:

- `source-evidence.txt`
- `git-diff.txt`
- `cargo-test-full.txt`

### AC2

PASS. Applying `SyncEntity { deleted: Some(true), ... }` now physically deletes the current row and records a tombstone using the delete HLC.

Raw evidence:

- `source-evidence.txt`
- `git-diff.txt`
- `cargo-test-full.txt`

### AC3

PASS. Applying a non-deleted entity now clears any existing tombstone for the same id after the upsert succeeds.

Raw evidence:

- `source-evidence.txt`
- `git-diff.txt`
- `test-summary.txt`

### AC4

PASS. `load_entities` now appends persisted tombstones as deleted `SyncEntity` values with empty `data`.

Raw evidence:

- `source-evidence.txt`
- `git-diff.txt`
- `test-summary.txt`

### AC5

PASS. Sync server/client successful delete paths now retain the delete HLC by inserting `entity.id -> entity.hlc` into the version vector instead of removing the id.

Raw evidence:

- `vector-evidence.txt`
- `git-diff.txt`

### AC6

PASS. Regression tests prove:

- deleted entities are emitted as tombstones
- tombstones survive storage backend recreation
- a later live entity clears the tombstone

Raw evidence:

- `cargo-test-full.txt`
- `test-summary.txt`

### AC7

PASS. Fresh verification commands were run and recorded.

Raw evidence:

- `cargo-check.txt`
- `cargo-test-tombstone.txt`
- `cargo-test-full.txt`
- `cargo-fmt-check.txt`
- `git-diff-check.txt`

## Commands

- `cargo check --manifest-path packages/ark-core/rust/Cargo.toml`: PASS
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml storage_backend_tombstone`: PASS
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml`: PASS
- `cargo fmt --manifest-path packages/ark-core/rust/Cargo.toml -- --check`: PASS
- `git diff --check -- ...`: PASS

## Notes

Existing warnings remain:

- `relay_url` and `relay_api_key` are currently unused in `ark-core-rpc`.
- `tests/relay_round_trip.rs` has pre-existing unused imports/variables.

These warnings are unrelated to durable tombstones and were left unchanged.
