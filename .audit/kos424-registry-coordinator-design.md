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
registry.begin_peer(peer_id, endpoint).await?;
let outcome = registry.handle_registry_frame(peer_id, endpoint, frame).await?;
publish_core_events(outcome.applied_events);
```

The relay message handler delegates `RegistrySyncStart`, `RegistryEventsPage`, `RegistrySyncReady`, and `RegistryEventUpdate` to the coordinator. Data-frame admission asks the same owner whether the peer is ready on the current endpoint. A local committed registry event calls `publish_delta` on that owner.

## Shape

Add `ark_sync::relay_registry::RelayRegistryCoordinator`. It owns the relay-specific per-peer state map and the registry exchange, readiness, retry, fanout, and revocation rules. It does not reuse `DirectRegistryAdmission`, which implements the distinct direct WebSocket nonce-proof protocol.

```rust
pub struct RelayRegistryCoordinator {
    local_device_id: String,
    storage: Arc<dyn SyncRuntimeStorage>,
    transport: Arc<dyn SyncTransport>,
    peers: Mutex<HashMap<String, RelayRegistryPeer>>,
}

pub struct RegistryOutcome {
    pub applied_events: Vec<RegistryEvent>,
    pub pairing_changed: bool,
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
    EventUpdate { session_id: String, event: RegistryEvent },
}

impl RelayRegistryCoordinator {
    pub fn new(
        local_device_id: String,
        storage: Arc<dyn SyncRuntimeStorage>,
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
    ) -> Result<RegistryOutcome, String>;
    pub async fn publish_delta(
        &self,
        events: &[RegistryEvent],
        disabled: &[String],
        origin: Option<&str>,
    ) -> Result<(), String>;
    pub async fn is_ready(&self, peer: &str, endpoint: &str) -> bool;
    pub async fn disconnect_peer(&self, peer: &str);
}
```

`RelayRegistryPeer` contains the endpoint, negotiated policy capability, retry count, incoming page bounds and cursor, both history digests, and the `Waiting`, `Exchanging`, `Ready`, or `Revoked` state. Its fields stay private. Core parses `LanSyncMessage` into `RegistryFrame` at the message boundary; the frame carries no socket or SQLite types.

The coordinator uses `SyncRuntimeStorage` for durable event application, history reads, peer policy, and the current vector. It uses only the `ark-sync::SyncTransport` contract for addressed control sends, data readiness, unbinding, and disconnects. Core keeps transport construction, pairing consent and identity enrollment, peer metadata, event callbacks, and Engine lifecycle.

The coordinator releases its state lock before every async storage or network call. Each operation captures the peer, endpoint, session ID, and page cursor, then revalidates them before committing state. Revocation records `Revoked` and unbinds the transport before awaiting control sends. Readiness is recorded only after `mark_peer_data_ready` succeeds while the session token is still current. These orderings preserve existing data-gate and stale-completion behavior.

## Synthesis decision

Candidate A is the base because it makes registry exchange and readiness one bounded owner and can reuse existing application and transport contracts. The cross-judge confirms the dependency direction and scope. Candidate B's serialized `SyncSession` actor remains a possible later design, but moving pairing UI, all sync lanes, timers, and shutdown at once is too broad for one verified KOS-424 unit. The design does not reuse `DirectRegistryAdmission`; relay Hello authentication, endpoint binding, retry, and readiness are a separate protocol.

## Tradeoffs accepted

- The coordinator owns a peer-state map separate from core's authenticated peer metadata. These maps represent different facts: registry data readiness and transport-authenticated peer details.
- Core invokes callbacks from `RegistryOutcome`, so event publication remains at the Engine boundary.
- The extraction uses the existing `SyncRuntimeStorage` aggregate; splitting its unrelated capabilities is outside this unit.

## Alternatives considered

- Move only `RegistryExchangeState` and leave transitions in core. This preserves the split state authority and exposes exchange choreography to callers.
- Move all of `RelaySync` into one session actor. This hides more behavior but expands the interface and cutover to pairing, ordinary sync, task merge, integrations, and lifecycle.

## Open questions and risks

- Can concurrent peer disconnect, page application, registry revocation, and digest completion preserve the current token checks without holding a lock across I/O?
- Do production SQLite and Iroh tests prove that revocation unbinds queued data before network sends after the move?

## Next implementation step

Implement the relay coordinator and migrate every registry exchange, readiness, and fanout caller in the same verified unit.
