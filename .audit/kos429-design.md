# KOS-429 device registry design

## Caller contract

Engine exposes a device snapshot, rename, and disable. Manager renders the canonical name and admission state, and calls Engine operations. Temporary disconnect remains a local transport operation.

```rust
let snapshot = engine.devices().await?;
engine.rename_device(device_id, DisplayName::parse("Office Mac")?).await?;
engine.disable_device(target_device_id).await?;
engine.disconnect_peer(target_device_id).await?; // local session only
```

A successful rename or disable means the signed event and projection are durable. A successful pairing means the identity binding is durable and registry admission completed. Neither path returns success when persistence or signature verification fails.

## Decision

Use a canonical signed event log as authority and a `DeviceRecord` map as its derived projection. Keep discovery `PeerRecord` and `PeerList` separate. Exchange registry control messages after cryptographic Hello and capability negotiation, but before data readiness. This follows candidate B's explicit `ReadyPeer` barrier and deterministic conflict handling. The candidates agreed on per-device signed events and a shared pre-data barrier. The cross-judge preferred B 22–21 for its clearer transport gates and event ordering.

Use the existing persisted Iroh identity as the device signing identity. Source inspection confirms `iroh::SecretKey` is persisted, `EndpointId` is its public key, Iroh exposes Ed25519 sign/verify, and pairing persists `device_id -> EndpointId`. This satisfies KOS-429's identity condition without introducing another secret to back up. A transport re-pair is an explicit key-epoch change authorized by an already admitted device; it does not change stable `device_id`, object ownership, or historical event signatures. Do not rotate the shared space secret.

Do not impose a six-device lifetime cap or a time-based registry-history TTL. The user commonly has 5–6 devices, but disabled devices and future replacements must not silently consume a permanent product limit. Bound individual messages and exchange pages. Retain signed registry history until a later design can compact it without losing proof; registry events are low-volume control data. KOS-429 does not implement checkpoint compaction.

## Data model and ownership

`DeviceId`, `EndpointId`, `RegistryEventId`, `ActorSequence`, `AdmissionEpoch`, and `DisplayName` are validated domain types with private fields. `RegistryOperation` is `Enroll`, `Rename`, `Disable`, or `RebindKey`. `RegistryEvent` contains event ID, actor device ID, actor sequence, prior event hash, target device ID, operation, admission epoch, and canonical signature. It never contains SQLite, JSON, or transport objects.

Events form one hash-linked sequence per author. Replicas merge independent authors' events by ID, verify each signature against the enrolled key epoch, deduplicate retries, and retain gaps as pending. A projection is rebuilt only from verified contiguous history. Disable adds an irreversible tombstone; no event by the target can clear it. Re-enable is outside this issue. An admitted device may rename or disable another device, but cannot disable itself and strand the identity that issued the command.

Rename is a deterministic last-writer-wins register using HLC plus stable `(actor_device_id, event_id)` tie-breakers. HLC orders rename candidates only; it never proves identity or authorizes disable. The signed log retains all rename operations. `display_name` falls back to the first enrolled OS name until an explicit rename; later host-name updates cannot overwrite an alias.

`PeerRecord` remains transport discovery metadata: addresses, last seen, platform, application version, and first-contact host name. Existing `sync.removed_peers` remains local-only and is never promoted to global disable. All object histories, CRDT state, HLC values, and source sequences keep their existing device owner.

The domain registry module owns canonical encoding, signature verification, event ordering, projection, and admission rules. SQLite registry commands own atomic persistence through `DbHandle`. Engine service parses JSON and calls those commands. Sync orchestration owns registry exchange and constructs opaque `ReadyPeer` only after successful admission. WebSocket, Iroh, and Manager do not implement policy independently.

```rust
pub(crate) struct ReadyPeer { /* private device id and session generation */ }

pub(crate) trait DeviceRegistry {
    async fn snapshot(&self) -> Result<DeviceSnapshot, RegistryError>;
    async fn append_local(&self, command: RegistryCommand)
        -> Result<CommittedRegistryEvent, RegistryError>;
    async fn apply_remote(&self, batch: RegistryBatch)
        -> Result<RegistryDelta, RegistryError>;
    async fn admit_session(&self, proof: RegistryProof, batch: RegistryBatch)
        -> Result<ReadyPeer, AdmissionError>;
    async fn is_admitted(&self, peer: &ReadyPeer) -> Result<bool, RegistryError>;
}
```

Each write command checks the current actor admission, allocates its next sequence, signs canonical event bytes, inserts the event, updates the projection, and records replication progress in one SQLite transaction. Remote apply rechecks author key epoch, authorization, replay position, and tombstone state in that same transaction. Every inbound object apply also rechecks the peer's current admission epoch in the same transaction as the object write, so a stale `ReadyPeer` cannot race past a committed disable. Outbound page reads recheck admission before returning data; disable cancels queued sends and closes sessions. Network I/O never holds a SQLite transaction. Notify peers only after commit. Pairing first validates the existing Hello/HMAC and explicit user consent, then commits the endpoint binding and enrollment/key-epoch event before marking the session ready. A failed commit or transport bind leaves no ready session. Existing `admit_transport_hello` must propagate bind and persistence errors instead of logging and continuing.

## Session protocol

The session state is `TransportConnected -> HelloVerified -> PairingDecision -> RegistryExchange -> DataReady`. `authenticated` or an Iroh endpoint binding alone is not data readiness. On reconnect, readiness starts over.

The device that receives a new pairing request parks the first Hello and asks for user consent. The device that entered the pairing code also parks the responder's first transport Hello. Neither side sends VersionVector while consent is pending or while durable enrollment is in progress. After the user accepts, each side commits its local endpoint binding and enrollment event before it starts sync. The responder then sends a `PairingAccepted` control frame to the exact transport endpoint. The initiator accepts that frame only for its active pairing attempt and the endpoint it dialed. `PairingAccepted` alone does not open data exchange; both peers still reach `DataReady` only after registry history is verified and both sides confirm admission.

The registry capability is mandatory for data admission. Existing protocol negotiation authenticates the selected capability list. A peer without the capability gets a bounded `upgrade_required` control refusal and closes before data. Unknown-field tolerance is not compatibility. Current clients all move together to this cutover; no old-peer fallback may exchange user data.

Both peers exchange bounded signed event pages and their per-actor cursors until all available contiguous registry history is applied and each side confirms the peer remains active. An unsupported message, bad proof, oversized page, sequence gap, persistence error, or disabled peer keeps the session out of `DataReady`. Gaps request replay and survive reconnect. Duplicate events are idempotent.

The custom WebSocket room relay currently has no authenticated peer-addressed delivery or per-recipient writer gate. It broadcasts room frames, so it cannot carry registry admission safely while a peer is pending or disabled. `RelayTransport` must therefore decline the registry capability and reject legacy peers before user data. Re-enabling this path requires a relay-server contract that authenticates sender identity, addresses recipient sessions, and proves non-ready peers never receive data; the server implementation is not present in the accessible `makekosmos` repositories.

Every data path requires `ReadyPeer`: VersionVector, SyncChanges, LiveChange, PeerList handling, signed integration frames, ACKs, snapshot pages, committed feeds, and queued outbound work. The Iroh writer permits only Hello, pairing responses, PairingAccepted, and registry control before readiness. Direct WebSocket client and server must use the same barrier. `PeerList` remains discovery-only.

After a disable event commits, mark all matching sessions not ready, close them, cancel queued sends, and reject every inbound/outbound user-data handler. A disabled peer may only complete the restricted control exchange needed to receive the signed history containing its tombstone, then the session closes. It cannot author registry updates or send data. Offline peers learn the event when they reconnect to an admitted peer; the contract is eventual, not instantaneous revocation of a disconnected device. Data already copied to a device is not remotely erased.

## RelaySync live registry continuation

The original admission contract also applies after the first exchange: rename and disable events must reach currently connected devices, and a disable must close every affected session before another data frame is processed. Two source-grounded sketches were compared for the missing Iroh path (`/tmp/ark-kos429-fanout-a.md` and `/tmp/ark-kos429-fanout-b.md`). Continue with the per-peer admission reducer (candidate B): the peer entry owns its endpoint, registry session id, generation, and admission phase for the full connection lifetime. Do not create a second process-wide readiness map alongside `RelayPeerState`.

Local committed events and remote `RegistryPageApply::applied_events` enter one service-owned registry-delta callback. Remote input is never relayed directly: only newly verified and committed events may fan out, so duplicate delivery is idempotent. The callback revokes every matching peer session and clears Iroh's data-ready transport gate before scheduling network writes. Each send carries the current peer-session token and is checked again at the writer boundary; a stale callback cannot target a replacement session. A disabled peer may receive only the signed registry control event on its current endpoint before the session closes. Reconnect uses the durable event history and digest exchange before user data becomes ready.

This continuation does not make the custom room WebSocket relay registry-capable. Its current broadcast-only transport cannot authenticate and address a specific pending or disabled recipient. It remains fail-closed before user data until the relay protocol/server can provide authenticated peer identity, addressed delivery, and a recipient-side data gate. KOS-429 stays open until this boundary and the direct/Iroh paths are proven by transport-level tests.

Acceptance evidence for this unit must include a multi-peer Iroh path, duplicate and invalid update handling, revocation of a third connected peer, stale-session rejection, and an assertion that queued user data is dropped after the disable is committed. A reconnect test must observe registry history completion before the first VersionVector. Tests should use the production SQLite registry commands and exercise the transport writer gate rather than only testing a helper map.

- Candidate A used a standalone session map. Rejected because it would create a parallel admission authority next to the peer state.
- Candidate B keeps one owner for connection and admission lifecycle and provides a callback seam for cross-transport fanout. Selected for implementation.

## Existing-code grounding

- `service/sync/start.rs` starts the direct `SyncServer` and, depending on configuration, a WebSocket relay or Iroh-backed `RelaySync`; all use cases need the same readiness rule.
- `sync_server_messages/hello.rs` and `sync_client_start/connect_and_receive.rs` set application authentication and exchange VersionVector before registry admission exists.
- `relay_impl_1.rs` marks a relay peer authenticated and calls `send_local_version_vector`; `relay_impl_2.rs` gates Iroh transport by endpoint but handles registry-independent data.
- `iroh_connections.rs` treats transport binding as permission for ordinary writes.
- `relay_pairing.rs::admit_transport_hello` ignores bind errors and can continue after peer-key persistence fails.
- `sync.peer_transport_keys` durably binds device IDs to Iroh EndpointIds, and `iroh_tail.rs` loads the corresponding persistent local key.
- Existing `sync.removed_peers` and `PeerList` do not represent space-wide device admission.

## Bootstrap and legacy migration

There is no global owner key to elect. Each installation has its own persisted Iroh signing key, and completed pairings already pin `device_id -> EndpointId` in `sync.peer_transport_keys`. Seed the event log locally from those durable pins: the local device signs one enrollment event for each known peer, in stable device-ID order, using its own per-author sequence. Re-running the seed is idempotent. When devices exchange the resulting author logs, independent sequences do not collide. If two trusted authors claim the same device ID with different signing keys, retain the evidence and quarantine that identity instead of choosing one silently.

Only the persisted transport-key map is a legacy trust anchor. `sync.peers` and `PeerList` are display/discovery metadata; matching `space_id` and optional HMAC are not proof of authorship. A new pairing can create a signed enrollment only after explicit consent and the exact endpoint binding has been persisted. The enrolled key must equal the accepted endpoint's Ed25519 key. The OS-reported device name remains a fallback; a signed user rename is the canonical alias and is not overwritten by later Hello metadata.

During rollout, capability negotiation separates old peers from registry-aware peers. Keep old transport pins available to verify the first registry exchange, but do not let a peer become data-ready until it has supplied and passed the required signed history. A legacy peer without the registry capability is rejected before user-data exchange.

See `.audit/kos429-grounding.md` and `/tmp/kos429-how.md`. The baseline `accept_pairing_connects_persists_and_survives_restart` passes, but its trace shows VersionVector is sent before pairing consent completes; that is the behavior this issue must change.

## Verification contract

Use production SQLite and actual WebSocket/Iroh session code. Prove rename propagation and restart, host-name fallback, deterministic concurrent rename, atomic pairing persistence before `DataReady`, and disabled-session teardown. Before user consent and before data readiness, assert no VersionVector, PeerList, entity, ACK, or feed is sent or applied. Reject old peers, forged IDs, bad signatures, gaps, replay, stale active records, and data queued before disable. Prove offline disable delivery on reconnect and local disconnect without registry mutation. Keep golden Hello and PeerList fixtures unchanged except the negotiated capability fields. Verify Engine operations and Manager use the service boundary without direct database access.

## Arena record

Two independent Luna candidates converged on per-author signed event history, explicit durable enrollment, and one pre-data barrier. Candidate B is the base for `ReadyPeer`, retry semantics, and full transport gates. The cross-judge preferred B 22–21. Candidate A's explicit separation of transport identity from device policy informed the selected key-epoch transition, but its extra signing key is rejected because the current persisted Iroh identity already provides the required signature API and durable pairing anchor. The independent reviewer also flagged the arbitrary six-device cap and unbounded checkpoint claims; this design removes the cap and defers compaction rather than inventing a retention rule.

## Implementation progress

The next direct-server step replaces the mutable `authenticated` flag with a private `ReadyPeer` containing the accepted connection ID, stable device ID, and signed registry session ID. Only successful registry admission after the outbound baseline and durable admission lookup can construct it. The connection ID binds readiness to the exact socket; registry session ID binds it to the signed challenge. Central reader and writer boundaries revalidate the token against the live peer-map entry and current registry projection without holding the peer lock across storage I/O. Existing write transactions remain the final authorization check. Reconnect, disable, replacement, or close drops the token. Keep this change scoped to the direct server and reuse the same type for other transports only when their session ownership can provide an equivalent binding.

Candidate A makes connection identity a dedicated wrapper and passes the capability into more handlers. Candidate B centralizes live-token validation around the existing peer-map key and retains the current handler layout. Use B's smaller boundary and graft A's explicit three-part binding. Do not add a separate session registry or mirror database admission in another mutable boolean.

The transport now parks the dialer's first responder Hello until it receives `PairingAccepted` from the exact Iroh endpoint and matching device ID. The responder persists the endpoint binding before sending that control frame, and it defers the initial VersionVector until the frame is sent. Pairing integration tests confirm that pending endpoints stay offline, exchange no objects, and reconnect after acceptance.

`domain::device_registry` now defines validated device IDs, display names, signing public keys, registry operations, signed event drafts, and a verified-event wrapper. Event signing uses a versioned, domain-separated canonical payload. Verification binds the actor and key epoch, rejects weak Ed25519 keys, and checks the per-author sequence link and HLC. Golden signature and event-hash tests pin the encoding.

Pairing now writes a signed enrollment event, its projection, and the legacy transport-key pin in one SQLite transaction. Enrollment is idempotent, survives reopen, and rolls back as a unit when a projection write fails. Both pairing paths hold the initial VersionVector until that command commits; an injected enrollment failure proves no peer becomes online and no registry event remains. A later Hello from a peer with a matching stored pairing key resends `PairingAccepted`, so a failed receipt can be retried without creating another enrollment event. An injected transport failure test covers that retry.

Iroh endpoint state now distinguishes `Observed`, `Bound`, and `DataReady`. Binding enables pairing control only; the connection writer drops outbound data and the reader drops inbound data until the pairing coordinator promotes the endpoint after enrollment and `PairingAccepted`. A loopback transport test covers both directions, and a repeated Hello for the same endpoint preserves the state. This closes the enrollment race, but data admission still occurs before signed registry exchange exists; moving that transition behind registry proof and replacing the device-ID gate with shared `ReadyPeer` remain required KOS-429 work.

Implementation progress: signed registry exchange now precedes readiness on direct WebSocket and Iroh paths, and peers without registry capability are rejected before user data. Direct SyncServer replaces its mutable `authenticated` flag with `ReadyPeer { connection_id, device_id, registry_session_id }`. Both reader dispatch and writer socket sends recheck that the token still matches the connection and that durable registry admission remains active. A failed admission lookup revokes data readiness, clears pending sync state, and closes the socket so normal disconnect cleanup can run. The direct server full suite passes (651 passed, 1 skipped); Clippy, formatting, source-name, and diff checks pass. This is not yet the shared cross-transport boundary in the target design. Relay peer-addressed delivery, remaining Iroh/SyncClient gating, disabled-device control-only catch-up, live rename/disable fanout, and final repository validation remain open. KOS-429 remains open; no later issue may treat `PairingAccepted` as the registry barrier.

Implementation must update this document if a source constraint requires a different contract before downstream code relies on it.
