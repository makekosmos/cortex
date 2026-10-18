use super::*;

pub(super) async fn was_sent(
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
) -> bool {
    authenticated_tx
        .lock()
        .await
        .as_ref()
        .is_some_and(|session| session.outbound_snapshot.has_been_sent())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    batch_id: &str,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    tx: &ClientMessageSender,
    feed_in_flight: &ClientFeedInFlight,
) -> bool {
    match apply(
        storage,
        peer_device_id,
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
    batch_id: &str,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    tx: &ClientMessageSender,
    feed_in_flight: &ClientFeedInFlight,
) -> Result<(), String> {
    let pending = authenticated_tx
        .lock()
        .await
        .as_ref()
        .filter(|session| session.peer_id == peer_device_id)
        .and_then(|session| session.outbound_snapshot.pending_ack().cloned());
    let Some(pending) = pending else {
        return Ok(());
    };
    if !ark_sync::outbound_snapshot::acknowledge_snapshot(
        storage.as_ref(),
        peer_device_id,
        &pending,
        batch_id,
        policy_identity.as_ref(),
    )
    .await?
    {
        return Ok(());
    }
    let mut session_guard = authenticated_tx.lock().await;
    let Some(session) = session_guard
        .as_mut()
        .filter(|session| session.peer_id == peer_device_id)
    else {
        return Ok(());
    };
    if !session.outbound_snapshot.record_acknowledged(&pending) {
        return Ok(());
    }
    let receipt_ack = session
        .protocol
        .capabilities
        .contains(&ark_sync::protocol::ProtocolCapability::DurableReceiptAckV1);
    let policy_bound = session
        .protocol
        .capabilities
        .contains(&ark_sync::protocol::ProtocolCapability::PolicyBoundDeliveryV1);
    drop(session_guard);
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
