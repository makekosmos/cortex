# Evidence: ark-core-rpc Local Write Sync State

## Verdict

PASS

## Acceptance Criteria

- AC1: PASS. `upsert_object`, `upsert_tracked_app`, `upsert_usage_session`, and `upsert_usage_event` update `lan_sync.version_vector`.
- AC2: PASS. `delete_object`, `delete_tracked_app`, `delete_usage_session`, and `delete_usage_event` update `lan_sync.version_vector` and record sync tombstones.
- AC3: PASS. Live upsert handling clears prior tombstones through `delete_sync_tombstone`.
- AC4: PASS. Write requests accept optional `device_id`; absent values use the stable fallback `ark-core-rpc-local`.
- AC5: PASS. `@arksync/node` sends `this.opts.deviceId` on object and usage write requests.
- AC6: PASS. Fresh Rust and TypeScript verification commands are recorded in this task directory.

## Raw Artifacts

- `cargo-check.txt`: `cargo check` for `ark-core`.
- `cargo-test-local.txt`: focused Rust tests for local sync metadata.
- `cargo-test-full.txt`: full Rust test suite for `ark-core`.
- `cargo-fmt-check.txt`: Rust format check.
- `arksync-node-typecheck.txt`: `@arksync/node` typecheck.
- `arksync-node-build.txt`: `@arksync/node` build.
- `verify-arksync-node-device-id.txt`: SDK write request proof script.
- `git-diff-check.txt`: whitespace/error diff check for touched files.
- `source-evidence.txt`: source locations for sync metadata and SDK device id wiring.
- `git-diff.txt`: current patch for the task.

## Notes

Full Rust tests pass. `tests/relay_round_trip.rs` still emits pre-existing unused import/variable warnings.
