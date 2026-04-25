# Evidence

Task: `2026-04-24-ark-sync-apply-errors`

Verification date: 2026-04-24

## Acceptance Criteria

### AC1

PASS. `StorageBackend::apply_entity` now returns `Result<(), String>`.

Raw evidence:

- `source-evidence.txt`
- `git-diff.txt`

### AC2

PASS. `SqliteStorageBackend::apply_entity_blocking` now returns `Result<(), String>` and propagates:

- JSON decode errors from `serde_json::from_value`
- database CRUD errors from upsert/delete helpers
- unknown sync entity types as `unknown sync entity type ...`

Raw evidence:

- `source-evidence.txt`
- `git-diff.txt`
- `cargo-test-regression.txt`

### AC3

PASS. Sync server/client `SyncChanges` and `LiveChange` paths now update version vectors, accepted counts, callbacks, and rebroadcasts only after `storage.apply_entity(...).await` returns `Ok(())`. Failed applies are logged and are not counted as accepted.

Raw evidence:

- `source-evidence.txt`
- `git-diff.txt`
- `cargo-test-full.txt`

### AC4

PASS. Local RPC and UniFFI broadcast paths now surface apply failures:

- `ark-core-rpc` `broadcast_change` uses `runtime.storage.apply_entity(&entity).await?`
- UniFFI `broadcast_change_json` maps the apply error into `ArkCoreError`

Raw evidence:

- `source-evidence.txt`
- `git-diff.txt`
- `cargo-check.txt`

### AC5

PASS. Regression tests cover both invalid sync payloads and unknown entity types:

- `storage_backend_rejects_invalid_sync_payload`
- `storage_backend_rejects_unknown_sync_entity_type`

Raw evidence:

- `cargo-test-regression.txt`
- `cargo-test-full.txt`

### AC6

PASS. Fresh verification commands were run and recorded.

Raw evidence:

- `cargo-check.txt`
- `cargo-test-regression.txt`
- `cargo-test-full.txt`
- `cargo-fmt-check.txt`
- `git-diff-check.txt`

## Commands

- `cargo check --manifest-path packages/ark-core/rust/Cargo.toml`: PASS
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml storage_backend_rejects`: PASS
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml`: PASS
- `cargo fmt --manifest-path packages/ark-core/rust/Cargo.toml -- --check`: PASS
- `git diff --check -- ...`: PASS

## Notes

Existing warnings remain:

- `relay_url` and `relay_api_key` are currently unused in `ark-core-rpc`.
- `tests/relay_round_trip.rs` has pre-existing unused imports/variables.

These warnings are unrelated to apply-error propagation and were left unchanged.
