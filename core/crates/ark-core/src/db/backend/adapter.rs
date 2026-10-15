use super::apply::is_full_core_task;
use super::SqliteStorageBackend;
use crate::db::{
    apply_signed_integration_changes, get_sync_kv, set_sync_kv, usage_log_complete_through,
    AuthenticatedCoreTaskWrite, CommandContext,
};
use crate::replication_storage::{
    AuthenticatedEntityBackend, IntegrationReplicationBackend, PeerIdentityBackend, StorageBackend,
};
use crate::types::{SyncEntity, VersionVector};

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
        let db = self.db.clone();
        let vector = vector.clone();
        let device_id = self.device_id();
        let profile = self.selective_sync_profile();
        tokio::task::spawn_blocking(move || {
            // Poison recovery: РµСЃР»Рё earlier panic Р·Р°РїРѕР»СѓС‡РёР» lock, РјС‹ РІСЃС‘
            // СЂР°РІРЅРѕ РјРѕР¶РµРј С‡РёС‚Р°С‚СЊ. Р­С‚Рѕ backend РґР»СЏ load (read-only path),
            // РґР°РЅРЅС‹Рµ РІРЅСѓС‚СЂРё guard'Р° С†РµР»С‹. Р‘РµР· recovery РєР°Р¶РґС‹Р№
            // РїРѕСЃР»РµРґСѓСЋС‰РёР№
            // sync round РІРѕР·РІСЂР°С‰Р°РµС‚ empty list в†’ multi-device sync silent
            // С„РµР№Р»РёС‚СЃСЏ РЅР°РІСЃРµРіРґР° РїРѕСЃР»Рµ РїРµСЂРІРѕРіРѕ panic'Р° РІ СЌС‚РѕРј
            // РїСЂРѕС†РµСЃСЃРµ.
            db.with_connection(|conn| {
                let entities = Self::collect_entities_blocking(conn, &vector, &device_id);
                profile.as_ref().map_or(entities.clone(), |profile| {
                    crate::data_platform::filter_entities_for_profile(profile, &entities)
                })
            })
        })
        .await
        .unwrap_or_default()
    }

    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
        let db = self.db.clone();
        let vector = vector.clone();
        let device_id = self.device_id();
        let profile = self.selective_sync_profile();
        tokio::task::spawn_blocking(move || {
            db.with_connection(|conn| {
                if let Some(profile) = profile {
                    let all = Self::collect_entities_blocking(conn, &vector, &device_id);
                    crate::data_platform::filter_entities_for_profile(&profile, &all)
                        .into_iter()
                        .skip(offset)
                        .take(limit)
                        .collect()
                } else {
                    Self::collect_entities_page_blocking(conn, &vector, &device_id, offset, limit)
                }
            })
        })
        .await
        .unwrap_or_default()
    }

    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
        let db = self.db.clone();
        let entity = entity.clone();
        tokio::task::spawn_blocking(move || {
            // Apply вЂ” write path. Poison recovery acceptable: SQLite
            // transactions atomic, partially-applied state РЅРµ РІРѕР·РјРѕР¶РµРЅ.
            // РђР»СЊС‚РµСЂРЅР°С‚РёРІР° (return Err) РґРµР»Р°РµС‚ sync
            // РЅРµСЂР°Р±РѕС‚РѕСЃРїРѕСЃРѕР±РЅС‹Рј РґРѕ process restart.
            db.with_connection(|conn| Self::apply_entity_blocking(conn, &entity))
        })
        .await
        .map_err(|e| e.to_string())?
    }

    async fn usage_complete_through(&self) -> std::collections::HashMap<String, u64> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            // Cursor-only claims (`None`): always a safe lower bound.
            // Live usage writes are stamped with `usage_tracker.device_id`,
            // and even where the sync id does reach the log (the one-time
            // migration backfill via `set_device_id`) its cursor is
            // contiguous, so it equals the head-bound claim anyway.
            db.with_connection(|conn| usage_log_complete_through(conn, None).unwrap_or_default())
        })
        .await
        .unwrap_or_default()
    }

    async fn get_kv(&self, key: &str) -> Option<String> {
        let db = self.db.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            db.with_connection(|conn| get_sync_kv(conn, &key).unwrap_or(None))
        })
        .await
        .unwrap_or(None)
    }

    async fn set_kv(&self, key: &str, value: &str) {
        let db = self.db.clone();
        let key = key.to_string();
        let value = value.to_string();
        let _ = tokio::task::spawn_blocking(move || {
            db.with_connection(|conn| {
                let _ = set_sync_kv(conn, &key, &value);
            });
        })
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
        let db = self.db.clone();
        let entity = entity.clone();
        let authenticated_peer_id = authenticated_peer_id.to_owned();
        tokio::task::spawn_blocking(move || {
            db.with_connection(|conn| {
                Self::apply_authenticated_entity_blocking(conn, &entity, &authenticated_peer_id)
            })
        })
        .await
        .map_err(|error| error.to_string())?
    }
}

#[async_trait::async_trait]
impl PeerIdentityBackend for SqliteStorageBackend {
    async fn authorized_transport_public_key(&self, device_id: &str) -> Option<String> {
        let db = self.db.clone();
        let device_id = device_id.to_string();
        tokio::task::spawn_blocking(move || {
            db.with_connection(|conn| {
                crate::db::load_authorized_node(conn, &device_id)
                    .ok()
                    .flatten()
                    .and_then(|node| node.transport_public_key)
            })
        })
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
