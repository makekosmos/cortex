use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::hlc::HLC;
use crate::integration_replication::SignedSyncEnvelope;
use crate::protocol::*;
use crate::sync_server::StorageBackend;

#[path = "sync_client_start.rs"]
mod sync_client_start;
use crate::types::*;

const TAG: &str = "[SyncClient]";
const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";
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

// Authenticated peer channel: (peer device_id, outbound sender) behind the
// shared mutex so `peer_authenticated` can publish it once the handshake lands.
type AuthenticatedTx = Arc<Mutex<Option<(String, mpsc::UnboundedSender<Message>)>>>;

// ---------------------------------------------------------------------------
// SyncClient
// ---------------------------------------------------------------------------

pub struct SyncClient {
    storage: Arc<dyn StorageBackend>,
    peer: Arc<RwLock<PeerRecord>>,
    device_id: String,
    device_name: String,
    space_id: String,
    own_addresses: Vec<String>,
    auth_secret: Option<String>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
    authenticated_tx: AuthenticatedTx,
    on_change: Arc<Mutex<Option<ClientOnChangeCallback>>>,
    on_connected: Arc<Mutex<Option<ClientOnConnectedCallback>>>,
    on_disconnected: Arc<Mutex<Option<ClientOnDisconnectedCallback>>>,
    on_peer_list: Arc<Mutex<Option<ClientOnPeerListCallback>>>,
}

impl SyncClient {
    pub fn new(
        storage: Arc<dyn StorageBackend>,
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
            stopped: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            authenticated_tx: Arc::new(Mutex::new(None)),
            on_change: Arc::new(Mutex::new(None)),
            on_connected: Arc::new(Mutex::new(None)),
            on_disconnected: Arc::new(Mutex::new(None)),
            on_peer_list: Arc::new(Mutex::new(None)),
        }
    }

    /// Send a live change to the currently connected peer, if any. The
    /// message is dropped silently if the WS connection is not up or not
    /// authenticated — the sender on the server side is expected to keep
    /// state.
    pub async fn broadcast_live_change(&self, entity: SyncEntity) {
        let guard = self.authenticated_tx.lock().await;
        if let Some((_, tx)) = guard.as_ref() {
            let change_id = generate_id();
            let msg = LanSyncMessage::LiveChange {
                change_id,
                entity,
                origin_device_id: None,
            };
            let _ = tx.send(Message::Text(serialize_message(&msg).into()));
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
        let Some((authenticated_peer_id, tx)) = guard.as_ref() else {
            return Err("LAN peer is not authenticated".into());
        };
        if frame.recipient_node_id != *authenticated_peer_id {
            return Err("signed integration frame is addressed to another peer".into());
        }
        self.storage
            .validate_outbound_signed_integration_frame(&frame, &self.space_id, &self.device_id)
            .await?;
        tx.send(Message::Text(
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
        if let Some((_, tx)) = self.authenticated_tx.lock().await.take() {
            let _ = tx.send(Message::Close(None));
        }
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

async fn load_version_vector(storage: &Arc<dyn StorageBackend>) -> VersionVector {
    match storage.get_kv(VERSION_VECTOR_KEY).await {
        Some(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        None => VersionVector::new(),
    }
}

async fn save_version_vector(storage: &Arc<dyn StorageBackend>, vector: &VersionVector) {
    let json = serde_json::to_string(vector).unwrap_or_default();
    storage.set_kv(VERSION_VECTOR_KEY, &json).await;
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
