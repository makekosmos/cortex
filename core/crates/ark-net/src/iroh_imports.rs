use std::sync::Arc;

use super::EndpointId;

use ark_domain::sync_contracts::wire::LanSyncMessage;

/// ALPN-идентификатор протокола ark-sync поверх iroh QUIC-стримов.
/// Версионируем отдельно от `protocol::PROTOCOL_VERSION` (см. spec §3.3).
pub const ARK_SYNC_ALPN: &[u8] = b"ark-sync/1";

/// Capacity for queued outbound messages.
pub(super) const OUTGOING_BROADCAST_CAPACITY: usize = 256;

pub(super) type SharedCompletion =
    Option<Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Result<(), String>>>>>>;

#[derive(Clone)]
pub(super) struct OutgoingMessage {
    pub(super) target: Option<EndpointId>,
    pub(super) msg: LanSyncMessage,
    pub(super) completion: SharedCompletion,
}
