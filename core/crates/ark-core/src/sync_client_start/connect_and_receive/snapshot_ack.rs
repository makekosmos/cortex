use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    pending_batch_id: &mut Option<String>,
    pending_policy_identity: &mut Option<ark_application::sync_storage::PolicyIdentity>,
    batch_id: &str,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
) -> bool {
    match apply(
        storage,
        peer_device_id,
        pending_batch_id,
        pending_policy_identity,
        batch_id,
        policy_identity,
        authenticated_tx,
        tx,
        feed_in_flight,
    )
    .await
    {
        Ok(()) => true,
        Err(error) => {
            eprintln!("{TAG} snapshot ACK handling failed: {error}");
            false
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    pending_batch_id: &mut Option<String>,
    pending_policy_identity: &mut Option<ark_application::sync_storage::PolicyIdentity>,
    batch_id: &str,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
) -> Result<(), String> {
    if pending_batch_id.as_deref() != Some(batch_id) {
        return Ok(());
    }
    if pending_policy_identity != &policy_identity {
        return Ok(());
    }
    if let Some(identity) = policy_identity.as_ref() {
        storage
            .complete_outbound_snapshot_identity(peer_device_id, identity)
            .await?;
    } else {
        let generation = storage
            .selective_sync_policy_generation_for_peer(peer_device_id)
            .await?;
        storage
            .complete_outbound_snapshot(peer_device_id, generation)
            .await?;
    }
    *pending_batch_id = None;
    *pending_policy_identity = None;
    let (receipt_ack, policy_bound) = if let Some(session) = authenticated_tx.lock().await.as_mut()
    {
        session.sync_complete = true;
        (
            session
                .protocol
                .capabilities
                .contains(&ark_sync::protocol::ProtocolCapability::DurableReceiptAckV1),
            session
                .protocol
                .capabilities
                .contains(&ark_sync::protocol::ProtocolCapability::PolicyBoundDeliveryV1),
        )
    } else {
        (false, false)
    };
    super::pump_client_committed_feed(
        storage,
        peer_device_id,
        tx,
        feed_in_flight,
        receipt_ack,
        policy_bound,
    )
    .await
}
