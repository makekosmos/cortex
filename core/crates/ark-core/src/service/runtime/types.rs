use super::*;

pub(super) async fn handle(state: &Arc<ServiceState>, request: Request) -> Result<Value, String> {
    match request {
        Request::TypesList => with_conn(state, |conn| {
            serde_json::to_value(crate::type_registry::list_type_summaries(conn)?)
                .map_err(|e| e.to_string())
        }),
        Request::TypesRegisterPackageDefinitions { registrations } => {
            with_write_tx(state, |conn| {
                for registration in &registrations {
                    crate::type_registry::register_type(conn, registration)?;
                }
                Ok(json!(true))
            })
        }
        Request::TypesGet { type_id, version } => with_conn(state, |conn| {
            serde_json::to_value(crate::type_registry::get_type(
                conn,
                &type_id,
                version.as_deref(),
            )?)
            .map_err(|e| e.to_string())
        }),
        Request::TypesListVersions { type_id } => with_conn(state, |conn| {
            let Some(canonical) = crate::type_registry::resolve_type_id(conn, &type_id)? else {
                return Ok(json!([]));
            };
            serde_json::to_value(crate::type_registry::list_type_versions(conn, &canonical)?)
                .map_err(|e| e.to_string())
        }),
        Request::TypesResolveAlias { alias } => with_conn(state, |conn| {
            serde_json::to_value(crate::type_registry::resolve_alias(conn, &alias)?)
                .map_err(|e| e.to_string())
        }),
        Request::ListObjectTypes => with_conn(state, |conn| {
            let object_types = db::list_object_types(conn)?;
            serde_json::to_value(object_types).map_err(|e| e.to_string())
        }),
        Request::GetObjectType { id } => with_conn(state, |conn| {
            let object_type = db::get_object_type(conn, &id)?;
            serde_json::to_value(object_type).map_err(|e| e.to_string())
        }),
        Request::UpsertObjectType {
            object_type,
            device_id,
        } => {
            // replay_pending_for_type теперь использует SAVEPOINT ark_replay_pending
            // вместо BEGIN IMMEDIATE, поэтому вкладывается в транзакцию из with_write_tx.
            // entity-строка + sync-meta записываются атомарно.
            let entity = with_write_tx(state, |conn| {
                db::upsert_object_type(conn, &object_type)?;
                let hlc = record_local_upsert(conn, "object_type", &object_type.id, device_id)?;
                Ok(make_sync_entity(
                    "object_type",
                    &object_type.id,
                    serde_json::to_value(&object_type).unwrap_or(json!({})),
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
        Request::DeleteObjectType { id, device_id } => {
            let entity = with_write_tx(state, |conn| {
                db::delete_object_type(conn, &id)?;
                let hlc = record_local_delete(conn, "object_type", &id, device_id)?;
                Ok(make_sync_entity(
                    "object_type",
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
        Request::ListObjectLinks => with_conn(state, |conn| {
            let object_links = db::list_object_links(conn)?;
            serde_json::to_value(object_links).map_err(|e| e.to_string())
        }),
        Request::UpsertObjectLink {
            object_link,
            device_id,
        } => {
            let entity = with_write_tx(state, |conn| {
                db::upsert_object_link(conn, &object_link)?;
                let hlc = record_local_upsert(conn, "object_link", &object_link.id, device_id)?;
                Ok(make_sync_entity(
                    "object_link",
                    &object_link.id,
                    serde_json::to_value(&object_link).unwrap_or(json!({})),
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
        Request::DeleteObjectLink { id, device_id } => {
            let entity = with_write_tx(state, |conn| {
                db::delete_object_link(conn, &id)?;
                let hlc = record_local_delete(conn, "object_link", &id, device_id)?;
                Ok(make_sync_entity(
                    "object_link",
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

        _ => unreachable!("request routed to the wrong runtime handler"),
    }
}
