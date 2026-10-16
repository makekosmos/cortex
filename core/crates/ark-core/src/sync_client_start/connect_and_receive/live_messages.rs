use super::*;

pub(super) async fn handle_signed_integration_frame(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    local_device_id: &str,
    space_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    frame: SignedSyncEnvelope,
) {
    let accepted = if frame.space_id != space_id
        || frame.origin_node_id != peer_device_id
        || frame.recipient_node_id != local_device_id
    {
        false
    } else {
        storage
            .apply_signed_integration_frame(&frame, space_id, peer_device_id, local_device_id)
            .await
            .is_ok()
    };
    send_msg(
        tx,
        &LanSyncMessage::SignedIntegrationAck {
            message_id: frame.message_id,
            accepted,
        },
    );
}

pub(super) async fn handle_live_change(
    storage: &Arc<dyn SyncRuntimeStorage>,
    authenticated_tx: &AuthenticatedTx,
    peer_device_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    on_change: &Arc<Mutex<Option<ClientOnChangeCallback>>>,
    change_id: String,
    entity: SyncEntity,
) -> Result<(), String> {
    let receipt_ack = authenticated_tx
        .lock()
        .await
        .as_ref()
        .is_some_and(|session| {
            session
                .protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
        });
    if receipt_ack
        && storage
            .receive_authenticated_entity_with_id(&entity, peer_device_id, &change_id)
            .await?
    {
        send_msg(
            tx,
            &LanSyncMessage::LiveReceiptAck {
                change_id: change_id.clone(),
            },
        );
    }

    let mut local_vector = load_version_vector(storage).await?;
    let should_apply = is_usage_entity(&entity)
        || local_vector
            .get(&entity.id)
            .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));
    let outcome = storage
        .apply_authenticated_entity_with_id(&entity, peer_device_id, &change_id)
        .await?;
    match outcome {
        crate::application::sync_storage::InboundApplyOutcome::Applied => {
            if should_apply && observe_non_usage_entity(&mut local_vector, &entity) {
                let stored_vector = load_version_vector(storage).await?;
                merge_usage_cursors(&mut local_vector, &stored_vector);
                save_version_vector(storage, &local_vector).await?;
            }
            if should_apply {
                if let Some(handler) = on_change.lock().await.as_ref() {
                    handler(entity);
                }
            }
        }
        crate::application::sync_storage::InboundApplyOutcome::Stale => {}
        crate::application::sync_storage::InboundApplyOutcome::Pending => return Ok(()),
    }
    send_msg(tx, &LanSyncMessage::LiveAck { change_id });
    Ok(())
}

pub(super) async fn handle_feed_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
    change_id: &str,
    ack: crate::application::sync_storage::OutboundAck,
    receipt_ack: bool,
) -> Result<(), String> {
    crate::sync_client::acknowledge_client_feed(
        storage,
        peer_device_id,
        change_id,
        tx,
        feed_in_flight,
        ack,
        receipt_ack,
    )
    .await
}

pub(super) async fn handle_applied_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
    authenticated_tx: &AuthenticatedTx,
    change_id: &str,
) -> Result<(), String> {
    let receipt_ack = authenticated_tx
        .lock()
        .await
        .as_ref()
        .is_some_and(|session| {
            session
                .protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
        });
    handle_feed_ack(
        storage,
        peer_device_id,
        tx,
        feed_in_flight,
        change_id,
        OutboundAck::Applied,
        receipt_ack,
    )
    .await
}

pub(super) async fn handle_receipt_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
    change_id: &str,
) -> Result<(), String> {
    handle_feed_ack(
        storage,
        peer_device_id,
        tx,
        feed_in_flight,
        change_id,
        OutboundAck::DurableReceipt,
        true,
    )
    .await
}
