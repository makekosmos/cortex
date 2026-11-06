use super::*;

pub(crate) async fn handle_request(
    state: &Arc<ServiceState>,
    request: Request,
) -> Result<Value, String> {
    match request {
        Request::UpsertTrackedApp {
            tracked_app,
            device_id,
        } => usage::upsert_tracked_app(state, tracked_app, device_id).await,
        Request::DeleteTrackedApp { id, device_id } => {
            usage::delete_tracked_app(state, id, device_id).await
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
        Request::CompactUsageSyncLog {
            older_than_days,
            batch_limit,
            device_id,
        } => usage::compact_usage_sync_log(state, older_than_days, batch_limit, device_id).await,
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
        Request::EstimateDataVolume { scopes } => {
            objects::estimate_data_volume(state, scopes).await
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
        Request::TaskMergeConflicts { object_id, unit } => {
            objects::task_merge_conflicts(state, object_id, unit).await
        }
        Request::TaskMergeAppend {
            merge_operation,
            expected_conflict_digest,
        } => objects::task_merge_append(state, merge_operation, expected_conflict_digest).await,
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
            operation_id,
            device_id,
        } => {
            objects::upsert_object(state, object, expected_snapshot, operation_id, device_id).await
        }
        Request::DeleteObject {
            id,
            expected_snapshot,
            operation_id,
            device_id,
        } => objects::delete_object(state, id, expected_snapshot, operation_id, device_id).await,

        Request::TypesList => types::types_list(state).await,
        Request::TypesRegister {
            type_id,
            version,
            schema,
            name,
            app_id,
        } => types::types_register(state, type_id, version, schema, name, app_id).await,
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
        Request::ReplaceObjectLink {
            object_link,
            device_id,
        } => types::replace_object_link(state, object_link, device_id).await,

        Request::GetSyncKv { key } => system::get_sync_kv(state, key).await,
        Request::SetSyncKv { key, value } => system::set_sync_kv(state, key, value).await,
        Request::ClearAll => system::clear_all(state).await,
        Request::DbBackup { dest_path } => system::db_backup(state, dest_path).await,
        Request::DbBackupList => system::db_backup_list(state).await,
        Request::DbBackupValidate { backup_id } => {
            system::db_backup_validate(state, backup_id).await
        }
        Request::DbBackupRestore { backup_id } => system::db_backup_restore(state, backup_id).await,
        Request::StartSync(params) => system::start_sync(state, params).await,
        Request::StopSync => system::stop_sync(state).await,
        Request::BroadcastChange { entity } => system::broadcast_change(state, entity).await,
        Request::GetConnectedPeers => system::get_connected_peers(state).await,
        Request::GetSyncSnapshot => system::get_sync_snapshot(state).await,
        Request::SyncPolicyGet => system::sync_policy_get(state).await,
        Request::SyncPolicySet {
            profile_id,
            name,
            active,
            expected_revision,
            rules,
        } => {
            system::sync_policy_set(state, profile_id, name, active, expected_revision, rules).await
        }
        Request::DeviceRegistrySnapshot => system::device_registry_snapshot(state).await,
        Request::DeviceRegistryRename {
            target_device_id,
            display_name,
        } => system::device_registry_rename(state, target_device_id, display_name).await,
        Request::DeviceRegistryDisable { target_device_id } => {
            system::device_registry_disable(state, target_device_id).await
        }
        Request::DisconnectPeer { device_id } => system::disconnect_peer(state, device_id).await,
        Request::ConnectWithPairingCode { pairing_code, bind } => {
            system::connect_with_pairing_code(state, pairing_code, bind).await
        }
        Request::AcceptPairing { device_id } => system::accept_pairing(state, device_id).await,
        Request::DeclinePairing { device_id } => system::decline_pairing(state, device_id).await,
        Request::CancelPairing => system::cancel_pairing(state).await,
        Request::LeaveSpace => system::leave_space(state).await,
        Request::AddSeedPeer { addresses } => system::add_seed_peer(state, addresses).await,
        Request::GetOwnAddresses { port } => system::own_addresses(state, port).await,
        Request::GetHostDeviceName => system::host_device_name(state).await,
        Request::GetOwnIrohTicket => system::get_own_iroh_ticket(state).await,
        Request::ShowPairingCode { bind } => system::show_pairing_code(state, bind).await,

        // Test-only fault injection: proves the worker recovers from a
        // panicking request instead of dying silently.
        #[cfg(test)]
        #[allow(clippy::panic)]
        Request::TestPanic => panic!("ark-service test panic injection"),
    }
}
