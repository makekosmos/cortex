use super::*;
use futures_util::{Sink, SinkExt};

pub(super) fn spawn<S>(
    mut ws_sink: S,
    mut rx: mpsc::UnboundedReceiver<Message>,
    storage: Arc<dyn SyncRuntimeStorage>,
    space_id: String,
    device_id: String,
) -> tokio::task::JoinHandle<()>
where
    S: Sink<Message> + Unpin + Send + 'static,
    S::Error: Send + 'static,
{
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let Message::Text(text) = &message {
                if let Some(LanSyncMessage::SignedIntegrationFrame { frame }) =
                    deserialize_message(text)
                {
                    if storage
                        .validate_outbound_signed_integration_frame(&frame, &space_id, &device_id)
                        .await
                        .is_err()
                    {
                        continue;
                    }
                }
            }
            if ws_sink.send(message).await.is_err() {
                break;
            }
        }
    })
}
