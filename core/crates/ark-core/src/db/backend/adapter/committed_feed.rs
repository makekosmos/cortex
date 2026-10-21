use ark_application::sync_storage::{
    OutboundAck, OutboundProgress, OutboundProgressMutation as ProgressMutation,
    OutboundProgressPolicy, OutboundProgressWrite,
};
use ark_domain::sync_policy::PolicyIdentity;

#[async_trait::async_trait]
impl ark_application::sync_storage::CommittedFeedPort for SqliteStorageBackend {
    fn selective_sync_policy_generation(&self) -> i64 {
        let Some(profile) = self.selective_sync_profile() else {
            return 0;
        };
        let mut rules = profile.rules.into_iter().collect::<Vec<_>>();
        rules.sort_by(|left, right| left.0.cmp(&right.0));
        let mut digest = sha2::Sha256::new();
        use sha2::Digest;
        for ((kind, id), rule) in rules {
            digest.update((kind.len() as u64).to_be_bytes());
            digest.update(kind.as_bytes());
            digest.update((id.len() as u64).to_be_bytes());
            digest.update(id.as_bytes());
            digest.update([match rule.mode {
                ark_domain::selective_sync::SyncMode::Full => 1,
                ark_domain::selective_sync::SyncMode::Metadata => 2,
                ark_domain::selective_sync::SyncMode::None => 3,
            }]);
            digest.update(rule.filter.digest());
        }
        let bytes = digest.finalize();
        i64::from_be_bytes(bytes[..8].try_into().expect("sha256 prefix is eight bytes")) & i64::MAX
    }

    async fn selective_sync_policy_generation_for_peer(
        &self,
        peer_device_id: &str,
    ) -> Result<i64, String> {
        Ok(self
            .load_accepted_peer_sync_policy(peer_device_id)
            .await?
            .map_or(0, |policy| policy.revision))
    }

    async fn committed_feed_snapshot(
        &self,
    ) -> Result<ark_application::sync_storage::FeedSnapshot, String> {
        self.db
            .query_with_store(
                CommandContext::sync_system(CommandClass::SyncLive),
                crate::db::CommittedFeedSnapshotQuery,
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn committed_feed_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<Vec<ark_application::sync_storage::CommittedFeedEntry>, String> {
        self.db
            .query_with_store(
                CommandContext::sync_system(CommandClass::SyncLive),
                crate::db::CommittedFeedPageQuery::new(after_position, high_watermark, limit),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn committed_feed_delivery_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<ark_application::sync_storage::CommittedFeedDeliveryPage, String> {
        self.db
            .query_with_store(
                CommandContext::sync_system(CommandClass::SyncLive),
                crate::db::CommittedFeedDeliveryPageQuery::new(
                    after_position,
                    high_watermark,
                    limit,
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn committed_feed_delivery_page_for_peer(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<
        ark_application::sync_storage::PolicyBoundFeedPage,
        ark_application::sync_storage::PolicyBoundFeedReadError,
    > {
        self.db
            .query_with_store(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::PolicyBoundFeedDeliveryPageQuery::new(
                    peer_device_id.to_owned(),
                    expected_identity.clone(),
                    after_position,
                    high_watermark,
                    limit,
                ),
            )
            .await
            .map_err(|error| match error {
                crate::db::DbError::StaleSyncPolicy => {
                    ark_application::sync_storage::PolicyBoundFeedReadError::StalePolicy
                }
                error => ark_application::sync_storage::PolicyBoundFeedReadError::Storage(
                    error.to_string(),
                ),
            })
    }

    async fn begin_outbound_policy(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<OutboundProgress, String> {
        match self
            .db
            .apply_committed_feed_progress(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(OutboundProgressWrite::new(
                    peer_device_id.to_owned(),
                    OutboundProgressPolicy::Generation(policy_generation),
                    ProgressMutation::Begin,
                )),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            crate::db::CommittedFeedProgress::Outbound(progress) => Ok(progress),
        }
    }

    async fn begin_outbound_policy_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
    ) -> Result<OutboundProgress, String> {
        match self
            .db
            .apply_committed_feed_progress(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(OutboundProgressWrite::new(
                    peer_device_id.to_owned(),
                    OutboundProgressPolicy::Identity(identity.clone()),
                    ProgressMutation::Begin,
                )),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            crate::db::CommittedFeedProgress::Outbound(progress) => Ok(progress),
        }
    }

    async fn complete_outbound_snapshot(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<OutboundProgress, String> {
        match self
            .db
            .apply_committed_feed_progress(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(OutboundProgressWrite::new(
                    peer_device_id.to_owned(),
                    OutboundProgressPolicy::Generation(policy_generation),
                    ProgressMutation::CompleteSnapshot,
                )),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            crate::db::CommittedFeedProgress::Outbound(progress) => Ok(progress),
        }
    }

    async fn complete_outbound_snapshot_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
    ) -> Result<OutboundProgress, String> {
        match self
            .db
            .apply_committed_feed_progress(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(OutboundProgressWrite::new(
                    peer_device_id.to_owned(),
                    OutboundProgressPolicy::Identity(identity.clone()),
                    ProgressMutation::CompleteSnapshot,
                )),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            crate::db::CommittedFeedProgress::Outbound(progress) => Ok(progress),
        }
    }

    async fn record_outbound_scan(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
        scanned_through: i64,
    ) -> Result<OutboundProgress, String> {
        match self
            .db
            .apply_committed_feed_progress(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(OutboundProgressWrite::new(
                    peer_device_id.to_owned(),
                    OutboundProgressPolicy::Generation(policy_generation),
                    ProgressMutation::RecordScan { scanned_through },
                )),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            crate::db::CommittedFeedProgress::Outbound(progress) => Ok(progress),
        }
    }

    async fn record_outbound_scan_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
        scanned_through: i64,
    ) -> Result<OutboundProgress, String> {
        match self
            .db
            .apply_committed_feed_progress(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(OutboundProgressWrite::new(
                    peer_device_id.to_owned(),
                    OutboundProgressPolicy::Identity(identity.clone()),
                    ProgressMutation::RecordScan { scanned_through },
                )),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            crate::db::CommittedFeedProgress::Outbound(progress) => Ok(progress),
        }
    }

    async fn acknowledge_outbound(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
        position: i64,
        ack: OutboundAck,
    ) -> Result<OutboundProgress, String> {
        match self
            .db
            .apply_committed_feed_progress(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(OutboundProgressWrite::new(
                    peer_device_id.to_owned(),
                    OutboundProgressPolicy::Generation(policy_generation),
                    ProgressMutation::Acknowledge { position, ack },
                )),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            crate::db::CommittedFeedProgress::Outbound(progress) => Ok(progress),
        }
    }

    async fn acknowledge_outbound_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
        position: i64,
        ack: OutboundAck,
    ) -> Result<OutboundProgress, String> {
        match self
            .db
            .apply_committed_feed_progress(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(OutboundProgressWrite::new(
                    peer_device_id.to_owned(),
                    OutboundProgressPolicy::Identity(identity.clone()),
                    ProgressMutation::Acknowledge { position, ack },
                )),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            crate::db::CommittedFeedProgress::Outbound(progress) => Ok(progress),
        }
    }
}
