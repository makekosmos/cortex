use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    pending_snapshot: &mut Option<ark_sync::outbound_snapshot::PendingSnapshotAck>,
    batch_id: &str,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
) -> bool {
    match apply(
        storage,
        peer_device_id,
        pending_snapshot,
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
    pending_snapshot: &mut Option<ark_sync::outbound_snapshot::PendingSnapshotAck>,
    batch_id: &str,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
) -> Result<(), String> {
    let Some(pending) = pending_snapshot.as_ref() else {
        return Ok(());
    };
    if !ark_sync::outbound_snapshot::acknowledge_snapshot(
        storage.as_ref(),
        peer_device_id,
        pending,
        batch_id,
        policy_identity.as_ref(),
    )
    .await?
    {
        return Ok(());
    }
    *pending_snapshot = None;
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
