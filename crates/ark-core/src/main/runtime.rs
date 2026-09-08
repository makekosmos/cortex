// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
        if let Err(e) = serve().await {
            eprintln!("ark-core-rpc fatal: {e}");
            std::process::exit(1);
        }
    });
}

async fn serve() -> Result<(), String> {
    // Event channel + stdout writer
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Value>();
    set_event_sender(event_tx);

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            write_event_line(&event);
        }
    });

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        let line: String = line;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(trimmed) {
            Ok(raw) => {
                let request_id = request_id_from_value(&raw);
                match serde_json::from_value::<Request>(raw) {
                    Ok(req) => match handle_request(req).await {
                        Ok(data) => response_ok(data, request_id),
                        Err(e) => response_error(e, request_id),
                    },
                    Err(e) => response_error(e.to_string(), request_id),
                }
            }
            Err(e) => response_error(e.to_string(), None),
        };
        write_response_line(&response);
    }
    Ok(())
}

fn request_id_from_value(value: &Value) -> Option<Value> {
    // SDK кладёт envelope-id в `_req_id`. Старый формат (`id`) тоже принимаем
    // для обратной совместимости с прежним протоколом.
    value
        .get("_req_id")
        .cloned()
        .or_else(|| value.get("id").cloned())
}

fn response_ok(data: Value, request_id: Option<Value>) -> Value {
    response_with_optional_id(json!({ "ok": true, "data": data }), request_id)
}

fn response_error(error: String, request_id: Option<Value>) -> Value {
    response_with_optional_id(json!({ "ok": false, "error": error }), request_id)
}

fn response_with_optional_id(mut response: Value, request_id: Option<Value>) -> Value {
    if let (Value::Object(map), Some(id)) = (&mut response, request_id) {
        // Эхо envelope-id под `_req_id`. Старый формат (`id`) тоже дублируем
        // для SDK-версий, читающих legacy-поле.
        map.insert("_req_id".to_string(), id.clone());
        map.insert("id".to_string(), id);
    }
    response
}

// ---------------------------------------------------------------------------
// Stdout writers (shared lock to prevent interleaving of response + event)
// ---------------------------------------------------------------------------

static STDOUT_LOCK: StdMutex<()> = StdMutex::new(());

fn write_line(value: &Value) {
    let json = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _guard = STDOUT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(json.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

fn write_response_line(value: &Value) {
    write_line(value);
}

fn write_event_line(value: &Value) {
    write_line(value);
}

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

fn with_conn<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    let outer = DB.lock().unwrap_or_else(|e| e.into_inner());
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
fn with_write_tx<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    with_conn(|conn| {
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

fn get_shared_conn() -> Result<Arc<StdMutex<rusqlite::Connection>>, String> {
    let guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .cloned()
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
/// НЕ process-wide — ark-core-rpc обслуживает интерактивные ARK ops.
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
/// Если SYNC не запущен — тихий no-op.
async fn broadcast_local_change(entity: SyncEntity) {
    let runtime = {
        let guard = SYNC.lock().await;
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
) -> ark_core::canonical_types::compatibility::LegacyRecord {
    ark_core::canonical_types::compatibility::LegacyRecord {
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
    records: &[ark_core::canonical_types::compatibility::LegacyRecord],
    device_id: Option<String>,
) -> Result<Vec<SyncEntity>, String> {
    ark_core::canonical_types::facades::write_legacy_records(conn, records, "rpc", device_id)
}

fn tombstone_legacy(
    conn: &rusqlite::Connection,
    id: &str,
    expected_type_id: &str,
    device_id: Option<String>,
) -> Result<SyncEntity, String> {
    ark_core::canonical_types::facades::delete_legacy_object(conn, id, expected_type_id, device_id)
}

// ---------------------------------------------------------------------------


#[path = "runtime/legacy.rs"]
mod runtime_legacy;
#[path = "runtime/objects.rs"]
mod runtime_objects;
#[path = "runtime/system.rs"]
mod runtime_system;
#[path = "runtime/types.rs"]
mod runtime_types;
#[path = "runtime/usage.rs"]
mod runtime_usage;
#[path = "runtime/integration.rs"]
mod runtime_integration;

async fn handle_request(request: Request) -> Result<Value, String> {
    match request {
        request @ (Request::Init { .. }
        | Request::LoadAll
        | Request::UpsertTodo { .. }
        | Request::DeleteTodo { .. }
        | Request::DeleteProject { .. }
        | Request::BatchUpsertTodos { .. }
        | Request::UpsertProject { .. }
        | Request::UpsertTag { .. }
        | Request::UpsertTrackedApp { .. }
        | Request::DeleteTrackedApp { .. }) => runtime_legacy::handle(request).await,

        request @ (Request::UpsertUsageSession { .. }
        | Request::DeleteUsageSession { .. }
        | Request::UpsertUsageEvent { .. }
        | Request::DeleteUsageEvent { .. }
        | Request::UpsertUsageSpan { .. }
        | Request::GetUsageTitleTotal { .. }
        | Request::GetUsageAnalytics { .. }
        | Request::ListRecentUsageProcesses { .. }
        | Request::SearchUsageProcesses { .. }
        | Request::GetUsageGamePlaytimeSummary { .. }) => runtime_usage::handle(request).await,

        request @ (Request::ListObjects
        | Request::ListObjectSummaries
        | Request::ListObjectsByType { .. }
        | Request::ListObjectSummariesByType { .. }
        | Request::ListRunningTimeEntries { .. }
        | Request::GetObjectsByIds { .. }
        | Request::SearchObjects { .. }
        | Request::GetObject { .. }
        | Request::GetObjectWriteSnapshot { .. }
        | Request::CanonicalGameList { .. }
        | Request::CanonicalGameGet { .. }
        | Request::CanonicalGameUpsert { .. }
        | Request::CanonicalAssetSources { .. }
        | Request::CanonicalSetBookCover { .. }
        | Request::UpsertObject { .. }
        | Request::DeleteObject { .. }) => runtime_objects::handle(request).await,

        request @ (Request::TypesList
        | Request::TypesRegisterPackageDefinitions { .. }
        | Request::TypesGet { .. }
        | Request::TypesListVersions { .. }
        | Request::TypesResolveAlias { .. }
        | Request::ListObjectTypes
        | Request::GetObjectType { .. }
        | Request::UpsertObjectType { .. }
        | Request::DeleteObjectType { .. }
        | Request::ListObjectLinks
        | Request::UpsertObjectLink { .. }
        | Request::DeleteObjectLink { .. }) => runtime_types::handle(request).await,

        request @ (Request::GetSyncKv { .. }
        | Request::SetSyncKv { .. }
        | Request::ExternalRefsUpsert { .. }
        | Request::ClearAll
        | Request::DeleteTrashed
        | Request::DbBackup { .. }
        | Request::StartSync { .. }
        | Request::StopSync
        | Request::BroadcastChange { .. }
        | Request::GetConnectedPeers
        | Request::GetSyncSnapshot
        | Request::DisconnectPeer { .. }
        | Request::ConnectWithPairingCode { .. }
        | Request::LeaveSpace
        | Request::AddSeedPeer { .. }
        | Request::GetOwnAddresses { .. }
        | Request::GetHostDeviceName
        | Request::GetOwnIrohTicket) => runtime_system::handle(request).await,

        request @ (Request::IntegrationPersistNodeAuthorization { .. }
        | Request::IntegrationPersistIntegrationGrant { .. }
        | Request::IntegrationPrepareSignedSync { .. }
        | Request::IntegrationValidateOutboundSignedSync { .. }
        | Request::IntegrationSendSignedSync { .. }
        | Request::IntegrationAcquireRefreshLease { .. }
        | Request::IntegrationPublishCredentialEnvelope { .. }
        | Request::IntegrationLoadLatestCredentialEnvelope { .. }
        | Request::IntegrationLookupIssuerEncryptionKey { .. }
        | Request::IntegrationLookupIssuerEncryptionKeyForPublish { .. }
        | Request::IntegrationVerificationStatus { .. }) => {
            runtime_integration::handle(request).await
        }
    }
}
