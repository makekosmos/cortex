use super::*;

pub(super) async fn handle_task_merge_batch(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    local_device_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    batch: ark_application::sync_storage::TaskMergeBatch,
) -> Result<(), String> {
    let local_profile = storage.load_local_sync_profile(local_device_id).await?;
    let expected_identity = crate::protocol::sync_policy_identity(local_profile.as_ref());
    if batch.identity != expected_identity {
        return Ok(());
    }
    let ack = storage
        .apply_task_merge_batch(peer_device_id, local_device_id, &expected_identity, &batch)
        .await?;
    send_msg(
        tx,
        &LanSyncMessage::TaskMergeBatchAck {
            batch_id: ack.batch_id,
            through_position: ack.through_position,
            policy_identity: expected_identity,
        },
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_feed_ack_message(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
    authenticated_tx: &AuthenticatedTx,
    change_id: &str,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    receipt_ack: bool,
) -> bool {
    let result = if receipt_ack {
        handle_receipt_ack(
            storage,
            peer_device_id,
            tx,
            feed_in_flight,
            authenticated_tx,
            change_id,
            policy_identity,
        )
        .await
    } else {
        handle_applied_ack(
            storage,
            peer_device_id,
            tx,
            feed_in_flight,
            authenticated_tx,
            change_id,
            policy_identity,
        )
        .await
    };
    if let Err(error) = result {
        let ack_kind = if receipt_ack {
            "receipt"
        } else {
            "committed feed"
        };
        eprintln!("{TAG} {ack_kind} ACK failed: {error}");
        false
    } else {
        true
    }
}

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

pub(super) async fn handle_signed_integration_message(
    authenticated: bool,
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    local_device_id: &str,
    space_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    frame: SignedSyncEnvelope,
) {
    if authenticated {
        handle_signed_integration_frame(
            storage,
            peer_device_id,
            local_device_id,
            space_id,
            tx,
            frame,
        )
        .await;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_live_change(
    storage: &Arc<dyn SyncRuntimeStorage>,
    authenticated_tx: &AuthenticatedTx,
    local_device_id: &str,
    peer_device_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    on_change: &Arc<Mutex<Option<ClientOnChangeCallback>>>,
    change_id: String,
    entity: SyncEntity,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
) -> Result<(), String> {
    let (policy_bound, local_profile) = {
        let session = authenticated_tx.lock().await;
        let policy_bound = session.as_ref().is_some_and(|session| {
            session
                .protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::PolicyBoundDeliveryV1)
        });
        (
            policy_bound,
            storage.load_local_sync_profile(local_device_id).await?,
        )
    };
    let expected_identity =
        policy_bound.then(|| crate::protocol::sync_policy_identity(local_profile.as_ref()));
    if policy_identity != expected_identity {
        return Err("stale_or_unnegotiated_sync_policy".into());
    }
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
            .receive_authenticated_entity_with_policy(
                &entity,
                peer_device_id,
                &change_id,
                local_device_id,
                expected_identity.clone(),
            )
            .await?
    {
        send_msg(
            tx,
            &LanSyncMessage::LiveReceiptAck {
                policy_identity: policy_identity.clone(),
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
        .apply_authenticated_entity_with_policy(
            &entity,
            peer_device_id,
            &change_id,
            local_device_id,
            expected_identity.clone(),
        )
        .await?;
    match outcome {
        ark_application::sync_storage::InboundApplyOutcome::Applied => {
            if should_apply && observe_non_usage_entity(&mut local_vector, &entity) {
                let stored_vector = load_version_vector(storage).await?;
                merge_usage_cursors(&mut local_vector, &stored_vector);
                match expected_identity.as_ref() {
                    Some(identity) => {
                        crate::sync_server::save_version_vector_with_policy(
                            storage,
                            local_device_id,
                            identity,
                            &local_vector,
                        )
                        .await?;
                    }
                    None => save_version_vector(storage, &local_vector).await?,
                }
            }
            if should_apply {
                if let Some(handler) = on_change.lock().await.as_ref() {
                    handler(entity);
                }
            }
        }
        ark_application::sync_storage::InboundApplyOutcome::Stale => {}
        ark_application::sync_storage::InboundApplyOutcome::Pending => return Ok(()),
    }
    send_msg(
        tx,
        &LanSyncMessage::LiveAck {
            change_id,
            policy_identity,
        },
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_feed_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
    change_id: &str,
    ack: ark_application::sync_storage::OutboundAck,
    receipt_ack: bool,
    policy_bound: bool,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
) -> Result<(), String> {
    crate::sync_client::acknowledge_client_feed(
        storage,
        peer_device_id,
        change_id,
        tx,
        feed_in_flight,
        ack,
        receipt_ack,
        policy_bound,
        policy_identity,
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
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
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
    let policy_bound = authenticated_tx
        .lock()
        .await
        .as_ref()
        .is_some_and(|session| {
            session
                .protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::PolicyBoundDeliveryV1)
        });
    handle_feed_ack(
        storage,
        peer_device_id,
        tx,
        feed_in_flight,
        change_id,
        OutboundAck::Applied,
        receipt_ack,
        policy_bound,
        policy_identity,
    )
    .await
}

pub(super) async fn handle_receipt_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    tx: &mpsc::UnboundedSender<Message>,
    feed_in_flight: &ClientFeedInFlight,
    authenticated_tx: &AuthenticatedTx,
    change_id: &str,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
) -> Result<(), String> {
    let policy_bound = authenticated_tx
        .lock()
        .await
        .as_ref()
        .is_some_and(|session| {
            session
                .protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::PolicyBoundDeliveryV1)
        });
    handle_feed_ack(
        storage,
        peer_device_id,
        tx,
        feed_in_flight,
        change_id,
        OutboundAck::DurableReceipt,
        true,
        policy_bound,
        policy_identity,
    )
    .await
}
