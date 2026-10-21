use super::{canonical_schema_hash, canonical_value, TypeVersion};
use semver::Version;
use serde_json::{Map, Value};

pub const LEGACY_VERSION: &str = "0.0.0-legacy";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalTypeVersion {
    pub schema_json: String,
    pub ui_schema_json: String,
    pub content_contract_json: String,
    pub relations_json: String,
    pub sync_policy_json: String,
    pub schema_hash: String,
}

pub fn canonicalize_type_version(version: &TypeVersion) -> Result<CanonicalTypeVersion, String> {
    validate_type_id(&version.type_id, "type id")?;
    validate_version(&version.version)?;

    let schema = parse_json(&version.schema_json, "schema_json")?;
    let ui_schema = parse_json(&version.ui_schema_json, "ui_schema_json")?;
    let content_contract = parse_json(&version.content_contract_json, "content_contract_json")?;
    let relations = parse_json(&version.relations_json, "relations_json")?;
    let sync_policy = parse_json(&version.sync_policy_json, "sync_policy_json")?;
    let schema_hash = canonical_schema_hash(
        &schema,
        &ui_schema,
        &content_contract,
        &relations,
        &sync_policy,
    )?;

    Ok(CanonicalTypeVersion {
        schema_json: canonical_json(&schema)?,
        ui_schema_json: canonical_json(&ui_schema)?,
        content_contract_json: canonical_json(&content_contract)?,
        relations_json: canonical_json(&relations)?,
        sync_policy_json: canonical_json(&sync_policy)?,
        schema_hash,
    })
}

pub fn legacy_compatibility_version(
    schema_json: &str,
    ui_schema_json: &str,
) -> Result<(String, String), String> {
    let canonical = canonicalize_legacy_definition(schema_json, ui_schema_json)?;
    Ok((
        format!("0.0.0+legacy.{}", &canonical.schema_hash[..12]),
        canonical.schema_hash,
    ))
}

pub fn canonicalize_legacy_definition(
    schema_json: &str,
    ui_schema_json: &str,
) -> Result<CanonicalTypeVersion, String> {
    let schema = parse_json(schema_json, "schema_json")?;
    let ui_schema = parse_json(ui_schema_json, "ui_schema_json")?;
    let content_contract = json_empty();
    let relations = json_empty_array();
    let sync_policy = json_empty();
    let schema_hash = canonical_schema_hash(
        &schema,
        &ui_schema,
        &content_contract,
        &relations,
        &sync_policy,
    )?;
    Ok(CanonicalTypeVersion {
        schema_json: canonical_json(&schema)?,
        ui_schema_json: canonical_json(&ui_schema)?,
        content_contract_json: canonical_json(&content_contract)?,
        relations_json: canonical_json(&relations)?,
        sync_policy_json: canonical_json(&sync_policy)?,
        schema_hash,
    })
}

pub fn validate_type_id(value: &str, kind: &str) -> Result<(), String> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > 255
        || value.contains('\0')
        || value.chars().any(|c| c.is_ascii_control())
    {
        return Err(format!("invalid {kind}"));
    }
    Ok(())
}

pub fn validate_version(value: &str) -> Result<Version, String> {
    if value == LEGACY_VERSION {
        return Err("reserved legacy version".into());
    }
    let parsed = Version::parse(value).map_err(|error| format!("invalid semver: {error}"))?;
    if parsed.to_string() != value {
        return Err("version is not canonical semver".into());
    }
    Ok(parsed)
}

fn parse_json(raw: &str, field: &str) -> Result<Value, String> {
    if raw.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    let value: Value =
        serde_json::from_str(raw).map_err(|error| format!("invalid {field}: {error}"))?;
    let expected_array = field == "relations_json";
    let correct_shape = if expected_array {
        value.is_array()
    } else {
        value.is_object()
    };
    if !correct_shape {
        return Err(format!(
            "{field} must be a JSON {}",
            if expected_array { "array" } else { "object" }
        ));
    }
    Ok(value)
}

fn canonical_json(value: &Value) -> Result<String, String> {
    serde_json::to_string(&canonical_value(value)).map_err(|error| error.to_string())
}

fn json_empty() -> Value {
    Value::Object(Map::new())
}

fn json_empty_array() -> Value {
    Value::Array(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_type_version, legacy_compatibility_version};
    use crate::canonical_type::TypeVersion;

    fn version() -> TypeVersion {
        TypeVersion {
            type_id: "example.note".into(),
            version: "1.0.0".into(),
            schema_json: r#"{"z":1,"a":2}"#.into(),
            ui_schema_json: "{}".into(),
            content_contract_json: "{}".into(),
            relations_json: "[]".into(),
            sync_policy_json: "{}".into(),
            schema_hash: String::new(),
            created_at: "now".into(),
        }
    }

    #[test]
    fn type_version_canonicalization_sorts_json_and_calculates_hash() {
        let canonical = canonicalize_type_version(&version()).unwrap();
        assert_eq!(canonical.schema_json, r#"{"a":2,"z":1}"#);
        assert_eq!(canonical.relations_json, "[]");
        assert_eq!(canonical.schema_hash.len(), 64);

        let mut with_hash = version();
        with_hash.schema_hash = canonical.schema_hash.clone();
        assert_eq!(canonicalize_type_version(&with_hash).unwrap(), canonical);
    }

    #[test]
    fn type_version_canonicalization_rejects_wrong_json_shape() {
        let mut invalid = version();
        invalid.relations_json = "{}".into();
        assert!(canonicalize_type_version(&invalid)
            .unwrap_err()
            .contains("relations_json must be a JSON array"));

        let mut invalid = version();
        invalid.schema_hash = "wrong".into();
        assert!(canonicalize_type_version(&invalid).is_ok());
    }

    #[test]
    fn legacy_version_hash_stays_stable() {
        let (version, hash) = legacy_compatibility_version("{}", "{}").unwrap();
        assert_eq!(version, format!("0.0.0+legacy.{}", &hash[..12]));
        assert_eq!(hash.len(), 64);
    }
}
