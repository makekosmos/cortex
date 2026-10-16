use super::apply::is_full_core_task;
use super::SqliteStorageBackend;
use crate::db::{
    apply_signed_integration_changes, AuthenticatedCoreTaskWrite, AuthorizedTransportKeyQuery,
    CommandClass, CommandContext, GetSyncKeyValueQuery, LoadEntitiesQuery,
    UsageCompleteThroughQuery,
};
use crate::replication_storage::{
    AuthenticatedEntityBackend, IntegrationReplicationBackend, PeerIdentityBackend, StorageBackend,
    SyncReadClass,
};
use crate::types::{SyncEntity, VersionVector};

fn publish_replay_notices(notices: Vec<crate::canonical_types::pending::ReplayNotice>) {
    for notice in notices {
        crate::events::emit_event(notice.event_payload());
    }
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
impl StorageBackend for SqliteStorageBackend {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity> {
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
            .unwrap_or_default()
    }

    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
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
            .unwrap_or_default()
    }

    async fn load_entities_page_for_peer(
        &self,
        peer_device_id: &str,
        class: SyncReadClass,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
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
            .unwrap_or_default()
    }

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

    async fn usage_complete_through(&self) -> std::collections::HashMap<String, u64> {
        self.db
            .query(
                CommandContext::sync_system(CommandClass::SyncLive),
                UsageCompleteThroughQuery::new(SyncReadClass::SyncLive),
            )
            .await
            .unwrap_or_default()
    }

    async fn usage_complete_through_for_peer(
        &self,
        peer_device_id: &str,
        class: SyncReadClass,
    ) -> std::collections::HashMap<String, u64> {
        self.db
            .query(
                CommandContext::sync_peer(peer_device_id.to_owned(), class.into()),
                UsageCompleteThroughQuery::new(class),
            )
            .await
            .unwrap_or_default()
    }

    async fn get_kv(&self, key: &str) -> Option<String> {
        self.db
            .query(
                CommandContext::sync_system(CommandClass::SyncLive),
                GetSyncKeyValueQuery::new(key.to_owned()),
            )
            .await
            .unwrap_or(None)
    }

    async fn set_kv(&self, key: &str, value: &str) {
        let _ = self
            .db
            .submit(
                CommandContext::sync_system(CommandClass::SyncLive),
                crate::db::SetSyncKeyValueCommand::new(key.to_owned(), value.to_owned()),
            )
            .await;
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

// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl AuthenticatedEntityBackend for SqliteStorageBackend {
    async fn apply_authenticated_entity(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
    ) -> Result<(), String> {
        if is_full_core_task(entity) {
            let command = AuthenticatedCoreTaskWrite::new(
                entity.id.clone(),
                entity.hlc.clone(),
                entity.origin_device_id.clone(),
                entity.data.clone(),
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
                crate::db::ApplyAuthenticatedSyncEntityCommand::new(entity.clone()),
            )
            .await
            .map_err(|error| error.to_string())?;
        publish_replay_notices(notices);
        Ok(())
    }
}

#[async_trait::async_trait]
impl PeerIdentityBackend for SqliteStorageBackend {
    async fn authorized_transport_public_key(&self, device_id: &str) -> Option<String> {
        self.db
            .query(
                CommandContext::sync_peer(device_id.to_owned(), CommandClass::SyncLive),
                AuthorizedTransportKeyQuery::new(device_id.to_owned()),
            )
            .await
            .unwrap_or(None)
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
        let db = self.db.clone();
        let frame = frame.clone();
        let space_id = expected_space_id.to_string();
        let origin_id = expected_origin_node_id.to_string();
        tokio::task::spawn_blocking(move || {
            db.with_connection(|conn| {
                crate::integration_replication::validate_outbound_signed_sync(
                    conn, &space_id, &origin_id, &frame,
                )
            })
        })
        .await
        .map_err(|error| error.to_string())?
    }

    async fn validate_outbound_signed_integration_frame_with_transport(
        &self,
        frame: &crate::integration_replication::SignedSyncEnvelope,
        expected_space_id: &str,
        expected_origin_node_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        let db = self.db.clone();
        let frame = frame.clone();
        let space_id = expected_space_id.to_string();
        let origin_id = expected_origin_node_id.to_string();
        let transport_key = transport_public_key.to_string();
        tokio::task::spawn_blocking(move || {
            db.with_connection(|conn| {
                crate::integration_replication::validate_outbound_signed_sync_with_transport(
                    conn,
                    &space_id,
                    &origin_id,
                    &frame,
                    &transport_key,
                )
            })
        })
        .await
        .map_err(|error| error.to_string())?
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

        let db = self.db.clone();
        let frame = frame.clone();
        let expected_space_id = expected_space_id.to_string();
        let expected_recipient_node_id = expected_recipient_node_id.to_string();
        tokio::task::spawn_blocking(move || {
            db.with_connection(|conn| {
                let reserved_at_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX);
                apply_signed_integration_changes(
                    conn,
                    &frame,
                    &expected_space_id,
                    &expected_recipient_node_id,
                    None,
                    reserved_at_ms,
                )
                .map_err(|error| error.to_string())
            })
        })
        .await
        .map_err(|e| e.to_string())?
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
        let db = self.db.clone();
        let frame = frame.clone();
        let expected_space_id = expected_space_id.to_string();
        let expected_recipient_node_id = expected_recipient_node_id.to_string();
        let transport_key = authenticated_transport_public_key.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            db.with_connection(|conn| {
                let reserved_at_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX);
                apply_signed_integration_changes(
                    conn,
                    &frame,
                    &expected_space_id,
                    &expected_recipient_node_id,
                    transport_key.as_deref(),
                    reserved_at_ms,
                )
                .map_err(|error| error.to_string())
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
