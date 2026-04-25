# Evidence

Task: `2026-04-24-ark-sidecar-request-id`

Verification date: 2026-04-24

## Acceptance Criteria

### AC1

PASS. Legacy requests without `id` still produce responses without `id`.

Raw evidence:

- `cargo-test-response-id.txt`
- `cargo-test-full.txt`
- `source-evidence.txt`

### AC2

PASS. Requests with `id` are accepted as metadata, and success/error responses echo the same `id`.

Raw evidence:

- `cargo-test-response-id.txt`
- `cargo-test-request-id.txt`
- `cargo-test-full.txt`
- `source-evidence.txt`

### AC3

PASS. Async event handling remains keyed by `event` frames in clients and the Rust sidecar id helpers are only used for response construction. No event frame shape was changed.

Raw evidence:

- `git-diff.txt`
- `verify-arksync-node-request-id.txt`

### AC4

PASS. The self-managed `@arksync/node` child-process path now attaches unique ids and resolves responses by matching response ids. The proof script resolves two responses out of order and confirms each resolves the matching request.

Raw evidence:

- `verify-arksync-node-request-id.ts`
- `verify-arksync-node-request-id.txt`
- `source-evidence.txt`

### AC5

PASS. Injected `requestFn` mode still sends legacy-shaped requests. The proof script verifies `ArkClient.start()` in injected mode sends only `start_sync` and does not add `id`.

Raw evidence:

- `verify-arksync-node-request-id.ts`
- `verify-arksync-node-request-id.txt`

### AC6

PASS. Fresh Rust and TypeScript verification commands were run and recorded.

Raw evidence:

- `cargo-check.txt`
- `cargo-test-response-id.txt`
- `cargo-test-request-id.txt`
- `cargo-test-full.txt`
- `cargo-fmt-check.txt`
- `arksync-node-typecheck.txt`
- `arksync-node-build.txt`
- `git-diff-check.txt`

## Commands

- `cargo check --manifest-path packages/ark-core/rust/Cargo.toml`: PASS
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml response_`: PASS
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml request_deserialization_ignores_optional_id_field`: PASS
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml`: PASS
- `cargo fmt --manifest-path packages/ark-core/rust/Cargo.toml -- --check`: PASS
- `bun run typecheck` in `packages/arksync-node`: PASS
- `bun run build` in `packages/arksync-node`: PASS
- `bun .agent/tasks/2026-04-24-ark-sidecar-request-id/verify-arksync-node-request-id.ts`: PASS
- `git diff --check -- ...`: PASS

## Notes

Existing warnings remain:

- `relay_url` and `relay_api_key` are currently unused in `ark-core-rpc`.
- `tests/relay_round_trip.rs` has pre-existing unused imports/variables.

These warnings are unrelated to sidecar request ids and were left unchanged.
