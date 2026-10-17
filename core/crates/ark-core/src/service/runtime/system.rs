use super::*;
use ark_application::external_ref::ExternalRefUpsert;

async fn publish_registry_event(
    state: &Arc<ServiceState>,
    event: &crate::domain::device_registry::RegistryEvent,
) {
    let sync = state.sync.lock().await;
    let Some(sync) = sync.as_ref() else {
        return;
    };
    sync.server.broadcast_registry_event(event.clone()).await;
    let clients = sync.clients.lock().await;
    for client in clients.values() {
        client.broadcast_registry_event(event.clone()).await;
    }
    if let Some(relay) = &sync.relay {
        if let Err(error) = relay.publish_registry_event(event.clone()).await {
            eprintln!("[sync] relay registry event publish failed: {error}");
        }
    }
}

pub(super) async fn device_registry_snapshot(state: &Arc<ServiceState>) -> Result<Value, String> {
    let snapshot = get_db_handle(state)?
        .query(db::CommandContext::engine(), db::LoadDeviceRegistryQuery)
        .await
        .map_err(|error| error.to_string())?;
    serde_json::to_value(snapshot).map_err(|error| error.to_string())
}

pub(super) async fn sync_policy_get(state: &Arc<ServiceState>) -> Result<Value, String> {
    let device_id = local_registry_device_id(state).await?;
    let profile = get_db_handle(state)?
        .query(
            db::CommandContext::engine(),
            db::LoadActiveLocalSyncProfileQuery::new(device_id),
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(profile
        .map(sync_profile_json)
        .transpose()?
        .unwrap_or(Value::Null))
}

pub(super) async fn sync_policy_set(
    state: &Arc<ServiceState>,
    profile_id: String,
    name: String,
    active: bool,
    expected_revision: Option<i64>,
    rules: Vec<SyncPolicyRuleInput>,
) -> Result<Value, String> {
    if rules.len() > 256 {
        return Err("too many sync profile rules".into());
    }
    let rules = rules
        .into_iter()
        .map(|rule| {
            let filter_json =
                serde_json::to_string(&rule.filter).map_err(|error| error.to_string())?;
            Ok(ark_application::sync_policy::SyncProfileRule {
                resource_kind: rule.resource_kind,
                resource_id: rule.resource_id,
                mode: rule.mode,
                filter: ark_domain::selective_sync::FilterV1::parse(&filter_json)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let device_id = local_registry_device_id(state).await?;
    let profile = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::SaveLocalSyncProfileCommand::new(
                device_id,
                profile_id,
                name,
                active,
                expected_revision,
                rules,
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
    sync_profile_json(profile)
}

fn sync_profile_json(profile: ark_application::sync_policy::SyncProfile) -> Result<Value, String> {
    let digest = profile.digest();
    let rules = profile
        .rules
        .into_iter()
        .map(|rule| {
            Ok(json!({
                "resourceKind": rule.resource_kind,
                "resourceId": rule.resource_id,
                "mode": rule.mode,
                "filter": serde_json::from_str::<Value>(rule.filter.canonical_json())
                    .map_err(|error| error.to_string())?,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({
        "profileId": profile.profile_id,
        "deviceId": profile.device_id,
        "revision": profile.revision,
        "name": profile.name,
        "active": profile.active,
        "digest": digest,
        "rules": rules,
    }))
}

async fn local_registry_device_id(state: &Arc<ServiceState>) -> Result<String, String> {
    get_sync_kv(state, db::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY.to_owned())
        .await?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "device_registry_local_identity_missing".to_owned())
}

pub(super) async fn device_registry_rename(
    state: &Arc<ServiceState>,
    target_device_id: crate::domain::device_registry::DeviceId,
    display_name: crate::domain::device_registry::DisplayName,
) -> Result<Value, String> {
    let local_device_id = local_registry_device_id(state).await?;
    let event = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::AppendLocalRegistryEventCommand::new(
                local_device_id,
                crate::domain::device_registry::RegistryCommand::Rename {
                    target_device_id,
                    display_name,
                },
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
    if let Some(event) = event.as_ref() {
        publish_registry_event(state, event).await;
    }
    serde_json::to_value(event).map_err(|error| error.to_string())
}

pub(super) async fn device_registry_disable(
    state: &Arc<ServiceState>,
    target_device_id: crate::domain::device_registry::DeviceId,
) -> Result<Value, String> {
    let local_device_id = local_registry_device_id(state).await?;
    let event = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::AppendLocalRegistryEventCommand::new(
                local_device_id,
                crate::domain::device_registry::RegistryCommand::Disable { target_device_id },
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
    if let Some(event) = event.as_ref() {
        publish_registry_event(state, event).await;
    }
    serde_json::to_value(event).map_err(|error| error.to_string())
}

pub(super) async fn get_sync_kv(state: &Arc<ServiceState>, key: String) -> Result<Value, String> {
    get_db_handle(state)?
        .query(
            db::CommandContext::engine(),
            db::SystemReadQuery::SyncKeyValue(key),
        )
        .await
        .map_err(|error| error.to_string())
}

pub(super) async fn set_sync_kv(
    state: &Arc<ServiceState>,
    key: String,
    value: String,
) -> Result<Value, String> {
    get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::SetLocalSyncKeyValueCommand::new(key, value),
        )
        .await
        .map(|()| json!(true))
        .map_err(|error| error.to_string())
}

pub(super) async fn external_refs_upsert(
    state: &Arc<ServiceState>,
    params: ExternalRefUpsert,
) -> Result<Value, String> {
    get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::UpsertExternalRefCommand::new(params),
        )
        .await
        .map(|()| json!(true))
        .map_err(|error| error.to_string())
}

pub(super) async fn clear_all(state: &Arc<ServiceState>) -> Result<Value, String> {
    get_db_handle(state)?
        .submit(
            db::CommandContext::engine_maintenance(),
            db::ClearAllCommand,
        )
        .await
        .map(|()| json!(null))
        .map_err(|error| error.to_string())
}

pub(super) async fn delete_trashed(state: &Arc<ServiceState>) -> Result<Value, String> {
    get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::DeleteTrashedCommand::new(local_write_device_id(None)),
        )
        .await
        .map(|count| json!(count))
        .map_err(|error| error.to_string())
}

pub(super) async fn db_backup(
    state: &Arc<ServiceState>,
    dest_path: String,
) -> Result<Value, String> {
    let src_path = current_db_path(state)?;
    let pages = backup_pages_per_step();
    let pause = std::time::Duration::from_millis(backup_pause_ms());
    let dest_for_thread = dest_path.clone();
    let state_for_thread = Arc::clone(state);
    // Fire-and-forget: копирование идёт на отдельном background-priority
    // потоке через ОТДЕЛЬНЫЙ read-коннекшн (не держит глобальный DB
    // mutex) — серийный RPC-loop сразу свободен для других запросов.
    // Завершение сообщается событием `db_backup_result`; mundus-engine
    // ждёт его, чтобы записать last_backup_ts + ротацию.
    // KOS-51: BACKUP_GATE удерживается на всё копирование —
    // `db_backup_restore` не может начаться посреди backup'а.
    std::thread::Builder::new()
        .name("ark-db-backup".to_string())
        .spawn(move || {
            let _bg = background_priority::BackgroundThreadGuard::enter();
            let _gate = state_for_thread.backup_gate.blocking_lock();
            let result = db::backup_to_file_chunked(&src_path, &dest_for_thread, pages, pause);
            let event = match &result {
                Ok(()) => json!({
                    "event": "db_backup_result",
                    "ok": true,
                    "dest": dest_for_thread,
                }),
                Err(e) => json!({
                    "event": "db_backup_result",
                    "ok": false,
                    "dest": dest_for_thread,
                    "error": e,
                }),
            };
            emit_event(event);
        })
        .map_err(|e| format!("spawn db_backup thread failed: {e}"))?;
    Ok(json!({ "started": true, "dest": dest_path }))
}

// KOS-51: list/validate — read-only над backups dir + live conn,
// BACKUP_GATE не нужен (не мутируют ничего).
pub(super) async fn db_backup_list(state: &Arc<ServiceState>) -> Result<Value, String> {
    let db_path = current_db_path(state)?;
    let backups = db::list_snapshots(&db_path)?;
    Ok(json!({ "backups": backups }))
}

pub(super) async fn db_backup_validate(
    state: &Arc<ServiceState>,
    backup_id: String,
) -> Result<Value, String> {
    let db_path = current_db_path(state)?;
    get_db_handle(state)?
        .query(
            db::CommandContext::engine(),
            db::SystemReadQuery::ValidateSnapshot { db_path, backup_id },
        )
        .await
        .map_err(|error| error.to_string())
}

// KOS-51: атомарный restore. Порядок захвата: BACKUP_GATE → DB handle.
// Restore держит оба на всю операцию — ни backup, ни
// обычные ARK ops не выполняются параллельно, момента с отсутствующей
// или частично заменённой primary DB нет (Online Backup API пишет
// destination транзакционно).
pub(super) async fn db_backup_restore(
    state: &Arc<ServiceState>,
    backup_id: String,
) -> Result<Value, String> {
    let db_path = current_db_path(state)?;
    let _gate = state.backup_gate.lock().await;
    let db_handle = get_db_handle(state)?;
    let validation = db_handle
        .query(
            db::CommandContext::engine(),
            db::SystemReadQuery::ValidateSnapshot {
                db_path: db_path.clone(),
                backup_id: backup_id.clone(),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    if validation["valid"] != json!(true) {
        return Err(validation["error"]
            .as_str()
            .unwrap_or("snapshot is not valid for this ARK database")
            .to_string());
    }

    let restart_params = state
        .sync
        .lock()
        .await
        .as_ref()
        .map(|runtime| runtime.start_params.clone());
    if restart_params.is_some() {
        handle_stop_sync(state).await;
    }

    let restore_result = db_handle
        .submit_maintenance(
            db::CommandContext::engine_maintenance(),
            db::RestoreSnapshotCommand::new(db_path, backup_id),
        )
        .await;

    let report = match (restore_result, restart_params) {
        (Ok(report), Some(params)) => {
            handle_start_sync_with_params(state, params)
                .await
                .map_err(|error| {
                    format!("restore succeeded but sync could not restart: {error}")
                })?;
            report
        }
        (Ok(report), None) => report,
        (Err(db::DbError::RestoreRefused(reason)), Some(params)) => {
            handle_start_sync_with_params(state, params)
                .await
                .map_err(|error| {
                    format!("restore was safely refused, but sync could not restart: {error}")
                })?;
            return Err(format!(
                "restore refused without changing the live database: {reason}"
            ));
        }
        (Err(error), Some(_)) => {
            return Err(format!(
                "restore failed; sync stays stopped because database state is uncertain: {error}"
            ));
        }
        (Err(error), None) => return Err(error.to_string()),
    };
    serde_json::to_value(report).map_err(|error| error.to_string())
}

pub(super) async fn start_sync(
    state: &Arc<ServiceState>,
    params: StartSyncParams,
) -> Result<Value, String> {
    handle_start_sync(state, params).await
}

pub(super) async fn stop_sync(state: &Arc<ServiceState>) -> Result<Value, String> {
    handle_stop_sync(state).await;
    Ok(json!(true))
}

pub(super) async fn broadcast_change(
    state: &Arc<ServiceState>,
    entity: SyncEntity,
) -> Result<Value, String> {
    handle_broadcast_change(state, entity).await
}

pub(super) async fn get_connected_peers(state: &Arc<ServiceState>) -> Result<Value, String> {
    handle_get_connected_peers(state).await
}

pub(super) async fn get_sync_snapshot(state: &Arc<ServiceState>) -> Result<Value, String> {
    handle_get_sync_snapshot(state).await
}

pub(super) async fn disconnect_peer(
    state: &Arc<ServiceState>,
    device_id: String,
) -> Result<Value, String> {
    handle_disconnect_peer(state, device_id).await
}

pub(super) async fn connect_with_pairing_code(
    state: &Arc<ServiceState>,
    pairing_code: String,
) -> Result<Value, String> {
    handle_connect_with_pairing_code(state, pairing_code).await
}

pub(super) async fn accept_pairing(
    state: &Arc<ServiceState>,
    device_id: String,
) -> Result<Value, String> {
    handle_accept_pairing(state, device_id).await
}

pub(super) async fn decline_pairing(
    state: &Arc<ServiceState>,
    device_id: String,
) -> Result<Value, String> {
    handle_decline_pairing(state, device_id).await
}

pub(super) async fn cancel_pairing(state: &Arc<ServiceState>) -> Result<Value, String> {
    handle_cancel_pairing(state).await
}

pub(super) async fn leave_space(state: &Arc<ServiceState>) -> Result<Value, String> {
    handle_stop_sync(state).await;
    // Also purge any persisted self-reference peer records so the
    // next start_sync on this or any other space does not inherit
    // phantom records. The underlying SyncServer did this inside its
    // own `start()` but we may be leaving without restarting.
    Ok(json!(true))
}

pub(super) async fn add_seed_peer(
    state: &Arc<ServiceState>,
    addresses: Vec<String>,
) -> Result<Value, String> {
    handle_add_seed_peer(state, addresses).await
}

pub(super) async fn own_addresses(
    _state: &Arc<ServiceState>,
    port: Option<u16>,
) -> Result<Value, String> {
    // Port 0 is not a usable listen port; report no addresses rather
    // than bogus ":0" targets.
    let port = port.unwrap_or(LAN_SYNC_PORT);
    let addrs = if port == 0 {
        Vec::new()
    } else {
        get_own_addresses(port)
    };
    Ok(json!(addrs))
}

pub(super) async fn host_device_name(_state: &Arc<ServiceState>) -> Result<Value, String> {
    Ok(json!(get_host_device_name()))
}

pub(super) async fn get_own_iroh_ticket(state: &Arc<ServiceState>) -> Result<Value, String> {
    handle_get_own_iroh_ticket(state).await
}

pub(super) async fn show_pairing_code(state: &Arc<ServiceState>) -> Result<Value, String> {
    handle_show_pairing_code(state).await
}
