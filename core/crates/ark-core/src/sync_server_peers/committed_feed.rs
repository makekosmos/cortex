use super::*;

const FEED_CHANGE_PREFIX: &str = "ark-feed-v1:";

pub(crate) async fn pump_committed_feed(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
) -> Result<(), String> {
    let (device_id, ready, in_flight, receipt_ack, policy_bound) = {
        let peers = peers.lock().await;
        let Some(peer) = peers.get(&peer_id) else {
            return Ok(());
        };
        (
            peer.device_id.clone(),
            peer.ready_peer.is_some() && peer.sync_complete,
            peer.in_flight_feed_position,
            peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
            }),
            peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::PolicyBoundDeliveryV1)
            }),
        )
    };
    if !ready || in_flight.is_some() {
        return Ok(());
    }

    let policy_identity = if policy_bound {
        Some(
            storage
                .accepted_policy_identity_for_peer(&device_id)
                .await?
                .ok_or_else(|| {
                    "accepted_policy_identity_missing_for_negotiated_peer".to_string()
                })?,
        )
    } else {
        None
    };
    let generation = match &policy_identity {
        Some(identity) => identity.revision,
        None => {
            storage
                .selective_sync_policy_generation_for_peer(&device_id)
                .await?
        }
    };
    let progress = match &policy_identity {
        Some(identity) => {
            storage
                .begin_outbound_policy_identity(&device_id, identity)
                .await?
        }
        None => {
            storage
                .begin_outbound_policy(&device_id, generation)
                .await?
        }
    };
    if !progress.initial_snapshot.is_complete() {
        return Ok(());
    }
    let snapshot = storage.committed_feed_snapshot().await?;
    let after_position = if receipt_ack {
        progress.receipt_through
    } else {
        progress.applied_through
    };
    let page = match &policy_identity {
        Some(identity) => {
            storage
                .committed_feed_delivery_page_for_peer(
                    &device_id,
                    identity,
                    after_position,
                    snapshot.high_watermark,
                    crate::db::MAX_FEED_PAGE_SIZE,
                )
                .await?
                .page
        }
        None => {
            storage
                .committed_feed_delivery_page(
                    after_position,
                    snapshot.high_watermark,
                    crate::db::MAX_FEED_PAGE_SIZE,
                )
                .await?
        }
    };
    let deliveries = match page {
        crate::application::sync_storage::CommittedFeedDeliveryPage::Deliveries(deliveries) => {
            deliveries
        }
        crate::application::sync_storage::CommittedFeedDeliveryPage::TooLarge {
            position,
            entity_id,
            serialized_bytes,
            max_bytes,
        } => {
            return Err(format!(
                "committed_feed_entity_too_large:{position}:{entity_id}:\
                 {serialized_bytes}:{max_bytes}"
            ));
        }
    };

    for delivery in deliveries {
        let position = delivery.entry.position;
        match &policy_identity {
            Some(identity) => {
                storage
                    .record_outbound_scan_identity(&device_id, identity, position)
                    .await?
            }
            None => {
                storage
                    .record_outbound_scan(&device_id, generation, position)
                    .await?
            }
        };
        let entity = if policy_identity.is_some() {
            Some(delivery.entity)
        } else {
            storage
                .filter_outgoing_entity_for_peer(&device_id, &delivery.entity)
                .await?
        };
        let Some(entity) = entity else {
            match &policy_identity {
                Some(identity) => {
                    storage
                        .acknowledge_outbound_identity(
                            &device_id,
                            identity,
                            position,
                            crate::application::sync_storage::OutboundAck::Applied,
                        )
                        .await?
                }
                None => {
                    storage
                        .acknowledge_outbound(
                            &device_id,
                            generation,
                            position,
                            crate::application::sync_storage::OutboundAck::Applied,
                        )
                        .await?
                }
            };
            continue;
        };
        let tx = {
            let mut peers = peers.lock().await;
            let Some(peer) = peers.get_mut(&peer_id) else {
                return Ok(());
            };
            if !peer.ready_peer.is_some()
                || !peer.sync_complete
                || peer.in_flight_feed_position.is_some()
            {
                return Ok(());
            }
            peer.in_flight_feed_position = Some(position);
            peer.in_flight_feed_policy_identity = policy_identity.clone();
            peer.tx.clone()
        };
        if policy_bound
            && storage
                .accepted_policy_identity_for_peer(&device_id)
                .await?
                != policy_identity
        {
            if let Some(peer) = peers.lock().await.get_mut(&peer_id) {
                peer.in_flight_feed_position = None;
                peer.in_flight_feed_policy_identity = None;
            }
            return Ok(());
        }
        let message = LanSyncMessage::LiveChange {
            policy_identity: policy_identity.clone(),
            change_id: format!("{FEED_CHANGE_PREFIX}{position}"),
            entity,
            origin_device_id: None,
        };
        if tx
            .send(Message::Text(serialize_message(&message).into()))
            .is_err()
        {
            if let Some(peer) = peers.lock().await.get_mut(&peer_id) {
                peer.in_flight_feed_position = None;
                peer.in_flight_feed_policy_identity = None;
            }
        }
        return Ok(());
    }

    if progress.scanned_through < snapshot.high_watermark {
        match &policy_identity {
            Some(identity) => {
                storage
                    .record_outbound_scan_identity(&device_id, identity, snapshot.high_watermark)
                    .await?
            }
            None => {
                storage
                    .record_outbound_scan(&device_id, generation, snapshot.high_watermark)
                    .await?
            }
        };
    }
    Ok(())
}

pub(crate) async fn handle_committed_feed_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
    change_id: &str,
    policy_identity: Option<crate::application::sync_storage::PolicyIdentity>,
) -> Result<(), String> {
    let Some(position) = change_id
        .strip_prefix(FEED_CHANGE_PREFIX)
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return Ok(());
    };
    let (device_id, receipt_ack, current_flight, expected_identity) = {
        let peers = peers.lock().await;
        let Some(peer) = peers.get(&peer_id) else {
            return Ok(());
        };
        (
            peer.device_id.clone(),
            peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
            }),
            peer.in_flight_feed_position == Some(position),
            peer.in_flight_feed_policy_identity.clone(),
        )
    };
    if expected_identity != policy_identity {
        return Ok(());
    }
    let (generation, progress) = match &policy_identity {
        Some(identity) => (
            identity.revision,
            storage
                .begin_outbound_policy_identity(&device_id, identity)
                .await?,
        ),
        None => {
            let generation = storage
                .selective_sync_policy_generation_for_peer(&device_id)
                .await?;
            (
                generation,
                storage
                    .begin_outbound_policy(&device_id, generation)
                    .await?,
            )
        }
    };
    if (receipt_ack && position > progress.receipt_through) || (!receipt_ack && !current_flight) {
        return Ok(());
    }
    match &policy_identity {
        Some(identity) => {
            storage
                .acknowledge_outbound_identity(
                    &device_id,
                    identity,
                    position,
                    crate::application::sync_storage::OutboundAck::Applied,
                )
                .await?
        }
        None => {
            storage
                .acknowledge_outbound(
                    &device_id,
                    generation,
                    position,
                    crate::application::sync_storage::OutboundAck::Applied,
                )
                .await?
        }
    };
    if current_flight {
        if let Some(peer) = peers.lock().await.get_mut(&peer_id) {
            peer.in_flight_feed_position = None;
            peer.in_flight_feed_policy_identity = None;
        }
    }
    if current_flight {
        pump_committed_feed(storage, peers, peer_id).await?;
    }
    Ok(())
}

pub(crate) async fn handle_committed_feed_receipt_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
    change_id: &str,
    policy_identity: Option<crate::application::sync_storage::PolicyIdentity>,
) -> Result<(), String> {
    let Some(position) = change_id
        .strip_prefix(FEED_CHANGE_PREFIX)
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return Ok(());
    };
    let (device_id, receipt_ack, current_flight, expected_identity) = {
        let peers = peers.lock().await;
        let Some(peer) = peers.get(&peer_id) else {
            return Ok(());
        };
        (
            peer.device_id.clone(),
            peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
            }),
            peer.in_flight_feed_position == Some(position),
            peer.in_flight_feed_policy_identity.clone(),
        )
    };
    if !receipt_ack {
        return Ok(());
    }
    if expected_identity != policy_identity {
        return Ok(());
    }
    let (generation, progress) = match &policy_identity {
        Some(identity) => (
            identity.revision,
            storage
                .begin_outbound_policy_identity(&device_id, identity)
                .await?,
        ),
        None => {
            let generation = storage
                .selective_sync_policy_generation_for_peer(&device_id)
                .await?;
            (
                generation,
                storage
                    .begin_outbound_policy(&device_id, generation)
                    .await?,
            )
        }
    };
    if !current_flight && position > progress.receipt_through {
        return Ok(());
    }
    match &policy_identity {
        Some(identity) => {
            storage
                .acknowledge_outbound_identity(
                    &device_id,
                    identity,
                    position,
                    crate::application::sync_storage::OutboundAck::DurableReceipt,
                )
                .await?
        }
        None => {
            storage
                .acknowledge_outbound(
                    &device_id,
                    generation,
                    position,
                    crate::application::sync_storage::OutboundAck::DurableReceipt,
                )
                .await?
        }
    };
    if current_flight {
        if let Some(peer) = peers.lock().await.get_mut(&peer_id) {
            peer.in_flight_feed_position = None;
            peer.in_flight_feed_policy_identity = None;
        }
        pump_committed_feed(storage, peers, peer_id).await?;
    }
    Ok(())
}
