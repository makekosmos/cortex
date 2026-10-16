use super::*;

pub(super) async fn handle(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    pending_batch_id: &mut Option<String>,
    batch_id: &str,
    authenticated_tx: &Arc<Mutex<Option<AuthenticatedPeerSession>>>,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
) -> Result<(), String> {
    if pending_batch_id.as_deref() != Some(batch_id) {
        return Ok(());
    }
    let generation = storage
        .selective_sync_policy_generation_for_peer(peer_device_id)
        .await?;
    storage
        .complete_outbound_snapshot(peer_device_id, generation)
        .await?;
    *pending_batch_id = None;
    let receipt_ack = if let Some(session) = authenticated_tx.lock().await.as_mut() {
        session.sync_complete = true;
        session
            .protocol
            .capabilities
            .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
    } else {
        false
    };
    super::pump_client_committed_feed(storage, peer_device_id, tx, feed_in_flight, receipt_ack)
        .await
}
