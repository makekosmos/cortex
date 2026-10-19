use ark_application::sync_policy::PolicyIdentity;
use ark_application::sync_storage::{CommittedFeedPort, OutboundAck};

const FEED_CHANGE_PREFIX: &str = "ark-feed-v1:";
pub const MAX_FEED_PAGE_SIZE: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedFlight {
    pub position: i64,
    pub policy_identity: Option<PolicyIdentity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedAck {
    pub change_id: String,
    pub policy_identity: Option<PolicyIdentity>,
    pub acknowledgement: OutboundAck,
    pub durable_receipt_ack: bool,
    pub in_flight: Option<FeedFlight>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedAckOutcome {
    Ignored,
    Persisted { current_flight: bool },
}

/// Validates the protocol acknowledgement against durable peer progress and
/// persists it through the application-owned committed-feed port. The caller
/// retains ownership of its connection and in-flight state.
pub async fn acknowledge<S: CommittedFeedPort + ?Sized>(
    storage: &S,
    peer_device_id: &str,
    ack: FeedAck,
) -> Result<FeedAckOutcome, String> {
    let Some(position) = ack
        .change_id
        .strip_prefix(FEED_CHANGE_PREFIX)
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return Ok(FeedAckOutcome::Ignored);
    };

    let (generation, progress) = match &ack.policy_identity {
        Some(identity) => (
            identity.revision,
            storage
                .begin_outbound_policy_identity(peer_device_id, identity)
                .await?,
        ),
        None => {
            let generation = storage
                .selective_sync_policy_generation_for_peer(peer_device_id)
                .await?;
            (
                generation,
                storage
                    .begin_outbound_policy(peer_device_id, generation)
                    .await?,
            )
        }
    };
    let current_flight = ack.in_flight.as_ref().is_some_and(|flight| {
        flight.position == position && flight.policy_identity == ack.policy_identity
    });

    match ack.acknowledgement {
        OutboundAck::DurableReceipt => {
            if !ack.durable_receipt_ack || (!current_flight && position > progress.receipt_through)
            {
                return Ok(FeedAckOutcome::Ignored);
            }
        }
        OutboundAck::Applied => {
            if (ack.durable_receipt_ack && position > progress.receipt_through)
                || (!ack.durable_receipt_ack && !current_flight)
            {
                return Ok(FeedAckOutcome::Ignored);
            }
        }
    }

    match &ack.policy_identity {
        Some(identity) => {
            storage
                .acknowledge_outbound_identity(
                    peer_device_id,
                    identity,
                    position,
                    ack.acknowledgement,
                )
                .await?
        }
        None => {
            storage
                .acknowledge_outbound(peer_device_id, generation, position, ack.acknowledgement)
                .await?
        }
    };

    Ok(FeedAckOutcome::Persisted { current_flight })
}

#[cfg(test)]
#[path = "committed_feed_delivery_tests.rs"]
mod tests;
