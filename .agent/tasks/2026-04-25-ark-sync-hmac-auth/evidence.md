# Evidence: ARK LAN sync HMAC authentication

## Verdict

PASS

## Acceptance Criteria

AC1 PASS. `LanSyncMessage::Hello` now has optional `auth_nonce` and `auth_hmac` fields with serde defaults. Legacy JSON without auth fields is covered by `protocol::tests::test_deserialize_ts_compatible_hello`.

AC2 PASS. `protocol.rs` provides nonce generation, HMAC-SHA256 calculation, secret normalization, and constant-time verification. Covered by `protocol::tests::test_hello_hmac_helpers_are_deterministic_and_verify` and `protocol::tests::test_auth_nonce_is_random_hex_64`.

AC3 PASS. `SyncServer` stores optional `auth_secret`, verifies inbound hello HMAC before marking a peer authenticated, and closes/rejects invalid hellos. Covered by `hmac_authenticated_sync_rejects_wrong_secret`.

AC4 PASS. `SyncClient` stores optional `auth_secret`, sends nonce/HMAC in outbound hello, and verifies authenticated server hellos when configured. Covered by `hmac_authenticated_sync_succeeds_with_matching_secret` and wrong-secret rejection.

AC5 PASS. `ark-core-rpc` `start_sync` accepts `auth_secret` and propagates it to the server, known-peer clients, seed clients, and beacon-discovered clients. The FFI config path mirrors the same propagation.

AC6 PASS. `@arksync/node` exposes `authSecret?: string` and sends `auth_secret` in `start_sync`. Covered by `verify-arksync-node-auth-secret.ts`.

AC7 PASS. Fresh verification ran against the current worktree; raw command outputs are saved in this task directory.

Docs updated to reflect the new HMAC-auth state and to keep relay documented as fail-fast until full relay state-machine wiring exists.

## Raw Artifacts

- `cargo-check.txt`
- `cargo-fmt-check.txt`
- `cargo-test.txt`
- `arksync-node-typecheck.txt`
- `arksync-node-build.txt`
- `verify-arksync-node-auth-secret.txt`
- `git-diff-check.txt`
- `problems.md`

## Commands

- `cargo check --manifest-path packages\ark-core\rust\Cargo.toml` PASS
- `cargo fmt --manifest-path packages\ark-core\rust\Cargo.toml --check` PASS
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml` PASS
- `bun run --cwd packages\arksync-node typecheck` PASS
- `bun run --cwd packages\arksync-node build` PASS
- `bun .agent\tasks\2026-04-25-ark-sync-hmac-auth\verify-arksync-node-auth-secret.ts` PASS
- `git diff --check` PASS with existing CRLF normalization warnings only

## Notes

This authenticates peers that know the shared secret. It does not encrypt LAN WebSocket traffic; encryption remains a separate future hardening step.
