# Evidence: ARK relay state machine

## Verdict

PASS

## Acceptance Criteria

AC1 PASS. `ark-core-rpc` no longer rejects relay options. `start_sync` accepts `relay_url` / `relay_api_key` and starts `RelaySync` when `relay_url` is present. Covered by `request_deserialization_accepts_relay_and_auth_config`.

AC2 PASS. `RelayTransport` sends HMAC-authenticated `hello` when `auth_secret` is configured and surfaces sender ids via `message_origin_device_id`.

AC3 PASS. `RelaySync` handles relay `hello`, verifies HMAC when configured, tracks authenticated peers, emits peer connect events, replies with local `hello`, and sends version vectors.

AC4 PASS. `RelaySync` applies inbound `sync_changes` and `live_change` through `StorageBackend::apply_entity`, updates version vectors, emits `entity_changed`, and logs apply errors instead of silently accepting them.

AC5 PASS. `RelaySync` responds to inbound relay version vectors by sending missing local entities as `sync_changes` batches.

AC6 PASS. `handle_broadcast_change` now broadcasts persisted local live changes through relay when relay is running.

AC7 PASS. `handle_stop_sync` stops relay transport.

AC8 PASS. `@arksync/node` no longer rejects relay options before calling the sidecar, and verifier proves relay options are forwarded.

AC9 PASS. Fresh verification ran against the current worktree; raw command outputs are saved in this task directory.

## Raw Artifacts

- `cargo-check.txt`
- `cargo-fmt-check.txt`
- `cargo-test.txt`
- `arksync-node-typecheck.txt`
- `arksync-node-build.txt`
- `verify-arksync-node-relay-options.txt`
- `git-diff-check.txt`
- `problems.md`

## Commands

- `cargo check --manifest-path packages\ark-core\rust\Cargo.toml` PASS
- `cargo fmt --manifest-path packages\ark-core\rust\Cargo.toml --check` PASS
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml` PASS
- `bun run --cwd packages\arksync-node typecheck` PASS
- `bun run --cwd packages\arksync-node build` PASS
- `bun .agent\tasks\2026-04-25-ark-relay-state-machine\verify-arksync-node-relay-options.ts` PASS
- `git diff --check` PASS with existing CRLF normalization warnings only

## Notes

This wires relay support for the desktop `ark-core-rpc` sidecar path. UniFFI/mobile relay lifecycle wiring is documented as a separate remaining pass.
