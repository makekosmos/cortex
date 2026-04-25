# Evidence: ARK UniFFI relay lifecycle

## Verdict

PASS

## Acceptance Criteria

AC1 PASS. `FfiSyncConfig` relay fields are now used by `ArkCore::start_sync`; when `relay_url` is present, the UniFFI path creates and starts `RelaySync` with `relay_api_key` and `auth_secret`.

AC2 PASS. UniFFI relay callbacks are wired to the existing `ArkEventListener`: relay entity changes call `on_entity_changed`, authenticated relay peers call `on_peer_connected`, and relay disconnects call `on_peer_disconnected`.

AC3 PASS. `ArkCore::broadcast_change_json` broadcasts persisted local live changes through relay when relay is running.

AC4 PASS. `ArkCore::stop_sync` stops the relay transport via `RelaySync::stop`.

AC5 PASS. `ArkCore::get_connected_peers` merges authenticated relay peers into the existing peer list without duplicating LAN/outbound entries.

AC6 PASS. Fresh verification ran against the current worktree; raw command outputs are saved in this task directory. The verifier checks the FFI relay lifecycle wiring shape, and full Rust tests cover the shared relay state machine behavior.

## Raw Artifacts

- `cargo-check.txt`
- `cargo-fmt-check.txt`
- `cargo-test.txt`
- `arksync-node-typecheck.txt`
- `arksync-node-build.txt`
- `verify-ffi-relay-wiring.txt`
- `git-diff-check.txt`
- `problems.md`

## Commands

- `cargo check --manifest-path packages\ark-core\rust\Cargo.toml` PASS
- `cargo fmt --manifest-path packages\ark-core\rust\Cargo.toml --check` PASS
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml` PASS
- `bun run --cwd packages\arksync-node typecheck` PASS
- `bun run --cwd packages\arksync-node build` PASS
- `bun .agent\tasks\2026-04-25-ark-ffi-relay-lifecycle\verify-ffi-relay-wiring.ts` PASS
- `git diff --check` PASS with existing CRLF normalization warnings only

## Notes

This brings relay lifecycle parity to the Rust UniFFI facade. Remaining mobile work is platform smoke testing and regenerating/consuming bindings in the mobile app build flow when needed.
