# ARK sync session boundary

## Problem

The original `ark-core::SyncServer` and `SyncClient` combined WebSocket I/O with authentication, protocol negotiation, registry admission, policy-bound snapshots, task-merge delivery, and shutdown. Moving those files unchanged would preserve the wrong ownership. The server extraction now places session protocol state and progress in `ark-sync`, concrete WebSocket/Iroh transports in `ark-net`, and application hosting and service composition in `ark-core`. The wire format and storage behavior remain unchanged.

The server session driver in `ark-sync` owns admission, readiness, inbound classification, snapshot/feed/task-merge/live protocol coordination, ACK validation, and session progress. `ark-net` owns socket framing and listener/connect operations. Core implements the server host callbacks for storage, registry side effects, peer-map projection, notifications, and service lifecycle. The direct client still has Core-owned orchestration and is not fully migrated. The target API below is a design sketch, not an implemented constructor contract.

## Usage

Target composition keeps socket and session ownership below `ArkService`:

```rust,ignore
let listener = ark_net::WebSocketSyncListener::new(bind);
let server = ark_sync::DirectSyncServer::new(storage, observer, identity);
server.start(listener).await?;
server.stop_and_join().await;
```

For an outgoing peer, Core should supply a connector configured by `ark-net`; `ark-sync` should own the peer session and reconnect policy. Core should translate completed session events into the existing Engine event stream. Current code has not completed this transfer.

## Shape

The target is one direct-sync session authority in `ark-sync`: protocol state transitions, per-peer admission, negotiated policy identity, snapshot/feed progress, ACK rules, replay decisions, and joinable session tasks. The private async server driver now owns these server-session decisions and protocol coordination. Its host interface is application-specific: Core performs storage-backed host operations and projects connection state, while it does not interpret message branches or decide protocol transitions. Keep pure transition logic private and deterministic for unit testing; do not expose a generic effect algebra.

The transport contract lives in `ark-sync`; `ark-net` implements it. It exchanges the current text payloads and does not expose sockets or WebSocket types:

```rust,ignore
#[async_trait]
pub trait SyncConnection: Send {
    async fn receive_text(&mut self) -> Result<Option<String>, TransportError>;
    async fn send_text(&mut self, text: String) -> Result<(), TransportError>;
    async fn close(&mut self) -> Result<(), TransportError>;
}

#[async_trait]
pub trait SyncAcceptor: Send + Sync {
    async fn accept(&self) -> Result<Option<Box<dyn SyncConnection>>, TransportError>;
    async fn close(&self) -> Result<(), TransportError>;
}

#[async_trait]
pub trait SyncConnector: Send + Sync {
    async fn connect(&self, peer: &PeerRecord)
        -> Result<Box<dyn SyncConnection>, TransportError>;
}

pub struct DirectSyncServer {}
pub struct DirectSyncClient {}

impl DirectSyncServer {
    pub fn new(storage: Arc<dyn SyncRuntimeStorage>, observer: Arc<dyn SyncObserver>,
               identity: LocalSyncIdentity) -> Self;
    pub async fn start(&self, acceptor: Arc<dyn SyncAcceptor>) -> Result<(), SyncError>;
    pub async fn stop_and_join(&self);
}
```

The exact public signatures are refined against current callers before implementation. Reuse the existing application-owned sync ports; add no broad storage supertrait. Keep JSON encode/decode in `ark-sync::protocol`, which owns `LanSyncMessage`; the network adapter maps WebSocket text to and from strings. Freeze exact serialized fixtures before moving callers. `ark-net` handles bind, connect, socket framing, control frames, and transport errors. `ark-core` retains configuration, service lifecycle, Engine callbacks, and API compatibility. `ark-store` remains an implementation of application ports and never imports sync or network code.

The transport session is per connection. Its authenticated device identity is established only by the existing protocol/admission flow; transport identity is not device identity. State transitions revalidate the connection/session generation after every storage await, so late work cannot make a disconnected, revoked, or replaced session ready. Stop closes admission, signals sessions, and joins their tasks before releasing storage handles. Existing idempotency receipts define what a canceled storage call may already have committed; cancellation never implies rollback.

## Source inventory

The former `sync_server_messages`, `sync_server_inbound`, and committed-feed/task-merge protocol paths have been removed from Core. `ark-sync::server_session::connected` now owns Hello/admission, policy-gated readiness, message classification and ordering, snapshot/feed/task-merge/live handling, ACK checks, and per-session progress. Core's connected-session host retains storage access, registry mutation/fanout callbacks, peer-map projection, user-visible notifications, and outer service lifecycle. These host effects remain outside the protocol driver because they belong to application composition.

`ark-sync::client_session` validates Hello frames and owns registry-admission state, received-policy validation and persistence decisions, policy advertisement, the initial outbound-policy cursor, version-vector access decisions, and typed receive classification. Core still owns direct client orchestration and some authenticated-session, callback, snapshot, live-change, feed, task-merge, and signed-integration dispatch. The client `start` loop still owns address retry and reader/writer tasks. Completing that client cutover is remaining work; address resolution, dialing, WebSocket framing, and close details belong in `ark-net`.

Storage calls already enter through `ark-application` capabilities: peer admission/identity and registry history; accepted per-peer policy; local profile; version vectors and usage-complete-through cursors; policy-bound entity pages; authenticated entity apply and durable receipts; committed-feed snapshot/ACK; task-merge delivery; and signed-integration authorization. The extraction must reuse these calls and preserve their transaction boundaries. It must not add a second executor API that simply renames each storage method.

Three orderings are load-bearing. First, an inbound live change persists a supported durable receipt, Core sends the receipt ACK, then `ark-sync::inbound_live_change::apply_live_change` applies it and stores version-vector progress before Core sends `LiveAck`. `Pending` receives no normal ACK; `Applied` and `Stale` do. Callbacks and fan-out happen only for applied changes. For the initial snapshot, `SyncAck` is sent only after the final page has been applied and its progress persisted. Second, outbound snapshots capture the initial vector and usage completeness before paging, check policy identity around page reads, and mark the final batch pending before sending it. Third, durable live-feed progress advances only after the current applied ACK with its matching policy identity; `ark-sync::committed_feed_delivery::acknowledge` owns the shared validation and persistence, while Core clears current in-flight state and pumps the next entry. Existing tests and production paths remain the oracle for these orderings; the extraction changes ownership only.

Selective sync remains a per-device transfer rule. Disabling a category suppresses future delivery to that device and never deletes its local rows, books, files, history, or receipts. Other devices that opted in remain eligible. Widening a policy changes its identity and invokes the existing backfill path. A future large-file transport can use the same class policy without changing the one-personal-database model.

## Synthesis decision

Two distinct candidates were compared. Candidate B's single transition owner scored higher on centralized invariants and dependency ownership; the independent cross-judge preferred it overall (graph 5/5, invariants 5/5, interface 3/5, rollout 4/5). Candidate A's direct async coordinator had a viable transport boundary but blurred state, reconnect, and storage ownership (4/5, 3/5, 2/5, 4/5). This design keeps B's single transition authority and generation checks, while placing its private executor in `ark-sync` and using A's per-connection transport contract. That removes B's public effect list and Core executor, which could become a second protocol handler. Neither candidate's illustrative API is accepted without checking every current branch and port call.

## Tradeoffs accepted

- `ark-sync` may use Tokio task/time primitives to own session lifecycle, while `ark-net` alone owns concrete sockets. This matches the session ownership boundary and keeps transport details replaceable.
- Direct WebSocket sessions and relay/Iroh multi-peer orchestration remain separate until their admission and lifecycle rules are proven equivalent; this extraction does not unify distinct protocols.
- The existing application storage traits are reused as the session's capability boundary. Narrower ports are introduced only when the source inventory shows a concrete cohesion or dependency problem.

## Alternatives considered

- Move `SyncServer` and `SyncClient` unchanged into `ark-sync`: rejected because it carries `TcpListener`, tungstenite messages, network setup, and task wiring into the sync layer.
- Keep a pure machine in `ark-sync` and execute a public effect algebra in Core: rejected because Core would interpret sync protocol transitions and the effect enum would mirror the broad storage API.
- Move only peer state or message handlers: rejected because protocol truth and ACK/admission decisions would remain split across crates.
- Put all direct-session rules in `ark-net`: rejected because a transport layer would own data policy and protocol decisions.

## Implementation reconciliation

Before moving each remaining production path, inventory its message branches, storage operations, send ordering, callbacks, and task boundary. If that inventory requires a large generic effect list or duplicates policy in Core, stop and reduce the extracted unit; do not paper over it with a broad port. Preserve exact wire fixtures and the production SQLite/WebSocket tests as behavior oracles. Move one vertical path at a time, migrate callers, then delete the old Core session and transport dependencies in the same cutover.

## Next implementation step

Finish the direct client cutover through the existing `ark-sync` session boundary. Move the remaining client protocol coordination, retry policy, and progress into its session owner; leave concrete socket lifecycle in `ark-net` and Core with application-host callbacks and composition. Preserve policy generation, ACK order, callbacks, and wire fixtures, and remove each obsolete Core path with its migrated callers.
