use super::*;
use crate::replication_storage::{PeerIdentityBackend, RegistryPageApply};
use ark_application::sync_storage::PeerSyncPolicyPort;

#[async_trait::async_trait]
impl PeerSyncPolicyPort for SqliteStorageBackend {
    async fn accepted_policy_identity_for_peer(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<ark_application::sync_storage::PolicyIdentity>, String> {
        Ok(Some(
            self.load_accepted_peer_sync_policy(peer_device_id)
                .await?
                .map_or_else(
                    ark_application::sync_policy::default_sync_policy_identity,
                    |policy| policy.identity(),
                ),
        ))
    }
}

#[async_trait::async_trait]
impl PeerIdentityBackend for SqliteStorageBackend {
    async fn load_local_sync_profile(
        &self,
        local_device_id: &str,
    ) -> Result<Option<ark_application::sync_policy::SyncProfile>, String> {
        self.db
            .query_with_store(
                CommandContext::sync_system(CommandClass::SyncLive),
                crate::db::LoadSyncProfileForSyncQuery::new(local_device_id.to_owned()),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn accept_peer_sync_policy(
        &self,
        peer_device_id: &str,
        revision: i64,
        digest: &str,
        active: bool,
        rules: &[ark_application::sync_policy::SyncProfileRule],
    ) -> Result<ark_application::sync_policy::AcceptedPeerSyncPolicy, String> {
        self.db
            .accept_peer_sync_policy(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::AcceptPeerSyncPolicyCommand::new(
                    peer_device_id.to_owned(),
                    revision,
                    digest.to_owned(),
                    active,
                    rules.to_vec(),
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn load_accepted_peer_sync_policy(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<ark_application::sync_policy::AcceptedPeerSyncPolicy>, String> {
        self.db
            .query_with_store(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::LoadAcceptedPeerSyncPolicyQuery::new(peer_device_id.to_owned()),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn sync_session_is_admitted(&self, peer_device_id: &str) -> Result<bool, String> {
        self.db
            .query_with_store(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::RegistryPeerAdmissionQuery::new(peer_device_id.to_owned()),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn registry_device_is_enabled(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<bool>, String> {
        self.db
            .query_with_store(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::RegistryDeviceEnabledQuery::new(peer_device_id.to_owned()),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String> {
        Ok(
            load_progress_value(self, ark_store::device_registry::PEER_TRANSPORT_KEYS_KEY)
                .await?
                .unwrap_or_default(),
        )
    }

    async fn store_peer_transport_keys(
        &self,
        keys: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        store_progress_value(
            self,
            ark_store::device_registry::PEER_TRANSPORT_KEYS_KEY,
            keys,
        )
        .await
    }

    async fn enroll_paired_device(
        &self,
        local_device_id: &str,
        target_device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        self.db
            .enroll_paired_device(
                CommandContext::engine(),
                crate::db::EnrollPairedDeviceCommand::new(
                    local_device_id.to_owned(),
                    target_device_id.to_owned(),
                    transport_public_key.to_owned(),
                ),
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<(i64, usize, Vec<ark_domain::device_registry::RegistryEvent>), String> {
        self.db
            .query_with_store(
                CommandContext::sync_system(CommandClass::SyncLive),
                crate::db::LoadRegistryEventsQuery::new(after_position, high_watermark, limit),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[ark_domain::device_registry::RegistryEvent],
    ) -> Result<RegistryPageApply, String> {
        self.db
            .apply_registry_events(
                CommandContext::sync_peer(sender_device_id.to_owned(), CommandClass::SyncLive),
                crate::db::ApplyRegistryEventsCommand::new(
                    sender_device_id.to_owned(),
                    events.to_vec(),
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn sign_registry_handshake(
        &self,
        local_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<ark_domain::device_registry::RegistryHandshakeProof, String> {
        self.db
            .submit(
                CommandContext::engine(),
                crate::db::SignRegistryHandshakeCommand::new(
                    local_device_id.to_owned(),
                    transcript.clone(),
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn verify_registry_handshake(
        &self,
        peer_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
        proof: &ark_domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String> {
        self.db
            .query_with_store(
                CommandContext::sync_system(CommandClass::SyncLive),
                crate::db::VerifyRegistryHandshakeQuery::new(
                    peer_device_id.to_owned(),
                    transcript.clone(),
                    proof.clone(),
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn authorized_transport_public_key(
        &self,
        device_id: &str,
    ) -> Result<Option<String>, String> {
        self.db
            .query_with_store(
                CommandContext::sync_peer(device_id.to_owned(), CommandClass::SyncLive),
                AuthorizedTransportKeyQuery::new(device_id.to_owned()),
            )
            .await
            .map_err(|error| error.to_string())
    }
}
