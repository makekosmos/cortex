// ---------------------------------------------------------------------------
// DB helpers — all take `state` (the per-service slot that used to be the
// sidecar's process-wide `DB`/`DB_PATH`/`BACKUP_GATE` statics).
// ---------------------------------------------------------------------------

use super::*;

#[cfg(test)]
pub(super) fn with_conn<T, F>(state: &ServiceState, f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    get_db_handle(state)?.with_connection(f)
}

pub(super) fn get_db_handle(state: &ServiceState) -> Result<db::DbHandle, String> {
    state
        .db
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .cloned()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())
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
    let origin =
        ark_domain::types::is_sequenced_usage_entity(entity_type).then(|| HLC::from_string(&hlc));
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

pub(super) fn emit_replay_notices(notices: Vec<ark_application::sync_storage::ReplayNotice>) {
    for notice in notices {
        emit_event(notice.event_payload());
    }
}

/// Spawns the live-change broadcast only when a sync runtime is running —
/// without one the detached task is a no-op anyway, and the captured
/// `Arc<ServiceState>` would keep the db connection alive past teardown
/// (KOS-270). A sync runtime that starts between the check and the spawn
/// misses nothing: it loads current state on startup.
pub(super) async fn maybe_broadcast_local_change(state: &Arc<ServiceState>, entity: SyncEntity) {
    maybe_broadcast_local_changes(state, vec![entity]).await;
}

/// Same guard for a batch of entities (one spawned task fanning out all).
pub(super) async fn maybe_broadcast_local_changes(
    state: &Arc<ServiceState>,
    entities: Vec<SyncEntity>,
) {
    if entities.is_empty() || state.sync.lock().await.is_none() {
        return;
    }
    let state = Arc::clone(state);
    tokio::spawn(async move {
        for entity in entities {
            broadcast_local_change(&state, entity).await;
        }
    });
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

    runtime.server.broadcast_live_change(None).await;

    let clients: Vec<_> = runtime.clients.lock().await.values().cloned().collect();
    for client in clients {
        client.broadcast_live_change().await;
    }
    if let Some(relay) = runtime.relay.as_ref() {
        // Ошибку отправки логируем, но не пробрасываем — live broadcast best-effort.
        if let Err(e) = relay.broadcast_live_change(entity.clone()).await {
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
) -> ark_application::canonical_compatibility::LegacyRecord {
    ark_application::canonical_compatibility::LegacyRecord {
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

// ---------------------------------------------------------------------------

mod dispatch;
mod integration;
mod legacy;
mod objects;
mod system;
mod types;
mod usage;

pub(crate) use dispatch::handle_request;
