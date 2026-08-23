use rusqlite::{params, Connection, OptionalExtension};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

pub const LEGACY_VERSION: &str = "0.0.0-legacy";

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
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
    let existing = conn.query_row("SELECT schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at FROM object_type_versions WHERE type_id=?1 AND version=?2", params![input.type_id, input.version], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?))).optional().map_err(|e| e.to_string())?;
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
    conn.execute("INSERT INTO object_type_versions (type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![input.type_id,input.version,schema,ui,content,relations,policy,hash,created_at]).map_err(|e| e.to_string())?;
    crate::canonical_types::pending::replay_pending_for_type(conn, &input.type_id, &input.version)?;
    Ok(())
}

pub fn ensure_legacy_type_version(
    conn: &Connection,
    type_id: &str,
    schema_json: &str,
    ui_schema_json: &str,
    created_at: &str,
) -> Result<(), String> {
    let current_version: Option<String> = conn
        .query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            params![type_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if current_version
        .as_deref()
        .is_some_and(|version| version != LEGACY_VERSION)
    {
        return Ok(());
    }
    let has_versions: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_versions')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if !has_versions {
        return Ok(());
    }
    let schema = parse_json(schema_json, "schema_json")?;
    let ui_schema = parse_json(ui_schema_json, "ui_schema_json")?;
    let empty = serde_json::json!({});
    let relations = serde_json::json!([]);
    let hash = canonical_schema_hash(&schema, &ui_schema, &empty, &relations, &empty)?;
    conn.execute(
        "INSERT OR IGNORE INTO object_type_versions (type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            type_id,
            LEGACY_VERSION,
            schema_json,
            ui_schema_json,
            "{}",
            "[]",
            "{}",
            hash,
            created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn resolve_object_type_identity(
    conn: &Connection,
    input_type_id: &str,
    requested: Option<&str>,
) -> Result<(String, String), String> {
    let canonical_type_id = resolve_type_id(conn, input_type_id)?
        .ok_or_else(|| format!("unknown object type '{input_type_id}'"))?;
    let current: String = conn
        .query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            params![canonical_type_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    // Legacy aliases were removed as registry authorities. Their historical
    // version is accepted at the boundary but must never be persisted.
    let version = match requested {
        None => current.clone(),
        Some(version) if input_type_id != canonical_type_id && version == LEGACY_VERSION => {
            current.clone()
        }
        Some(version) => version.to_string(),
    };
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM object_type_versions WHERE type_id=?1 AND version=?2)",
            params![canonical_type_id, version],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if !exists {
        return Err(format!(
            "unknown object type version '{canonical_type_id}@{version}'"
        ));
    }
    Ok((canonical_type_id, version))
}

pub fn resolve_object_type_version(
    conn: &Connection,
    type_id: &str,
    requested: Option<&str>,
) -> Result<String, String> {
    resolve_object_type_identity(conn, type_id, requested).map(|(_, version)| version)
}
pub fn list_type_summaries(conn: &Connection) -> Result<Vec<TypeSummary>, String> {
    let mut stmt = conn.prepare("SELECT id,owner_kind,owner_id,current_version,status,base_type_id FROM object_types ORDER BY id ASC").map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([], |r| {
            Ok(TypeSummary {
                type_id: r.get(0)?,
                owner_kind: r.get(1)?,
                owner_id: r.get(2)?,
                current_version: r.get(3)?,
                status: r.get(4)?,
                base_type_id: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

pub fn list_all_type_versions(conn: &Connection) -> Result<Vec<TypeVersion>, String> {
    let mut values = Vec::new();
    let mut stmt = conn.prepare("SELECT type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at FROM object_type_versions ORDER BY type_id,version").map_err(|e| e.to_string())?;
    for row in stmt
        .query_map([], |r| {
            Ok(TypeVersion {
                type_id: r.get(0)?,
                version: r.get(1)?,
                schema_json: r.get(2)?,
                ui_schema_json: r.get(3)?,
                content_contract_json: r.get(4)?,
                relations_json: r.get(5)?,
                sync_policy_json: r.get(6)?,
                schema_hash: r.get(7)?,
                created_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
    {
        values.push(row.map_err(|e| e.to_string())?);
    }
    values.sort_by(|a, b| {
        a.type_id
            .cmp(&b.type_id)
            .then_with(|| {
                Version::parse(&a.version)
                    .ok()
                    .cmp(&Version::parse(&b.version).ok())
            })
            .then_with(|| a.version.cmp(&b.version))
    });
    Ok(values)
}

pub fn list_aliases(conn: &Connection) -> Result<Vec<AliasRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT alias,canonical_type_id,created_at FROM object_type_aliases ORDER BY alias",
        )
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([], |r| {
            Ok(AliasRecord {
                alias: r.get(0)?,
                canonical_type_id: r.get(1)?,
                created_at: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}
pub fn list_type_versions(conn: &Connection, type_id: &str) -> Result<Vec<TypeVersion>, String> {
    let mut stmt = conn.prepare("SELECT type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at FROM object_type_versions WHERE type_id=?1").map_err(|e| e.to_string())?;
    let mut values = stmt
        .query_map(params![type_id], |r| {
            Ok(TypeVersion {
                type_id: r.get(0)?,
                version: r.get(1)?,
                schema_json: r.get(2)?,
                ui_schema_json: r.get(3)?,
                content_contract_json: r.get(4)?,
                relations_json: r.get(5)?,
                sync_policy_json: r.get(6)?,
                schema_hash: r.get(7)?,
                created_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    values.sort_by(
        |a, b| match (Version::parse(&a.version), Version::parse(&b.version)) {
            (Ok(a_version), Ok(b_version)) => a_version
                .cmp(&b_version)
                .then_with(|| a.version.cmp(&b.version)),
            (Err(_), Err(_)) => a.version.cmp(&b.version),
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        },
    );
    for value in &values {
        if value.version != LEGACY_VERSION {
            validate_version(&value.version)
                .map_err(|error| format!("corrupt stored version: {error}"))?;
        }
    }
    Ok(values)
}

pub fn register_alias(conn: &Connection, alias: &AliasRecord) -> Result<(), String> {
    validate_id(&alias.alias, "alias")?;
    validate_id(&alias.canonical_type_id, "canonical type id")?;
    if alias.alias == alias.canonical_type_id {
        return Err("self alias".into());
    }
    if conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![alias.alias],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err("alias collides with canonical type".into());
    }
    if conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![alias.canonical_type_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err("unknown canonical type".into());
    }
    let existing = conn
        .query_row(
            "SELECT canonical_type_id FROM object_type_aliases WHERE alias=?1",
            params![alias.alias],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(existing) = existing {
        if existing == alias.canonical_type_id {
            return Ok(());
        }
        return Err("alias conflict".into());
    }
    conn.execute(
        "INSERT INTO object_type_aliases(alias,canonical_type_id,created_at) VALUES (?1,?2,?3)",
        params![alias.alias, alias.canonical_type_id, alias.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn resolve_alias(conn: &Connection, alias: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT canonical_type_id FROM object_type_aliases WHERE alias=?1",
        params![alias],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn resolve_type_id(conn: &Connection, input: &str) -> Result<Option<String>, String> {
    if conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![input],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Ok(Some(input.to_string()));
    }
    resolve_alias(conn, input)
}

pub fn get_type(
    conn: &Connection,
    input: &str,
    requested_version: Option<&str>,
) -> Result<Option<TypeGetResponse>, String> {
    let Some(type_id) = resolve_type_id(conn, input)? else {
        return Ok(None);
    };
    let summary = list_type_summaries(conn)?
        .into_iter()
        .find(|s| s.type_id == type_id);
    let Some(summary) = summary else {
        return Ok(None);
    };
    let version = requested_version.unwrap_or(&summary.current_version);
    let definition = list_type_versions(conn, &type_id)?
        .into_iter()
        .find(|v| v.version == version);
    Ok(definition.map(|definition| TypeGetResponse {
        summary,
        definition,
    }))
}
pub fn set_current_version(conn: &Connection, type_id: &str, version: &str) -> Result<(), String> {
    if version != LEGACY_VERSION {
        validate_version(version)?;
    }
    let exists = conn
        .query_row(
            "SELECT 1 FROM object_type_versions WHERE type_id=?1 AND version=?2",
            params![type_id, version],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        return Err("current version definition missing".into());
    }
    if conn
        .execute(
            "UPDATE object_types SET current_version=?1 WHERE id=?2",
            params![version, type_id],
        )
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("canonical type missing".into());
    }
    Ok(())
}

pub fn set_status(conn: &Connection, type_id: &str, status: &str) -> Result<(), String> {
    if !matches!(status, "active" | "deprecated" | "pending") {
        return Err("invalid status".into());
    }
    if conn
        .execute(
            "UPDATE object_types SET status=?1 WHERE id=?2",
            params![status, type_id],
        )
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("canonical type missing".into());
    }
    Ok(())
}

pub fn set_base_type(conn: &Connection, type_id: &str, base: Option<&str>) -> Result<(), String> {
    if base == Some(type_id) {
        return Err("base self-reference".into());
    }
    if let Some(base) = base {
        if conn
            .query_row(
                "SELECT 1 FROM object_types WHERE id=?1",
                params![base],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err("base type missing".into());
        }
        let mut cursor = Some(base.to_string());
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id.clone()) {
                return Err("base type cycle".into());
            }
            if id == type_id {
                return Err("base type cycle".into());
            }
            cursor = conn
                .query_row(
                    "SELECT base_type_id FROM object_types WHERE id=?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .flatten();
        }
    }
    if conn
        .execute(
            "UPDATE object_types SET base_type_id=?1 WHERE id=?2",
            params![base, type_id],
        )
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("canonical type missing".into());
    }
    Ok(())
}

pub fn register_type(conn: &Connection, registration: &TypeRegistration) -> Result<(), String> {
    validate_id(&registration.type_id, "type id")?;
    if registration.owner_kind.trim().is_empty() {
        return Err("invalid owner kind".into());
    }
    if registration.name.trim().is_empty() {
        return Err("invalid type name".into());
    }
    validate_version(&registration.version)?;
    if !matches!(
        registration.status.as_str(),
        "active" | "deprecated" | "pending"
    ) {
        return Err("invalid status".into());
    }
    if let Some(base) = &registration.base_type_id {
        validate_id(base, "base type id")?;
    }
    for alias in &registration.aliases {
        if alias.canonical_type_id != registration.type_id {
            return Err("alias canonical type mismatch".into());
        }
    }
    if conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![registration.type_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        // Package definitions are replayed on every runtime start. A replay
        // is safe only when the canonical summary and requested version are
        // identical; conflicting definitions remain errors.
        let existing_name: String = conn
            .query_row(
                "SELECT name FROM object_types WHERE id=?1",
                params![registration.type_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let existing = get_type(conn, &registration.type_id, Some(&registration.version))?
            .ok_or_else(|| "canonical type already exists".to_string())?;
        let definition = &existing.definition;
        let canonical = canonical_definition(&TypeVersion {
            type_id: registration.type_id.clone(),
            version: registration.version.clone(),
            schema_json: registration.schema_json.clone(),
            ui_schema_json: registration.ui_schema_json.clone(),
            content_contract_json: registration.content_contract_json.clone(),
            relations_json: registration.relations_json.clone(),
            sync_policy_json: registration.sync_policy_json.clone(),
            schema_hash: registration.schema_hash.clone(),
            created_at: registration.created_at.clone(),
        })?;
        let canonical_hash = canonical
            .4
            .split('|')
            .next()
            .ok_or_else(|| "canonical hash metadata missing".to_string())?;
        if existing_name == registration.name
            && existing.summary.owner_kind == registration.owner_kind
            && existing.summary.owner_id == registration.owner_id
            && existing.summary.status == registration.status
            && existing.summary.base_type_id == registration.base_type_id
            && definition.type_id == registration.type_id
            && definition.version == registration.version
            && definition.schema_json == canonical.0
            && definition.ui_schema_json == canonical.1
            && definition.content_contract_json == canonical.2
            && definition.relations_json == canonical.3
            && definition.sync_policy_json
                == serde_json::to_string(&canonical_value(&parse_json(
                    &registration.sync_policy_json,
                    "sync_policy_json",
                )?))
                .map_err(|e| e.to_string())?
            && definition.schema_hash == canonical_hash
            && definition.created_at == registration.created_at
        {
            return Ok(());
        }
        return Err("canonical type already exists".into());
    }

    if conn
        .query_row(
            "SELECT 1 FROM object_type_aliases WHERE alias=?1",
            params![registration.type_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err("canonical type id collides with alias".into());
    }

    conn.execute_batch("SAVEPOINT ark_registry_registration")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        conn.execute(
                "INSERT INTO object_types (id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked,owner_kind,owner_id,current_version,status,base_type_id) VALUES (?1,?2,?3,?4,?5,?5,0,?6,?7,?8,?9,?10)",
                params![registration.type_id, registration.name, registration.schema_json, registration.ui_schema_json, registration.created_at, registration.owner_kind, registration.owner_id, registration.version, registration.status, registration.base_type_id],
            )
            .map_err(|e| e.to_string())?;
        insert_type_version(
            conn,
            &TypeVersion {
                type_id: registration.type_id.clone(),
                version: registration.version.clone(),
                schema_json: registration.schema_json.clone(),
                ui_schema_json: registration.ui_schema_json.clone(),
                content_contract_json: registration.content_contract_json.clone(),
                relations_json: registration.relations_json.clone(),
                sync_policy_json: registration.sync_policy_json.clone(),
                schema_hash: registration.schema_hash.clone(),
                created_at: registration.created_at.clone(),
            },
            &registration.created_at,
        )?;
        for alias in &registration.aliases {
            register_alias(conn, alias)?;
        }
        set_current_version(conn, &registration.type_id, &registration.version)?;
        set_status(conn, &registration.type_id, &registration.status)?;
        set_base_type(
            conn,
            &registration.type_id,
            registration.base_type_id.as_deref(),
        )?;
        crate::canonical_types::pending::replay_pending_for_type(
            conn,
            &registration.type_id,
            &registration.version,
        )?;
        Ok::<(), String>(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT ark_registry_registration")
            .map_err(|e| e.to_string()),
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT ark_registry_registration; RELEASE SAVEPOINT ark_registry_registration");
            Err(error)
        }
    }
}

pub fn migrate_phase2(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ark_phase2_registry")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        conn.execute_batch("CREATE TABLE IF NOT EXISTS sync_pending_objects (id TEXT PRIMARY KEY, payload TEXT NOT NULL, awaited_type_id TEXT NOT NULL, received_at TEXT NOT NULL); CREATE INDEX IF NOT EXISTS idx_sync_pending_awaited_type ON sync_pending_objects(awaited_type_id); CREATE TABLE IF NOT EXISTS object_type_versions (type_id TEXT NOT NULL, version TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL DEFAULT '{}', content_contract_json TEXT NOT NULL DEFAULT '{}', relations_json TEXT NOT NULL DEFAULT '[]', sync_policy_json TEXT NOT NULL DEFAULT '{}', schema_hash TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(type_id,version), FOREIGN KEY(type_id) REFERENCES object_types(id)); CREATE TABLE IF NOT EXISTS object_type_aliases (alias TEXT PRIMARY KEY, canonical_type_id TEXT NOT NULL, created_at TEXT NOT NULL, FOREIGN KEY(canonical_type_id) REFERENCES object_types(id));").map_err(|e| e.to_string())?;
        for (table, column, definition) in [
            (
                "object_types",
                "owner_kind",
                "TEXT NOT NULL DEFAULT 'system'",
            ),
            ("object_types", "owner_id", "TEXT"),
            (
                "object_types",
                "current_version",
                "TEXT NOT NULL DEFAULT '0.0.0-legacy'",
            ),
            ("object_types", "status", "TEXT NOT NULL DEFAULT 'active'"),
            ("object_types", "base_type_id", "TEXT"),
            (
                "objects",
                "type_version",
                "TEXT NOT NULL DEFAULT '0.0.0-legacy'",
            ),
            (
                "sync_pending_objects",
                "awaited_type_version",
                "TEXT NOT NULL DEFAULT '0.0.0-legacy'",
            ),
        ] {
            let exists = conn
                .query_row(
                    &format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name=?1"),
                    params![column],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .is_some();
            if !exists {
                conn.execute_batch(&format!(
                    "ALTER TABLE {table} ADD COLUMN {column} {definition}"
                ))
                .map_err(|e| e.to_string())?;
            }
        }
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_object_type_versions_lookup ON object_type_versions(type_id,version); CREATE INDEX IF NOT EXISTS idx_object_type_aliases_canonical ON object_type_aliases(canonical_type_id); CREATE INDEX IF NOT EXISTS idx_objects_type_version ON objects(type_id,type_version); CREATE INDEX IF NOT EXISTS idx_sync_pending_awaited_type_version ON sync_pending_objects(awaited_type_id,awaited_type_version);").map_err(|e|e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id,schema_json,ui_schema_json,created_at,current_version FROM object_types",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        for (id, schema, ui, created, current_version) in rows {
            if current_version != LEGACY_VERSION {
                continue;
            }
            let v = TypeVersion {
                type_id: id.clone(),
                version: LEGACY_VERSION.into(),
                schema_json: schema,
                ui_schema_json: ui,
                content_contract_json: "{}".into(),
                relations_json: "[]".into(),
                sync_policy_json: "{}".into(),
                schema_hash: String::new(),
                created_at: created.clone(),
            };
            let schema_v = parse_json(&v.schema_json, "schema_json")?;
            let ui_v = parse_json(&v.ui_schema_json, "ui_schema_json")?;
            let hash = canonical_schema_hash(
                &schema_v,
                &ui_v,
                &json_empty(),
                &json_empty_array(),
                &json_empty(),
            )?;
            conn.execute("INSERT OR IGNORE INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES (?1,?2,?3,?4,'{}','[]','{}',?5,?6)",params![id,LEGACY_VERSION,serde_json::to_string(&canonical_value(&schema_v)).map_err(|e|e.to_string())?,serde_json::to_string(&canonical_value(&ui_v)).map_err(|e|e.to_string())?,hash,created]).map_err(|e|e.to_string())?;
        }
        Ok::<(), String>(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT ark_phase2_registry")
            .map_err(|e| e.to_string()),
        Err(e) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_phase2_registry; RELEASE SAVEPOINT ark_phase2_registry",
            );
            Err(e)
        }
    }
}
fn json_empty() -> Value {
    Value::Object(Map::new())
}
fn json_empty_array() -> Value {
    Value::Array(Vec::new())
}

pub fn ensure_builtin_versions(conn: &Connection) -> Result<(), String> {
    let mut stmt=conn.prepare("SELECT id,schema_json,ui_schema_json,created_at FROM object_types WHERE NOT EXISTS (SELECT 1 FROM object_type_versions v WHERE v.type_id=object_types.id AND v.version=?1)").map_err(|e|e.to_string())?;
    let rows = stmt
        .query_map(params![LEGACY_VERSION], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    for (id, schema, ui, created) in rows {
        insert_legacy_type_version(conn, &id, &schema, &ui, &created)?;
    }
    Ok(())
}

fn insert_legacy_type_version(
    conn: &Connection,
    type_id: &str,
    schema: &str,
    ui_schema: &str,
    created_at: &str,
) -> Result<(), String> {
    let schema_value = parse_json(schema, "schema_json")?;
    let ui_value = parse_json(ui_schema, "ui_schema_json")?;
    let schema_json =
        serde_json::to_string(&canonical_value(&schema_value)).map_err(|e| e.to_string())?;
    let ui_schema_json =
        serde_json::to_string(&canonical_value(&ui_value)).map_err(|e| e.to_string())?;
    let hash = canonical_schema_hash(
        &schema_value,
        &ui_value,
        &json_empty(),
        &json_empty_array(),
        &json_empty(),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES (?1,?2,?3,?4,'{}','[]','{}',?5,?6)",
        params![type_id, LEGACY_VERSION, schema_json, ui_schema_json, hash, created_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
