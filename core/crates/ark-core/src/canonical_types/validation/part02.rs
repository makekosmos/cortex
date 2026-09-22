
fn validate_schema(
    schema: &Value,
    value: &Value,
    pointer: &str,
    open: bool,
) -> Result<(), CanonicalValidationError> {
    let object = schema
        .as_object()
        .ok_or_else(|| invariant(pointer, "schema"))?;
    if let Some(types) = object.get("type") {
        let matches_type = |name: &str| match name {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ => false,
        };
        let ok = types.as_str().map(matches_type).unwrap_or_else(|| {
            types
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).any(matches_type))
                .unwrap_or(false)
        });
        if !ok {
            return Err(invalid(pointer, "type"));
        }
        if value.is_null() {
            return Ok(());
        }
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        if !values.iter().any(|candidate| candidate == value) {
            return Err(invalid(pointer, "enum"));
        }
    }
    if let Some(min) = object.get("minimum").and_then(Value::as_f64) {
        if value.as_f64().is_some_and(|n| n < min) {
            return Err(invalid(pointer, "minimum"));
        }
    }
    if let Some(max) = object.get("maximum").and_then(Value::as_f64) {
        if value.as_f64().is_some_and(|n| n > max) {
            return Err(invalid(pointer, "maximum"));
        }
    }
    if let Some(min) = object.get("minLength").and_then(Value::as_u64) {
        if value
            .as_str()
            .is_some_and(|s| s.chars().count() < min as usize)
        {
            return Err(invalid(pointer, "minLength"));
        }
    }
    if let Some(required) = object.get("required").and_then(Value::as_array) {
        let map = value.as_object().ok_or_else(|| invalid(pointer, "type"))?;
        for name in required.iter().filter_map(Value::as_str) {
            if !map.contains_key(name) {
                return Err(invalid(&child(pointer, name), "required"));
            }
        }
    }
    if let Some(map) = value.as_object() {
        if let Some(properties) = object.get("properties").and_then(Value::as_object) {
            if let Some(required) = object.get("required").and_then(Value::as_array) {
                for name in required.iter().filter_map(Value::as_str) {
                    if let (Some(field), Some(sub)) = (map.get(name), properties.get(name)) {
                        validate_schema(sub, field, &child(pointer, name), name == "extensions")?;
                    }
                }
            }
            for (name, sub) in properties {
                let required = object
                    .get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(name)));
                if !required {
                    if let Some(field) = map.get(name) {
                        validate_schema(sub, field, &child(pointer, name), name == "extensions")?;
                    }
                }
            }
            if !open && object.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
                for name in map.keys() {
                    if !properties.contains_key(name) {
                        return Err(invalid(&child(pointer, name), "additionalProperties"));
                    }
                }
            }
        }
    }
    if let Some(items) = object.get("items") {
        if let Some(values) = value.as_array() {
            for (index, item) in values.iter().enumerate() {
                validate_schema(items, item, &child(pointer, &index.to_string()), false)?;
            }
            if object.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
                for i in 0..values.len() {
                    if values[..i].iter().any(|prior| prior == &values[i]) {
                        return Err(invalid(pointer, "uniqueItems"));
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_content_contract(
    registration: &TypeRegistration,
    contract: &Value,
) -> Result<(), CanonicalValidationError> {
    let object = contract
        .as_object()
        .ok_or_else(|| invariant("", "contentContract"))?;
    let allowed_fields = [
        "mediaType",
        "version",
        "rootType",
        "allowedNodes",
        "allowedMarks",
        "attributes",
        "shape",
    ];
    for key in object.keys() {
        if !allowed_fields.contains(&key.as_str()) {
            return Err(invariant(&child("", key), key));
        }
    }
    let media_type = object.get("mediaType").and_then(Value::as_str);
    match media_type {
        Some(RICH_MEDIA) => {
            for required in [
                "version",
                "rootType",
                "allowedNodes",
                "allowedMarks",
                "attributes",
            ] {
                if !object.contains_key(required) {
                    return Err(invariant(&child("", required), required));
                }
            }
            if object.get("version").and_then(Value::as_u64) != Some(1) {
                return Err(invariant("/version", "version"));
            }
            if object.get("rootType").and_then(Value::as_str) != Some("doc") {
                return Err(invariant("/rootType", "rootType"));
            }
            for field in ["allowedNodes", "allowedMarks"] {
                let array = object
                    .get(field)
                    .and_then(Value::as_array)
                    .ok_or_else(|| invariant(&child("", field), field))?;
                if !array.iter().all(Value::is_string) {
                    return Err(invariant(&child("", field), field));
                }
            }
            if !object.get("attributes").is_some_and(Value::is_object) {
                return Err(invariant("/attributes", "attributes"));
            }
        }
        Some("application/json") => {
            if object.get("shape").and_then(Value::as_str) != Some("object") {
                return Err(invariant("/shape", "shape"));
            }
        }
        _ => return Err(invariant("/mediaType", "mediaType")),
    }
    let expected = canonical_type_registrations()
        .ok()
        .and_then(|registrations| {
            registrations
                .into_iter()
                .find(|r| r.type_id == registration.type_id)
        });
    let expected_contract =
        expected.and_then(|r| serde_json::from_str::<Value>(&r.content_contract_json).ok());
    let expected = expected_contract.ok_or_else(|| invariant("", "contentContract"))?;
    let fields: &[&str] = if media_type == Some(RICH_MEDIA) {
        &[
            "mediaType",
            "version",
            "rootType",
            "allowedNodes",
            "allowedMarks",
            "attributes",
        ]
    } else {
        &["mediaType", "shape"]
    };
    for field in fields {
        if object.get(*field) != expected.get(*field) {
            return Err(invariant(&child("", field), field));
        }
    }
    Ok(())
}

fn validate_content(contract: &Value, content: &Value) -> Result<(), CanonicalValidationError> {
    let object = contract
        .as_object()
        .ok_or_else(|| invariant("", "contentContract"))?;
    match object.get("mediaType").and_then(Value::as_str) {
        Some(RICH_MEDIA) => validate_rich_text(object, content),
        Some("application/json") => {
            if object.get("shape").and_then(Value::as_str) == Some("object") && !content.is_object()
            {
                Err(invalid("", "shape"))
            } else {
                Ok(())
            }
        }
        _ => Err(invariant("/mediaType", "mediaType")),
    }
}

fn validate_rich_text(
    contract: &serde_json::Map<String, Value>,
    content: &Value,
) -> Result<(), CanonicalValidationError> {
    let root = content.as_object().ok_or_else(|| invalid("", "type"))?;
    let root_type = contract
        .get("rootType")
        .and_then(Value::as_str)
        .unwrap_or("doc");
    if root.get("type").and_then(Value::as_str) != Some(root_type) {
        return Err(invalid("/type", "rootType"));
    }
    validate_node(contract, content, "")
}
