use super::*;

pub(super) async fn types_list(state: &Arc<ServiceState>) -> Result<Value, String> {
    with_conn(state, |conn| {
        serde_json::to_value(crate::type_registry::list_type_summaries(conn)?)
            .map_err(|e| e.to_string())
    })
}

pub(super) async fn types_register_package_definitions(
    state: &Arc<ServiceState>,
    registrations: Vec<crate::type_registry::TypeRegistration>,
) -> Result<Value, String> {
    with_write_tx(state, |conn| {
        for registration in &registrations {
            crate::type_registry::register_type(conn, registration)?;
        }
        Ok(json!(true))
    })
}

pub(super) async fn types_get(
    state: &Arc<ServiceState>,
    type_id: String,
    version: Option<String>,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        serde_json::to_value(crate::type_registry::get_type(
            conn,
            &type_id,
            version.as_deref(),
        )?)
        .map_err(|e| e.to_string())
    })
}

pub(super) async fn types_list_versions(
    state: &Arc<ServiceState>,
    type_id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let Some(canonical) = crate::type_registry::resolve_type_id(conn, &type_id)? else {
            return Ok(json!([]));
        };
        serde_json::to_value(crate::type_registry::list_type_versions(conn, &canonical)?)
            .map_err(|e| e.to_string())
    })
}

pub(super) async fn types_resolve_alias(
    state: &Arc<ServiceState>,
    alias: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        serde_json::to_value(crate::type_registry::resolve_alias(conn, &alias)?)
            .map_err(|e| e.to_string())
    })
}

pub(super) async fn list_object_types(state: &Arc<ServiceState>) -> Result<Value, String> {
    with_conn(state, |conn| {
        let object_types = db::list_object_types(conn)?;
        serde_json::to_value(object_types).map_err(|e| e.to_string())
    })
}

pub(super) async fn get_object_type(
    state: &Arc<ServiceState>,
    id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let object_type = db::get_object_type(conn, &id)?;
        serde_json::to_value(object_type).map_err(|e| e.to_string())
    })
}

pub(super) async fn upsert_object_type(
    state: &Arc<ServiceState>,
    object_type: ObjectType,
    device_id: Option<String>,
) -> Result<Value, String> {
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
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}

pub(super) async fn delete_object_type(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
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
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}

pub(super) async fn list_object_links(state: &Arc<ServiceState>) -> Result<Value, String> {
    with_conn(state, |conn| {
        let object_links = db::list_object_links(conn)?;
        serde_json::to_value(object_links).map_err(|e| e.to_string())
    })
}

pub(super) async fn upsert_object_link(
    state: &Arc<ServiceState>,
    object_link: ObjectLink,
    device_id: Option<String>,
) -> Result<Value, String> {
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
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}

pub(super) async fn delete_object_link(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
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
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}
