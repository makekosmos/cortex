
fn validate_node(
    contract: &serde_json::Map<String, Value>,
    value: &Value,
    pointer: &str,
) -> Result<(), CanonicalValidationError> {
    let object = value.as_object().ok_or_else(|| invalid(pointer, "node"))?;
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&child(pointer, "type"), "type"))?;
    let nodes = contract
        .get("allowedNodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(pointer, "allowedNodes"))?;
    if !nodes.iter().any(|v| v.as_str() == Some(node_type)) {
        return Err(invalid(&child(pointer, "type"), "allowedNodes"));
    }
    let allowed_keys: &[&str] = if node_type == "text" {
        &["type", "text", "marks"]
    } else {
        &["type", "content", "attrs", "marks"]
    };
    for key in object.keys() {
        if !allowed_keys.contains(&key.as_str()) {
            return Err(invalid(&child(pointer, key), "nodeShape"));
        }
    }
    if node_type == "text" && object.get("text").and_then(Value::as_str).is_none() {
        return Err(invalid(&child(pointer, "text"), "type"));
    }
    if let Some(attrs) = object.get("attrs") {
        validate_attrs(contract, node_type, attrs, &child(pointer, "attrs"))?;
    }
    if let Some(marks) = object.get("marks") {
        let marks = marks
            .as_array()
            .ok_or_else(|| invalid(&child(pointer, "marks"), "type"))?;
        for (i, mark) in marks.iter().enumerate() {
            validate_mark(
                contract,
                mark,
                &child(&child(pointer, "marks"), &i.to_string()),
            )?;
        }
    }
    if let Some(children) = object.get("content") {
        let children = children
            .as_array()
            .ok_or_else(|| invalid(&child(pointer, "content"), "type"))?;
        for (i, child_value) in children.iter().enumerate() {
            validate_node(
                contract,
                child_value,
                &child(&child(pointer, "content"), &i.to_string()),
            )?;
        }
    }
    Ok(())
}

fn validate_mark(
    contract: &serde_json::Map<String, Value>,
    value: &Value,
    pointer: &str,
) -> Result<(), CanonicalValidationError> {
    let object = value.as_object().ok_or_else(|| invalid(pointer, "mark"))?;
    let mark_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&child(pointer, "type"), "type"))?;
    let marks = contract
        .get("allowedMarks")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(pointer, "allowedMarks"))?;
    if !marks.iter().any(|v| v.as_str() == Some(mark_type)) {
        return Err(invalid(&child(pointer, "type"), "allowedMarks"));
    }
    for key in object.keys() {
        if key != "type" && key != "attrs" {
            return Err(invalid(&child(pointer, key), "markShape"));
        }
    }
    if mark_type == "link" {
        let attrs = object
            .get("attrs")
            .ok_or_else(|| invalid(&child(pointer, "attrs"), "required"))?;
        validate_attrs(contract, "link", attrs, &child(pointer, "attrs"))?;
    } else if object.contains_key("attrs") {
        return Err(invalid(&child(pointer, "attrs"), "attributes"));
    }
    Ok(())
}

fn validate_attrs(
    contract: &serde_json::Map<String, Value>,
    kind: &str,
    value: &Value,
    pointer: &str,
) -> Result<(), CanonicalValidationError> {
    let attrs = value.as_object().ok_or_else(|| invalid(pointer, "type"))?;
    let definitions = contract
        .get("attributes")
        .and_then(Value::as_object)
        .and_then(|a| a.get(kind))
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(pointer, "attributes"))?;
    for name in attrs.keys() {
        if !definitions.contains_key(name) {
            return Err(invalid(&child(pointer, name), "attributes"));
        }
    }
    for (name, schema) in definitions {
        check_schema(schema, &child(pointer, name), false)
            .map_err(|e| invariant(&e.pointer, e.keyword.as_deref().unwrap_or("schema")))?;
        if let Some(item) = attrs.get(name) {
            validate_schema(schema, item, &child(pointer, name), false)?;
        } else if schema.get("default").is_none() {
            return Err(invalid(&child(pointer, name), "required"));
        }
    }
    Ok(())
}
