# Evidence

Task: `2026-04-24-ark-relay-fail-fast`

Verification date: 2026-04-24

## Acceptance Criteria

### AC1

PASS. `ark-core-rpc` `start_sync` now returns `Relay sync is not supported by ark-core-rpc yet` when `relay_url` or `relay_api_key` is provided.

Raw evidence:

- `source-evidence.txt`
- `cargo-test-relay.txt`
- `cargo-test-full.txt`

### AC2

PASS. `@arksync/node` `ArkClient.start()` now throws `@arksync/node: relay sync is not supported yet` when `relayUrl` or `relayApiKey` is provided.

Raw evidence:

- `verify-arksync-node-relay.ts`
- `verify-arksync-node-relay.txt`
- `source-evidence.txt`

### AC3

PASS. Starting sync without relay options remains unchanged. The SDK relay proof script starts an injected client without relay options and verifies it sends one `start_sync` request with `relay_url: null` and `relay_api_key: null`.

Raw evidence:

- `verify-arksync-node-relay.ts`
- `verify-arksync-node-relay.txt`
- `cargo-test-full.txt`

### AC4

PASS. Injected `requestFn` mode remains intact: validation happens before `requestFn` is called when relay options are present, and legacy no-relay injected start still calls `requestFn` normally.

Raw evidence:

- `verify-arksync-node-relay.ts`
- `verify-arksync-node-relay.txt`

### AC5

PASS. Fresh Rust and TypeScript verification commands were run and recorded.

Raw evidence:

- `cargo-check.txt`
- `cargo-test-relay.txt`
- `cargo-test-full.txt`
- `cargo-fmt-check.txt`
- `arksync-node-typecheck.txt`
- `arksync-node-build.txt`
- `git-diff-check.txt`

## Commands

- `cargo check --manifest-path packages/ark-core/rust/Cargo.toml`: PASS
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml start_sync_rejects_relay_config`: PASS
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml`: PASS
- `cargo fmt --manifest-path packages/ark-core/rust/Cargo.toml -- --check`: PASS
- `bun run typecheck` in `packages/arksync-node`: PASS
- `bun run build` in `packages/arksync-node`: PASS
- `bun .agent/tasks/2026-04-24-ark-relay-fail-fast/verify-arksync-node-relay.ts`: PASS
- `git diff --check -- ...`: PASS

## Notes

The previous Rust warning for unused `relay_url` / `relay_api_key` is gone because those fields are now explicitly validated. Existing warnings in `tests/relay_round_trip.rs` remain unrelated and were left unchanged.
