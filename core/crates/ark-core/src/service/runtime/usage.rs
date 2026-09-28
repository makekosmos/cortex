use super::*;

pub(super) async fn handle(state: &Arc<ServiceState>, request: Request) -> Result<Value, String> {
    match request {
        Request::UpsertUsageSession {
            usage_session,
            device_id,
        } => {
            let entity = with_write_tx(state, |conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::upsert_usage_session(conn, &usage_session)?;
                let hlc = record_local_upsert(conn, "usage_session", &usage_session.id, device_id)?;
                Ok(make_sync_entity(
                    "usage_session",
                    &usage_session.id,
                    serde_json::to_value(&usage_session).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            let state = Arc::clone(state);
            tokio::spawn(async move {
                broadcast_local_change(&state, entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteUsageSession { id, device_id } => {
            let entity = with_write_tx(state, |conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::delete_usage_session(conn, &id)?;
                let hlc = record_local_delete(conn, "usage_session", &id, device_id)?;
                Ok(make_sync_entity(
                    "usage_session",
                    &id,
                    json!({}),
                    hlc,
                    Some(true),
                ))
            })?;
            let state = Arc::clone(state);
            tokio::spawn(async move {
                broadcast_local_change(&state, entity).await;
            });
            Ok(json!(true))
        }

        Request::UpsertUsageEvent {
            usage_event,
            device_id,
        } => {
            let entity = with_write_tx(state, |conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::upsert_usage_event(conn, &usage_event)?;
                let hlc = record_local_upsert(conn, "usage_event", &usage_event.id, device_id)?;
                Ok(make_sync_entity(
                    "usage_event",
                    &usage_event.id,
                    serde_json::to_value(&usage_event).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            let state = Arc::clone(state);
            tokio::spawn(async move {
                broadcast_local_change(&state, entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteUsageEvent { id, device_id } => {
            let entity = with_write_tx(state, |conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::delete_usage_event(conn, &id)?;
                let hlc = record_local_delete(conn, "usage_event", &id, device_id)?;
                Ok(make_sync_entity(
                    "usage_event",
                    &id,
                    json!({}),
                    hlc,
                    Some(true),
                ))
            })?;
            let state = Arc::clone(state);
            tokio::spawn(async move {
                broadcast_local_change(&state, entity).await;
            });
            Ok(json!(true))
        }
        Request::UpsertUsageSpan { usage_span } => {
            let entities = with_write_tx(state, |conn| {
                db::ensure_usage_sequence_migrated(conn, &usage_span.device_id)?;
                db::upsert_usage_span(conn, &usage_span)?
                    .into_iter()
                    .map(|day| {
                        let hlc = record_local_upsert(
                            conn,
                            "usage_day",
                            &day.id,
                            Some(usage_span.device_id.clone()),
                        )?;
                        Ok(make_sync_entity(
                            "usage_day",
                            &day.id,
                            serde_json::to_value(&day).unwrap_or(json!({})),
                            hlc,
                            None,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })?;
            let ids = entities
                .iter()
                .map(|entity| entity.id.clone())
                .collect::<Vec<_>>();
            let state = Arc::clone(state);
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(&state, entity).await;
                }
            });
            Ok(json!({ "usageDayIds": ids }))
        }
        Request::GetUsageTitleTotal { query } => with_conn(state, |conn| {
            serde_json::to_value(db::get_usage_title_total(conn, &query)?)
                .map_err(|error| error.to_string())
        }),
        Request::GetUsageAnalytics {
            range_days,
            top_apps_limit,
            recent_sessions_limit,
        } => with_conn(state, |conn| {
            let snapshot = db::load_usage_analytics(
                conn,
                range_days.unwrap_or(21),
                top_apps_limit.unwrap_or(8),
                recent_sessions_limit.unwrap_or(24),
            )?;
            serde_json::to_value(snapshot).map_err(|e| e.to_string())
        }),
        Request::ListRecentUsageProcesses { limit } => with_conn(state, |conn| {
            let candidates = db::list_recent_usage_processes(conn, limit.unwrap_or(10))?;
            serde_json::to_value(candidates).map_err(|e| e.to_string())
        }),
        Request::SearchUsageProcesses { query, limit } => with_conn(state, |conn| {
            let candidates = db::search_usage_processes(conn, &query, limit.unwrap_or(10))?;
            serde_json::to_value(candidates).map_err(|e| e.to_string())
        }),
        Request::GetUsageGamePlaytimeSummary {
            bindings,
            range_start,
            range_end,
        } => with_conn(state, |conn| {
            let summary = db::load_usage_game_playtime_summary(
                conn,
                &bindings,
                range_start.as_deref(),
                range_end.as_deref(),
            )?;
            serde_json::to_value(summary).map_err(|e| e.to_string())
        }),
        _ => unreachable!("request routed to the wrong runtime handler"),
    }
}
