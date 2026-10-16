use super::apply::is_full_core_task;
use super::SqliteStorageBackend;
use crate::application::sync_storage::{
    PeerProgressPort, SyncEntityMutationPort, SyncEntityReadPort, SyncReadClass,
};
use crate::db::{
    ApplySignedIntegrationFrameCommand, AuthenticatedCoreTaskWrite, AuthorizedTransportKeyQuery,
    CommandClass, CommandContext, GetSyncKeyValueQuery, IdentityErrorStyle, LoadEntitiesQuery,
    UsageCompleteThroughQuery, ValidateOutboundSignedFrameQuery,
};
use crate::replication_storage::{
    AuthenticatedEntityBackend, IntegrationReplicationBackend, PeerIdentityBackend,
};
use crate::types::{SyncEntity, VersionVector};
use serde::de::DeserializeOwned;
use serde::Serialize;

fn publish_replay_notices(notices: Vec<crate::canonical_types::pending::ReplayNotice>) {
    for notice in notices {
        crate::events::emit_event(notice.event_payload());
    }
}

async fn load_progress_value<T: DeserializeOwned>(
    storage: &SqliteStorageBackend,
    key: &str,
) -> Result<Option<T>, String> {
    let raw = storage
        .db
        .query(
            CommandContext::sync_system(CommandClass::SyncLive),
            GetSyncKeyValueQuery::new(key.to_owned()),
        )
        .await
        .map_err(|error| error.to_string())?;
    raw.map(|raw| {
        serde_json::from_str(&raw)
            .map_err(|error| format!("invalid persisted sync progress at {key}: {error}"))
    })
    .transpose()
}

async fn store_progress_value<T: Serialize + ?Sized>(
    storage: &SqliteStorageBackend,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let raw = serde_json::to_string(value).map_err(|error| error.to_string())?;
    storage
        .db
        .submit(
            CommandContext::sync_system(CommandClass::SyncLive),
            crate::db::SetSyncKeyValueCommand::new(key.to_owned(), raw),
        )
        .await
        .map_err(|error| error.to_string())
}

impl SqliteStorageBackend {
    pub fn set_selective_sync_profile(
        &self,
        profile: Option<crate::data_platform::SelectiveSyncProfile>,
    ) {
        *self
            .selective_profile
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = profile;
    }

    fn selective_sync_profile(&self) -> Option<crate::data_platform::SelectiveSyncProfile> {
        self.selective_profile
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

#[async_trait::async_trait]
#[async_trait::async_trait]
impl SyncEntityReadPort for SqliteStorageBackend {
    async fn load_entities(&self, vector: &VersionVector) -> Result<Vec<SyncEntity>, String> {
        self.db
            .query(
                CommandContext::sync_system(CommandClass::SyncLive),
                LoadEntitiesQuery::all(
                    vector.clone(),
                    self.device_id(),
                    self.selective_sync_profile(),
                    SyncReadClass::SyncLive,
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        let class = SyncReadClass::for_peer_vector(vector);
        self.db
            .query(
                CommandContext::sync_system(class.into()),
                LoadEntitiesQuery::page(
                    vector.clone(),
                    self.device_id(),
                    self.selective_sync_profile(),
                    class,
                    offset,
                    limit,
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn load_entities_page_for_peer(
        &self,
        peer_device_id: &str,
        class: SyncReadClass,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        self.db
            .query(
                CommandContext::sync_peer(peer_device_id.to_owned(), class.into()),
                LoadEntitiesQuery::page(
                    vector.clone(),
                    self.device_id(),
                    self.selective_sync_profile(),
                    class,
                    offset,
                    limit,
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn usage_complete_through(
        &self,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        self.db
            .query(
                CommandContext::sync_system(CommandClass::SyncLive),
                UsageCompleteThroughQuery::new(SyncReadClass::SyncLive),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn usage_complete_through_for_peer(
        &self,
        peer_device_id: &str,
        class: SyncReadClass,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        self.db
            .query(
                CommandContext::sync_peer(peer_device_id.to_owned(), class.into()),
                UsageCompleteThroughQuery::new(class),
            )
            .await
            .map_err(|error| error.to_string())
    }

    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
        let Some(profile) = self.selective_sync_profile() else {
            return Some(entity.clone());
        };
        crate::data_platform::filter_entities_for_profile(&profile, std::slice::from_ref(entity))
            .into_iter()
            .next()
    }
}

include!("adapter/committed_feed.rs");

#[async_trait::async_trait]
impl SyncEntityMutationPort for SqliteStorageBackend {
    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
        let notices = self
            .db
            .submit(
                CommandContext::sync_system(CommandClass::SyncLive),
                crate::db::ApplySyncEntityCommand::new(entity.clone()),
            )
            .await
            .map_err(|error| error.to_string())?;
        publish_replay_notices(notices);
        Ok(())
    }
}
#[async_trait::async_trait]
impl PeerProgressPort for SqliteStorageBackend {
    async fn load_version_vector(&self) -> Result<VersionVector, String> {
        Ok(
            load_progress_value(self, crate::application::sync_storage::VERSION_VECTOR_KEY)
                .await?
                .unwrap_or_default(),
        )
    }

    async fn store_version_vector(&self, vector: &VersionVector) -> Result<(), String> {
        store_progress_value(
            self,
            crate::application::sync_storage::VERSION_VECTOR_KEY,
            vector,
        )
        .await
    }

    async fn load_known_peers(&self) -> Result<Option<Vec<crate::types::PeerRecord>>, String> {
        load_progress_value(self, crate::application::sync_storage::KNOWN_PEERS_KEY).await
    }

    async fn store_known_peers(&self, peers: &[crate::types::PeerRecord]) -> Result<(), String> {
        store_progress_value(
            self,
            crate::application::sync_storage::KNOWN_PEERS_KEY,
            peers,
        )
        .await
    }

    async fn load_removed_peer_ids(&self) -> Result<Vec<String>, String> {
        Ok(
            load_progress_value(self, crate::application::sync_storage::REMOVED_PEERS_KEY)
                .await?
                .unwrap_or_default(),
        )
    }

    async fn store_removed_peer_ids(&self, peer_ids: &[String]) -> Result<(), String> {
        store_progress_value(
            self,
            crate::application::sync_storage::REMOVED_PEERS_KEY,
            peer_ids,
        )
        .await
    }
}

// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl AuthenticatedEntityBackend for SqliteStorageBackend {
    async fn apply_authenticated_entity(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
    ) -> Result<(), String> {
        let inbox_message_id = crate::db::entity_inbox_message_id(
            authenticated_peer_id,
            &entity.entity_type,
            &entity.id,
            &entity.hlc,
        );
        self.apply_authenticated_entity_with_id(entity, authenticated_peer_id, &inbox_message_id)
            .await
    }

    async fn apply_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        inbox_message_id: &str,
    ) -> Result<(), String> {
        let inbox_payload_json =
            serde_json::to_string(entity).map_err(|error| error.to_string())?;
        self.db
            .submit(
                CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                crate::db::ReceiveInboxReceiptCommand::new(
                    authenticated_peer_id.to_owned(),
                    inbox_message_id.to_owned(),
                    inbox_payload_json.clone(),
                ),
            )
            .await
            .map_err(|error| error.to_string())?;
        if is_full_core_task(entity) {
            let command = AuthenticatedCoreTaskWrite::new(
                entity.id.clone(),
                entity.hlc.clone(),
                entity.origin_device_id.clone(),
                entity.data.clone(),
                inbox_message_id.to_owned(),
                inbox_payload_json,
            );
            return self
                .db
                .submit(
                    CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                    command,
                )
                .await
                .map(|_| ())
                .map_err(|error| error.to_string());
        }
        let notices = self
            .db
            .submit(
                CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                crate::db::ApplyAuthenticatedSyncEntityCommand::with_message_id(
                    entity.clone(),
                    inbox_message_id.to_owned(),
                ),
            )
            .await
            .map_err(|error| error.to_string())?;
        publish_replay_notices(notices);
        Ok(())
    }
}

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

#[async_trait::async_trait]
impl IntegrationReplicationBackend for SqliteStorageBackend {
    async fn validate_outbound_signed_integration_frame(
        &self,
        frame: &crate::integration_replication::SignedSyncEnvelope,
        expected_space_id: &str,
        expected_origin_node_id: &str,
    ) -> Result<(), String> {
        self.db
            .query(
                CommandContext::sync_system(CommandClass::SyncLive),
                ValidateOutboundSignedFrameQuery::Authorized {
                    frame: frame.clone(),
                    expected_space_id: expected_space_id.to_owned(),
                    expected_origin_node_id: expected_origin_node_id.to_owned(),
                },
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn validate_outbound_signed_integration_frame_with_transport(
        &self,
        frame: &crate::integration_replication::SignedSyncEnvelope,
        expected_space_id: &str,
        expected_origin_node_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        self.db
            .query(
                CommandContext::sync_system(CommandClass::SyncLive),
                ValidateOutboundSignedFrameQuery::BoundTransport {
                    frame: frame.clone(),
                    expected_space_id: expected_space_id.to_owned(),
                    expected_origin_node_id: expected_origin_node_id.to_owned(),
                    transport_public_key: transport_public_key.to_owned(),
                },
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn apply_signed_integration_frame(
        &self,
        frame: &crate::integration_replication::SignedSyncEnvelope,
        expected_space_id: &str,
        authenticated_peer_id: &str,
        expected_recipient_node_id: &str,
    ) -> Result<(), String> {
        if frame.space_id != expected_space_id {
            return Err("signed integration frame has the wrong space".into());
        }
        if frame.origin_node_id != authenticated_peer_id {
            return Err("signed integration frame origin is not the authenticated peer".into());
        }
        if frame.recipient_node_id != expected_recipient_node_id {
            return Err("signed integration frame is addressed to another node".into());
        }

        self.db
            .submit(
                CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                ApplySignedIntegrationFrameCommand::new(
                    frame.clone(),
                    expected_space_id.to_owned(),
                    expected_recipient_node_id.to_owned(),
                    None,
                    IdentityErrorStyle::Detailed,
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn apply_signed_integration_frame_with_transport(
        &self,
        frame: &crate::integration_replication::SignedSyncEnvelope,
        expected_space_id: &str,
        authenticated_peer_id: &str,
        expected_recipient_node_id: &str,
        authenticated_transport_public_key: Option<&str>,
    ) -> Result<(), String> {
        if frame.space_id != expected_space_id
            || frame.origin_node_id != authenticated_peer_id
            || frame.recipient_node_id != expected_recipient_node_id
        {
            return Err("signed integration frame identity mismatch".into());
        }
        self.db
            .submit(
                CommandContext::authenticated_peer(authenticated_peer_id.to_owned()),
                ApplySignedIntegrationFrameCommand::new(
                    frame.clone(),
                    expected_space_id.to_owned(),
                    expected_recipient_node_id.to_owned(),
                    authenticated_transport_public_key.map(str::to_owned),
                    IdentityErrorStyle::Generic,
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }
}
