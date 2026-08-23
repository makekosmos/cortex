//! Pure validation for the frozen canonical props/content subset.

use crate::canonical_types::definitions::canonical_type_registrations;
use crate::type_registry::TypeRegistration;
use serde_json::Value;

const DRAFT_SCHEMA: &str = "https://json-schema.org/draft/2020-12/schema";
const RICH_MEDIA: &str = "application/vnd.kosmos.richtext+json";
const SCHEMA_TYPES: &[&str] = &[
    "object", "array", "string", "number", "integer", "boolean", "null",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalValidationCode {
    InvalidField,
    InvariantViolation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalValidationError {
    pub code: CanonicalValidationCode,
    pub pointer: String,
    pub keyword: Option<String>,
}

pub fn validate_canonical(
    registration: &TypeRegistration,
    props: &Value,
    content: &Value,
) -> Result<(), CanonicalValidationError> {
    let schema: Value =
        serde_json::from_str(&registration.schema_json).map_err(|_| invariant("", "schema"))?;
    check_schema(&schema, "", true)?;
    validate_schema(&schema, props, "", false)?;

    let contract: Value = serde_json::from_str(&registration.content_contract_json)
        .map_err(|_| invariant("", "contentContract"))?;
    check_content_contract(registration, &contract)?;
    validate_content(&contract, content)
}

fn invariant(pointer: &str, keyword: &str) -> CanonicalValidationError {
    CanonicalValidationError {
        code: CanonicalValidationCode::InvariantViolation,
        pointer: pointer.to_owned(),
        keyword: Some(keyword.to_owned()),
    }
}
fn invalid(pointer: &str, keyword: &str) -> CanonicalValidationError {
    CanonicalValidationError {
        code: CanonicalValidationCode::InvalidField,
        pointer: pointer.to_owned(),
        keyword: Some(keyword.to_owned()),
    }
}
fn esc(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
fn child(pointer: &str, segment: &str) -> String {
    format!("{pointer}/{}", esc(segment))
}

fn check_schema(schema: &Value, pointer: &str, root: bool) -> Result<(), CanonicalValidationError> {
    let object = schema
        .as_object()
        .ok_or_else(|| invariant(pointer, "schema"))?;
    const ALLOWED: &[&str] = &[
        "$schema",
        "type",
        "required",
        "properties",
        "additionalProperties",
        "enum",
        "minimum",
        "maximum",
        "minLength",
        "items",
        "uniqueItems",
        "default",
        "format",
    ];
    for (key, value) in object {
        let key_pointer = child(pointer, key);
        if !ALLOWED.contains(&key.as_str()) {
            return Err(invariant(&key_pointer, key));
        }
        match key.as_str() {
            "$schema" => {
                if value.as_str() != Some(DRAFT_SCHEMA) {
                    return Err(invariant(&key_pointer, "$schema"));
                }
            }
            "type" => {
                let valid = value
                    .as_str()
                    .map(|s| SCHEMA_TYPES.contains(&s))
                    .unwrap_or_else(|| {
                        value
                            .as_array()
                            .map(|a| {
                                !a.is_empty()
                                    && a.iter().all(|v| {
                                        v.as_str().is_some_and(|s| SCHEMA_TYPES.contains(&s))
                                    })
                            })
                            .unwrap_or(false)
                    });
                if !valid {
                    return Err(invariant(&key_pointer, "type"));
                }
            }
            "required" => {
                if !value
                    .as_array()
                    .is_some_and(|a| a.iter().all(Value::is_string))
                {
                    return Err(invariant(&key_pointer, "required"));
                }
            }
            "properties" => {
                let properties = value
                    .as_object()
                    .ok_or_else(|| invariant(&key_pointer, "properties"))?;
                for (name, sub) in properties {
                    check_schema(sub, &child(&key_pointer, name), false)?;
                }
            }
            "additionalProperties" | "uniqueItems" => {
                if !value.is_boolean() {
                    return Err(invariant(&key_pointer, key));
                }
            }
            "enum" => {
                if !value.is_array() {
                    return Err(invariant(&key_pointer, "enum"));
                }
            }
            "minimum" | "maximum" => {
                if !value.is_number() {
                    return Err(invariant(&key_pointer, key));
                }
            }
            "minLength" => {
                if !value.as_u64().is_some() {
                    return Err(invariant(&key_pointer, "minLength"));
                }
            }
            "items" => {
                if !value.is_object() {
                    return Err(invariant(&key_pointer, "items"));
                }
                check_schema(value, &key_pointer, false)?;
            }
            "default" => {}
            "format" => {
                if !value.is_string() {
                    return Err(invariant(&key_pointer, "format"));
                }
            }
            _ => unreachable!(),
        }
    }
    if root && object.get("$schema").and_then(Value::as_str) != Some(DRAFT_SCHEMA) {
        return Err(invariant(&child(pointer, "$schema"), "$schema"));
    }
    Ok(())
}

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
        .unwrap();
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
        .unwrap();
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
