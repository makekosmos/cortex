//! Backward-compatibility rules for additive type-version evolution.
//!
//! A new version of a registered type may only grow its schema: fields can be
//! added (and must stay optional), but existing fields may not disappear,
//! become required, or change their declared JSON type. The checks operate on
//! already-validated schemas (see [`super::validate_type_schema`]) and return
//! human-readable messages that Engine passes through to the caller.

use serde_json::{Map, Value};

/// Validate that `candidate` is an additive extension of `previous`.
pub fn check_additive_schema_change(previous: &Value, candidate: &Value) -> Result<(), String> {
    compare_schema(previous, candidate, &[])
}

fn compare_schema(previous: &Value, candidate: &Value, path: &[String]) -> Result<(), String> {
    let previous = previous
        .as_object()
        .ok_or_else(|| format!("{} is not a JSON schema object", describe(path)))?;
    let candidate = candidate
        .as_object()
        .ok_or_else(|| format!("{} is not a JSON schema object", describe(path)))?;

    compare_types(previous, candidate, path)?;
    compare_required(previous, candidate, path)?;
    compare_properties(previous, candidate, path)?;

    if let (Some(previous_items), Some(candidate_items)) = (
        previous.get("items").and_then(Value::as_object),
        candidate.get("items").and_then(Value::as_object),
    ) {
        let mut item_path = path.to_vec();
        item_path.push("[]".into());
        compare_schema(
            &Value::Object(previous_items.clone()),
            &Value::Object(candidate_items.clone()),
            &item_path,
        )?;
    }
    Ok(())
}

fn compare_types(
    previous: &Map<String, Value>,
    candidate: &Map<String, Value>,
    path: &[String],
) -> Result<(), String> {
    let previous_type = declared_type(previous.get("type"));
    let candidate_type = declared_type(candidate.get("type"));
    if previous_type != candidate_type {
        return Err(format!(
            "{} changed type from {} to {}",
            describe(path),
            describe_type(&previous_type),
            describe_type(&candidate_type)
        ));
    }
    Ok(())
}

fn compare_required(
    previous: &Map<String, Value>,
    candidate: &Map<String, Value>,
    path: &[String],
) -> Result<(), String> {
    let previous = required_names(previous);
    let candidate = required_names(candidate);
    for name in &previous {
        if !candidate.contains(name) {
            return Err(format!(
                "{} was removed from required",
                describe_field(name, path)
            ));
        }
    }
    for name in &candidate {
        if !previous.contains(name) {
            return Err(format!("{} became required", describe_field(name, path)));
        }
    }
    Ok(())
}

fn compare_properties(
    previous: &Map<String, Value>,
    candidate: &Map<String, Value>,
    path: &[String],
) -> Result<(), String> {
    let Some(previous_properties) = previous.get("properties").and_then(Value::as_object) else {
        return Ok(());
    };
    let empty = Map::new();
    let candidate_properties = candidate
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    for (name, previous_schema) in previous_properties {
        let candidate_schema = candidate_properties
            .get(name)
            .ok_or_else(|| format!("{} was removed", describe_field(name, path)))?;
        let mut field_path = path.to_vec();
        field_path.push(name.clone());
        compare_schema(previous_schema, candidate_schema, &field_path)?;
    }
    Ok(())
}

fn required_names(schema: &Map<String, Value>) -> Vec<String> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Normalize `"type": "string"` and `"type": ["string"]` to a stable set so
/// reordering a union is not reported as a change.
fn declared_type(value: Option<&Value>) -> Option<Vec<String>> {
    match value {
        None => None,
        Some(Value::String(name)) => Some(vec![name.clone()]),
        Some(Value::Array(names)) => {
            let mut names = names
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            names.sort();
            names.dedup();
            Some(names)
        }
        Some(other) => Some(vec![other.to_string()]),
    }
}

fn describe(path: &[String]) -> String {
    if path.is_empty() {
        "schema".into()
    } else {
        format!("field '{}'", path.join("."))
    }
}

fn describe_field(name: &str, path: &[String]) -> String {
    if path.is_empty() {
        format!("field '{name}'")
    } else {
        format!("field '{}.{name}'", path.join("."))
    }
}

fn describe_type(value: &Option<Vec<String>>) -> String {
    match value {
        Some(names) if names.len() == 1 => format!("'{}'", names[0]),
        Some(names) => format!("'{}'", names.join("|")),
        None => "an unspecified type".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::check_additive_schema_change;
    use serde_json::json;

    fn base() -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "priority": {"type": "integer"}
            },
            "required": ["title"]
        })
    }

    #[test]
    fn adding_an_optional_field_is_compatible() {
        let mut candidate = base();
        candidate["properties"]["note"] = json!({"type": "string"});
        check_additive_schema_change(&base(), &candidate).unwrap();
    }

    #[test]
    fn removing_a_field_is_rejected() {
        let mut candidate = base();
        candidate["properties"]
            .as_object_mut()
            .unwrap()
            .remove("priority");
        let error = check_additive_schema_change(&base(), &candidate).unwrap_err();
        assert!(error.contains("field 'priority' was removed"), "{error}");
    }

    #[test]
    fn changing_a_field_type_is_rejected() {
        let mut candidate = base();
        candidate["properties"]["priority"] = json!({"type": "string"});
        let error = check_additive_schema_change(&base(), &candidate).unwrap_err();
        assert!(
            error.contains("changed type from 'integer' to 'string'"),
            "{error}"
        );
    }

    #[test]
    fn adding_or_promoting_required_fields_is_rejected() {
        let mut candidate = base();
        candidate["properties"]["note"] = json!({"type": "string"});
        candidate["required"] = json!(["title", "note"]);
        let error = check_additive_schema_change(&base(), &candidate).unwrap_err();
        assert!(error.contains("field 'note' became required"), "{error}");
    }

    #[test]
    fn nested_optional_fields_are_allowed_but_nested_type_changes_are_not() {
        let previous = json!({
            "type": "object",
            "properties": {"address": {
                "type": "object",
                "properties": {"city": {"type": "string"}}
            }}
        });
        let mut candidate = previous.clone();
        candidate["properties"]["address"]["properties"]["zip"] = json!({"type": "string"});
        check_additive_schema_change(&previous, &candidate).unwrap();

        candidate["properties"]["address"]["properties"]["city"] = json!({"type": "integer"});
        let error = check_additive_schema_change(&previous, &candidate).unwrap_err();
        assert!(error.contains("field 'address.city'"), "{error}");
    }
}
