async fn authorize_app_request(
    operation: &str,
    mut params: Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
) -> Result<Value, &'static str> {
    let map = params.as_object_mut().ok_or("app params must be an object")?;
    if let Some(denied) = crate::runtime_grants::scoped_capability_denied(operation, grant) {
        return Err(denied);
    }
    match operation {
        operation if crate::runtime_grants::dictation_operation_capability(operation).is_some() => {
            if !grant.allows_dictation_operation(operation) {
                return Err("dictation grant denied");
            }
        }
        operation if crate::runtime_grants::focus_operation_capability(operation).is_some() => {
            if !grant.allows_focus_operation(operation) {
                return Err("focus grant denied");
            }
        }
        operation if crate::runtime_grants::agents_operation_capability(operation).is_some() => {
            if !grant.allows_agents_operation(operation) {
                return Err("agents grant denied");
            }
        }
        operation if crate::runtime_grants::app_network_operation_scope(operation).is_some() =>
            crate::runtime_grants::require_app_network_scope(operation, grant)?,
        operation if !operation.starts_with("agents.") && grant.allows_worker_operation(operation) => {}
        "list_objects_by_type" | "list_object_summaries_by_type" => {
            let raw_type = map
                .get("type_id")
                .or_else(|| map.get("typeId"))
                .and_then(Value::as_str)
                .ok_or("type_id is required")?;
            let canonical = canonical_type_id(raw_type);
            grant_authorizes(grant, &canonical, None, "read", &[], &[])?;
            map.insert("type_id".into(), Value::String(canonical));
            map.remove("typeId");
        }
        "list_objects" | "list_object_summaries" | "list_object_types" => {
            if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "get_object" => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object id is required")?;
            let object = internal_app_lookup(dispatcher, client, "get_object", json!({ "id": id }))
                .await
                .ok_or("data grant denied")?;
            if let Some(object) = object.as_object() {
                let (type_id, version) = object_type_and_version(&Value::Object(object.clone()))?;
                grant_authorizes(grant, &type_id, Some(&version), "read", &[], &[])?;
            } else if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "get_object_type" => {
            let raw_type = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object type id is required")?;
            let canonical = canonical_type_id(raw_type);
            grant_authorizes(grant, &canonical, None, "read", &[], &[])?;
            map.insert("id".into(), Value::String(canonical));
        }
        "search_objects" | "list_object_links" => {
            if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "upsert_object" => {
            let object = map.get("object").ok_or("object is required")?;
            let (type_id, type_version) = object_type_and_version_for_write(object, grant)?;
            let (fields, _) = object_field_inputs(object)?;
            let object_id = param_str(object, "id", "id").map(str::to_owned);
            let create_request = || DataRequest::CreateObject {
                type_id: type_id.clone(),
                type_version: type_version.clone(),
                object_id: object_id.clone(),
                fields: fields.clone(),
                links: vec![],
            };
            let update_request = |object_id: String| DataRequest::UpdateObject {
                type_id: type_id.clone(),
                type_version: type_version.clone(),
                object_id,
                expected_hlc: None,
                fields: fields.clone(),
                links: vec![],
            };
            let (create, update, existing, snapshot) = match object_id.as_deref() {
                None => (
                    data_request_allowed(grant, create_request()).is_ok(),
                    false,
                    None,
                    None,
                ),
                Some(object_id) => {
                    let snapshot = object_write_snapshot(dispatcher, client, object_id).await?;
                    if snapshot.get("exists").and_then(Value::as_bool) == Some(false) {
                        (
                            data_request_allowed(grant, create_request()).is_ok(),
                            false,
                            None,
                            Some(snapshot),
                        )
                    } else {
                        let (existing_type, existing_version) = object_type_and_version(&snapshot)?;
                        if existing_type != type_id || existing_version != type_version {
                            return Err("data grant denied");
                        }
                        let existing = internal_app_lookup(
                            dispatcher,
                            client,
                            "get_object",
                            json!({ "id": object_id }),
                        )
                        .await
                        .ok_or("data grant denied")?;
                        if object_type_and_version(&existing)?
                            != (existing_type.clone(), existing_version.clone())
                        {
                            return Err("data grant denied");
                        }
                        (
                            false,
                            data_request_allowed(grant, update_request(object_id.to_owned()))
                                .is_ok(),
                            Some(existing),
                            Some(snapshot),
                        )
                    }
                }
            };
            if !create && !update {
                return Err("data grant denied");
            }
            if let Some(object) = map.get_mut("object").and_then(Value::as_object_mut) {
                object.retain(|key, _| {
                    matches!(
                        key.as_str(),
                        "id" | "typeId"
                            | "type_id"
                            | "typeVersion"
                            | "type_version"
                            | "title"
                            | "contentJson"
                            | "content_json"
                            | "propsJson"
                            | "props_json"
                    )
                });
                object.insert("typeId".into(), Value::String(type_id));
                object.insert("typeVersion".into(), Value::String(type_version));
                let now = Value::String(chrono::Utc::now().to_rfc3339());
                object.insert(
                    "createdAt".into(),
                    existing
                        .as_ref()
                        .and_then(|value| value.get("createdAt"))
                        .cloned()
                        .unwrap_or_else(|| now.clone()),
                );
                object.insert("updatedAt".into(), now);
                object.insert(
                    "deletedAt".into(),
                    existing
                        .as_ref()
                        .and_then(|value| value.get("deletedAt"))
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                object.remove("type_id");
                object.remove("type_version");
            }
            if let Some(snapshot) = snapshot {
                map.insert("expectedSnapshot".into(), snapshot);
            }
        }
        "delete_object" => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object id is required")?
                .to_owned();
            let snapshot = object_write_snapshot(dispatcher, client, &id).await?;
            let (type_id, version) = object_type_and_version(&snapshot)?;
            data_request_allowed(
                grant,
                DataRequest::DeleteObject {
                    type_id,
                    type_version: version,
                    object_id: id,
                    expected_hlc: None,
                },
            )?;
            map.insert("expectedSnapshot".into(), snapshot);
        }
        "upsert_object_link" => {
            let link = map
                .get("object_link")
                .or_else(|| map.get("objectLink"))
                .ok_or("object_link is required")?;
            let (source_id, target_id, relation) = link_parts(link)?;
            let source = internal_app_lookup(
                dispatcher,
                client,
                "get_object",
                json!({ "id": source_id }),
            )
            .await
            .ok_or("data grant denied")?;
            let target = internal_app_lookup(
                dispatcher,
                client,
                "get_object",
                json!({ "id": target_id }),
            )
            .await
            .ok_or("data grant denied")?;
            authorize_link(grant, &source, &target, relation, true)?;
        }
        "delete_object_link" => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object link id is required")?;
            let links = internal_app_lookup(dispatcher, client, "list_object_links", json!({}))
                .await
                .ok_or("data grant denied")?;
            let link = links
                .as_array()
                .and_then(|links| {
                    links
                        .iter()
                        .find(|link| link.get("id").and_then(Value::as_str) == Some(id))
                })
                .ok_or("data grant denied")?;
            let (source_id, target_id, relation) = link_parts(link)?;
            let source = internal_app_lookup(
                dispatcher,
                client,
                "get_object",
                json!({ "id": source_id }),
            )
            .await
            .ok_or("data grant denied")?;
            let target = internal_app_lookup(
                dispatcher,
                client,
                "get_object",
                json!({ "id": target_id }),
            )
            .await
            .ok_or("data grant denied")?;
            authorize_link(grant, &source, &target, relation, true)?;
        }
        _ => return Err("unsupported app operation"),
    }
    Ok(params)
}


fn link_parts(link: &Value) -> Result<(&str, &str, &str), &'static str> {
    let source = param_str(link, "source_object_id", "sourceObjectId")
        .ok_or("source_object_id is required")?;
    let target = param_str(link, "target_object_id", "targetObjectId")
        .ok_or("target_object_id is required")?;
    let relation = param_str(link, "link_type", "linkType").ok_or("link_type is required")?;
    Ok((source, target, relation))
}

fn authorize_link(
    grant: &LaunchGrant,
    source: &Value,
    target: &Value,
    relation: &str,
    write: bool,
) -> Result<(), &'static str> {
    let (source_type, source_version) = object_type_and_version(source)?;
    let (target_type, target_version) = object_type_and_version(target)?;
    let source_rule = grant_rule_matches(
        grant,
        &source_type,
        Some(&source_version),
        if write { "link" } else { "read" },
    )
    .ok_or("data grant denied")?;
    let relations = if write {
        &source_rule.relations_write
    } else {
        &source_rule.relations_read
    };
    if !relations.iter().any(|value| value == relation) {
        return Err("data grant denied");
    }
    grant_authorizes(grant, &target_type, Some(&target_version), "read", &[], &[])
}
