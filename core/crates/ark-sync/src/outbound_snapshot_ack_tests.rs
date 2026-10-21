use crate::outbound_snapshot::{acknowledge_snapshot, PendingSnapshotAck};
use ark_application::sync_storage::{
    CommittedFeedDeliveryPage, CommittedFeedEntry, CommittedFeedPort, OutboundAck,
    OutboundProgress, PolicyIdentity,
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct Storage {
    completions: AtomicUsize,
}

#[async_trait::async_trait]
impl CommittedFeedPort for Storage {
    fn selective_sync_policy_generation(&self) -> i64 {
        1
    }

    async fn selective_sync_policy_generation_for_peer(&self, _: &str) -> Result<i64, String> {
        Err("unexpected per-peer generation read".into())
    }

    async fn committed_feed_snapshot(
        &self,
    ) -> Result<ark_application::sync_storage::FeedSnapshot, String> {
        Err("unexpected feed snapshot read".into())
    }

    async fn committed_feed_page(
        &self,
        _: i64,
        _: i64,
        _: usize,
    ) -> Result<Vec<CommittedFeedEntry>, String> {
        Err("unexpected feed page read".into())
    }

    async fn committed_feed_delivery_page(
        &self,
        _: i64,
        _: i64,
        _: usize,
    ) -> Result<CommittedFeedDeliveryPage, String> {
        Err("unexpected feed delivery read".into())
    }

    async fn committed_feed_delivery_page_for_peer(
        &self,
        _: &str,
        _: &PolicyIdentity,
        _: i64,
        _: i64,
        _: usize,
    ) -> Result<
        ark_application::sync_storage::PolicyBoundFeedPage,
        ark_application::sync_storage::PolicyBoundFeedReadError,
    > {
        Err("unexpected policy-bound feed read".into())
    }

    async fn begin_outbound_policy(&self, _: &str, _: i64) -> Result<OutboundProgress, String> {
        Err("unexpected outbound progress begin".into())
    }

    async fn begin_outbound_policy_identity(
        &self,
        _: &str,
        _: &PolicyIdentity,
    ) -> Result<OutboundProgress, String> {
        Err("unexpected policy-bound outbound progress begin".into())
    }

    async fn complete_outbound_snapshot(
        &self,
        _: &str,
        _: i64,
    ) -> Result<OutboundProgress, String> {
        self.completions.fetch_add(1, Ordering::SeqCst);
        Ok(OutboundProgress::default())
    }

    async fn complete_outbound_snapshot_identity(
        &self,
        _: &str,
        _: &PolicyIdentity,
    ) -> Result<OutboundProgress, String> {
        self.completions.fetch_add(1, Ordering::SeqCst);
        Ok(OutboundProgress::default())
    }

    async fn record_outbound_scan(
        &self,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<OutboundProgress, String> {
        Err("unexpected outbound scan".into())
    }

    async fn record_outbound_scan_identity(
        &self,
        _: &str,
        _: &PolicyIdentity,
        _: i64,
    ) -> Result<OutboundProgress, String> {
        Err("unexpected policy-bound outbound scan".into())
    }

    async fn acknowledge_outbound(
        &self,
        _: &str,
        _: i64,
        _: i64,
        _: OutboundAck,
    ) -> Result<OutboundProgress, String> {
        Err("unexpected outbound acknowledgment".into())
    }

    async fn acknowledge_outbound_identity(
        &self,
        _: &str,
        _: &PolicyIdentity,
        _: i64,
        _: OutboundAck,
    ) -> Result<OutboundProgress, String> {
        Err("unexpected policy-bound outbound acknowledgment".into())
    }
}

#[tokio::test]
async fn stale_snapshot_ack_does_not_complete_storage_progress() {
    let storage = Storage::default();
    let expected = PolicyIdentity::new(2, "a".repeat(64), true).unwrap();
    let stale = PolicyIdentity::new(1, "b".repeat(64), true).unwrap();
    let pending = PendingSnapshotAck {
        batch_id: "batch-2".into(),
        policy_identity: Some(expected),
    };

    assert!(
        !acknowledge_snapshot(&storage, "peer", &pending, "batch-2", Some(&stale))
            .await
            .unwrap()
    );
    assert!(!acknowledge_snapshot(
        &storage,
        "peer",
        &pending,
        "stale-batch",
        pending.policy_identity.as_ref(),
    )
    .await
    .unwrap());
    assert_eq!(storage.completions.load(Ordering::SeqCst), 0);
}
