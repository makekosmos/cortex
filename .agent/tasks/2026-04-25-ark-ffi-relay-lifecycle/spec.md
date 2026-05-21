# Task: Wire relay lifecycle through ARK UniFFI facade

## Context

The desktop `ark-core-rpc` sidecar now starts a relay sync bridge when `relay_url` is provided. The UniFFI `ArkCore::start_sync` path still carries `relay_url` / `relay_api_key` fields but does not start the relay bridge. This leaves mobile/native callers behind the desktop runtime contract.

This task wires the same `RelaySync` lifecycle into the UniFFI facade without changing renderer or Electron integration.

## Acceptance Criteria

AC1. `FfiSyncConfig` relay fields (`relay_url`, `relay_api_key`, `auth_secret`) are used by `ArkCore::start_sync` to start `RelaySync` when `relay_url` is present.

AC2. UniFFI relay callbacks use the existing `ArkEventListener` methods: inbound relay entity changes call `on_entity_changed`, authenticated relay peers call `on_peer_connected`, and relay disconnects call `on_peer_disconnected`.

AC3. `ArkCore::broadcast_change_json` sends locally persisted live changes through relay when relay is running.

AC4. `ArkCore::stop_sync` stops the relay transport.

AC5. `ArkCore::get_connected_peers` includes authenticated relay peers without duplicating LAN/outbound entries.

AC6. Verification artifacts prove:

- Rust compiles and full tests pass;
- a targeted unit/integration verifier covers the FFI relay lifecycle wiring shape;
- generated bindings remain buildable by `cargo check`;
- docs accurately state desktop and UniFFI relay support.
