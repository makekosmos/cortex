# Evidence: ARK RPC Object Type/Link Local Sync State

Verification result: PASS

## Acceptance Criteria

- AC1: PASS. `UpsertObjectType`, `DeleteObjectType`, `UpsertObjectLink`, and `DeleteObjectLink` now accept optional `device_id`.
- AC2: PASS. Focused Rust test verifies object type/link upserts record caller-device HLCs in `lan_sync.version_vector`.
- AC3: PASS. Focused Rust test verifies object type/link deletes create durable `sync_tombstones` rows.
- AC4: PASS. `@arksync/node` verifier confirms object type/link writes send `device_id`.
- AC5: PASS. Fresh Rust checks/tests/formatting, `@arksync/node` typecheck/build, SDK verifier, and full `git diff --check` passed.

## Raw Artifacts

- `cargo-check.txt`
- `cargo-test-focused.txt`
- `cargo-test-full.txt`
- `cargo-fmt-check.txt`
- `arksync-node-typecheck.txt`
- `arksync-node-build.txt`
- `verify-arksync-node-device-id.ts`
- `verify-arksync-node-device-id.txt`
- `git-diff-check.txt`
- `git-diff-check-scoped.txt`
- `source-evidence.txt`
- `git-diff.txt`
- `problems.md`

## Notes

- `cargo test` still reports pre-existing warnings in `tests/relay_round_trip.rs`; all tests pass.
- `git diff --check` reports CRLF conversion warnings only and exits 0.
