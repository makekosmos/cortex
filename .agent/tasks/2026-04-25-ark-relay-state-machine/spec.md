# Task: Wire ARK relay sync into the runtime state machine

## Context

`ark-core-rpc` currently rejects `relay_url` / `relay_api_key` even though a relay server and a low-level `RelayTransport` exist. This is intentional fail-fast behavior because relay transport is not yet connected to the actual sync state machine.

This task must replace that false-negative state with real relay sync support. The goal is not to build a final encrypted relay protocol; the goal is to make relay mode functionally honest and tested:

- `start_sync` with relay options starts LAN sync plus relay transport.
- Relay peers exchange `hello`, `version_vector`, `sync_changes`, and `live_change` messages.
- Relay traffic observes the same HMAC peer-auth behavior as LAN when `auth_secret` is configured.
- `stop_sync` shuts relay down.

## Acceptance Criteria

AC1. `ark-core-rpc start_sync` no longer rejects `relay_url` / `relay_api_key`; it starts relay support when `relay_url` is present.

AC2. Relay transport sends authenticated `hello` messages when `auth_secret` is configured and surfaces the sender device id for incoming relay frames.

AC3. Relay runtime processes remote relay `hello` messages, verifies HMAC when configured, tracks connected relay peers, and sends the local version vector to authenticated peers.

AC4. Relay runtime applies inbound `sync_changes` and `live_change` messages through the same `StorageBackend` path as LAN sync, updates version vectors, emits `entity_changed`, and does not silently accept apply errors.

AC5. Relay runtime responds to inbound version vectors by sending missing local entities as `sync_changes` batches.

AC6. Local `broadcast_change` sends live changes through relay as well as LAN/outbound clients.

AC7. `stop_sync` stops the relay transport.

AC8. `@arksync/node` no longer rejects relay options before calling the sidecar.

AC9. Verification artifacts prove:
- relay options reach `ark-core-rpc`;
- two runtimes can sync an existing entity through relay only;
- relay live change propagation works;
- wrong relay HMAC secret is rejected;
- Rust and TypeScript checks pass against the current codebase.
