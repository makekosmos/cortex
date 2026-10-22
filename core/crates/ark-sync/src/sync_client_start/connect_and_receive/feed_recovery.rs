use super::*;

pub(super) async fn pump(
    storage: &Arc<dyn SyncRuntimeStorage>,
    authenticated_tx: &AuthenticatedTx,
    feed_in_flight: &ClientFeedInFlight,
) -> Result<(), String> {
    let authenticated_peer = authenticated_tx.lock().await.as_ref().and_then(|session| {
        session.outbound_snapshot.has_been_acknowledged().then(|| {
            (
                session.peer_id.clone(),
                session.tx.clone(),
                session
                    .protocol
                    .capabilities
                    .contains(&ProtocolCapability::DurableReceiptAckV1),
                session
                    .protocol
                    .capabilities
                    .contains(&ProtocolCapability::PolicyBoundDeliveryV1),
            )
        })
    });
    if let Some((peer_id, tx, receipt_ack, policy_bound)) = authenticated_peer {
        super::pump_client_committed_feed(
            storage,
            &peer_id,
            &tx,
            feed_in_flight,
            receipt_ack,
            policy_bound,
        )
        .await?;
    }
    Ok(())
}
