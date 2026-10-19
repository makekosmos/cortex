use super::{ConnectedServerSession, ServerSessionActor};
use crate::{
    committed_feed_delivery::{
        acknowledge, FeedAck, FeedAckOutcome, FeedFlight, MAX_FEED_PAGE_SIZE,
    },
    protocol::{LanSyncMessage, ProtocolCapability},
    session_io::SessionIoSender,
};
use ark_application::sync_storage::{CommittedFeedDeliveryPage, OutboundAck};

const FEED_CHANGE_PREFIX: &str = "ark-feed-v1:";

impl ConnectedServerSession {
    pub(in crate::server_session) async fn pump_committed_feed(
        &mut self,
        actor: &ServerSessionActor,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(device_id) = self.peer.device_id.clone() else {
            return false;
        };
        if actor.is_revoked() || !self.snapshot_ready() || self.feed_flight.is_some() {
            return !actor.is_revoked();
        }
        let receipt_ack = self.supports(ProtocolCapability::DurableReceiptAckV1);
        let policy_bound = self.supports(ProtocolCapability::PolicyBoundDeliveryV1);
        let policy_identity = if policy_bound {
            match self
                .storage
                .accepted_policy_identity_for_peer(&device_id)
                .await
            {
                Ok(Some(identity)) if !actor.is_revoked() => Some(identity),
                Ok(_) => return !actor.is_revoked(),
                Err(error) => return self.log_feed_error(actor, error),
            }
        } else {
            None
        };
        let generation = match &policy_identity {
            Some(identity) => identity.revision,
            None => match self
                .storage
                .selective_sync_policy_generation_for_peer(&device_id)
                .await
            {
                Ok(generation) if !actor.is_revoked() => generation,
                Ok(_) => return false,
                Err(error) => return self.log_feed_error(actor, error),
            },
        };
        let progress = match &policy_identity {
            Some(identity) => {
                self.storage
                    .begin_outbound_policy_identity(&device_id, identity)
                    .await
            }
            None => {
                self.storage
                    .begin_outbound_policy(&device_id, generation)
                    .await
            }
        };
        let progress = match progress {
            Ok(progress) if !actor.is_revoked() => progress,
            Ok(_) => return false,
            Err(error) => return self.log_feed_error(actor, error),
        };
        if !progress.initial_snapshot.is_complete() {
            return true;
        }
        let snapshot = match self.storage.committed_feed_snapshot().await {
            Ok(snapshot) if !actor.is_revoked() => snapshot,
            Ok(_) => return false,
            Err(error) => return self.log_feed_error(actor, error),
        };
        let after_position = if receipt_ack {
            progress.receipt_through
        } else {
            progress.applied_through
        };
        let page = match &policy_identity {
            Some(identity) => self
                .storage
                .committed_feed_delivery_page_for_peer(
                    &device_id,
                    identity,
                    after_position,
                    snapshot.high_watermark,
                    MAX_FEED_PAGE_SIZE,
                )
                .await
                .map(|page| page.page),
            None => {
                self.storage
                    .committed_feed_delivery_page(
                        after_position,
                        snapshot.high_watermark,
                        MAX_FEED_PAGE_SIZE,
                    )
                    .await
            }
        };
        let page = match page {
            Ok(page) if !actor.is_revoked() => page,
            Ok(_) => return false,
            Err(error) => return self.log_feed_error(actor, error),
        };
        match page {
            CommittedFeedDeliveryPage::SkipTo { through_position } => {
                if !self
                    .record_feed_progress(
                        actor,
                        &device_id,
                        &policy_identity,
                        generation,
                        through_position,
                    )
                    .await
                {
                    return !actor.is_revoked();
                }
                self.ack_feed_progress(
                    actor,
                    &device_id,
                    &policy_identity,
                    generation,
                    through_position,
                )
                .await
            }
            CommittedFeedDeliveryPage::TooLarge {
                position,
                entity_id,
                serialized_bytes,
                max_bytes,
            } => {
                eprintln!(
                    "committed feed entity too large: {position}:{entity_id}:{serialized_bytes}:{max_bytes}"
                );
                true
            }
            CommittedFeedDeliveryPage::Deliveries(deliveries) => {
                for delivery in deliveries {
                    if actor.is_revoked() {
                        return false;
                    }
                    let position = delivery.entry.position;
                    if !self
                        .record_feed_scan(actor, &device_id, &policy_identity, generation, position)
                        .await
                    {
                        return !actor.is_revoked();
                    }
                    let entity = if policy_identity.is_some() {
                        Some(delivery.entity)
                    } else {
                        match self
                            .storage
                            .filter_outgoing_entity_for_peer(&device_id, &delivery.entity)
                            .await
                        {
                            Ok(entity) if !actor.is_revoked() => entity,
                            Ok(_) => return false,
                            Err(error) => return self.log_feed_error(actor, error),
                        }
                    };
                    let Some(entity) = entity else {
                        if !self
                            .ack_feed_progress(
                                actor,
                                &device_id,
                                &policy_identity,
                                generation,
                                position,
                            )
                            .await
                        {
                            return !actor.is_revoked();
                        }
                        continue;
                    };
                    self.feed_flight = Some(FeedFlight {
                        position,
                        policy_identity: policy_identity.clone(),
                    });
                    if policy_bound {
                        let active_identity = match self
                            .storage
                            .accepted_policy_identity_for_peer(&device_id)
                            .await
                        {
                            Ok(identity) if !actor.is_revoked() => identity,
                            Ok(_) => {
                                self.feed_flight = None;
                                return false;
                            }
                            Err(error) => {
                                self.feed_flight = None;
                                return self.log_feed_error(actor, error);
                            }
                        };
                        if active_identity != policy_identity {
                            self.feed_flight = None;
                            return true;
                        }
                    }
                    if sender
                        .send_bulk_wait(LanSyncMessage::LiveChange {
                            policy_identity: policy_identity.clone(),
                            change_id: format!("{FEED_CHANGE_PREFIX}{position}"),
                            entity,
                            origin_device_id: None,
                        })
                        .await
                        .is_err()
                        || actor.is_revoked()
                    {
                        self.feed_flight = None;
                        return false;
                    }
                    return true;
                }
                if progress.scanned_through < snapshot.high_watermark {
                    self.record_feed_scan(
                        actor,
                        &device_id,
                        &policy_identity,
                        generation,
                        snapshot.high_watermark,
                    )
                    .await
                } else {
                    true
                }
            }
        }
    }

    pub(super) async fn handle_feed_ack(
        &mut self,
        actor: &ServerSessionActor,
        change_id: String,
        policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
        acknowledgement: OutboundAck,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(device_id) = self.peer.device_id.as_deref() else {
            return false;
        };
        let receipt_ack = self.supports(ProtocolCapability::DurableReceiptAckV1);
        let expected_identity = self
            .feed_flight
            .as_ref()
            .and_then(|flight| flight.policy_identity.clone());
        if expected_identity != policy_identity
            || (acknowledgement == OutboundAck::DurableReceipt && !receipt_ack)
        {
            return true;
        }
        let outcome = match acknowledge(
            self.storage.as_ref(),
            device_id,
            FeedAck {
                change_id,
                policy_identity,
                acknowledgement,
                durable_receipt_ack: receipt_ack,
                in_flight: self.feed_flight.clone(),
            },
        )
        .await
        {
            Ok(outcome) if !actor.is_revoked() => outcome,
            Ok(_) => return false,
            Err(error) => return self.log_feed_error(actor, error),
        };
        if matches!(
            outcome,
            FeedAckOutcome::Persisted {
                current_flight: true
            }
        ) {
            self.feed_flight = None;
            self.pump_committed_feed(actor, sender).await
        } else {
            true
        }
    }

    async fn record_feed_scan(
        &self,
        actor: &ServerSessionActor,
        device_id: &str,
        identity: &Option<ark_application::sync_storage::PolicyIdentity>,
        generation: i64,
        through_position: i64,
    ) -> bool {
        let result = match identity {
            Some(identity) => {
                self.storage
                    .record_outbound_scan_identity(device_id, identity, through_position)
                    .await
            }
            None => {
                self.storage
                    .record_outbound_scan(device_id, generation, through_position)
                    .await
            }
        };
        result.map_or_else(
            |error| self.log_feed_error(actor, error),
            |_| !actor.is_revoked(),
        )
    }

    async fn ack_feed_progress(
        &self,
        actor: &ServerSessionActor,
        device_id: &str,
        identity: &Option<ark_application::sync_storage::PolicyIdentity>,
        generation: i64,
        through_position: i64,
    ) -> bool {
        let result = match identity {
            Some(identity) => {
                self.storage
                    .acknowledge_outbound_identity(
                        device_id,
                        identity,
                        through_position,
                        OutboundAck::Applied,
                    )
                    .await
            }
            None => {
                self.storage
                    .acknowledge_outbound(
                        device_id,
                        generation,
                        through_position,
                        OutboundAck::Applied,
                    )
                    .await
            }
        };
        result.map_or_else(
            |error| self.log_feed_error(actor, error),
            |_| !actor.is_revoked(),
        )
    }

    async fn record_feed_progress(
        &self,
        actor: &ServerSessionActor,
        device_id: &str,
        identity: &Option<ark_application::sync_storage::PolicyIdentity>,
        generation: i64,
        through_position: i64,
    ) -> bool {
        self.record_feed_scan(actor, device_id, identity, generation, through_position)
            .await
            && self
                .ack_feed_progress(actor, device_id, identity, generation, through_position)
                .await
    }

    fn log_feed_error(&self, actor: &ServerSessionActor, error: String) -> bool {
        if actor.is_revoked() {
            return false;
        }
        eprintln!("committed feed operation failed: {error}");
        true
    }
}
