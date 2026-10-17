use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::hlc::HLC;
use crate::integration_replication::SignedSyncEnvelope;
use crate::protocol::*;
use crate::replication_storage::SyncRuntimeStorage;
use crate::task_set::{abort_and_join, join_all, track_task};

#[path = "sync_client/committed_feed.rs"]
mod committed_feed;
pub(crate) use committed_feed::{acknowledge_client_feed, pump_client_committed_feed};

#[path = "sync_client_start.rs"]
mod sync_client_start;
use crate::types::*;

const TAG: &str = "[SyncClient]";
const CONNECT_TIMEOUT_MS: u64 = 5_000;
const RECONNECT_BASE_MS: u64 = 2_000;
const RECONNECT_MAX_MS: u64 = 30_000;
const SYNC_LOAD_PAGE_SIZE: usize = 100;

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

pub type ClientOnChangeCallback = Arc<dyn Fn(SyncEntity) + Send + Sync>;
pub type ClientOnConnectedCallback = Arc<dyn Fn(PeerRecord) + Send + Sync>;
pub type ClientOnDisconnectedCallback = Arc<dyn Fn(String) + Send + Sync>;
pub type ClientOnPeerListCallback = Arc<dyn Fn(Vec<PeerRecord>) + Send + Sync>;
pub type ClientOnRegistryEventsCallback =
    Arc<dyn Fn(Vec<ark_domain::device_registry::RegistryEvent>) + Send + Sync>;

// Authenticated peer channel: (peer device_id, outbound sender) behind the
// shared mutex so the connection-scoped negotiated session becomes visible
// only after the complete handshake succeeds.
#[derive(Clone)]
pub(crate) struct AuthenticatedPeerSession {
    pub(crate) peer_id: String,
    pub(crate) registry_session_id: String,
    pub(crate) tx: mpsc::UnboundedSender<Message>,
    pub(crate) sync_started: bool,
    pub(crate) sync_complete: bool,
    pub(crate) protocol: crate::protocol::NegotiatedProtocol,
}

type AuthenticatedTx = Arc<Mutex<Option<AuthenticatedPeerSession>>>;
pub(crate) type ClientFeedInFlight =
    Arc<Mutex<Option<(i64, Option<ark_application::sync_storage::PolicyIdentity>)>>>;

// ---------------------------------------------------------------------------
// SyncClient
// ---------------------------------------------------------------------------

pub struct SyncClient {
    storage: Arc<dyn SyncRuntimeStorage>,
    peer: Arc<RwLock<PeerRecord>>,
    device_id: String,
    device_name: String,
    space_id: String,
    own_addresses: Vec<String>,
    auth_secret: Option<String>,
    task_merge_bootstrap_v2: bool,
    stopped: Arc<std::sync::atomic::AtomicBool>,
    authenticated_tx: AuthenticatedTx,
    feed_in_flight: ClientFeedInFlight,
    on_change: Arc<Mutex<Option<ClientOnChangeCallback>>>,
    on_connected: Arc<Mutex<Option<ClientOnConnectedCallback>>>,
    on_disconnected: Arc<Mutex<Option<ClientOnDisconnectedCallback>>>,
    on_peer_list: Arc<Mutex<Option<ClientOnPeerListCallback>>>,
    on_registry_events: Arc<Mutex<Option<ClientOnRegistryEventsCallback>>>,
    /// The connection loop from `start` and each connection's writer. They
    /// hold `storage` (the shared ark.db connection); `stop_and_join` aborts
    /// and joins them so none outlives teardown (KOS-270, KOS-369).
    tasks: Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    /// Short callback tasks spawned on this client's behalf (see
    /// `callback_registry`). `stop_and_join` waits for them.
    callbacks: Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl SyncClient {
    pub fn new(
        storage: Arc<dyn SyncRuntimeStorage>,
        peer: PeerRecord,
        device_id: String,
        device_name: String,
        space_id: String,
        own_addresses: Vec<String>,
        auth_secret: Option<String>,
    ) -> Self {
        Self {
            storage,
            peer: Arc::new(RwLock::new(peer)),
            device_id,
            device_name,
            space_id,
            own_addresses,
            auth_secret: normalize_auth_secret(auth_secret),
            task_merge_bootstrap_v2: true,
            stopped: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            authenticated_tx: Arc::new(Mutex::new(None)),
            feed_in_flight: Arc::new(Mutex::new(None)),
            on_change: Arc::new(Mutex::new(None)),
            on_connected: Arc::new(Mutex::new(None)),
            on_disconnected: Arc::new(Mutex::new(None)),
            on_peer_list: Arc::new(Mutex::new(None)),
            on_registry_events: Arc::new(Mutex::new(None)),
            tasks: Arc::new(std::sync::Mutex::new(Vec::new())),
            callbacks: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_task_merge_bootstrap_v2(mut self) -> Self {
        self.task_merge_bootstrap_v2 = true;
        self
    }

    /// Wake the durable feed sender after a local commit. The entity itself is
    /// read from the feed so reconnects replay the same cursor-derived ID.
    pub async fn broadcast_live_change(&self) {
        let guard = self.authenticated_tx.lock().await;
        if let Some(session) = guard.as_ref() {
            if !session.sync_complete {
                return;
            }
            let peer_id = session.peer_id.clone();
            let tx = session.tx.clone();
            let receipt_ack = session
                .protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1);
            let policy_bound = session
                .protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::PolicyBoundDeliveryV1);
            drop(guard);
            if let Err(error) = pump_client_committed_feed(
                &self.storage,
                &peer_id,
                &tx,
                &self.feed_in_flight,
                receipt_ack,
                policy_bound,
            )
            .await
            {
                eprintln!("{TAG} committed feed wake failed: {error}");
            }
        }
    }

    pub async fn broadcast_registry_event(
        &self,
        event: ark_domain::device_registry::RegistryEvent,
    ) {
        let mut guard = self.authenticated_tx.lock().await;
        let Some(session) = guard.as_mut() else {
            return;
        };
        send_msg(
            &session.tx,
            &LanSyncMessage::RegistryEventUpdate {
                session_id: session.registry_session_id.clone(),
                event: event.clone(),
            },
        );
        if matches!(
            event.operation(),
            ark_domain::device_registry::RegistryOperation::Disable
        ) && event.target_device_id().as_str() == session.peer_id
        {
            self.stopped
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let session = guard.take().expect("authenticated session was present");
            let _ = session.tx.send(Message::Close(None));
        }
    }

    pub async fn send_signed_integration_frame(
        &self,
        frame: SignedSyncEnvelope,
    ) -> Result<(), String> {
        if frame.space_id != self.space_id {
            return Err("signed integration frame has the wrong space".into());
        }
        if frame.origin_node_id != self.device_id {
            return Err("signed integration frame origin is not this node".into());
        }
        let guard = self.authenticated_tx.lock().await;
        let Some(session) = guard.as_ref() else {
            return Err("LAN peer is not authenticated".into());
        };
        if frame.recipient_node_id != session.peer_id {
            return Err("signed integration frame is addressed to another peer".into());
        }
        self.storage
            .validate_outbound_signed_integration_frame(&frame, &self.space_id, &self.device_id)
            .await?;
        session
            .tx
            .send(Message::Text(
                serialize_message(&LanSyncMessage::SignedIntegrationFrame { frame }).into(),
            ))
            .map_err(|_| "LAN peer connection is closed".into())
    }

    pub async fn set_on_change(&self, handler: ClientOnChangeCallback) {
        *self.on_change.lock().await = Some(handler);
    }

    pub async fn set_on_connected(&self, handler: ClientOnConnectedCallback) {
        *self.on_connected.lock().await = Some(handler);
    }

    pub async fn set_on_disconnected(&self, handler: ClientOnDisconnectedCallback) {
        *self.on_disconnected.lock().await = Some(handler);
    }

    pub async fn set_on_peer_list(&self, handler: ClientOnPeerListCallback) {
        *self.on_peer_list.lock().await = Some(handler);
    }

    pub async fn set_on_registry_events(&self, handler: ClientOnRegistryEventsCallback) {
        *self.on_registry_events.lock().await = Some(handler);
    }

    /// The device_id of the peer this client dials — the authenticated id
    /// once Hello completes (seed/bootstrap records carry a `seed-*`
    /// placeholder until then).
    pub fn peer_device_id(&self) -> String {
        self.peer
            .try_read()
            .map(|peer| peer.device_id.clone())
            .unwrap_or_default()
    }

    /// Stop the client and don't reconnect.
    pub fn stop(&self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Actively close the current connection, if any, and stop reconnects.
    pub async fn disconnect(&self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(session) = self.authenticated_tx.lock().await.take() {
            let _ = session.tx.send(Message::Close(None));
        }
    }

    /// Stop the client and wait until its tasks have ended. A loop asleep in
    /// its reconnect backoff does not look at `stopped` until the sleep ends,
    /// so the tasks are aborted and joined, not only signalled. Same
    /// abort-and-drain shape as `IrohTransport::stop`.
    pub async fn stop_and_join(&self) {
        self.disconnect().await;
        abort_and_join(&self.tasks).await;
        // Callback work is short and finite: wait for it instead of aborting,
        // so a db write it has started is not left running past the join.
        join_all(&self.callbacks).await;
    }

    /// The task list for callbacks that spawn work on this client's behalf.
    pub(crate) fn callback_registry(
        &self,
    ) -> Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>> {
        self.callbacks.clone()
    }

    /// Replace the current peer record (typically used when the beacon
    /// discovers updated addresses for a known device).
    pub async fn update_peer(&self, peer: PeerRecord) {
        *self.peer.write().await = peer;
    }

    /// Snapshot of the peer this client is dialling.
    pub async fn current_peer(&self) -> PeerRecord {
        self.peer.read().await.clone()
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(std::sync::atomic::Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn load_version_vector(
    storage: &Arc<dyn SyncRuntimeStorage>,
) -> Result<VersionVector, String> {
    storage.load_version_vector().await
}

async fn save_version_vector(
    storage: &Arc<dyn SyncRuntimeStorage>,
    vector: &VersionVector,
) -> Result<(), String> {
    storage.store_version_vector(vector).await
}

fn send_msg(tx: &mpsc::UnboundedSender<Message>, msg: &LanSyncMessage) {
    let json = serialize_message(msg);
    let _ = tx.send(Message::Text(json.into()));
}

/// Race connections to all addresses; return the first successful WebSocket + winning address.
async fn race_connect(
    addresses: &[String],
) -> Option<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
)> {
    use tokio::sync::oneshot;

    if addresses.is_empty() {
        return None;
    }

    let (result_tx, result_rx) = oneshot::channel();
    let result_tx = Arc::new(Mutex::new(Some(result_tx)));
    let mut handles = Vec::new();

    for addr in addresses {
        let url = format!("ws://{addr}");
        let addr_clone = addr.clone();
        let result_tx = result_tx.clone();

        let handle = tokio::spawn(async move {
            let connect_future = connect_async(&url);
            if let Ok(Ok((ws_stream, _))) =
                tokio::time::timeout(Duration::from_millis(CONNECT_TIMEOUT_MS), connect_future)
                    .await
            {
                let mut guard = result_tx.lock().await;
                if let Some(tx) = guard.take() {
                    let _ = tx.send((ws_stream, addr_clone));
                }
            }
        });
        handles.push(handle);
    }

    match result_rx.await {
        Ok(result) => {
            // Cancel other candidates
            for handle in handles {
                handle.abort();
            }
            Some(result)
        }
        Err(_) => None,
    }
}
