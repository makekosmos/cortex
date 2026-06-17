# Evidence

## Changed files

- core/ark/crates/ark-core/rust/src/main.rs
- core/ark/crates/ark-core/rust/src/sync_server.rs
- core/ark/crates/ark-core/rust/src/sync_client.rs

## Fixes

- `connect_with_pairing_code` now accepts both `pairing_code` and `code`, rebuilds restart params from the live sync runtime, forces iroh on, injects the supplied ticket, and restarts sync instead of returning the old unsupported placeholder.
- `disconnect_peer` now tears down active inbound sessions via `SyncServer::disconnect_peer`, closes matching outbound `SyncClient` sessions, removes them from the runtime map, and still emits `peer_disconnected` + `peer_list_updated`.
- Added runtime start-parameter capture so pairing restart uses the current production sync config instead of guessing from the current transport snapshot.
- Added sync-server and RPC tests covering pairing aliasing, restart-param rewriting, outbound-client teardown, and inbound-session disconnect/blocklist behavior.

## Checks

- `rtk cargo check -p ark-core --bin ark-core-rpc --features iroh-spike`
- `rtk cargo test -p ark-core --bin ark-core-rpc pairing_restart_params_force_iroh_and_replace_ticket -- --nocapture`
- `rtk cargo test -p ark-core --bin ark-core-rpc request_deserialization_accepts_code_alias_for_pairing -- --nocapture`
- `rtk cargo test -p ark-core --bin ark-core-rpc disconnect_peer_stops_matching_outbound_client_and_emits_events -- --nocapture`
- `rtk cargo test -p ark-core sync_server -- --nocapture`
- `rtk cargo check -p kepler-backend`
- `rtk bun run --cwd platform/desktop typecheck`
- `rtk bun run docs:check`
- `rtk bun run ark:guard:writes`
