// ---------------------------------------------------------------------------
// DB helpers — all take `state` (the per-service slot that used to be the
// sidecar's process-wide `DB`/`DB_PATH`/`BACKUP_GATE` statics).
// ---------------------------------------------------------------------------

use super::*;

pub(super) fn with_conn<T, F>(state: &ServiceState, f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    let outer = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let shared = outer
        .as_ref()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())?
        .clone();
    drop(outer);
    let inner = shared.lock().unwrap_or_else(|e| e.into_inner());
    f(&inner)
}

/// Выполняет write-замыкание в одной SQLite-транзакции поверх shared conn.
/// COMMIT при Ok, ROLLBACK при Err. Гарантирует атомарность entity + FTS + sync-meta:
/// вложенные SAVEPOINT внутри db::* работают внутри этого BEGIN, при ошибке откатывается всё.
pub(super) fn with_write_tx<T, F>(state: &ServiceState, f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    with_conn(state, |conn| {
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| e.to_string())?;
        match f(conn) {
            Ok(v) => {
                conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
                Ok(v)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    })
}

pub(super) fn get_shared_conn(
    state: &ServiceState,
) -> Result<Arc<StdMutex<rusqlite::Connection>>, String> {
    let guard = state.db.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())
}

/// `&mut` вариант `with_conn` — для операций, меняющих саму базу под
/// `Connection` (например `conn.restore` в `db_backup_restore`, KOS-51).
/// Глобальный DB mutex удерживается на всё замыкание: никакой другой ARK op
/// не выполняется параллельно.
pub(super) fn with_conn_mut<T, F>(state: &ServiceState, f: F) -> Result<T, String>
where
    F: FnOnce(&mut rusqlite::Connection) -> Result<T, String>,
{
    let outer = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let shared = outer
        .as_ref()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())?
        .clone();
    drop(outer);
    let mut inner = shared.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut inner)
}

/// Путь к live ARK DB (из Init). Общий accessor для `db_backup`,
/// `db_backup_list/validate/restore` — restore резолвит `<dir>/backups/`
/// от этого пути.
pub(super) fn current_db_path(state: &ServiceState) -> Result<String, String> {
    state
        .db_path
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())
}

// --- DB backup: background-priority thread + chunking config ---------------

/// Страниц за один шаг online-backup. Меньше шаг → мягче для диска. Override
/// `ARK_BACKUP_PAGES_PER_STEP`.
fn backup_pages_per_step() -> i32 {
    std::env::var("ARK_BACKUP_PAGES_PER_STEP")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(256)
}

/// Пауза (мс) между шагами online-backup, чтобы не насыщать диск. Override
/// `ARK_BACKUP_PAUSE_MS`.
fn backup_pause_ms() -> u64 {
    std::env::var("ARK_BACKUP_PAUSE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5)
}

/// Thread-scoped background priority (CPU + I/O) для backup-потока.
/// `THREAD_MODE_BACKGROUND_BEGIN` на Windows; no-op иначе. RAII (Drop → END).
/// НЕ process-wide — сервис обслуживает интерактивные ARK ops.
#[cfg(windows)]
mod background_priority {
    pub struct BackgroundThreadGuard {
        active: bool,
    }
    impl BackgroundThreadGuard {
        pub fn enter() -> Self {
            use windows::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
            };
            // SAFETY: меняем приоритет только текущего потока через псевдо-handle.
            let ok = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
            Self { active: ok.is_ok() }
        }
    }
    impl Drop for BackgroundThreadGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            use windows::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_END,
            };
            // SAFETY: симметричный END для ранее успешного BEGIN на том же потоке.
            unsafe {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
            }
        }
    }
}

#[cfg(not(windows))]
mod background_priority {
    pub struct BackgroundThreadGuard;
    impl BackgroundThreadGuard {
        pub fn enter() -> Self {
            Self
        }
    }
}

fn local_write_device_id(device_id: Option<String>) -> String {
    device_id
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "ark-core-rpc-local".to_string())
}

fn record_local_upsert(
    conn: &rusqlite::Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: Option<String>,
) -> Result<String, String> {
    let device_id = local_write_device_id(device_id);
    let hlc = db::bump_sync_version_vector(conn, entity_type, entity_id, &device_id, false)?;
    db::delete_sync_tombstone(conn, entity_id)?;
    Ok(hlc)
}

fn record_local_delete(
    conn: &rusqlite::Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: Option<String>,
) -> Result<String, String> {
    record_local_delete_with_type(conn, entity_type, entity_id, device_id, None)
}

fn record_local_delete_with_type(
    conn: &rusqlite::Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: Option<String>,
    type_id: Option<&str>,
) -> Result<String, String> {
    let device_id = local_write_device_id(device_id);
    let hlc = db::bump_sync_version_vector(conn, entity_type, entity_id, &device_id, true)?;
    db::record_sync_tombstone_with_type(conn, entity_type, entity_id, type_id, &hlc)?;
    Ok(hlc)
}

/// Строит SyncEntity из сырого serde_json::Value объекта (уже сериализованного).
/// `data_value` ожидается `Value::Object`; "id" удаляется как в `to_data_map` в db.rs.
/// Для delete: передавай `Value::Object(Map::new())` + `deleted = Some(true)`.
fn make_sync_entity(
    entity_type: &str,
    id: &str,
    data_value: Value,
    hlc: String,
    deleted: Option<bool>,
) -> SyncEntity {
    let data = match data_value {
        Value::Object(mut map) => {
            map.remove("id");
            map
        }
        _ => serde_json::Map::new(),
    };
    let origin = db::is_sequenced_usage_entity(entity_type).then(|| HLC::from_string(&hlc));
    SyncEntity {
        entity_type: entity_type.to_string(),
        id: id.to_string(),
        data,
        hlc,
        deleted,
        origin_device_id: origin.as_ref().map(|value| value.device_id.clone()),
        origin_seq: origin.map(|value| value.counter),
    }
}

/// Fan-out локального изменения всем подключённым пирам (server sessions +
/// outbound clients + relay/iroh). Не применяет entity к storage и не
/// перебивает HLC — данные уже записаны через `record_local_*`.
/// Если sync не запущен — тихий no-op.
pub(super) async fn broadcast_local_change(state: &ServiceState, entity: SyncEntity) {
    let runtime = {
        let guard = state.sync.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return,
        }
    };

    runtime
        .server
        .broadcast_live_change(entity.clone(), None)
        .await;

    let clients: Vec<_> = runtime.clients.lock().await.values().cloned().collect();
    for client in clients {
        client.broadcast_live_change(entity.clone()).await;
    }
    if let Some(relay) = runtime.relay.as_ref() {
        // Ошибку отправки логируем, но не пробрасываем — live broadcast best-effort.
        if let Err(e) = relay.broadcast_live_change(entity.clone()) {
            eprintln!("[ark-core] broadcast_local_change relay error: {e}");
        }
    }
}

fn legacy_content() -> Value {
    json!({"type":"doc","content":[{"type":"paragraph"}]})
}

fn legacy_record(
    id: &str,
    legacy_type_id: &str,
    title: &str,
    props: Value,
    created_at: &str,
    deleted_at: Option<String>,
) -> crate::canonical_types::compatibility::LegacyRecord {
    crate::canonical_types::compatibility::LegacyRecord {
        id: id.into(),
        legacy_type_id: legacy_type_id.into(),
        title: title.into(),
        content: legacy_content(),
        props,
        created_at: created_at.into(),
        updated_at: created_at.into(),
        deleted_at,
    }
}

fn write_legacy_graph(
    conn: &rusqlite::Connection,
    records: &[crate::canonical_types::compatibility::LegacyRecord],
    device_id: Option<String>,
) -> Result<Vec<SyncEntity>, String> {
    crate::canonical_types::facades::write_legacy_records(conn, records, "rpc", device_id)
}

fn tombstone_legacy(
    conn: &rusqlite::Connection,
    id: &str,
    expected_type_id: &str,
    device_id: Option<String>,
) -> Result<SyncEntity, String> {
    crate::canonical_types::facades::delete_legacy_object(conn, id, expected_type_id, device_id)
}

// ---------------------------------------------------------------------------

mod integration;
mod legacy;
mod objects;
mod system;
mod types;
mod usage;

pub(crate) async fn handle_request(
    state: &Arc<ServiceState>,
    request: Request,
) -> Result<Value, String> {
    match request {
        Request::Init { db_path } => legacy::init(state, db_path).await,
        Request::LoadAll => legacy::load_all(state).await,
        Request::UpsertTodo { todo, device_id } => {
            legacy::upsert_todo(state, todo, device_id).await
        }
        Request::DeleteTodo { id, device_id } => legacy::delete_todo(state, id, device_id).await,
        Request::DeleteProject { id, device_id } => {
            legacy::delete_project(state, id, device_id).await
        }
        Request::BatchUpsertTodos { todos, device_id } => {
            legacy::batch_upsert_todos(state, todos, device_id).await
        }
        Request::UpsertProject { project, device_id } => {
            legacy::upsert_project(state, project, device_id).await
        }
        Request::UpsertTag { tag, device_id } => legacy::upsert_tag(state, tag, device_id).await,
        Request::UpsertTrackedApp {
            tracked_app,
            device_id,
        } => legacy::upsert_tracked_app(state, tracked_app, device_id).await,
        Request::DeleteTrackedApp { id, device_id } => {
            legacy::delete_tracked_app(state, id, device_id).await
        }

        Request::UpsertUsageSession {
            usage_session,
            device_id,
        } => usage::upsert_usage_session(state, usage_session, device_id).await,
        Request::DeleteUsageSession { id, device_id } => {
            usage::delete_usage_session(state, id, device_id).await
        }
        Request::UpsertUsageEvent {
            usage_event,
            device_id,
        } => usage::upsert_usage_event(state, usage_event, device_id).await,
        Request::DeleteUsageEvent { id, device_id } => {
            usage::delete_usage_event(state, id, device_id).await
        }
        Request::UpsertUsageSpan { usage_span } => {
            usage::upsert_usage_span(state, usage_span).await
        }
        Request::GetUsageTitleTotal { query } => usage::get_usage_title_total(state, query).await,
        Request::GetUsageAnalytics {
            range_days,
            top_apps_limit,
            recent_sessions_limit,
        } => {
            usage::get_usage_analytics(state, range_days, top_apps_limit, recent_sessions_limit)
                .await
        }
        Request::ListRecentUsageProcesses { limit } => {
            usage::list_recent_usage_processes(state, limit).await
        }
        Request::SearchUsageProcesses { query, limit } => {
            usage::search_usage_processes(state, query, limit).await
        }
        Request::GetUsageGamePlaytimeSummary {
            bindings,
            range_start,
            range_end,
        } => usage::get_usage_game_playtime_summary(state, bindings, range_start, range_end).await,

        Request::ListObjects => objects::list_objects(state).await,
        Request::ListObjectSummaries => objects::list_object_summaries(state).await,
        Request::ListObjectsByType { type_id } => {
            objects::list_objects_by_type(state, type_id).await
        }
        Request::ListObjectSummariesByType { type_id } => {
            objects::list_object_summaries_by_type(state, type_id).await
        }
        Request::ListRunningTimeEntries { source } => {
            objects::list_running_time_entries(state, source).await
        }
        Request::GetObjectsByIds { ids } => objects::get_objects_by_ids(state, ids).await,
        Request::SearchObjects { query } => objects::search_objects(state, query).await,
        Request::GetObject { id } => objects::get_object(state, id).await,
        Request::GetObjectWriteSnapshot { id } => {
            objects::get_object_write_snapshot(state, id).await
        }
        Request::CanonicalGameList { device_id } => {
            objects::canonical_game_list(state, device_id).await
        }
        Request::CanonicalGameGet { id, device_id } => {
            objects::canonical_game_get(state, id, device_id).await
        }
        Request::CanonicalGameUpsert { game, device_id } => {
            objects::canonical_game_upsert(state, game, device_id).await
        }
        Request::CanonicalAssetSources { object_ids } => {
            objects::canonical_asset_sources(state, object_ids).await
        }
        Request::CanonicalSetBookCover {
            book_id,
            source_ref,
            existing_image_id,
            alt_text,
            device_id,
        } => {
            objects::canonical_set_book_cover(
                state,
                book_id,
                source_ref,
                existing_image_id,
                alt_text,
                device_id,
            )
            .await
        }
        Request::UpsertObject {
            object,
            expected_snapshot,
            device_id,
        } => objects::upsert_object(state, object, expected_snapshot, device_id).await,
        Request::DeleteObject {
            id,
            expected_snapshot,
            device_id,
        } => objects::delete_object(state, id, expected_snapshot, device_id).await,

        Request::TypesList => types::types_list(state).await,
        Request::TypesRegisterPackageDefinitions { registrations } => {
            types::types_register_package_definitions(state, registrations).await
        }
        Request::TypesGet { type_id, version } => types::types_get(state, type_id, version).await,
        Request::TypesListVersions { type_id } => types::types_list_versions(state, type_id).await,
        Request::TypesResolveAlias { alias } => types::types_resolve_alias(state, alias).await,
        Request::ListObjectTypes => types::list_object_types(state).await,
        Request::GetObjectType { id } => types::get_object_type(state, id).await,
        Request::UpsertObjectType {
            object_type,
            device_id,
        } => types::upsert_object_type(state, object_type, device_id).await,
        Request::DeleteObjectType { id, device_id } => {
            types::delete_object_type(state, id, device_id).await
        }
        Request::ListObjectLinks => types::list_object_links(state).await,
        Request::UpsertObjectLink {
            object_link,
            device_id,
        } => types::upsert_object_link(state, object_link, device_id).await,
        Request::DeleteObjectLink { id, device_id } => {
            types::delete_object_link(state, id, device_id).await
        }

        Request::GetSyncKv { key } => system::get_sync_kv(state, key).await,
        Request::SetSyncKv { key, value } => system::set_sync_kv(state, key, value).await,
        Request::ExternalRefsUpsert {
            connector_id,
            account_id,
            external_type,
            external_id,
            object_id,
            revision,
            hash,
            state: ref_state,
        } => {
            system::external_refs_upsert(
                state,
                connector_id,
                account_id,
                external_type,
                external_id,
                object_id,
                revision,
                hash,
                ref_state,
            )
            .await
        }
        Request::ClearAll => system::clear_all(state).await,
        Request::DeleteTrashed => system::delete_trashed(state).await,
        Request::DbBackup { dest_path } => system::db_backup(state, dest_path).await,
        Request::DbBackupList => system::db_backup_list(state).await,
        Request::DbBackupValidate { backup_id } => {
            system::db_backup_validate(state, backup_id).await
        }
        Request::DbBackupRestore { backup_id } => system::db_backup_restore(state, backup_id).await,
        Request::StartSync {
            space_id,
            device_id,
            device_name,
            port,
            seed_addresses,
            relay_url,
            relay_api_key,
            auth_secret,
            use_iroh,
            iroh_peer_ticket,
            discovery_enabled,
            bind,
        } => {
            system::start_sync(
                state,
                space_id,
                device_id,
                device_name,
                port,
                seed_addresses,
                relay_url,
                relay_api_key,
                auth_secret,
                use_iroh,
                iroh_peer_ticket,
                discovery_enabled,
                bind,
            )
            .await
        }
        Request::StopSync => system::stop_sync(state).await,
        Request::BroadcastChange { entity } => system::broadcast_change(state, entity).await,
        Request::GetConnectedPeers => system::get_connected_peers(state).await,
        Request::GetSyncSnapshot => system::get_sync_snapshot(state).await,
        Request::DisconnectPeer { device_id } => system::disconnect_peer(state, device_id).await,
        Request::ConnectWithPairingCode { pairing_code } => {
            system::connect_with_pairing_code(state, pairing_code).await
        }
        Request::LeaveSpace => system::leave_space(state).await,
        Request::AddSeedPeer { addresses } => system::add_seed_peer(state, addresses).await,
        Request::GetOwnAddresses { port } => system::own_addresses(state, port).await,
        Request::GetHostDeviceName => system::host_device_name(state).await,
        Request::GetOwnIrohTicket => system::get_own_iroh_ticket(state).await,

        Request::IntegrationPersistNodeAuthorization {
            authorization_operation,
            node,
            grant,
            device_id,
        } => {
            integration::integration_persist_node_authorization(
                state,
                authorization_operation,
                node,
                grant,
                device_id,
            )
            .await
        }
        Request::IntegrationPersistIntegrationGrant { grant, device_id } => {
            integration::integration_persist_integration_grant(state, grant, device_id).await
        }
        Request::IntegrationPrepareSignedSync {
            space_id,
            origin_node_id,
            integration_id,
            recipient_node_id,
            message_id,
        } => {
            integration::integration_prepare_signed_sync(
                state,
                space_id,
                origin_node_id,
                integration_id,
                recipient_node_id,
                message_id,
            )
            .await
        }
        Request::IntegrationValidateOutboundSignedSync {
            space_id,
            origin_node_id,
            frame,
        } => {
            integration::integration_validate_outbound_signed_sync(
                state,
                space_id,
                origin_node_id,
                frame,
            )
            .await
        }
        Request::IntegrationSendSignedSync { frame } => {
            integration::integration_send_signed_sync(state, frame).await
        }
        Request::IntegrationAcquireRefreshLease {
            integration_id,
            holder_node_id,
            credential_generation,
            now_ms,
            ttl_ms,
            expected_fencing_token,
            device_id,
        } => {
            integration::integration_acquire_refresh_lease(
                state,
                integration_id,
                holder_node_id,
                credential_generation,
                now_ms,
                ttl_ms,
                expected_fencing_token,
                device_id,
            )
            .await
        }
        Request::IntegrationPublishCredentialEnvelope {
            envelope,
            device_id,
            now_ms,
        } => {
            integration::integration_publish_credential_envelope(state, envelope, device_id, now_ms)
                .await
        }
        Request::IntegrationLoadLatestCredentialEnvelope {
            integration_id,
            recipient_node_id,
        } => {
            integration::integration_load_latest_credential_envelope(
                state,
                integration_id,
                recipient_node_id,
            )
            .await
        }
        Request::IntegrationLookupIssuerEncryptionKey {
            space_id,
            integration_id,
            recipient_node_id,
            issuer_node_id,
            credential_generation,
            expected_issuer_key_id,
        } => {
            integration::integration_lookup_issuer_encryption_key(
                state,
                space_id,
                integration_id,
                recipient_node_id,
                issuer_node_id,
                credential_generation,
                expected_issuer_key_id,
            )
            .await
        }
        Request::IntegrationLookupIssuerEncryptionKeyForPublish {
            space_id,
            integration_id,
            recipient_node_id,
            issuer_node_id,
            expected_issuer_key_id,
        } => {
            integration::integration_lookup_issuer_encryption_key_for_publish(
                state,
                space_id,
                integration_id,
                recipient_node_id,
                issuer_node_id,
                expected_issuer_key_id,
            )
            .await
        }
        Request::IntegrationVerificationStatus {
            integration_id,
            local_node_id,
            now_ms,
        } => {
            integration::integration_verification_status(
                state,
                integration_id,
                local_node_id,
                now_ms,
            )
            .await
        }

        // Test-only fault injection: proves the worker recovers from a
        // panicking request instead of dying silently.
        #[cfg(test)]
        #[allow(clippy::panic)]
        Request::TestPanic => panic!("ark-service test panic injection"),
    }
}
