# KOS-429 transport grounding

## Overview

`ArkService` owns device-registry writes and publishes a signed event to the running sync transports. SQLite owns the authoritative signed registry and materialized device projection. A transport may carry user data only after the connection's peer has completed registry admission and the database still says that peer is active.

## How it works

Local rename/disable requests go through `service/runtime/system.rs` and submit `AppendLocalRegistryEventCommand`. The command signs and applies the event in one DB transaction. `publish_registry_event` currently broadcasts it to the direct `SyncServer` and each `SyncClient`.

Direct WebSocket sessions exchange the signed registry before `ReadyPeer` is installed. The token binds the exact socket, stable device ID, and registry session; both inbound dispatch and outbound socket writes recheck it and current durable admission. A newly disabled peer can reconnect only for the control exchange needed to learn its tombstone, then must be rejected before data readiness.

Iroh uses `RelaySync` over `SyncTransport`. `MessageReceivedFromTransport` includes both the claimed device ID and authenticated Iroh endpoint. Pairing binds those values; registry exchange pages are applied to SQLite and readiness is granted only after registry digests match. The transport gates queued broadcast data on `mark_peer_data_ready`, and `unbind_peer` removes that permission. Existing reconnect exchange sends the full signed history.

After local commit, Engine also publishes the event to `SyncRuntime.relay`. Iroh sends live updates only to peers whose registry admission is ready for the active endpoint and session. Receivers verify and persist the event, forward only newly applied events, and revoke disabled peers. Reconnect performs signed history catch-up before the peer can become data-ready. Direct WebSocket clients keep the registry session after readiness and apply live updates; the server broadcasts verified events and revokes disabled sessions.

## Transport boundaries

`RelaySync` owns Iroh peer/session orchestration, `SyncTransport` owns endpoint-bound control and data queues, and SQLite owns registry validity. Direct `SyncServer` binds `ReadyPeer` to the socket connection, device ID, and registry session. Iroh binds readiness to the authenticated endpoint and registry session in `RelayRegistryAdmission`. Both paths check current durable admission before accepting or sending user data.

The custom WebSocket room relay does not expose an authenticated transport key or peer-addressed send. It does not advertise registry admission; RelaySync rejects the missing capability before data. Linear KOS-429 explicitly allows this path to remain unavailable until the relay can prove endpoint identity and isolate registry control delivery. The server implementation is not in this checkout, so client code cannot claim the relay enforces recipient isolation.

## Change boundary

For direct WebSocket and Iroh, bind each ready session to its authenticated connection or endpoint and registry session. Only the active session can authorize a `RegistryEventUpdate`. Relay only newly applied and verified events. Applying a disable revokes matching sessions and removes their transport data permission. Keep signed-history exchange as the recovery path for offline peers.

Production SQLite and transport tests cover three-device Iroh rename/disable fanout, third-peer revocation while other peers remain live, duplicate event handling, direct WebSocket client/server admission, and offline reconnect catch-up before user data. The focused tests and full `ark-core --lib` suite pass at 540/540. The room relay remains fail-closed by design.
