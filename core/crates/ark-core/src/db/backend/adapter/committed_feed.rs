#[async_trait::async_trait]
impl crate::application::sync_storage::CommittedFeedPort for SqliteStorageBackend {
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
                crate::data_platform::SyncMode::Full => 1,
                crate::data_platform::SyncMode::Metadata => 2,
                crate::data_platform::SyncMode::None => 3,
            }]);
            digest.update(rule.filter.digest());
        }
        let bytes = digest.finalize();
        i64::from_be_bytes(bytes[..8].try_into().expect("sha256 prefix is eight bytes")) & i64::MAX
    }

    async fn committed_feed_snapshot(
        &self,
    ) -> Result<crate::application::sync_storage::FeedSnapshot, String> {
        self.db
            .query(
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
    ) -> Result<Vec<crate::application::sync_storage::CommittedFeedEntry>, String> {
        self.db
            .query(
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
    ) -> Result<crate::application::sync_storage::CommittedFeedDeliveryPage, String> {
        self.db
            .query(
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

    async fn begin_outbound_policy(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<crate::application::sync_storage::OutboundProgress, String> {
        match self
            .db
            .submit(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(
                    crate::db::CommittedFeedProgressOperation::BeginOutboundPolicy {
                        peer_device_id: peer_device_id.to_owned(),
                        policy_generation,
                    },
                ),
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
    ) -> Result<crate::application::sync_storage::OutboundProgress, String> {
        match self
            .db
            .submit(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(
                    crate::db::CommittedFeedProgressOperation::CompleteOutboundSnapshot {
                        peer_device_id: peer_device_id.to_owned(),
                        policy_generation,
                    },
                ),
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
    ) -> Result<crate::application::sync_storage::OutboundProgress, String> {
        match self
            .db
            .submit(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(
                    crate::db::CommittedFeedProgressOperation::RecordOutboundScan {
                        peer_device_id: peer_device_id.to_owned(),
                        policy_generation,
                        scanned_through,
                    },
                ),
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
        ack: crate::application::sync_storage::OutboundAck,
    ) -> Result<crate::application::sync_storage::OutboundProgress, String> {
        match self
            .db
            .submit(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::CommittedFeedProgressCommand::new(
                    crate::db::CommittedFeedProgressOperation::AcknowledgeOutbound {
                        peer_device_id: peer_device_id.to_owned(),
                        policy_generation,
                        position,
                        ack,
                    },
                ),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            crate::db::CommittedFeedProgress::Outbound(progress) => Ok(progress),
        }
    }
}
