use super::*;
use crate::replication_storage::{PeerIdentityBackend, RegistryPageApply};

#[async_trait::async_trait]
impl PeerIdentityBackend for SqliteStorageBackend {
    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String> {
        Ok(load_progress_value(
            self,
            crate::application::sync_storage::PEER_TRANSPORT_KEYS_KEY,
        )
        .await?
        .unwrap_or_default())
    }

    async fn store_peer_transport_keys(
        &self,
        keys: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        store_progress_value(
            self,
            crate::application::sync_storage::PEER_TRANSPORT_KEYS_KEY,
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
            .submit(
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
    ) -> Result<
        (
            i64,
            usize,
            Vec<crate::domain::device_registry::RegistryEvent>,
        ),
        String,
    > {
        self.db
            .query(
                CommandContext::sync_system(CommandClass::SyncLive),
                crate::db::LoadRegistryEventsQuery::new(after_position, high_watermark, limit),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[crate::domain::device_registry::RegistryEvent],
    ) -> Result<RegistryPageApply, String> {
        self.db
            .submit(
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
        transcript: &crate::domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<crate::domain::device_registry::RegistryHandshakeProof, String> {
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
        transcript: &crate::domain::device_registry::RegistryHandshakeTranscript,
        proof: &crate::domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String> {
        self.db
            .query(
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
            .query(
                CommandContext::sync_peer(device_id.to_owned(), CommandClass::SyncLive),
                AuthorizedTransportKeyQuery::new(device_id.to_owned()),
            )
            .await
            .map_err(|error| error.to_string())
    }
}
