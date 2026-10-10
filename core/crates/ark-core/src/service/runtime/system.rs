use super::*;
use crate::data_platform::ExternalRefUpsert;

pub(super) async fn get_sync_kv(state: &Arc<ServiceState>, key: String) -> Result<Value, String> {
    with_conn(state, |conn| {
        let value = db::get_sync_kv(conn, &key)?;
        Ok(json!(value))
    })
}

pub(super) async fn set_sync_kv(
    state: &Arc<ServiceState>,
    key: String,
    value: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        db::set_sync_kv(conn, &key, &value)?;
        Ok(json!(true))
    })
}

pub(super) async fn external_refs_upsert(
    state: &Arc<ServiceState>,
    params: ExternalRefUpsert,
) -> Result<Value, String> {
    with_write_tx(state, |conn| {
        crate::data_platform::ensure_schema(conn)?;
        crate::data_platform::upsert_external_ref(conn, &params)?;
        Ok(json!(true))
    })
}

pub(super) async fn clear_all(state: &Arc<ServiceState>) -> Result<Value, String> {
    with_conn(state, |conn| {
        db::clear_all(conn)?;
        Ok(json!(null))
    })
}

pub(super) async fn delete_trashed(state: &Arc<ServiceState>) -> Result<Value, String> {
    with_conn(state, |conn| {
        let count = db::delete_trashed(conn, &local_write_device_id(None))?;
        Ok(json!(count))
    })
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
            let _gate = state_for_thread
                .backup_gate
                .lock()
                .unwrap_or_else(|e| e.into_inner());
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
    with_conn(state, |conn| {
        let validation = db::validate_snapshot(conn, &db_path, &backup_id)?;
        serde_json::to_value(validation).map_err(|e| e.to_string())
    })
}

// KOS-51: атомарный restore. Порядок захвата: BACKUP_GATE → DB mutex
// (with_conn_mut). Restore держит оба на всю операцию — ни backup, ни
// обычные ARK ops не выполняются параллельно, момента с отсутствующей
// или частично заменённой primary DB нет (Online Backup API пишет
// destination транзакционно).
pub(super) async fn db_backup_restore(
    state: &Arc<ServiceState>,
    backup_id: String,
) -> Result<Value, String> {
    let db_path = current_db_path(state)?;
    let _gate = state.backup_gate.lock().unwrap_or_else(|e| e.into_inner());
    with_conn_mut(state, |conn| {
        let report = db::restore_snapshot(conn, &db_path, &backup_id)?;
        serde_json::to_value(report).map_err(|e| e.to_string())
    })
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
