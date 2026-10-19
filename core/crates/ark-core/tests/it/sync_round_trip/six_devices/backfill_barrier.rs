use super::*;

pub(super) struct BackfillBarrier {
    pub(super) inner: Arc<SqliteStorageBackend>,
    pub(super) requested: Arc<Notify>,
    pub(super) release: watch::Receiver<bool>,
}

#[async_trait::async_trait]
impl SyncEntityReadPort for BackfillBarrier {
    async fn load_entities(&self, vector: &VersionVector) -> Result<Vec<SyncEntity>, String> {
        self.inner.load_entities(vector).await
    }

    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        self.requested.notify_one();
        let mut release = self.release.clone();
        while !*release.borrow() {
            if release.changed().await.is_err() {
                return Ok(Vec::new());
            }
        }
        self.inner.load_entities_page(vector, offset, limit).await
    }

    async fn usage_complete_through(&self) -> Result<HashMap<String, u64>, String> {
        self.inner.usage_complete_through().await
    }

    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
        self.inner.filter_outgoing_entity(entity)
    }
    async fn load_entities_page_for_peer(
        &self,
        peer: &str,
        class: ark_application::sync_storage::SyncReadClass,
        vector: &ark_core::VersionVector,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        self.requested.notify_one();
        let mut release = self.release.clone();
        while !*release.borrow() {
            if release.changed().await.is_err() {
                return Ok(Vec::new());
            }
        }
        self.inner
            .load_entities_page_for_peer(peer, class, vector, offset, limit)
            .await
    }
    async fn load_policy_bound_entities_page_for_peer(
        &self,
        peer: &str,
        class: ark_application::sync_storage::SyncReadClass,
        vector: &ark_core::VersionVector,
        offset: usize,
        limit: usize,
        expected_identity: &ark_application::sync_storage::PolicyIdentity,
    ) -> Result<
        ark_application::sync_storage::PolicyBoundSnapshotPage,
        ark_application::sync_storage::PolicyBoundSnapshotReadError,
    > {
        self.inner
            .load_policy_bound_entities_page_for_peer(
                peer,
                class,
                vector,
                offset,
                limit,
                expected_identity,
            )
            .await
    }
    async fn usage_complete_through_for_peer(
        &self,
        peer: &str,
        class: ark_application::sync_storage::SyncReadClass,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        self.inner
            .usage_complete_through_for_peer(peer, class)
            .await
    }
    async fn filter_outgoing_entity_for_peer(
        &self,
        peer: &str,
        entity: &SyncEntity,
    ) -> Result<Option<SyncEntity>, String> {
        self.inner
            .filter_outgoing_entity_for_peer(peer, entity)
            .await
    }
}
#[async_trait::async_trait]
impl SyncEntityMutationPort for BackfillBarrier {
    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
        self.inner.apply_entity(entity).await
    }
}

#[async_trait::async_trait]
impl PeerProgressPort for BackfillBarrier {
    async fn load_version_vector(&self) -> Result<ark_core::VersionVector, String> {
        self.inner.load_version_vector().await
    }
    async fn store_version_vector(&self, value: &ark_core::VersionVector) -> Result<(), String> {
        self.inner.store_version_vector(value).await
    }
    async fn store_version_vector_with_policy(
        &self,
        local_device_id: &str,
        policy_identity: &ark_application::sync_storage::PolicyIdentity,
        vector: &ark_core::VersionVector,
    ) -> Result<(), String> {
        self.inner
            .store_version_vector_with_policy(local_device_id, policy_identity, vector)
            .await
    }
    async fn load_known_peers(&self) -> Result<Option<Vec<ark_core::PeerRecord>>, String> {
        self.inner.load_known_peers().await
    }
    async fn store_known_peers(&self, value: &[ark_core::PeerRecord]) -> Result<(), String> {
        self.inner.store_known_peers(value).await
    }
    async fn load_removed_peer_ids(&self) -> Result<Vec<String>, String> {
        self.inner.load_removed_peer_ids().await
    }
    async fn store_removed_peer_ids(&self, value: &[String]) -> Result<(), String> {
        self.inner.store_removed_peer_ids(value).await
    }
}

#[async_trait::async_trait]
impl ark_application::sync_storage::CommittedFeedPort for BackfillBarrier {
    fn selective_sync_policy_generation(&self) -> i64 {
        self.inner.selective_sync_policy_generation()
    }

    async fn selective_sync_policy_generation_for_peer(&self, peer: &str) -> Result<i64, String> {
        self.inner
            .selective_sync_policy_generation_for_peer(peer)
            .await
    }

    async fn committed_feed_snapshot(
        &self,
    ) -> Result<ark_application::sync_storage::FeedSnapshot, String> {
        self.inner.committed_feed_snapshot().await
    }

    async fn committed_feed_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<Vec<ark_application::sync_storage::CommittedFeedEntry>, String> {
        self.inner
            .committed_feed_page(after_position, high_watermark, limit)
            .await
    }

    async fn committed_feed_delivery_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<ark_application::sync_storage::CommittedFeedDeliveryPage, String> {
        self.inner
            .committed_feed_delivery_page(after_position, high_watermark, limit)
            .await
    }

    async fn committed_feed_delivery_page_for_peer(
        &self,
        peer: &str,
        identity: &ark_application::sync_storage::PolicyIdentity,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<
        ark_application::sync_storage::PolicyBoundFeedPage,
        ark_application::sync_storage::PolicyBoundFeedReadError,
    > {
        self.inner
            .committed_feed_delivery_page_for_peer(
                peer,
                identity,
                after_position,
                high_watermark,
                limit,
            )
            .await
    }

    async fn begin_outbound_policy(
        &self,
        peer: &str,
        generation: i64,
    ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
        self.inner.begin_outbound_policy(peer, generation).await
    }
    async fn complete_outbound_snapshot(
        &self,
        peer: &str,
        generation: i64,
    ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
        self.inner
            .complete_outbound_snapshot(peer, generation)
            .await
    }
    async fn begin_outbound_policy_identity(
        &self,
        peer: &str,
        identity: &ark_application::sync_storage::PolicyIdentity,
    ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
        self.inner
            .begin_outbound_policy_identity(peer, identity)
            .await
    }
    async fn complete_outbound_snapshot_identity(
        &self,
        peer: &str,
        identity: &ark_application::sync_storage::PolicyIdentity,
    ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
        self.inner
            .complete_outbound_snapshot_identity(peer, identity)
            .await
    }
    async fn record_outbound_scan(
        &self,
        peer: &str,
        generation: i64,
        through: i64,
    ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
        self.inner
            .record_outbound_scan(peer, generation, through)
            .await
    }
    async fn record_outbound_scan_identity(
        &self,
        peer: &str,
        identity: &ark_application::sync_storage::PolicyIdentity,
        through: i64,
    ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
        self.inner
            .record_outbound_scan_identity(peer, identity, through)
            .await
    }
    async fn acknowledge_outbound(
        &self,
        peer: &str,
        generation: i64,
        position: i64,
        ack: ark_application::sync_storage::OutboundAck,
    ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
        self.inner
            .acknowledge_outbound(peer, generation, position, ack)
            .await
    }
    async fn acknowledge_outbound_identity(
        &self,
        peer: &str,
        identity: &ark_application::sync_storage::PolicyIdentity,
        position: i64,
        ack: ark_application::sync_storage::OutboundAck,
    ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
        self.inner
            .acknowledge_outbound_identity(peer, identity, position, ack)
            .await
    }
}

#[async_trait::async_trait]
impl IntegrationReplicationBackend for BackfillBarrier {
    async fn validate_outbound_signed_integration_frame(
        &self,
        frame: &ark_core::integration_replication::SignedSyncEnvelope,
        space: &str,
        origin: &str,
    ) -> Result<(), String> {
        self.inner
            .validate_outbound_signed_integration_frame(frame, space, origin)
            .await
    }

    async fn apply_signed_integration_frame(
        &self,
        frame: &ark_core::integration_replication::SignedSyncEnvelope,
        space: &str,
        peer: &str,
        recipient: &str,
    ) -> Result<(), String> {
        self.inner
            .apply_signed_integration_frame(frame, space, peer, recipient)
            .await
    }
}

#[async_trait::async_trait]
impl PeerIdentityBackend for BackfillBarrier {
    async fn load_local_sync_profile(
        &self,
        local_device_id: &str,
    ) -> Result<Option<ark_application::sync_policy::SyncProfile>, String> {
        self.inner.load_local_sync_profile(local_device_id).await
    }

    async fn accept_peer_sync_policy(
        &self,
        peer_device_id: &str,
        revision: i64,
        digest: &str,
        active: bool,
        rules: &[ark_application::sync_policy::SyncProfileRule],
    ) -> Result<ark_application::sync_policy::AcceptedPeerSyncPolicy, String> {
        self.inner
            .accept_peer_sync_policy(peer_device_id, revision, digest, active, rules)
            .await
    }

    async fn load_accepted_peer_sync_policy(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<ark_application::sync_policy::AcceptedPeerSyncPolicy>, String> {
        self.inner
            .load_accepted_peer_sync_policy(peer_device_id)
            .await
    }

    async fn sync_session_is_admitted(&self, peer_device_id: &str) -> Result<bool, String> {
        self.inner.sync_session_is_admitted(peer_device_id).await
    }

    async fn authorized_transport_public_key(
        &self,
        device_id: &str,
    ) -> Result<Option<String>, String> {
        self.inner.authorized_transport_public_key(device_id).await
    }

    async fn registry_device_is_enabled(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<bool>, String> {
        self.inner.registry_device_is_enabled(peer_device_id).await
    }

    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String> {
        self.inner.load_peer_transport_keys().await
    }
    async fn store_peer_transport_keys(
        &self,
        keys: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        self.inner.store_peer_transport_keys(keys).await
    }

    async fn enroll_paired_device(
        &self,
        local_device_id: &str,
        target_device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        self.inner
            .enroll_paired_device(local_device_id, target_device_id, transport_public_key)
            .await
    }

    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<(i64, usize, Vec<ark_domain::device_registry::RegistryEvent>), String> {
        self.inner
            .load_device_registry_events_page(after_position, high_watermark, limit)
            .await
    }

    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[ark_domain::device_registry::RegistryEvent],
    ) -> Result<ark_core::replication_storage::RegistryPageApply, String> {
        self.inner
            .apply_device_registry_events(sender_device_id, events)
            .await
    }

    async fn sign_registry_handshake(
        &self,
        local_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<ark_domain::device_registry::RegistryHandshakeProof, String> {
        self.inner
            .sign_registry_handshake(local_device_id, transcript)
            .await
    }

    async fn verify_registry_handshake(
        &self,
        peer_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
        proof: &ark_domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String> {
        self.inner
            .verify_registry_handshake(peer_device_id, transcript, proof)
            .await
    }
}
