use super::*;
use futures_util::{Sink, SinkExt};

pub(super) struct Context {
    storage: Arc<dyn SyncRuntimeStorage>,
    peer: Arc<RwLock<PeerRecord>>,
    space_id: String,
    device_id: String,
}

impl Context {
    pub(super) fn new(
        storage: Arc<dyn SyncRuntimeStorage>,
        peer: Arc<RwLock<PeerRecord>>,
        space_id: String,
        device_id: String,
    ) -> Self {
        Self {
            storage,
            peer,
            space_id,
            device_id,
        }
    }
}

pub(super) fn spawn<S>(
    mut ws_sink: S,
    mut rx: mpsc::UnboundedReceiver<Message>,
    context: Context,
) -> tokio::task::JoinHandle<()>
where
    S: Sink<Message> + Unpin + Send + 'static,
    S::Error: Send + 'static,
{
    let Context {
        storage,
        peer,
        space_id,
        device_id,
    } = context;
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let Message::Text(text) = &message {
                if let Some(sync_message) = deserialize_message(text) {
                    if crate::sync_server::is_user_data_message(&sync_message) {
                        let peer_device_id = peer.read().await.device_id.clone();
                        if !matches!(
                            storage.sync_session_is_admitted(&peer_device_id).await,
                            Ok(true)
                        ) {
                            continue;
                        }
                    }
                    if let LanSyncMessage::SignedIntegrationFrame { frame } = sync_message {
                        if storage
                            .validate_outbound_signed_integration_frame(
                                &frame, &space_id, &device_id,
                            )
                            .await
                            .is_err()
                        {
                            continue;
                        }
                    }
                }
            }
            if ws_sink.send(message).await.is_err() {
                break;
            }
        }
    })
}
