use super::*;

#[async_trait::async_trait]
impl PeerSyncPolicyPort for SqliteStorageBackend {
    async fn accept_peer_sync_policy(
        &self,
        peer_device_id: &str,
        revision: i64,
        digest: &str,
        rules: &[crate::data_platform::SyncProfileRule],
    ) -> Result<crate::data_platform::AcceptedPeerSyncPolicy, String> {
        self.db
            .submit(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::AcceptPeerSyncPolicyCommand::new(
                    peer_device_id.to_owned(),
                    revision,
                    digest.to_owned(),
                    rules.to_vec(),
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn load_accepted_peer_sync_policy(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<crate::data_platform::AcceptedPeerSyncPolicy>, String> {
        self.db
            .query(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::LoadAcceptedPeerSyncPolicyQuery::new(peer_device_id.to_owned()),
            )
            .await
            .map_err(|error| error.to_string())
    }
}
