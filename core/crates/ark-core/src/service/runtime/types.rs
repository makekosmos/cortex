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
    let replay_notices = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::RegisterPackageTypesCommand::new(registrations),
        )
        .await
        .map_err(|error| error.to_string())?;
    emit_replay_notices(replay_notices);
    Ok(json!(true))
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
    let commit = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::UpsertObjectTypeCommand::new(object_type, device_id),
        )
        .await
        .map_err(|error| error.to_string())?;
    emit_replay_notices(commit.replay_notices);
    maybe_broadcast_local_changes(state, commit.entities).await;
    Ok(json!(true))
}

pub(super) async fn delete_object_type(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let commit = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::DeleteObjectTypeCommand::new(id, device_id),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, commit.entities).await;
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
    let commit = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::UpsertObjectLinkCommand::new(object_link, device_id),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, commit.entities).await;
    Ok(json!(true))
}

pub(super) async fn delete_object_link(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let commit = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::DeleteObjectLinkCommand::new(id, device_id),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, commit.entities).await;
    Ok(json!(true))
}

/// Single-link replace (e.g. a task's one `project` link). Removes every
/// other link with the same source and link_type, then upserts the new one,
/// all in one database command. The deletes come first so the UNIQUE
/// (source, target, link_type) constraint is free for the new row. A rejected
/// upsert (missing source/target, via the foreign keys) rolls the deletes back.
pub(super) async fn replace_object_link(
    state: &Arc<ServiceState>,
    object_link: ObjectLink,
    device_id: Option<String>,
) -> Result<Value, String> {
    let commit = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::ReplaceObjectLinkCommand::new(object_link, device_id),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, commit.mutation.entities).await;
    serde_json::to_value(&commit.object_link).map_err(|e| e.to_string())
}
