use super::super::*;
use crate::outbound_snapshot::SnapshotEmission;
use tokio::task::JoinHandle;

pub(super) fn spawn(
    storage: Arc<dyn SyncRuntimeStorage>,
    peer_device_id: String,
    remote_vector: ark_domain::types::VersionVector,
    snapshot_already_sent: bool,
    policy_bound: bool,
    tx: ClientMessageSender,
    authenticated_tx: AuthenticatedTx,
) -> JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        let mut snapshot = crate::outbound_snapshot::OutboundSnapshot::begin(
            storage.as_ref(),
            peer_device_id,
            remote_vector,
            snapshot_already_sent,
            policy_bound,
        )
        .await?;
        snapshot
            .drive(storage.as_ref(), |emission| {
                let tx = tx.clone();
                let authenticated_tx = authenticated_tx.clone();
                async move {
                    match emission {
                        SnapshotEmission::Page(messages) => {
                            for message in messages {
                                tx.send_snapshot_batch(message).await?;
                            }
                        }
                        SnapshotEmission::Terminal { ack, message } => {
                            if let Some(session) = authenticated_tx.lock().await.as_mut() {
                                session.outbound_snapshot.record_sent(ack);
                            }
                            tx.send_snapshot_batch(*message).await?;
                        }
                    }
                    Ok(())
                }
            })
            .await?;
        Ok(())
    })
}
