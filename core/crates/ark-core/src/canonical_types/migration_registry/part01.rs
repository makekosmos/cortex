// Phase 3 checkpoint C: exact canonical registry installation and legacy registry evidence.
// This module is deliberately independent from object migration and init wiring.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::definitions::canonical_type_registrations;
use crate::type_registry::{legacy_compatibility_version, TypeRegistration};

pub const CONTRACT_VERSION: &str = "phase3-canonical-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    Storage(String),
    CanonicalConflict { type_id: String },
    CanonicalAliasCollision { type_id: String },
    AliasTargetConflict { alias: String },
    LegacyPromotionConflict { type_id: String },
    ArchiveConflict { type_id: String },
    InvariantViolation { detail: String },
    InjectedFailure,
}

impl RegistryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Storage(_) => "Storage",
            Self::CanonicalConflict { .. } => "CanonicalConflict",
            Self::CanonicalAliasCollision { .. } => "CanonicalAliasCollision",
            Self::AliasTargetConflict { .. } => "AliasTargetConflict",
            Self::LegacyPromotionConflict { .. } => "LegacyPromotionConflict",
            Self::ArchiveConflict { .. } => "ArchiveConflict",
            Self::InvariantViolation { .. } => "InvariantViolation",
            Self::InjectedFailure => "InjectedFailure",
        }
    }
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for RegistryError {}

fn storage(error: impl std::fmt::Display) -> RegistryError {
    RegistryError::Storage(error.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryPlan {
    pub contract_version: String,
    pub canonical_type_ids: Vec<String>,
    legacy_type_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryReport {
    pub contract_version: String,
    pub installed: usize,
    pub archived: usize,
    pub promoted_aliases: Vec<String>,
}

#[derive(Debug, Clone)]
struct ArchiveEvidence {
    legacy_type_id: String,
    canonical_type_id: String,
    summary_json: String,
    versions_json: String,
    inbound_aliases_json: String,
    source_hash: String,
    archived_at: String,
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

fn compact(value: &Value) -> Result<String, RegistryError> {
    serde_json::to_string(&canonical_json(value)).map_err(storage)
}

fn hash(value: &Value) -> Result<String, RegistryError> {
    Ok(format!("{:x}", Sha256::digest(compact(value)?.as_bytes())))
}

fn normalized_json(raw: &str, field: &str) -> Result<String, RegistryError> {
    let value: Value =
        serde_json::from_str(raw).map_err(|_| RegistryError::InvariantViolation {
            detail: field.into(),
        })?;
    compact(&value)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, RegistryError> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(storage)
}

fn registration_version(
    registration: &TypeRegistration,
) -> Result<(String, String, String, String, String), RegistryError> {
    Ok((
        normalized_json(&registration.schema_json, "schema_json")?,
        normalized_json(&registration.ui_schema_json, "ui_schema_json")?,
        normalized_json(&registration.content_contract_json, "content_contract_json")?,
        normalized_json(&registration.relations_json, "relations_json")?,
        normalized_json(&registration.sync_policy_json, "sync_policy_json")?,
    ))
}

fn summary_json(conn: &Connection, type_id: &str) -> Result<String, RegistryError> {
    conn.query_row(
        concat!(
            "SELECT json_object('id',id,'name',name,'schema_json',schema_json,",
            "'ui_schema_json',ui_schema_json,'created_at',created_at,'updated_at',",
            "updated_at,'system_locked',system_locked,'owner_kind',owner_kind,'owner_id',",
            "owner_id,'current_version',current_version,'status',status,'base_type_id',",
            "base_type_id) FROM object_types WHERE id=?1"
        ),
        [type_id],
        |row| row.get(0),
    )
    .map_err(storage)
}

fn versions_json(conn: &Connection, type_id: &str) -> Result<String, RegistryError> {
    let mut statement = conn
        .prepare(concat!(
            "SELECT version,schema_json,ui_schema_json,content_contract_json,",
            "relations_json,sync_policy_json,schema_hash,created_at FROM ",
            "object_type_versions WHERE type_id=?1"
        ))
        .map_err(storage)?;
    let mut versions = statement
        .query_map([type_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    versions.sort_by(|a, b| {
        Version::parse(&a.0)
            .ok()
            .cmp(&Version::parse(&b.0).ok())
            .then_with(|| a.0.cmp(&b.0))
    });
    let values: Vec<Value> = versions
        .into_iter()
        .map(
            |(version, schema, ui, content, relations, policy, schema_hash, created_at)| {
                serde_json::json!({
                    "version": version, "schema_json": schema, "ui_schema_json": ui,
                    "content_contract_json": content, "relations_json": relations,
                    "sync_policy_json": policy, "schema_hash": schema_hash, "created_at": created_at
                })
            },
        )
        .collect();
    compact(&Value::Array(values))
}

fn inbound_aliases_json(conn: &Connection, type_id: &str) -> Result<String, RegistryError> {
    let mut statement = conn
        .prepare(concat!(
            "SELECT alias,canonical_type_id,created_at FROM object_type_aliases WHERE ",
            "canonical_type_id=?1 ORDER BY alias"
        ))
        .map_err(storage)?;
    let aliases = statement
        .query_map([type_id], |row| {
            Ok(serde_json::json!({
                "alias": row.get::<_, String>(0)?,
                "canonical_type_id": row.get::<_, String>(1)?,
                "created_at": row.get::<_, String>(2)?
            }))
        })
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    compact(&Value::Array(aliases))
}

fn archive_evidence(
    conn: &Connection,
    legacy: &str,
    canonical: &str,
) -> Result<ArchiveEvidence, RegistryError> {
    let summary = summary_json(conn, legacy)?;
    let versions = versions_json(conn, legacy)?;
    let inbound = inbound_aliases_json(conn, legacy)?;
    let summary_value: Value = serde_json::from_str(&summary).map_err(storage)?;
    let versions_value: Value = serde_json::from_str(&versions).map_err(storage)?;
    let inbound_value: Value = serde_json::from_str(&inbound).map_err(storage)?;
    let source_hash = hash(&serde_json::json!({
        "legacyTypeId": legacy, "canonicalTypeId": canonical,
        "summary": summary_value, "versions": versions_value, "inboundAliases": inbound_value
    }))?;
    let archived_at: String = conn
        .query_row(
            "SELECT updated_at FROM object_types WHERE id=?1",
            [legacy],
            |row| row.get(0),
        )
        .map_err(storage)?;
    Ok(ArchiveEvidence {
        legacy_type_id: legacy.into(),
        canonical_type_id: canonical.into(),
        summary_json: summary,
        versions_json: versions,
        inbound_aliases_json: inbound,
        source_hash,
        archived_at,
    })
}

fn existing_archive(
    conn: &Connection,
    evidence: &ArchiveEvidence,
) -> Result<Option<bool>, RegistryError> {
    if !table_exists(conn, "legacy_type_definition_archive")? {
        return Ok(None);
    }
    conn.query_row(
        concat!(
            "SELECT summary_json,versions_json,inbound_aliases_json,source_hash,",
            "archived_at,canonical_type_id FROM legacy_type_definition_archive WHERE ",
            "contract_version=?1 AND legacy_type_id=?2"
        ),
        params![CONTRACT_VERSION, evidence.legacy_type_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    )
    .optional()
    .map(|row| {
        row.map(
            |(summary, versions, inbound, source_hash, archived_at, canonical)| {
                summary == evidence.summary_json
                    && versions == evidence.versions_json
                    && inbound == evidence.inbound_aliases_json
                    && source_hash == evidence.source_hash
                    && archived_at == evidence.archived_at
                    && canonical == evidence.canonical_type_id
            },
        )
    })
    .map_err(storage)
}

fn canonical_exists(
    conn: &Connection,
    registration: &TypeRegistration,
) -> Result<bool, RegistryError> {
    conn.query_row(
        "SELECT 1 FROM object_types WHERE id=?1",
        [registration.type_id.as_str()],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(storage)
}
