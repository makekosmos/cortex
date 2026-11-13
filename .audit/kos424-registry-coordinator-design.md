# KOS-424 relay registry coordinator

## Problem

`ark-sync` already owns the sync wire protocol, direct registry admission, transport contract, and bounded registry-history page builder. Relay registry exchange, readiness, and revocation still live in `ark-core` and share state with peer composition. Moving the whole `RelaySync` actor would also move pairing UX, ordinary sync, merge delivery, integration messages, and shutdown behavior. The extraction must keep the existing relay protocol and SQLite schema unchanged while moving one cohesive policy owner.

## Usage

Core constructs the coordinator with the existing application storage port and selected `SyncTransport`:

```rust
let registry = RelayRegistryCoordinator::new(
    local_device_id,
    storage.clone(),
    transport.clone(),
);
registry.begin_peer(peer_id, endpoint, policy_negotiated).await;
let outcome = registry
    .handle_registry_frame(peer_id, endpoint, frame, &connected_peers)
    .await?;
publish_core_outcome(outcome);
```

The relay message handler delegates `RegistrySyncStart`, `RegistryEventsPage`, `RegistrySyncReady`, and `RegistryEventUpdate` to the coordinator. Data-frame admission asks the same owner whether the peer is ready on the current endpoint. A local committed registry event calls `publish_delta` on that owner.

## Shape

Add `ark_sync::relay_registry::RelayRegistryCoordinator`. It owns the relay-specific per-peer state map and the registry exchange, readiness, retry, fanout, and revocation rules. It does not reuse `DirectRegistryAdmission`, which implements the distinct direct WebSocket nonce-proof protocol.

```rust
pub struct RelayRegistryCoordinator<S: RegistryAdmissionStorage + ?Sized> {
    local_device_id: String,
    storage: Arc<S>,
    transport: Arc<dyn SyncTransport>,
    peers: Mutex<HashMap<String, RelayRegistryPeer>>,
}

pub struct RegistryOutcome {
    pub applied_events: Vec<RegistryEvent>,
    pub disabled_device_ids: Vec<String>,
    pub revoked_peer_ids: Vec<String>,
    pub sender_is_admitted: bool,
    pub local_device_is_admitted: bool,
    pub became_ready: bool,
    pub policy_negotiated: bool,
    /// A durable apply may succeed before a later state or network operation fails.
    /// Keep its effects available so Core can publish callbacks before the error.
    pub operation_error: Option<String>,
}

pub enum RegistryFrame {
    SyncStart { session_id: String },
    EventsPage {
        session_id: String,
        page_index: u32,
        page_count: u32,
        events: Vec<RegistryEvent>,
    },
    SyncReady { session_id: String, digest: String },
    EventUpdate { session_id: String, event: Box<RegistryEvent> },
}

impl<S: RegistryAdmissionStorage + ?Sized> RelayRegistryCoordinator<S> {
    pub fn new(
        local_device_id: String,
        storage: Arc<S>,
        transport: Arc<dyn SyncTransport>,
    ) -> Self;

    pub async fn begin_peer(
        &self,
        peer: &str,
        endpoint: &str,
        policy_negotiated: bool,
    );
    pub async fn start_exchange(&self, peer: &str, endpoint: &str) -> Result<(), String>;
    pub async fn handle_registry_frame(
        &self,
        peer: &str,
        endpoint: &str,
        frame: RegistryFrame,
        connected_peers: &[(String, String)],
    ) -> Result<RegistryOutcome, String>;
    pub async fn publish_delta(
        &self,
        events: &[RegistryEvent],
        disabled: &[String],
        origin: Option<&str>,
        connected_peers: &[(String, String)],
    ) -> RegistryOutcome;
    pub async fn is_ready(&self, peer: &str, endpoint: &str) -> bool;
    pub async fn disconnect_peer(&self, peer: &str);
}
```

`RelayRegistryPeer` contains the endpoint, negotiated policy capability, retry count, incoming page bounds and cursor, both history digests, and the `Waiting`, `Exchanging`, `Ready`, or `Revoked` state. Its fields stay private. Core parses `LanSyncMessage` into `RegistryFrame` at the message boundary; the frame carries no socket or SQLite types.

The coordinator is generic over the narrow application `RegistryAdmissionStorage` port for durable event application and history reads. It uses only the `ark-sync::SyncTransport` contract for addressed control sends, data readiness, unbinding, and disconnects. Core keeps transport construction, pairing consent and identity enrollment, peer metadata, event callbacks, and Engine lifecycle; it supplies the authenticated peer and endpoint inventory only for local-device revocation. Core continues to load the negotiated sync profile and version vector after readiness.

The coordinator releases its state lock before every async storage or network call. Each operation captures the peer, endpoint, session ID, and page cursor, then revalidates them before committing state. Revocation records `Revoked` and unbinds the transport before awaiting control sends. Readiness is recorded only after `mark_peer_data_ready` succeeds while the session token is still current. These orderings preserve existing data-gate and stale-completion behavior.

Core still owns the authenticated peer inventory and callbacks. For a local-device disable, pass current peer bindings into the coordinator so it can revoke every connected endpoint without taking ownership of Core's separate pairing facts. Applied registry events must be returned even when later fanout/revoke sends fail, so Core publishes the durable event callback before surfacing the transport error. The outcome also carries storage admission flags, revoked peer IDs for Core metadata cleanup, and whether selective policy was negotiated. After successful readiness, Core marks outgoing pairing accepted, sends the negotiated sync profile when supported, sends the version vector, then notifies pairing state.

## Synthesis decision

Candidate A is the base because it makes registry exchange and readiness one bounded owner and can reuse existing application and transport contracts. The cross-judge confirms the dependency direction and scope. Candidate B's serialized `SyncSession` actor remains a possible later design, but moving pairing UI, all sync lanes, timers, and shutdown at once is too broad for one verified KOS-424 unit. The design does not reuse `DirectRegistryAdmission`; relay Hello authentication, endpoint binding, retry, and readiness are a separate protocol.

## Tradeoffs accepted

- The coordinator owns registry state separate from core's authenticated peer metadata. These maps represent different facts: registry data readiness and transport-authenticated peer details. Core supplies its current bound peer inventory only for local-device revocation.
- Core invokes callbacks from `RegistryOutcome`, so event publication remains at the Engine boundary.
- The coordinator uses the narrow `RegistryAdmissionStorage` port; Core retains the aggregate storage adapter for its other responsibilities.

## Alternatives considered

- Move only `RegistryExchangeState` and leave transitions in core. This preserves the split state authority and exposes exchange choreography to callers.
- Move all of `RelaySync` into one session actor. This hides more behavior but expands the interface and cutover to pairing, ordinary sync, task merge, integrations, and lifecycle.

## Open questions and risks

- Concurrent peer disconnect, page application, registry revocation, and digest completion revalidate endpoint/session/page tokens around I/O without holding the state lock across storage or network calls.
- Production SQLite and Iroh tests cover durable catch-up, live registry updates, selective-policy negotiation, and revocation after the move.
- Coordinator tests cover retaining applied events across later send failures and revoking connected peers missing from registry state; Core remains responsible for publishing those outcomes and preserving sync-profile/vector callbacks.

## Next implementation step

Finish the relay coordinator cutover by running the broader sync, mobile/desktop build, dependency-graph, and full issue acceptance gates; then continue extracting the remaining sync orchestration.
