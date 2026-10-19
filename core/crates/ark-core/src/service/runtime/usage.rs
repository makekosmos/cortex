use super::*;

async fn submit_local_usage_write(
    state: &ServiceState,
    device_id: String,
    write: crate::application::usage::LocalUsageWrite,
) -> Result<Vec<SyncEntity>, String> {
    let mutations = get_db_handle(state)?
        .submit_local_usage_write(db::CommandContext::engine(), device_id, write)
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
/// `ark-store::usage_sync_log`). Возвращает `has_more`, чтобы Engine loop
/// знал, когда остановиться.
pub(super) async fn compact_usage_sync_log(
    state: &Arc<ServiceState>,
    older_than_days: Option<i64>,
    batch_limit: Option<i64>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let days = older_than_days
        .filter(|days| *days > 0)
        .unwrap_or(ark_store::usage_sync_log::USAGE_SYNC_LOG_RETENTION_DAYS);
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let limit = batch_limit.unwrap_or(2_000).clamp(1, 50_000);
    let deleted = get_db_handle(state)?
        .submit(
            db::CommandContext::engine_maintenance(),
            db::CompactUsageSyncLogCommand::new(device_id, cutoff, limit),
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "deleted": deleted,
        "has_more": deleted as i64 == limit,
    }))
}

pub(super) async fn get_usage_title_total(
    state: &Arc<ServiceState>,
    query: String,
) -> Result<Value, String> {
    usage_read(state, db::UsageReadQuery::TitleTotal(query)).await
}

pub(super) async fn get_usage_analytics(
    state: &Arc<ServiceState>,
    range_days: Option<i64>,
    top_apps_limit: Option<i64>,
    recent_sessions_limit: Option<i64>,
) -> Result<Value, String> {
    usage_read(
        state,
        db::UsageReadQuery::Analytics {
            range_days: range_days.unwrap_or(21),
            top_apps_limit: top_apps_limit.unwrap_or(8),
            recent_sessions_limit: recent_sessions_limit.unwrap_or(24),
            // Resolved by the Engine at service construction, not from request data.
            windows_dir: state.windows_dir.clone(),
        },
    )
    .await
}

pub(super) async fn list_recent_usage_processes(
    state: &Arc<ServiceState>,
    limit: Option<i64>,
) -> Result<Value, String> {
    usage_read(
        state,
        db::UsageReadQuery::RecentProcesses(limit.unwrap_or(10)),
    )
    .await
}

pub(super) async fn search_usage_processes(
    state: &Arc<ServiceState>,
    query: String,
    limit: Option<i64>,
) -> Result<Value, String> {
    usage_read(
        state,
        db::UsageReadQuery::SearchProcesses {
            query,
            limit: limit.unwrap_or(10),
        },
    )
    .await
}

pub(super) async fn get_usage_game_playtime_summary(
    state: &Arc<ServiceState>,
    bindings: Vec<UsageGamePlaytimeBinding>,
    range_start: Option<String>,
    range_end: Option<String>,
) -> Result<Value, String> {
    usage_read(
        state,
        db::UsageReadQuery::GamePlaytimeSummary {
            bindings,
            range_start,
            range_end,
        },
    )
    .await
}

async fn usage_read(state: &ServiceState, query: db::UsageReadQuery) -> Result<Value, String> {
    get_db_handle(state)?
        .query(db::CommandContext::engine(), query)
        .await
        .map_err(|error| error.to_string())
}
