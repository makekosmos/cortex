use rusqlite::{params, Connection, OptionalExtension};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const LEGACY_VERSION: &str = "0.0.0-legacy";

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AliasRecord {
    pub alias: String,
    pub canonical_type_id: String,
    pub created_at: String,
}

pub fn legacy_compatibility_version(
    schema_json: &str,
    ui_schema_json: &str,
) -> Result<(String, String), String> {
    let schema = parse_json(schema_json, "schema_json")?;
    let ui_schema = parse_json(ui_schema_json, "ui_schema_json")?;
    let hash = canonical_schema_hash(
        &schema,
        &ui_schema,
        &json_empty(),
        &json_empty_array(),
        &json_empty(),
    )?;
    Ok((format!("0.0.0+legacy.{}", &hash[..12]), hash))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeGetResponse {
    pub summary: TypeSummary,
    pub definition: TypeVersion,
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

fn canonical_value(value: &Value) -> Value {
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

fn parse_json(raw: &str, field: &str) -> Result<Value, String> {
    if raw.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    let value: Value = serde_json::from_str(raw).map_err(|e| format!("invalid {field}: {e}"))?;
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
    let bytes = serde_json::to_vec(&tuple).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn canonical_definition(
    version: &TypeVersion,
) -> Result<(String, String, String, String, String), String> {
    let schema = parse_json(&version.schema_json, "schema_json")?;
    let ui = parse_json(&version.ui_schema_json, "ui_schema_json")?;
    let content = parse_json(&version.content_contract_json, "content_contract_json")?;
    let relations = parse_json(&version.relations_json, "relations_json")?;
    let policy = parse_json(&version.sync_policy_json, "sync_policy_json")?;
    let hash = canonical_schema_hash(&schema, &ui, &content, &relations, &policy)?;
    Ok((
        serde_json::to_string(&canonical_value(&schema)).map_err(|e| e.to_string())?,
        serde_json::to_string(&canonical_value(&ui)).map_err(|e| e.to_string())?,
        serde_json::to_string(&canonical_value(&content)).map_err(|e| e.to_string())?,
        serde_json::to_string(&canonical_value(&relations)).map_err(|e| e.to_string())?,
        format!(
            "{}|{}|{}|{}|{}",
            hash,
            serde_json::to_string(&canonical_value(&policy)).map_err(|e| e.to_string())?,
            version.type_id,
            version.version,
            version.created_at
        ),
    ))
}

fn validate_id(value: &str, kind: &str) -> Result<(), String> {
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

fn validate_version(value: &str) -> Result<Version, String> {
    if value == LEGACY_VERSION {
        return Err("reserved legacy version".into());
    }
    let parsed = Version::parse(value).map_err(|e| format!("invalid semver: {e}"))?;
    if parsed.to_string() != value {
        return Err("version is not canonical semver".into());
    }
    Ok(parsed)
}

pub fn insert_type_version(
    conn: &Connection,
    input: &TypeVersion,
    created_at: &str,
) -> Result<(), String> {
    validate_id(&input.type_id, "type id")?;
    validate_version(&input.version)?;
    let (schema, ui, content, relations, hash_and_meta) = canonical_definition(input)?;
    let hash = hash_and_meta
        .split('|')
        .next()
        .ok_or_else(|| "canonical hash metadata missing".to_string())?
        .to_string();
    if !input.schema_hash.is_empty() && input.schema_hash != hash {
        return Err("schema hash mismatch".into());
    }
    let canonical_exists = conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![input.type_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if canonical_exists.is_none() {
        return Err("canonical type missing".into());
    }
    let existing = conn.query_row(concat!("SELECT schema_json,ui_schema_json,content_contract_json,relations_json,","sync_policy_json,schema_hash,created_at FROM object_type_versions WHERE ","type_id=?1 AND version=?2"), params![input.type_id, input.version], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?))).optional().map_err(|e| e.to_string())?;
    let policy = parse_json(&input.sync_policy_json, "sync_policy_json")?;
    let policy = serde_json::to_string(&canonical_value(&policy)).map_err(|e| e.to_string())?;
    if let Some((a, b, c, d, e, f, _)) = existing {
        if a == schema && b == ui && c == content && d == relations && e == policy && f == hash {
            crate::canonical_types::pending::replay_pending_for_type(
                conn,
                &input.type_id,
                &input.version,
            )?;
            return Ok(());
        }
        return Err("immutable type version conflict".into());
    }
    conn.execute(concat!("INSERT INTO object_type_versions (type_id,version,schema_json,","ui_schema_json,content_contract_json,relations_json,sync_policy_json,","schema_hash,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"), params![input.type_id,input.version,schema,ui,content,relations,policy,hash,created_at]).map_err(|e| e.to_string())?;
    crate::canonical_types::pending::replay_pending_for_type(conn, &input.type_id, &input.version)?;
    Ok(())
}
