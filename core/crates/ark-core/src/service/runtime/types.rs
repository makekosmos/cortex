use super::*;

async fn type_read(state: &ServiceState, query: db::TypeReadQuery) -> Result<Value, String> {
    get_db_handle(state)?
        .query(db::CommandContext::engine(), query)
        .await
        .map_err(|error| error.to_string())
}

pub(super) async fn types_list(state: &Arc<ServiceState>) -> Result<Value, String> {
    type_read(state, db::TypeReadQuery::ListSummaries).await
}

pub(super) async fn types_register_package_definitions(
    state: &Arc<ServiceState>,
    registrations: Vec<crate::type_registry::TypeRegistration>,
) -> Result<Value, String> {
    let replay_notices = get_db_handle(state)?
        .submit_register_package_types(db::CommandContext::engine(), registrations)
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
    type_read(state, db::TypeReadQuery::Get { type_id, version }).await
}

pub(super) async fn types_list_versions(
    state: &Arc<ServiceState>,
    type_id: String,
) -> Result<Value, String> {
    type_read(state, db::TypeReadQuery::ListVersions(type_id)).await
}

pub(super) async fn types_resolve_alias(
    state: &Arc<ServiceState>,
    alias: String,
) -> Result<Value, String> {
    type_read(state, db::TypeReadQuery::ResolveAlias(alias)).await
}

pub(super) async fn list_object_types(state: &Arc<ServiceState>) -> Result<Value, String> {
    type_read(state, db::TypeReadQuery::ListObjectTypes).await
}

pub(super) async fn get_object_type(
    state: &Arc<ServiceState>,
    id: String,
) -> Result<Value, String> {
    type_read(state, db::TypeReadQuery::GetObjectType(id)).await
}

pub(super) async fn upsert_object_type(
    state: &Arc<ServiceState>,
    object_type: ObjectType,
    device_id: Option<String>,
) -> Result<Value, String> {
    let commit = get_db_handle(state)?
        .submit_upsert_object_type(db::CommandContext::engine(), object_type, device_id)
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
        .submit_delete_object_type(db::CommandContext::engine(), id, device_id)
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, commit.entities).await;
    Ok(json!(true))
}

pub(super) async fn list_object_links(state: &Arc<ServiceState>) -> Result<Value, String> {
    type_read(state, db::TypeReadQuery::ListObjectLinks).await
}

pub(super) async fn upsert_object_link(
    state: &Arc<ServiceState>,
    object_link: ObjectLink,
    device_id: Option<String>,
) -> Result<Value, String> {
    let commit = get_db_handle(state)?
        .submit_upsert_object_link(db::CommandContext::engine(), object_link, device_id)
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
        .submit_delete_object_link(db::CommandContext::engine(), id, device_id)
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
        .submit_replace_object_link(db::CommandContext::engine(), object_link, device_id)
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, commit.mutation.entities).await;
    serde_json::to_value(&commit.object_link).map_err(|e| e.to_string())
}
