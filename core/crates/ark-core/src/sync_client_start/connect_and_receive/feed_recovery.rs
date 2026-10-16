use super::*;

pub(super) async fn pump(
    storage: &Arc<dyn SyncRuntimeStorage>,
    authenticated_tx: &AuthenticatedTx,
    feed_in_flight: &ClientFeedInFlight,
) -> Result<(), String> {
    let authenticated_peer = authenticated_tx.lock().await.as_ref().and_then(|session| {
        session.sync_complete.then(|| {
            (
                session.peer_id.clone(),
                session.tx.clone(),
                session
                    .protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1),
            )
        })
    });
    if let Some((peer_id, tx, receipt_ack)) = authenticated_peer {
        super::pump_client_committed_feed(storage, &peer_id, &tx, feed_in_flight, receipt_ack)
            .await?;
    }
    Ok(())
}
