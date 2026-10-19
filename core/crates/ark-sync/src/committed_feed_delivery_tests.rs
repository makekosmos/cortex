use super::{acknowledge, FeedAck, FeedAckOutcome, FeedFlight};
use ark_application::sync_policy::PolicyIdentity;
use ark_application::sync_storage::{
    CommittedFeedDeliveryPage, CommittedFeedEntry, CommittedFeedPort, FeedSnapshot, OutboundAck,
    OutboundProgress,
};
use std::sync::Mutex;

struct FakeFeedStore {
    identity: PolicyIdentity,
    progress: Mutex<OutboundProgress>,
    acknowledgements: Mutex<Vec<(i64, OutboundAck)>>,
    begin_calls: Mutex<usize>,
}

impl FakeFeedStore {
    fn new(progress: OutboundProgress) -> Self {
        Self {
            identity: identity(2),
            progress: Mutex::new(progress),
            acknowledgements: Mutex::new(Vec::new()),
            begin_calls: Mutex::new(0),
        }
    }

    fn begin(&self, generation: i64) -> Result<OutboundProgress, String> {
        *self.begin_calls.lock().unwrap() += 1;
        let progress = *self.progress.lock().unwrap();
        if generation != progress.policy_generation {
            return Err("outbound_feed_policy_changed".into());
        }
        Ok(progress)
    }

    fn acknowledge(
        &self,
        generation: i64,
        position: i64,
        acknowledgement: OutboundAck,
    ) -> Result<OutboundProgress, String> {
        let mut progress = self.progress.lock().unwrap();
        if !progress.initial_snapshot.is_complete()
            || generation != progress.policy_generation
            || position < 0
            || position > progress.scanned_through
        {
            return Err("invalid_outbound_feed_ack".into());
        }
        match acknowledgement {
            OutboundAck::DurableReceipt => {
                progress.receipt_through = progress.receipt_through.max(position);
            }
            OutboundAck::Applied => {
                progress.receipt_through = progress.receipt_through.max(position);
                progress.applied_through = progress.applied_through.max(position);
            }
        }
        self.acknowledgements
            .lock()
            .unwrap()
            .push((position, acknowledgement));
        Ok(*progress)
    }
}

#[async_trait::async_trait]
impl CommittedFeedPort for FakeFeedStore {
    fn selective_sync_policy_generation(&self) -> i64 {
        self.progress.lock().unwrap().policy_generation
    }

    async fn selective_sync_policy_generation_for_peer(&self, _: &str) -> Result<i64, String> {
        Err("unused".into())
    }

    async fn committed_feed_snapshot(&self) -> Result<FeedSnapshot, String> {
        Err("unused".into())
    }

    async fn committed_feed_page(
        &self,
        _after_position: i64,
        _high_watermark: i64,
        _limit: usize,
    ) -> Result<Vec<CommittedFeedEntry>, String> {
        Err("unused".into())
    }

    async fn committed_feed_delivery_page(
        &self,
        _after_position: i64,
        _high_watermark: i64,
        _limit: usize,
    ) -> Result<CommittedFeedDeliveryPage, String> {
        Err("unused".into())
    }

    async fn committed_feed_delivery_page_for_peer(
        &self,
        _: &str,
        _: &PolicyIdentity,
        _: i64,
        _: i64,
        _: usize,
    ) -> Result<ark_application::sync_storage::PolicyBoundFeedPage, String> {
        Err("unused".into())
    }

    async fn begin_outbound_policy(
        &self,
        _peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<OutboundProgress, String> {
        self.begin(policy_generation)
    }

    async fn begin_outbound_policy_identity(
        &self,
        _peer_device_id: &str,
        identity: &PolicyIdentity,
    ) -> Result<OutboundProgress, String> {
        if identity != &self.identity {
            return Err("outbound_feed_policy_changed".into());
        }
        self.begin(identity.revision)
    }

    async fn complete_outbound_snapshot(
        &self,
        _peer_device_id: &str,
        _policy_generation: i64,
    ) -> Result<OutboundProgress, String> {
        Err("unused".into())
    }

    async fn complete_outbound_snapshot_identity(
        &self,
        _: &str,
        _: &PolicyIdentity,
    ) -> Result<OutboundProgress, String> {
        Err("unused".into())
    }

    async fn record_outbound_scan(
        &self,
        _peer_device_id: &str,
        _policy_generation: i64,
        _scanned_through: i64,
    ) -> Result<OutboundProgress, String> {
        Err("unused".into())
    }

    async fn record_outbound_scan_identity(
        &self,
        _: &str,
        _: &PolicyIdentity,
        _: i64,
    ) -> Result<OutboundProgress, String> {
        Err("unused".into())
    }

    async fn acknowledge_outbound(
        &self,
        _peer_device_id: &str,
        policy_generation: i64,
        position: i64,
        acknowledgement: OutboundAck,
    ) -> Result<OutboundProgress, String> {
        self.acknowledge(policy_generation, position, acknowledgement)
    }

    async fn acknowledge_outbound_identity(
        &self,
        _peer_device_id: &str,
        identity: &PolicyIdentity,
        position: i64,
        acknowledgement: OutboundAck,
    ) -> Result<OutboundProgress, String> {
        if identity != &self.identity {
            return Err("outbound_feed_policy_changed".into());
        }
        self.acknowledge(identity.revision, position, acknowledgement)
    }
}

fn identity(revision: i64) -> PolicyIdentity {
    PolicyIdentity {
        revision,
        digest: format!("{revision:064x}"),
        active: true,
    }
}

fn progress() -> OutboundProgress {
    OutboundProgress {
        policy_generation: 2,
        initial_snapshot: ark_application::sync_storage::InitialSnapshot::Complete { through: 4 },
        scanned_through: 4,
        receipt_through: 0,
        applied_through: 0,
    }
}

fn ack(change_id: &str, acknowledgement: OutboundAck, in_flight: Option<FeedFlight>) -> FeedAck {
    FeedAck {
        change_id: change_id.to_owned(),
        policy_identity: Some(identity(2)),
        acknowledgement,
        durable_receipt_ack: true,
        in_flight,
    }
}

#[tokio::test]
async fn invalid_change_id_is_ignored_without_storage_access() {
    let storage = FakeFeedStore::new(progress());
    let result = acknowledge(
        &storage,
        "peer",
        ack("not-a-feed-change", OutboundAck::Applied, None),
    )
    .await
    .unwrap();

    assert_eq!(result, FeedAckOutcome::Ignored);
    assert_eq!(*storage.begin_calls.lock().unwrap(), 0);
    assert!(storage.acknowledgements.lock().unwrap().is_empty());
}

#[tokio::test]
async fn stale_policy_identity_is_rejected_before_progress_changes() {
    let storage = FakeFeedStore::new(progress());
    let mut stale = ack(
        "ark-feed-v1:1",
        OutboundAck::Applied,
        Some(FeedFlight {
            position: 1,
            policy_identity: Some(identity(1)),
        }),
    );
    stale.policy_identity = Some(identity(1));

    let result = acknowledge(&storage, "peer", stale).await;

    assert_eq!(result.unwrap_err(), "outbound_feed_policy_changed");
    assert_eq!(storage.progress.lock().unwrap().applied_through, 0);
    assert!(storage.acknowledgements.lock().unwrap().is_empty());
}

#[tokio::test]
async fn acknowledgement_beyond_scanned_progress_is_rejected() {
    let storage = FakeFeedStore::new(progress());
    let mut beyond_scanned = ack(
        "ark-feed-v1:5",
        OutboundAck::Applied,
        Some(FeedFlight {
            position: 5,
            policy_identity: Some(identity(2)),
        }),
    );
    beyond_scanned.durable_receipt_ack = false;
    let result = acknowledge(&storage, "peer", beyond_scanned).await;

    assert_eq!(result.unwrap_err(), "invalid_outbound_feed_ack");
    assert_eq!(storage.progress.lock().unwrap().applied_through, 0);
    assert!(storage.acknowledgements.lock().unwrap().is_empty());
}

#[tokio::test]
async fn duplicate_applied_ack_is_idempotent_after_receipt() {
    let mut initial = progress();
    initial.receipt_through = 3;
    initial.applied_through = 3;
    let storage = FakeFeedStore::new(initial);
    let duplicate = ack("ark-feed-v1:3", OutboundAck::Applied, None);

    let first = acknowledge(&storage, "peer", duplicate.clone())
        .await
        .unwrap();
    let second = acknowledge(&storage, "peer", duplicate).await.unwrap();

    assert_eq!(
        first,
        FeedAckOutcome::Persisted {
            current_flight: false
        }
    );
    assert_eq!(
        second,
        FeedAckOutcome::Persisted {
            current_flight: false
        }
    );
    assert_eq!(storage.progress.lock().unwrap().applied_through, 3);
    assert_eq!(storage.acknowledgements.lock().unwrap().len(), 2);
}
