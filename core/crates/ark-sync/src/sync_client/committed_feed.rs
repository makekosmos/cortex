use std::sync::Arc;

use crate::committed_feed_delivery::{FeedAck, FeedAckOutcome, FeedFlight};
use crate::protocol::LanSyncMessage;
use ark_application::sync_storage::SyncRuntimeStorage;

use crate::sync_client::{ClientFeedInFlight, ClientMessageSender};

#[doc(hidden)]
pub async fn pump_client_committed_feed(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    tx: &ClientMessageSender,
    in_flight: &ClientFeedInFlight,
    receipt_ack: bool,
    policy_bound: bool,
) -> Result<(), String> {
    let mut in_flight = in_flight.lock().await;
    if in_flight.is_some() {
        return Ok(());
    }
    let policy_identity = if policy_bound {
        Some(
            storage
                .accepted_policy_identity_for_peer(peer_device_id)
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
                .selective_sync_policy_generation_for_peer(peer_device_id)
                .await?
        }
    };
    let progress = match &policy_identity {
        Some(identity) => {
            storage
                .begin_outbound_policy_identity(peer_device_id, identity)
                .await?
        }
        None => {
            storage
                .begin_outbound_policy(peer_device_id, generation)
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
                    peer_device_id,
                    identity,
                    after_position,
                    snapshot.high_watermark,
                    crate::committed_feed_delivery::MAX_FEED_PAGE_SIZE,
                )
                .await?
                .page
        }
        None => {
            storage
                .committed_feed_delivery_page(
                    after_position,
                    snapshot.high_watermark,
                    crate::committed_feed_delivery::MAX_FEED_PAGE_SIZE,
                )
                .await?
        }
    };
    let deliveries = match page {
        ark_application::sync_storage::CommittedFeedDeliveryPage::Deliveries(deliveries) => {
            deliveries
        }
        ark_application::sync_storage::CommittedFeedDeliveryPage::SkipTo { through_position } => {
            match &policy_identity {
                Some(identity) => {
                    storage
                        .record_outbound_scan_identity(peer_device_id, identity, through_position)
                        .await?;
                    storage
                        .acknowledge_outbound_identity(
                            peer_device_id,
                            identity,
                            through_position,
                            ark_application::sync_storage::OutboundAck::Applied,
                        )
                        .await?;
                }
                None => {
                    storage
                        .record_outbound_scan(peer_device_id, generation, through_position)
                        .await?;
                    storage
                        .acknowledge_outbound(
                            peer_device_id,
                            generation,
                            through_position,
                            ark_application::sync_storage::OutboundAck::Applied,
                        )
                        .await?;
                }
            }
            return Ok(());
        }
        ark_application::sync_storage::CommittedFeedDeliveryPage::TooLarge {
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
                    .record_outbound_scan_identity(peer_device_id, identity, position)
                    .await?
            }
            None => {
                storage
                    .record_outbound_scan(peer_device_id, generation, position)
                    .await?
            }
        };
        let entity = if policy_identity.is_some() {
            Some(delivery.entity)
        } else {
            storage
                .filter_outgoing_entity_for_peer(peer_device_id, &delivery.entity)
                .await?
        };
        let Some(entity) = entity else {
            match &policy_identity {
                Some(identity) => {
                    storage
                        .acknowledge_outbound_identity(
                            peer_device_id,
                            identity,
                            position,
                            ark_application::sync_storage::OutboundAck::Applied,
                        )
                        .await?
                }
                None => {
                    storage
                        .acknowledge_outbound(
                            peer_device_id,
                            generation,
                            position,
                            ark_application::sync_storage::OutboundAck::Applied,
                        )
                        .await?
                }
            };
            continue;
        };
        let change_id = format!("ark-feed-v1:{position}");
        let message = LanSyncMessage::LiveChange {
            policy_identity: policy_identity.clone(),
            change_id,
            entity,
            origin_device_id: None,
        };
        if policy_bound
            && storage
                .accepted_policy_identity_for_peer(peer_device_id)
                .await?
                != policy_identity
        {
            return Ok(());
        }
        *in_flight = Some((position, policy_identity.clone()));
        if tx.send(message).await.is_err() {
            *in_flight = None;
        }
        return Ok(());
    }

    if progress.scanned_through < snapshot.high_watermark {
        match &policy_identity {
            Some(identity) => {
                storage
                    .record_outbound_scan_identity(
                        peer_device_id,
                        identity,
                        snapshot.high_watermark,
                    )
                    .await?
            }
            None => {
                storage
                    .record_outbound_scan(peer_device_id, generation, snapshot.high_watermark)
                    .await?
            }
        };
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[doc(hidden)]
pub async fn acknowledge_client_feed(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_device_id: &str,
    change_id: &str,
    tx: &ClientMessageSender,
    flight: &ClientFeedInFlight,
    ack: ark_application::sync_storage::OutboundAck,
    receipt_ack: bool,
    policy_bound: bool,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
) -> Result<(), String> {
    let mut in_flight = flight.lock().await;
    let flight_state = in_flight.as_ref().map(|(position, identity)| FeedFlight {
        position: *position,
        policy_identity: identity.clone(),
    });
    let outcome = crate::committed_feed_delivery::acknowledge(
        storage.as_ref(),
        peer_device_id,
        FeedAck {
            change_id: change_id.to_owned(),
            policy_identity,
            acknowledgement: ack,
            durable_receipt_ack: receipt_ack,
            in_flight: flight_state,
        },
    )
    .await?;
    let FeedAckOutcome::Persisted { current_flight } = outcome else {
        return Ok(());
    };
    if current_flight {
        *in_flight = None;
    }
    drop(in_flight);
    if current_flight {
        pump_client_committed_feed(
            storage,
            peer_device_id,
            tx,
            flight,
            receipt_ack,
            policy_bound,
        )
        .await?;
    }
    Ok(())
}
