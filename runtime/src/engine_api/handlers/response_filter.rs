fn filter_object_value(value: &Value, grant: &LaunchGrant) -> Option<Value> {
    let mut object = value.as_object()?.clone();
    let (type_id, version) = object_type_and_version(value).ok()?;
    let rule = grant_rule_matches(grant, &type_id, Some(&version), "read")?;
    if !rule.fields_read.iter().any(|field| field == "title") {
        object.remove("title");
    }
    if !rule.fields_read.iter().any(|field| field == "content") {
        object.remove("contentJson");
        object.remove("content_json");
    }
    let props_key = if object.contains_key("propsJson") {
        Some("propsJson")
    } else if object.contains_key("props_json") {
        Some("props_json")
    } else {
        None
    };
    if let Some(props) = props_key
        .and_then(|key| object.get_mut(key))
        .and_then(Value::as_object_mut)
    {
        props.retain(|key, _| {
            rule.fields_read
                .iter()
                .any(|field| field == &format!("props.{key}"))
        });
    }
    Some(Value::Object(object))
}

fn filter_object_array(value: &mut Value, grant: &LaunchGrant) {
    let Some(objects) = value.as_array_mut() else {
        return;
    };
    objects.retain_mut(|object| {
        let Some(filtered) = filter_object_value(object, grant) else {
            return false;
        };
        *object = filtered;
        true
    });
}

fn filter_type_value(value: &Value, grant: &LaunchGrant) -> Option<Value> {
    let mut object = value.as_object()?.clone();
    let id = object
        .get("id")
        .or_else(|| object.get("typeId"))
        .and_then(Value::as_str)?;
    let canonical = canonical_type_id(id);
    let rule = grant_rule_matches(grant, &canonical, None, "read")?;
    for key in ["schemaJson", "uiSchemaJson"] {
        let Some(document) = object.get_mut(key) else {
            continue;
        };
        let Some(mut document_json) = document
            .as_str()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        else {
            continue;
        };
        let allowed_props = rule
            .fields_read
            .iter()
            .filter_map(|field| field.strip_prefix("props."))
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(properties) = document_json
            .get_mut("properties")
            .and_then(Value::as_object_mut)
        {
            properties.retain(|field, _| allowed_props.contains(field.as_str()));
        }
        if let Some(required) = document_json
            .get_mut("required")
            .and_then(Value::as_array_mut)
        {
            required.retain(|field| {
                field
                    .as_str()
                    .is_some_and(|field| allowed_props.contains(field))
            });
        }
        for field in [
            "visibleFields",
            "hiddenFields",
            "featuredFields",
            "readOnlyFields",
            "fieldOrder",
        ] {
            if let Some(values) = document_json.get_mut(field).and_then(Value::as_array_mut) {
                values.retain(|value| {
                    value
                        .as_str()
                        .is_some_and(|value| allowed_props.contains(value))
                });
            }
        }
        if let Ok(raw) = serde_json::to_string(&document_json) {
            *document = Value::String(raw);
        }
    }
    Some(Value::Object(object))
}

async fn filter_link_array(
    value: &mut Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
) {
    let Some(links) = value.as_array_mut() else {
        return;
    };
    let mut filtered = Vec::with_capacity(links.len());
    for link in links.iter() {
        let Ok((source_id, target_id, relation)) = link_parts(link) else {
            continue;
        };
        let Some(source) =
            internal_app_lookup(dispatcher, client, "get_object", json!({ "id": source_id })).await
        else {
            continue;
        };
        let Some(target) =
            internal_app_lookup(dispatcher, client, "get_object", json!({ "id": target_id })).await
        else {
            continue;
        };
        if authorize_link(grant, &source, &target, relation, false).is_ok() {
            filtered.push(link.clone());
        }
    }
    *links = filtered;
}

fn sanitize_dictation_config(value: &Value) -> Value {
    let Some(input) = value.as_object() else {
        return Value::Object(serde_json::Map::new());
    };
    let mut output = json!({
        "hotkey": input.get("hotkey").and_then(Value::as_str),
        "language": input.get("language").and_then(Value::as_str),
        "injectMode": input.get("injectMode").and_then(Value::as_str)
            .filter(|value| matches!(*value, "auto_paste" | "clipboard_only")),
        "provider": input.get("provider").and_then(Value::as_str)
            .filter(|value| matches!(*value, "groq" | "local")),
        "model": input.get("model").and_then(Value::as_str),
        "localModelId": input.get("localModelId").and_then(Value::as_str),
        "providerEnabled": input.get("providerEnabled").and_then(Value::as_bool),
    });
    output
        .as_object_mut()
        .expect("object")
        .retain(|_, value| !value.is_null());
    output
}

fn sanitize_dictation_get_config(value: &Value) -> Value {
    let Some(input) = value.as_object() else {
        return Value::Object(serde_json::Map::new());
    };
    let mut output = serde_json::Map::new();
    if let Some(config) = input.get("config") {
        output.insert("config".into(), sanitize_dictation_config(config));
    }
    if let Some(value) = input.get("hasApiKey").and_then(Value::as_bool) {
        output.insert("hasApiKey".into(), Value::Bool(value));
    }
    Value::Object(output)
}

fn sanitize_dictation_state(value: &Value) -> Value {
    let Some(input) = value.as_object() else {
        return Value::Object(serde_json::Map::new());
    };
    let mut output = serde_json::Map::new();
    if let Some(value) = input.get("state").and_then(Value::as_str) {
        output.insert("state".into(), Value::String(value.into()));
    }
    if let Some(value @ ("unknown" | "granted" | "denied" | "prompt")) =
        input.get("microphonePermission").and_then(Value::as_str)
    {
        output.insert("microphonePermission".into(), Value::String(value.into()));
    }
    Value::Object(output)
}

fn sanitize_dictation_local_models(value: &Value) -> Value {
    let Some(input) = value.as_object() else {
        return Value::Object(serde_json::Map::new());
    };
    let mut output = serde_json::Map::new();
    if let Some(value) = input.get("commandInstalled").and_then(Value::as_bool) {
        output.insert("commandInstalled".into(), Value::Bool(value));
    }
    let models = input
        .get("models")
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    let model = model.as_object()?;
                    let id = model.get("id").and_then(Value::as_str)?;
                    let name = model.get("name").and_then(Value::as_str)?;
                    Some(json!({
                        "id": id,
                        "name": name,
                        "transcriptionSupported": model
                            .get("transcriptionSupported")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        "directory": model
                            .get("directory")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        "downloaded": model
                            .get("downloaded")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    output.insert("models".into(), Value::Array(models));
    Value::Object(output)
}

async fn filter_app_response(
    operation: &str,
    response: Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
    type_id: Option<&str>,
) -> Value {
    if let Some(error) = public_app_error_response(&response) {
        // `{ok:false}` envelopes reaching this point carried the internal
        // reason until the redaction above — log it before it is lost.
        let reason = response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        crate::observability::app_rpc::log_app_rpc_rejection(
            client.class.as_deref().unwrap_or("-"),
            operation,
            type_id,
            crate::observability::app_rpc::RejectionReason::Dispatch(reason),
        );
        return error;
    }
    let mut response = response;
    let data = if response.get("data").is_some() {
        response.get_mut("data").expect("data exists")
    } else {
        &mut response
    };
    match operation {
        "dictation.get_config" | "dictation.update_config" => {
            *data = sanitize_dictation_get_config(data);
        }
        "dictation.get_state" => *data = sanitize_dictation_state(data),
        "dictation.list_local_models" => *data = sanitize_dictation_local_models(data),
        "list_objects"
        | "list_objects_by_type"
        | "list_object_summaries"
        | "list_object_summaries_by_type" => filter_object_array(data, grant),
        "get_object" => {
            *data = filter_object_value(data, grant).unwrap_or(Value::Null);
        }
        "list_object_types" => {
            if let Some(types) = data.as_array_mut() {
                types.retain_mut(|item| {
                    let Some(filtered) = filter_type_value(item, grant) else {
                        return false;
                    };
                    *item = filtered;
                    true
                });
            }
        }
        "get_object_type" => {
            *data = filter_type_value(data, grant).unwrap_or(Value::Null);
        }
        "search_objects" => {
            if let Some(results) = data.as_array_mut() {
                let mut filtered = Vec::with_capacity(results.len());
                for result in results.iter() {
                    let Some(id) = result.get("entryId").and_then(Value::as_str) else {
                        continue;
                    };
                    let Some(object) =
                        internal_app_lookup(dispatcher, client, "get_object", json!({ "id": id }))
                            .await
                    else {
                        continue;
                    };
                    if filter_object_value(&object, grant).is_some() {
                        let mut result = result.clone();
                        if let Some(map) = result.as_object_mut() {
                            map.insert("text".into(), Value::String(String::new()));
                        }
                        filtered.push(result);
                    }
                }
                *results = filtered;
            }
        }
        "list_object_links" => filter_link_array(data, grant, dispatcher, client).await,
        _ => {}
    }
    response
}
