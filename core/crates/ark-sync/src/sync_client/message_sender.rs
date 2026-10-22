use std::sync::Arc;

use ark_application::sync_storage::SyncRuntimeStorage;
use ark_domain::types::PeerRecord;
use tokio::sync::{mpsc, Mutex, RwLock};

use ark_sync_contracts::session_io::SessionIoSender;
use ark_sync_contracts::wire::LanSyncMessage;

#[doc(hidden)]
pub type ClientFeedInFlight =
    Arc<Mutex<Option<(i64, Option<ark_domain::sync_policy::PolicyIdentity>)>>>;

#[derive(Clone)]
#[doc(hidden)]
pub struct ClientMessageSender {
    transport: ClientMessageTransport,
    authenticated_peer_id: Arc<RwLock<Option<String>>>,
}

#[derive(Clone)]
enum ClientMessageTransport {
    Session {
        sender: SessionIoSender,
        storage: Arc<dyn SyncRuntimeStorage>,
        peer: Arc<RwLock<PeerRecord>>,
        space_id: String,
        device_id: String,
    },
    Test(mpsc::Sender<LanSyncMessage>),
}

impl ClientMessageSender {
    pub(crate) fn new(
        transport: SessionIoSender,
        storage: Arc<dyn SyncRuntimeStorage>,
        peer: Arc<RwLock<PeerRecord>>,
        space_id: String,
        device_id: String,
    ) -> Self {
        Self {
            transport: ClientMessageTransport::Session {
                sender: transport,
                storage,
                peer,
                space_id,
                device_id,
            },
            authenticated_peer_id: Arc::new(RwLock::new(None)),
        }
    }

    pub(crate) async fn set_authenticated_peer_id(&self, peer_id: String) {
        *self.authenticated_peer_id.write().await = Some(peer_id);
    }

    pub(crate) async fn send(&self, message: LanSyncMessage) -> Result<(), String> {
        self.send_with_queue_mode(message, false).await
    }

    pub(crate) async fn send_snapshot_batch(&self, message: LanSyncMessage) -> Result<(), String> {
        self.send_with_queue_mode(message, true).await
    }

    async fn send_with_queue_mode(
        &self,
        message: LanSyncMessage,
        wait_for_bulk_capacity: bool,
    ) -> Result<(), String> {
        match &self.transport {
            ClientMessageTransport::Session {
                sender,
                storage,
                peer,
                space_id,
                device_id,
            } => {
                if ark_sync_contracts::wire::requires_registry_admission(&message) {
                    let authenticated_peer_id = self.authenticated_peer_id.read().await.clone();
                    let peer_device_id = match authenticated_peer_id {
                        Some(peer_device_id) => peer_device_id,
                        None => peer.read().await.device_id.clone(),
                    };
                    if !matches!(
                        storage.sync_session_is_admitted(&peer_device_id).await,
                        Ok(true)
                    ) {
                        return Ok(());
                    }
                }
                if let LanSyncMessage::SignedIntegrationFrame { frame } = &message {
                    if storage
                        .validate_outbound_signed_integration_frame(frame, space_id, device_id)
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                let result = if wait_for_bulk_capacity {
                    sender.send_bulk_wait(message).await
                } else {
                    sender.send(message).await
                };
                result.map_err(|error| match error {
                    ark_sync_contracts::session_io::SessionIoError::OutboundFull => {
                        "LAN peer outbound queue is full".to_owned()
                    }
                    _ => "LAN peer connection is closed".to_owned(),
                })
            }
            ClientMessageTransport::Test(sender) => sender
                .send(message)
                .await
                .map_err(|_| "test sync message receiver is closed".to_owned()),
        }
    }

    #[doc(hidden)]
    pub fn test_channel() -> (Self, mpsc::Receiver<LanSyncMessage>) {
        let (sender, receiver) = mpsc::channel(64);
        (
            Self {
                transport: ClientMessageTransport::Test(sender),
                authenticated_peer_id: Arc::new(RwLock::new(None)),
            },
            receiver,
        )
    }

    pub(crate) fn close(&self) {
        match &self.transport {
            ClientMessageTransport::Session { sender, .. } => sender.close(),
            ClientMessageTransport::Test(_) => {}
        }
    }
}
