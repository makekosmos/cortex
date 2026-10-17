// Iroh QUIC transport. Each connection uses separate reader and writer tasks
// so cancellation cannot consume a frame prefix and desynchronize later reads.
// Reconnect uses exponential backoff. Version-vector catch-up repairs changes
// missed while no peer connection is active. Desktop-only Iroh features are
// owned by ark-net.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint::{presets, BindOpts};
use iroh::{Endpoint, Watcher};
use iroh_tickets::endpoint::EndpointTicket;
use iroh_tickets::Ticket;
use tokio::sync::{broadcast, mpsc, watch, Notify};

use crate::sync_bind::SyncBind;
use ark_sync::protocol::{
    deserialize_message, message_origin_device_id, serialize_message, LanSyncMessage,
    PROTOCOL_VERSION,
};
use ark_sync::protocol_negotiation::{
    compute_hello_auth_hmac, generate_auth_nonce, normalize_auth_secret,
};
use ark_sync::transport::{SyncTransport, TransportEvent};
use ark_sync::{SignedSyncEnvelope, SyncRuntimeStorage};

/// ALPN-идентификатор протокола ark-sync поверх iroh QUIC-стримов.
/// Версионируем отдельно от `protocol::PROTOCOL_VERSION` (см. spec §3.3).
pub const ARK_SYNC_ALPN: &[u8] = b"ark-sync/1";

/// Ёмкость broadcast-канала исходящих сообщений. Выбрана щедро: реальные
/// CRDT-потоки не достигают такого burst. При `RecvError::Lagged` задача
/// просто пропускает старые кадры — они будут компенсированы VV-обменом
/// при следующем (ре)коннекте (см. модульный doc).
const OUTGOING_BROADCAST_CAPACITY: usize = 256;

// Shared slot for the caller-side oneshot that resolves when a queued
// message has actually been written to the peer.
type SharedCompletion =
    Option<Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Result<(), String>>>>>>;

#[derive(Clone)]
struct OutgoingMessage {
    target: Option<EndpointId>,
    msg: LanSyncMessage,
    completion: SharedCompletion,
}

#[derive(Clone)]
struct OutboundStorage {
    storage: Arc<dyn SyncRuntimeStorage>,
    space_id: String,
    origin_node_id: String,
}
