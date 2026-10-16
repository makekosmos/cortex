use super::*;

pub(super) async fn pump(
    storage: &Arc<dyn SyncRuntimeStorage>,
    authenticated_tx: &AuthenticatedTx,
    feed_in_flight: &ClientFeedInFlight,
) -> Result<(), String> {
    let authenticated_peer = authenticated_tx
        .lock()
        .await
        .as_ref()
        .and_then(|(peer_id, tx, complete)| (*complete).then(|| (peer_id.clone(), tx.clone())));
    if let Some((peer_id, tx)) = authenticated_peer {
        super::pump_client_committed_feed(storage, &peer_id, &tx, feed_in_flight).await?;
    }
    Ok(())
}
