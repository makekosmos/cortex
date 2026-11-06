use super::*;

async fn type_read(state: &ServiceState, query: db::TypeReadQuery) -> Result<Value, String> {
    get_db_handle(state)?
        .query_with_store(db::CommandContext::engine(), query)
        .await
        .map_err(|error| error.to_string())
}

pub(super) async fn types_list(state: &Arc<ServiceState>) -> Result<Value, String> {
    type_read(state, db::TypeReadQuery::ListSummaries).await
}

/// Register a type supplied by an application through `types.register`.
/// `schema` is either the JSON schema object or its serialized form; the type
/// is stored in ARK and survives Engine restarts.
pub(super) async fn types_register(
    state: &Arc<ServiceState>,
    type_id: String,
    version: String,
    schema: Value,
    name: Option<String>,
    app_id: Option<String>,
) -> Result<Value, String> {
    let schema_json = match schema {
        Value::Object(_) => serde_json::to_string(&schema).map_err(|error| error.to_string())?,
        Value::String(raw) => {
            let parsed: Value = serde_json::from_str(&raw)
                .map_err(|error| format!("invalid schema JSON: {error}"))?;
            serde_json::to_string(&parsed).map_err(|error| error.to_string())?
        }
        _ => return Err("schema must be a JSON object".into()),
    };
    let owner_id = app_id
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            type_id
                .rsplit_once('.')
                .map(|(namespace, _)| namespace.to_owned())
        })
        .ok_or_else(|| format!("type id '{type_id}' has no namespace"))?;
    let registration = crate::type_registry::TypeRegistration {
        name: name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| type_id.clone()),
        type_id: type_id.clone(),
        schema_json,
        ui_schema_json: "{}".into(),
        content_contract_json: "{}".into(),
        relations_json: "[]".into(),
        sync_policy_json: "{}".into(),
        version: version.clone(),
        schema_hash: String::new(),
        owner_kind: "app".into(),
        owner_id: Some(owner_id),
        status: "active".into(),
        base_type_id: None,
        aliases: Vec::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let replay_notices = get_db_handle(state)?
        .submit_register_application_type(db::CommandContext::engine(), registration)
        .await
        .map_err(|error| error.to_string())?;
    emit_replay_notices(replay_notices);
    Ok(json!({ "typeId": type_id, "version": version }))
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
    for entity in &commit.entities {
        emit_event(super::objects::links_changed(entity, "upsert"));
    }
    maybe_broadcast_local_changes(state, commit.entities).await;
    Ok(json!(true))
}

pub(super) async fn delete_object_link(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let commit = get_db_handle(state)?
        .submit_delete_object_link(db::CommandContext::engine(), id.clone(), device_id)
        .await
        .map_err(|error| error.to_string())?;
    for entity in &commit.entities {
        emit_event(super::objects::links_changed(entity, "delete"));
    }
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
    emit_event(super::objects::links_changed_from(
        &commit.object_link,
        "replace",
    ));
    serde_json::to_value(&commit.object_link).map_err(|e| e.to_string())
}
