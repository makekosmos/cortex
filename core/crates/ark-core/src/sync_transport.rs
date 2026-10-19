pub use ark_sync::transport::*;

use std::sync::Arc;

/// The selected transport family and the capabilities guaranteed by it.
///
/// Relay has only shared lifecycle/broadcast operations. Iroh carries the
/// complete peer-session contract. Transport-specific callers must match the
/// variant before invoking peer-session operations.
#[derive(Clone)]
pub enum SyncTransportHandle {
    Relay(Arc<dyn SyncTransport>),
    Iroh {
        transport: Arc<dyn IrohSessionTransport>,
        peer_control: Arc<dyn ark_sync::relay_registry::PeerSessionControl>,
    },
}

impl SyncTransportHandle {
    pub fn iroh_with_peer_control(
        transport: Arc<dyn IrohSessionTransport>,
        peer_control: Arc<dyn ark_sync::relay_registry::PeerSessionControl>,
    ) -> Self {
        Self::Iroh {
            transport,
            peer_control,
        }
    }

    pub fn relay(transport: Arc<dyn SyncTransport>) -> Self {
        Self::Relay(transport)
    }

    pub fn peer_control(&self) -> Option<Arc<dyn ark_sync::relay_registry::PeerSessionControl>> {
        match self {
            Self::Relay(_) => None,
            Self::Iroh { peer_control, .. } => Some(peer_control.clone()),
        }
    }

    pub fn session(&self) -> Option<Arc<dyn IrohSessionTransport>> {
        match self {
            Self::Relay(_) => None,
            Self::Iroh { transport, .. } => Some(transport.clone()),
        }
    }

    pub async fn start(
        &self,
        events: tokio::sync::mpsc::UnboundedSender<TransportEvent>,
    ) -> Result<(), String> {
        match self {
            Self::Relay(transport) => transport.start(events).await,
            Self::Iroh { transport, .. } => transport.start(events).await,
        }
    }

    pub fn send(&self, message: ark_sync::protocol::LanSyncMessage) -> Result<(), String> {
        match self {
            Self::Relay(transport) => transport.send(message),
            Self::Iroh { transport, .. } => transport.send(message),
        }
    }

    pub async fn stop(&self) {
        match self {
            Self::Relay(transport) => transport.stop().await,
            Self::Iroh { transport, .. } => transport.stop().await,
        }
    }
}

impl<T> From<Arc<T>> for SyncTransportHandle
where
    T: SyncTransport + 'static,
{
    fn from(transport: Arc<T>) -> Self {
        Self::Relay(transport)
    }
}

impl From<Arc<dyn SyncTransport>> for SyncTransportHandle {
    fn from(transport: Arc<dyn SyncTransport>) -> Self {
        Self::Relay(transport)
    }
}
