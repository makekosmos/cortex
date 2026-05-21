# Task: ARK LAN sync HMAC authentication

## Context

ARK LAN/P2P sync currently authenticates peers only by `space_id` and `device_id` in the `hello` message. That is not enough for a shared local-first store because any process on the LAN that knows or guesses a `space_id` can attempt to join the sync mesh.

This task adds an optional HMAC proof to the LAN sync `hello` handshake. The feature must be backward-compatible for development: no secret means the current unauthenticated behavior remains available. When a node is configured with an auth secret, inbound peer hellos must include a valid nonce + HMAC before they become authenticated or receive sync data.

## Acceptance Criteria

AC1. `LanSyncMessage::Hello` supports optional `auth_nonce` and `auth_hmac` fields while still deserializing legacy hello JSON without those fields.

AC2. ARK core provides deterministic HMAC-SHA256 helpers for `space_id`, `device_id`, and nonce, uses a random nonce for outgoing hellos, and verifies HMACs with constant-time comparison.

AC3. `SyncServer` rejects inbound hello messages when `auth_secret` is configured and the incoming nonce/HMAC is missing or invalid; rejected peers must not be marked authenticated and must not receive version vectors or sync changes.

AC4. `SyncClient` includes nonce/HMAC in outgoing hello messages when configured with `auth_secret`; clients without a secret retain existing behavior.

AC5. `ark-core-rpc` `start_sync` accepts optional `auth_secret` and passes it to all inbound server sessions and outbound clients, including known peers, seed peers, and beacon-discovered peers.

AC6. `@arksync/node` exposes `authSecret?: string` and includes it as `auth_secret` in `start_sync`.

AC7. Verification artifacts prove:

- legacy hello compatibility;
- HMAC helper correctness and invalid secret rejection;
- a matching-secret sync succeeds;
- a wrong-secret client is rejected;
- Rust and TypeScript checks pass against the current codebase.
