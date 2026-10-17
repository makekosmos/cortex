use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub mod definitions;
pub mod validation;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AliasRecord {
    pub alias: String,
    pub canonical_type_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeRegistration {
    pub type_id: String,
    pub name: String,
    pub schema_json: String,
    pub ui_schema_json: String,
    pub content_contract_json: String,
    pub relations_json: String,
    pub sync_policy_json: String,
    pub version: String,
    pub schema_hash: String,
    pub owner_kind: String,
    pub owner_id: Option<String>,
    pub status: String,
    pub base_type_id: Option<String>,
    pub aliases: Vec<AliasRecord>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TypeSummary {
    pub type_id: String,
    pub owner_kind: String,
    pub owner_id: Option<String>,
    pub current_version: String,
    pub status: String,
    pub base_type_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TypeVersion {
    pub type_id: String,
    pub version: String,
    pub schema_json: String,
    pub ui_schema_json: String,
    pub content_contract_json: String,
    pub relations_json: String,
    pub sync_policy_json: String,
    pub schema_hash: String,
    pub created_at: String,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalIngressError {
    pub category: &'static str,
    pub code: &'static str,
    pub pointer: Option<String>,
}

impl CanonicalIngressError {
    pub fn invalid(code: &'static str) -> Self {
        Self {
            category: "invalid_request",
            code,
            pointer: None,
        }
    }

    pub fn invariant(code: &'static str) -> Self {
        Self {
            category: "definition_invariant",
            code,
            pointer: None,
        }
    }
}

impl std::fmt::Display for CanonicalIngressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "canonical_ingress:{}:{}", self.category, self.code)?;
        if let Some(pointer) = &self.pointer {
            write!(f, ":{pointer}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CanonicalIngressError {}

pub fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut sorted = BTreeMap::new();
            for (key, value) in object {
                sorted.insert(key.clone(), canonical_value(value));
            }
            Value::Object(sorted.into_iter().collect::<Map<_, _>>())
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_value).collect()),
        other => other.clone(),
    }
}

#[derive(Serialize)]
struct CanonicalTuple {
    schema: Value,
    #[serde(rename = "uiSchema")]
    ui_schema: Value,
    #[serde(rename = "contentContract")]
    content_contract: Value,
    relations: Value,
    #[serde(rename = "syncPolicy")]
    sync_policy: Value,
}

pub fn canonical_schema_hash(
    schema: &Value,
    ui_schema: &Value,
    content_contract: &Value,
    relations: &Value,
    sync_policy: &Value,
) -> Result<String, String> {
    let tuple = CanonicalTuple {
        schema: canonical_value(schema),
        ui_schema: canonical_value(ui_schema),
        content_contract: canonical_value(content_contract),
        relations: canonical_value(relations),
        sync_policy: canonical_value(sync_policy),
    };
    let bytes = serde_json::to_vec(&tuple).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::TypeRegistration;

    #[test]
    fn type_registration_keeps_legacy_snake_case_json_keys() {
        let source = include_str!("../tests/fixtures/type-registration.json");
        let registration: TypeRegistration = serde_json::from_str(source).unwrap();
        let value = serde_json::to_value(registration).unwrap();
        assert!(value.get("type_id").is_some());
        assert!(value.get("schema_json").is_some());
        assert!(value.get("typeId").is_none());
    }
}
