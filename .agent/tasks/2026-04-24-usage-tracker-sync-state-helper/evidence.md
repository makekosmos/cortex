# Evidence: Usage Tracker Uses Ark Core Sync-State Helper

Verification result: PASS

## Acceptance Criteria

- AC1: PASS. Local `VERSION_VECTOR_KEY` and `fn bump_version_vector` were removed from `services/usage-tracker/src/main.rs`.
- AC2: PASS. Tracked app/session/event persistence now calls `ark_core::db::bump_sync_version_vector`.
- AC3: PASS. `usage_tracker.device_id` still uses `get_sync_kv`/`set_sync_kv`; focused test verifies generation and reload.
- AC4: PASS. `cargo check`, `cargo test`, `cargo fmt --check`, source grep, and `git diff --check` passed.

## Raw Artifacts

- `cargo-check.txt`
- `cargo-test.txt`
- `cargo-fmt-check.txt`
- `source-evidence.txt`
- `git-diff-check.txt`
- `git-diff.txt`

## Notes

- `git diff --check` reports CRLF conversion warnings only and exits 0.
