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

async fn filter_app_response(
    operation: &str,
    response: Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
) -> Value {
    if response.get("ok").and_then(Value::as_bool) == Some(false) {
        return response;
    }
    let mut response = response;
    let data = if response.get("data").is_some() {
        response.get_mut("data").expect("data exists")
    } else {
        &mut response
    };
    match operation {
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
                        internal_app_lookup(dispatcher, client, "get_object", json!({ "id": id })).await
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
