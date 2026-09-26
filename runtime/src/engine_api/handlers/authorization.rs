#[derive(Debug, Deserialize)]
struct AppRpcEnvelope {
    #[serde(rename = "_req_id", alias = "id", default)]
    request_id: Option<String>,
    operation: String,
    #[serde(default)]
    params: Value,
}

const APP_GRAPH_READS: &[&str] = &[
    "list_object_types",
    "list_objects",
    "list_objects_by_type",
    "list_object_summaries",
    "list_object_summaries_by_type",
    "get_object",
    "get_object_type",
    "search_objects",
    "list_object_links",
];
const APP_GRAPH_WRITES: &[&str] = &[
    "upsert_object",
    "delete_object",
    "upsert_object_link",
    "delete_object_link",
];
fn parse_app_rpc(
    value: Value,
    grant: &LaunchGrant,
) -> Result<(Option<String>, String, Value), &'static str> {
    let envelope: AppRpcEnvelope = serde_json::from_value(value)
        .map_err(|_| "app request must contain operation and params")?;
    let operation = envelope.operation.as_str();
    if !(APP_GRAPH_READS.contains(&operation)
        || APP_GRAPH_WRITES.contains(&operation)
        || (crate::runtime_grants::dictation_operation_capability(operation).is_some()
            && grant.allows_dictation_operation(operation))
        || (crate::runtime_grants::focus_operation_capability(operation).is_some()
            && grant.allows_focus_operation(operation))
        || (crate::runtime_grants::agents_operation_capability(operation).is_some()
            && grant.allows_agents_operation(operation))
        || crate::runtime_grants::app_network_operation_scope(operation)
            .is_some_and(|scope| grant.allows_app_network_scope(scope))
        || (!operation.starts_with("agents.") && grant.allows_worker_operation(operation)))
    {
        return Err("unsupported app operation");
    }
    let params = if envelope.params.is_null() {
        Value::Object(serde_json::Map::new())
    } else if envelope.params.is_object() {
        envelope.params
    } else {
        return Err("app params must be an object");
    };
    Ok((envelope.request_id, envelope.operation, params))
}

fn canonical_type_id(type_id: &str) -> String {
    ark_core::canonical_types::definitions::canonical_type_registrations()
        .ok()
        .and_then(|registrations| {
            registrations.into_iter().find_map(|registration| {
                (registration.type_id == type_id
                    || registration
                        .aliases
                        .iter()
                        .any(|alias| alias.alias == type_id))
                .then_some(registration.type_id)
            })
        })
        .unwrap_or_else(|| type_id.to_owned())
}

fn param_str<'a>(params: &'a Value, snake: &str, camel: &str) -> Option<&'a str> {
    params
        .get(snake)
        .or_else(|| params.get(camel))
        .and_then(Value::as_str)
}

fn grant_rule_matches<'a>(
    grant: &'a LaunchGrant,
    type_id: &str,
    type_version: Option<&str>,
    action: &str,
) -> Option<&'a crate::runtime_grants::GrantRule> {
    grant.rules.iter().find(|rule| {
        rule.type_id == type_id
            && rule.actions.contains(action)
            && type_version.is_none_or(|version| {
                semver::Version::parse(version).ok().is_some_and(|version| {
                    rule.versions.iter().any(|requirement| {
                        semver::VersionReq::parse(requirement)
                            .ok()
                            .is_some_and(|requirement| requirement.matches(&version))
                    })
                })
            })
    })
}

fn grant_authorizes(
    grant: &LaunchGrant,
    type_id: &str,
    type_version: Option<&str>,
    action: &str,
    fields: &[String],
    relations: &[String],
) -> Result<(), &'static str> {
    let rule = grant_rule_matches(grant, type_id, type_version, action).ok_or("data grant denied")?;
    let allowed_fields = if matches!(action, "read" | "subscribe") {
        &rule.fields_read
    } else {
        &rule.fields_write
    };
    let allowed_relations = if matches!(action, "read" | "subscribe") {
        &rule.relations_read
    } else {
        &rule.relations_write
    };
    if fields.iter().any(|field| !allowed_fields.contains(field))
        || relations
            .iter()
            .any(|relation| !allowed_relations.contains(relation))
    {
        return Err("data grant denied");
    }
    Ok(())
}

fn broad_read_authorized(grant: &LaunchGrant) -> bool {
    grant.rules.iter().any(|rule| rule.actions.contains("read"))
}

fn object_field_inputs(object: &Value) -> Result<(Vec<FieldInput>, Vec<String>), &'static str> {
    let map = object.as_object().ok_or("object must be an object")?;
    let mut fields = Vec::new();
    let mut names = Vec::new();
    for (key, field_id) in [("title", "title"), ("contentJson", "content"), ("content_json", "content")] {
        if let Some(value) = map.get(key) {
            if key == "content_json" && map.contains_key("contentJson") {
                continue;
            }
            fields.push(FieldInput {
                field_id: field_id.to_owned(),
                value: serde_json::from_value(value.clone()).map_err(|_| "invalid object field")?,
            });
            names.push(field_id.to_owned());
        }
    }
    let props = map.get("propsJson").or_else(|| map.get("props_json"));
    if let Some(Value::Object(props)) = props {
        for (key, value) in props {
            let field_id = format!("props.{key}");
            fields.push(FieldInput {
                field_id: field_id.clone(),
                value: serde_json::from_value(value.clone()).map_err(|_| "invalid object field")?,
            });
            names.push(field_id);
        }
    } else if props.is_some() {
        return Err("propsJson must be an object");
    }
    Ok((fields, names))
}

fn object_type_and_version(object: &Value) -> Result<(String, String), &'static str> {
    let type_id = param_str(object, "type_id", "typeId").ok_or("object type is required")?;
    let canonical = canonical_type_id(type_id);
    let type_version = param_str(object, "type_version", "typeVersion")
        .map(str::to_owned)
        .or_else(|| {
            ark_core::canonical_types::definitions::canonical_type_registrations()
                .ok()
                .and_then(|registrations| {
                    registrations
                        .into_iter()
                        .find(|registration| registration.type_id == canonical)
                        .map(|registration| registration.version)
                })
        })
        .ok_or("object type version is required")?;
    Ok((canonical, type_version))
}

fn object_type_and_version_for_write(
    object: &Value,
    grant: &LaunchGrant,
) -> Result<(String, String), &'static str> {
    let type_id = canonical_type_id(param_str(object, "type_id", "typeId").ok_or("object type is required")?);
    if let Some(version) = param_str(object, "type_version", "typeVersion") {
        return Ok((type_id, version.to_owned()));
    }
    let version = grant
        .rules
        .iter()
        .find(|rule| rule.type_id == type_id)
        .and_then(|rule| (rule.versions.len() == 1).then(|| &rule.versions[0]))
        .and_then(|requirement| requirement.strip_prefix('='))
        .filter(|version| semver::Version::parse(version).is_ok())
        .ok_or("object type version is required")?;
    Ok((type_id, version.to_owned()))
}

fn data_request_allowed(grant: &LaunchGrant, request: DataRequest) -> Result<(), &'static str> {
    grant.authorize_request(&request).map_err(|_| "data grant denied")
}

async fn internal_app_lookup(
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
    operation: &str,
    params: Value,
) -> Option<Value> {
    let request = DispatchRequest {
        request_id: Some(request_id()),
        operation: Operation::Named(operation.to_owned()),
        params,
        client: client.clone(),
    };
    let response = dispatcher.dispatch(request).await.ok()?;
    if response.get("ok").is_some() {
        response
            .get("ok")
            .and_then(Value::as_bool)
            .filter(|ok| *ok)
            .and_then(|_| response.get("data").cloned())
    } else {
        Some(response)
    }
}

async fn object_write_snapshot(
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
    id: &str,
) -> Result<Value, &'static str> {
    internal_app_lookup(
        dispatcher,
        client,
        "get_object_write_snapshot",
        json!({ "id": id }),
    )
    .await
    .ok_or("data grant denied")
}

fn launch_binding_current(
    package_service: &PackageService,
    asset: &AssetGrant,
    grant: &LaunchGrant,
) -> bool {
    package_service
        .resolve_app(&asset.id, Some(&asset.version))
        .ok()
        .and_then(|resolved| {
            (resolved.package.hash.eq_ignore_ascii_case(&asset.hash)
                && resolved.grant.package_id == grant.package_id
                && resolved.grant.package_version == grant.package_version
                && resolved.grant.manifest_digest == grant.manifest_digest)
                .then_some(())
        })
        .is_some()
}
