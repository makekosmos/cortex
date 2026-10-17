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
                    .contains(&crate::protocol::ProtocolCapability::TaskMergeOperationsV1)
            }),
            peer.tx.clone(),
        )
    };
    if !ready || !supported {
        return Ok(());
    }
    let Some(identity) = storage
        .accepted_policy_identity_for_peer(&device_id)
        .await?
    else {
        return Err("accepted_policy_identity_missing_for_task_merge_peer".into());
    };
    let Some(batch) = storage
        .prepare_task_merge_batch(&device_id, &identity, 100)
        .await?
    else {
        return Ok(());
    };
    if storage
        .accepted_policy_identity_for_peer(&device_id)
        .await?
        .as_ref()
        != Some(&identity)
    {
        return Ok(());
    }
    let still_ready = peers.lock().await.get(&peer_id).is_some_and(|peer| {
        peer.ready_peer.is_some()
            && peer.sync_complete
            && peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::TaskMergeOperationsV1)
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
    identity: &crate::application::sync_storage::PolicyIdentity,
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
                    .contains(&crate::protocol::ProtocolCapability::TaskMergeOperationsV1)
            })
        {
            return Ok(());
        }
        peer.device_id.clone()
    };
    storage
        .acknowledge_task_merge_batch(&device_id, identity, batch_id, through_position)
        .await?;
    pump_task_merge_batch(storage, peers, peer_id).await
}
