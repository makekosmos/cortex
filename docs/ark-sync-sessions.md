# ARK sync session ownership

`ark-sync` owns the direct sync protocol and each peer session. `ark-net` owns
WebSocket setup and the transport worker. `ark-core` composes these crates,
provides storage, and connects session callbacks to the Engine and service.
The migration preserves the current wire messages and storage ordering.

## Direct client

`ark_sync::DirectClient` owns one outgoing peer session. It reads an
`ark_application::sync_storage::SyncRuntimeStorage` capability and a
`DirectClientConnector`. The `ark-net` WebSocket connector implements that
port and returns a `SessionIo` plus the selected peer address.

`DirectClient` owns address retry, Hello negotiation, registry admission,
reconnect state, sync-policy negotiation, snapshot and feed progress,
task-merge delivery, live changes, ACK handling, and session task shutdown.
Each connected peer has one authenticated session record. The session becomes
visible to senders only after registry admission succeeds.

Core creates the client with its local identity and platform metadata. It
registers callbacks for entity changes, peer lifecycle, peer lists, and
registry events. Those callbacks keep Engine event emission, server peer-map
updates, and registry fanout in Core. The `DirectClient` callback task registry
lets service shutdown wait for work that those callbacks start.

```rust,ignore
let client = ark_sync::DirectClient::new(
	storage,
	ark_sync::DirectClientConfig::new(
		peer,
		device_id,
		device_name,
		space_id,
		own_addresses,
		auth_secret,
	),
	Arc::new(ark_net::websocket::DirectClientConnector),
);
client.start();
client.stop_and_join().await;
```

The `ark-core::direct_client::new_direct_client` factory supplies the local
platform metadata and the ArkNet connector for the service and integration
tests.

## Direct server

`ark-sync` owns the connected server session phase, admission, readiness,
inbound message classification, snapshot and feed coordination, task-merge
delivery, live-change ordering, ACK checks, and per-session progress. Core
provides the application host for database operations, registry effects,
peer-map updates, callbacks, and service lifecycle.

`ark-net` owns TCP listener binding, WebSocket accept and dial operations,
socket framing, and the transport supervisor. `SessionIo` gives the sync
drivers bounded control and bulk queues and waits for the transport worker
during teardown. The sync layer does not depend on concrete socket types.

## Ordering guarantees

The session drivers preserve the current protocol ordering. Registry
admission finishes before either peer can send replicated data. A snapshot
page is applied and its progress is persisted before the receiver sends
`SyncAck`. Outbound snapshots keep a fixed watermark and verify policy
identity around page reads.

Live-feed progress advances only after the current ACK matches its change ID
and accepted policy identity. Durable receipt ACKs advance receipt progress
without claiming that the application applied the entity. A pending live
change receives no normal ACK. Snapshot callbacks keep their existing
Applied and Stale behavior, while live callbacks run only for Applied changes.

Selective sync controls delivery per peer. Narrowing a policy does not delete
local data. Widening a policy starts the existing backfill path. These rules
also leave room for a later large-file transport without splitting the
person's database.

## Dependency boundary

`ark-sync` depends on `ark-application` and `ark-domain`. It has no dependency
on `ark-core`, `ark-store`, or `ark-net`. `ark-net` implements the connector
port. `ark-store` implements the application storage ports and does not know
about sync sessions or network transports.

The migration keeps one session authority in `ark-sync`. Core does not
interpret direct-client or direct-server protocol branches. It performs
application work requested by the session and updates service-level state.
