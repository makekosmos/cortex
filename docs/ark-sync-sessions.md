# ARK sync session boundary

## Problem

`ark-core::SyncServer` and `SyncClient` currently combine direct WebSocket I/O with authentication, protocol negotiation, registry admission, policy-bound snapshots, feed acknowledgements, task-merge delivery, and session shutdown. Moving these files unchanged would preserve the wrong ownership. KOS-424 requires `ark-sync` to own sessions while `ark-net` owns sockets and `ark-core` remains the `ArkService` composition and JSON boundary. The wire format and storage behavior must remain unchanged during this extraction.

## Usage

Core composes the existing SQLite-backed application port with the network adapter and session service:

```rust,ignore
let listener = ark_net::WebSocketSyncListener::new(bind);
let server = ark_sync::DirectSyncServer::new(storage, observer, identity);
server.start(listener).await?;
// Existing ArkService requests and JSON responses are unchanged.
server.stop_and_join().await;
```

For an outgoing peer, Core supplies the connector configured by `ark-net`; `ark-sync` owns the peer session and reconnect policy. Core continues to translate typed session events into the existing Engine event stream.

## Shape

`ark-sync` owns one direct-sync session authority: protocol state transitions, per-peer admission, negotiated policy identity, snapshot/feed progress, ACK rules, replay decisions, and joinable session tasks. Keep pure transition logic private and deterministic for unit testing; a private async driver in the same crate applies transitions through application ports and the transport contract. Do not expose a generic effect algebra or have `ark-core` interpret protocol effects.

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

pub struct DirectSyncServer { /* private session state and joined tasks */ }
pub struct DirectSyncClient { /* private reconnect and current-session state */ }

impl DirectSyncServer {
    pub fn new(storage: Arc<dyn SyncRuntimeStorage>, observer: Arc<dyn SyncObserver>,
               identity: LocalSyncIdentity) -> Self;
    pub async fn start(&self, acceptor: Arc<dyn SyncAcceptor>) -> Result<(), SyncError>;
    pub async fn stop_and_join(&self);
}
```

The exact public signatures are refined against current callers before implementation. Reuse the existing application-owned sync ports; add no broad storage supertrait. Keep JSON encode/decode in `ark-sync::protocol`, which owns `LanSyncMessage`; the network adapter maps WebSocket text to and from strings. Freeze exact serialized fixtures before moving callers. `ark-net` handles bind, connect, socket framing, control frames, and transport errors. `ark-core` retains configuration, service lifecycle, Engine callbacks, and API compatibility. `ark-store` remains an implementation of application ports and never imports sync or network code.

The transport session is per connection. Its authenticated device identity is established only by the existing protocol/admission flow; transport identity is not device identity. State transitions revalidate the connection/session generation after every storage await, so late work cannot make a disconnected, revoked, or replaced session ready. Stop closes admission, signals sessions, and joins their tasks before releasing storage handles. Existing idempotency receipts define what a canceled storage call may already have committed; cancellation never implies rollback.

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

Before moving production callers, inventory every server/client message branch, storage operation, send ordering, callback, and task boundary. If that inventory requires a large generic effect list or duplicates policy in Core, stop and reduce the extracted unit; do not paper over it with a broad port. Preserve exact wire fixtures and the production SQLite/WebSocket tests as behavior oracles. Move one vertical path at a time, migrate callers, then delete the old Core session and transport dependencies in the same cutover.

## Next implementation step

Build the branch-to-storage/send/event inventory from `sync_server_messages`, `sync_server_inbound`, `sync_client_start`, and `sync_client` before defining final signatures.
