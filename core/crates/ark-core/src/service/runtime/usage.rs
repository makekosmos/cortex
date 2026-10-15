use super::*;

async fn submit_local_usage_write(
    state: &ServiceState,
    device_id: String,
    write: crate::application::usage::LocalUsageWrite,
) -> Result<Vec<SyncEntity>, String> {
    let mutations = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::LocalUsageWriteCommand::new(device_id, write),
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(mutations
        .into_iter()
        .map(|mutation| {
            make_sync_entity(
                mutation.entity_type,
                &mutation.id,
                mutation.data,
                mutation.hlc,
                mutation.deleted,
            )
        })
        .collect())
}

pub(super) async fn upsert_usage_session(
    state: &Arc<ServiceState>,
    usage_session: UsageSession,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entities = submit_local_usage_write(
        state,
        local_write_device_id(device_id),
        crate::application::usage::LocalUsageWrite::UpsertSession(usage_session),
    )
    .await?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn delete_usage_session(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entities = submit_local_usage_write(
        state,
        local_write_device_id(device_id),
        crate::application::usage::LocalUsageWrite::DeleteSession(id),
    )
    .await?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn upsert_usage_event(
    state: &Arc<ServiceState>,
    usage_event: UsageEvent,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entities = submit_local_usage_write(
        state,
        local_write_device_id(device_id),
        crate::application::usage::LocalUsageWrite::UpsertEvent(usage_event),
    )
    .await?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn delete_usage_event(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entities = submit_local_usage_write(
        state,
        local_write_device_id(device_id),
        crate::application::usage::LocalUsageWrite::DeleteEvent(id),
    )
    .await?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn upsert_usage_span(
    state: &Arc<ServiceState>,
    usage_span: UsageSpanWrite,
) -> Result<Value, String> {
    let entities = submit_local_usage_write(
        state,
        usage_span.device_id.clone(),
        crate::application::usage::LocalUsageWrite::UpsertSpan(usage_span),
    )
    .await?;
    let ids = entities
        .iter()
        .map(|entity| entity.id.clone())
        .collect::<Vec<_>>();
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!({ "usageDayIds": ids }))
}

/// Один батч компакции журнала usage-синка (правило — в
/// `db::compact_usage_sync_log`). Возвращает `has_more`, чтобы Engine loop
/// знал, когда остановиться.
pub(super) async fn compact_usage_sync_log(
    state: &Arc<ServiceState>,
    older_than_days: Option<i64>,
    batch_limit: Option<i64>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let days = older_than_days
        .filter(|days| *days > 0)
        .unwrap_or(db::USAGE_SYNC_LOG_RETENTION_DAYS);
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let limit = batch_limit.unwrap_or(2_000).clamp(1, 50_000);
    with_write_tx(state, |conn| {
        let deleted = db::compact_usage_sync_log(conn, device_id.as_deref(), &cutoff, limit)?;
        Ok(json!({
            "deleted": deleted,
            "has_more": deleted as i64 == limit,
        }))
    })
}

pub(super) async fn get_usage_title_total(
    state: &Arc<ServiceState>,
    query: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        serde_json::to_value(db::get_usage_title_total(conn, &query)?)
            .map_err(|error| error.to_string())
    })
}

pub(super) async fn get_usage_analytics(
    state: &Arc<ServiceState>,
    range_days: Option<i64>,
    top_apps_limit: Option<i64>,
    recent_sessions_limit: Option<i64>,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let snapshot = db::load_usage_analytics(
            conn,
            range_days.unwrap_or(21),
            top_apps_limit.unwrap_or(8),
            recent_sessions_limit.unwrap_or(24),
            // Resolved by the Engine via GetSystemWindowsDirectoryW at service
            // construction — a request param would let clients lie about what
            // counts as a system app (KOS-287).
            state.windows_dir.as_deref(),
        )?;
        serde_json::to_value(snapshot).map_err(|e| e.to_string())
    })
}

pub(super) async fn list_recent_usage_processes(
    state: &Arc<ServiceState>,
    limit: Option<i64>,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let candidates = db::list_recent_usage_processes(conn, limit.unwrap_or(10))?;
        serde_json::to_value(candidates).map_err(|e| e.to_string())
    })
}

pub(super) async fn search_usage_processes(
    state: &Arc<ServiceState>,
    query: String,
    limit: Option<i64>,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let candidates = db::search_usage_processes(conn, &query, limit.unwrap_or(10))?;
        serde_json::to_value(candidates).map_err(|e| e.to_string())
    })
}

pub(super) async fn get_usage_game_playtime_summary(
    state: &Arc<ServiceState>,
    bindings: Vec<UsageGamePlaytimeBinding>,
    range_start: Option<String>,
    range_end: Option<String>,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let summary = db::load_usage_game_playtime_summary(
            conn,
            &bindings,
            range_start.as_deref(),
            range_end.as_deref(),
        )?;
        serde_json::to_value(summary).map_err(|e| e.to_string())
    })
}
