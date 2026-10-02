// Pure validation for the frozen canonical props/content subset.

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
    for (key, value) in object {
        let key_pointer = child(pointer, key);
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
                if value.as_u64().is_none() {
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
                // Closed set: a typo'd format would silently degrade to an
                // annotation, so unknown names are an invariant violation.
                // `date` is enforced on values; `date-time` and `uri` remain
                // documentation — see `validate_schema` for why.
                match value.as_str() {
                    Some("date" | "date-time" | "uri") => {}
                    _ => return Err(invariant(&key_pointer, "format")),
                }
            }
            _ => return Err(invariant(&key_pointer, key)),
        }
    }
    if root && object.get("$schema").and_then(Value::as_str) != Some(DRAFT_SCHEMA) {
        return Err(invariant(&child(pointer, "$schema"), "$schema"));
    }
    Ok(())
}
