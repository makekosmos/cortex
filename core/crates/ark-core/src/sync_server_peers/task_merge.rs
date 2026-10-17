use super::*;

pub(crate) async fn pump_task_merge_batch(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
) -> Result<(), String> {
    let (device_id, ready, supported, tx) = {
        let peers = peers.lock().await;
        let Some(peer) = peers.get(&peer_id) else {
            return Ok(());
        };
        (
            peer.device_id.clone(),
            peer.ready_peer.is_some() && peer.sync_complete,
            peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::TaskMergeBootstrapV2)
            }),
            peer.tx.clone(),
        )
    };
    if !ready || !supported {
        return Ok(());
    }
    let Some(batch) =
        ark_sync::task_merge_delivery::prepare_outbound_batch(storage.as_ref(), &device_id).await?
    else {
        return Ok(());
    };
    let still_ready = peers.lock().await.get(&peer_id).is_some_and(|peer| {
        peer.ready_peer.is_some()
            && peer.sync_complete
            && peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::TaskMergeBootstrapV2)
            })
    });
    if still_ready {
        let message = LanSyncMessage::TaskMergeBatch { batch };
        if tx
            .send(Message::Text(serialize_message(&message).into()))
            .is_err()
        {
            return Err("task_merge_batch_send_failed".into());
        }
    }
    Ok(())
}

pub(crate) async fn handle_task_merge_batch_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
    batch_id: &str,
    through_position: i64,
    identity: &ark_application::sync_storage::PolicyIdentity,
) -> Result<(), String> {
    let device_id = {
        let peers = peers.lock().await;
        let Some(peer) = peers.get(&peer_id) else {
            return Ok(());
        };
        if !peer.ready_peer.is_some()
            || !peer.sync_complete
            || !peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::TaskMergeBootstrapV2)
            })
        {
            return Ok(());
        }
        peer.device_id.clone()
    };
    if ark_sync::task_merge_delivery::acknowledge_outbound_batch(
        storage.as_ref(),
        &device_id,
        identity,
        batch_id,
        through_position,
    )
    .await?
    {
        pump_task_merge_batch(storage, peers, peer_id).await?;
    }
    Ok(())
}
