//! Phase 3 checkpoint C: exact canonical registry installation and legacy registry evidence.
//! This module is deliberately independent from object migration and init wiring.

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
        "SELECT json_object('id',id,'name',name,'schema_json',schema_json,'ui_schema_json',ui_schema_json,'created_at',created_at,'updated_at',updated_at,'system_locked',system_locked,'owner_kind',owner_kind,'owner_id',owner_id,'current_version',current_version,'status',status,'base_type_id',base_type_id) FROM object_types WHERE id=?1",
        [type_id],
        |row| row.get(0),
    )
    .map_err(storage)
}

fn versions_json(conn: &Connection, type_id: &str) -> Result<String, RegistryError> {
    let mut statement = conn
        .prepare("SELECT version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at FROM object_type_versions WHERE type_id=?1")
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
        .prepare("SELECT alias,canonical_type_id,created_at FROM object_type_aliases WHERE canonical_type_id=?1 ORDER BY alias")
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
        "SELECT summary_json,versions_json,inbound_aliases_json,source_hash,archived_at,canonical_type_id FROM legacy_type_definition_archive WHERE contract_version=?1 AND legacy_type_id=?2",
        params![CONTRACT_VERSION, evidence.legacy_type_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?)),
    )
    .optional()
    .map(|row| row.map(|(summary, versions, inbound, source_hash, archived_at, canonical)| {
        summary == evidence.summary_json && versions == evidence.versions_json && inbound == evidence.inbound_aliases_json && source_hash == evidence.source_hash && archived_at == evidence.archived_at && canonical == evidence.canonical_type_id
    }))
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

fn is_generated_legacy_definition(
    conn: &Connection,
    registration: &TypeRegistration,
) -> Result<bool, RegistryError> {
    let row: Option<(String, String, String, String, Option<String>, String, Option<String>, i64)> = conn
        .query_row(
            "SELECT current_version,schema_json,ui_schema_json,owner_kind,owner_id,status,base_type_id,system_locked FROM object_types WHERE id=?1",
            [registration.type_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
        )
        .optional()
        .map_err(storage)?;
    let Some((
        current_version,
        schema,
        ui_schema,
        owner_kind,
        owner_id,
        status,
        base_type_id,
        system_locked,
    )) = row
    else {
        return Ok(false);
    };
    let Ok((expected_version, expected_hash)) = legacy_compatibility_version(&schema, &ui_schema)
    else {
        return Ok(false);
    };
    if current_version != expected_version
        || owner_kind != "system"
        || owner_id.is_some()
        || status != "active"
        || base_type_id.is_some()
        || system_locked != 1
    {
        return Ok(false);
    }
    let version: Option<(String, String, String, String, String, String)> = conn
        .query_row(
            "SELECT schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
            params![registration.type_id, current_version],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .optional()
        .map_err(storage)?;
    let Some((version_schema, version_ui, content, relations, policy, hash)) = version else {
        return Ok(false);
    };
    Ok(
        normalized_json(&schema, "schema_json")?
            == normalized_json(&version_schema, "schema_json")?
            && normalized_json(&ui_schema, "ui_schema_json")?
                == normalized_json(&version_ui, "ui_schema_json")?
            && normalized_json(&content, "content_contract_json")? == "{}"
            && normalized_json(&relations, "relations_json")? == "[]"
            && normalized_json(&policy, "sync_policy_json")? == "{}"
            && hash == expected_hash,
    )
}

/// Read-only registry inventory and collision validation. No schema or data is changed.
pub fn preflight_registry(conn: &Connection) -> Result<RegistryPlan, RegistryError> {
    let registrations = canonical_type_registrations()
        .map_err(|detail| RegistryError::InvariantViolation { detail })?;
    let canonical_ids: Vec<_> = registrations.iter().map(|r| r.type_id.clone()).collect();
    let mut legacy_type_ids = Vec::new();
    for registration in &registrations {
        let canonical_as_alias: Option<String> = conn
            .query_row(
                "SELECT canonical_type_id FROM object_type_aliases WHERE alias=?1",
                [registration.type_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage)?;
        if canonical_as_alias.is_some() {
            return Err(RegistryError::CanonicalAliasCollision {
                type_id: registration.type_id.clone(),
            });
        }
        if canonical_exists(conn, registration)? {
            if !is_generated_legacy_definition(conn, registration)? {
                let row: (String, Option<String>, String, String, i64, Option<String>, String) = conn.query_row(
                "SELECT name,owner_id,current_version,status,system_locked,base_type_id,owner_kind FROM object_types WHERE id=?1",
                [registration.type_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
            ).map_err(storage)?;
                if row.0 != registration.name
                    || row.1 != registration.owner_id
                    || row.2 != registration.version
                    || row.3 != registration.status
                    || row.5 != registration.base_type_id
                    || row.6 != registration.owner_kind
                {
                    return Err(RegistryError::CanonicalConflict {
                        type_id: registration.type_id.clone(),
                    });
                }
                let version = registration_version(registration)?;
                let existing: Option<(String, String, String, String, String, String)> = conn.query_row(
                "SELECT schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
                params![registration.type_id, registration.version],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            ).optional().map_err(storage)?;
                let expected_hash = registration.schema_hash.clone();
                let matches = existing
                    .map(|row| {
                        row.0 == version.0
                            && row.1 == version.1
                            && row.2 == version.2
                            && row.3 == version.3
                            && row.4 == version.4
                            && row.5 == expected_hash
                    })
                    .unwrap_or(false);
                if !matches {
                    return Err(RegistryError::CanonicalConflict {
                        type_id: registration.type_id.clone(),
                    });
                }
            }
        }
        for alias in &registration.aliases {
            let target: Option<String> = conn
                .query_row(
                    "SELECT canonical_type_id FROM object_type_aliases WHERE alias=?1",
                    [alias.alias.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(storage)?;
            if let Some(target) = target {
                if target != registration.type_id {
                    return Err(RegistryError::AliasTargetConflict {
                        alias: alias.alias.clone(),
                    });
                }
            }
            if canonical_ids.iter().any(|id| id == &alias.alias) {
                return Err(RegistryError::CanonicalAliasCollision {
                    type_id: alias.alias.clone(),
                });
            }
            if conn
                .query_row(
                    "SELECT 1 FROM object_types WHERE id=?1",
                    [alias.alias.as_str()],
                    |_| Ok(()),
                )
                .optional()
                .map_err(storage)?
                .is_some()
            {
                if !legacy_type_ids.contains(&alias.alias) {
                    legacy_type_ids.push(alias.alias.clone());
                }
            }
        }
        if conn
            .query_row(
                "SELECT 1 FROM object_types WHERE id=?1",
                [registration.aliases[0].alias.as_str()],
                |_| Ok(()),
            )
            .optional()
            .map_err(storage)?
            .is_some()
        {
            let legacy = &registration.aliases[0].alias;
            let evidence = archive_evidence(conn, legacy, &registration.type_id)?;
            if let Some(false) = existing_archive(conn, &evidence)? {
                return Err(RegistryError::ArchiveConflict {
                    type_id: legacy.clone(),
                });
            }
        }
    }
    legacy_type_ids.sort();
    Ok(RegistryPlan {
        contract_version: CONTRACT_VERSION.into(),
        canonical_type_ids: canonical_ids,
        legacy_type_ids,
    })
}

fn ensure_archive_schema(conn: &Connection) -> Result<(), RegistryError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS legacy_type_definition_archive (contract_version TEXT NOT NULL, legacy_type_id TEXT NOT NULL, canonical_type_id TEXT NOT NULL, summary_json TEXT NOT NULL, versions_json TEXT NOT NULL, inbound_aliases_json TEXT NOT NULL, source_hash TEXT NOT NULL, archived_at TEXT NOT NULL, PRIMARY KEY(contract_version,legacy_type_id)); CREATE INDEX IF NOT EXISTS idx_legacy_type_definition_archive_canonical ON legacy_type_definition_archive(canonical_type_id);").map_err(storage)
}

fn install_definition(
    conn: &Connection,
    registration: &TypeRegistration,
) -> Result<(), RegistryError> {
    let exists = canonical_exists(conn, registration)?;
    if !exists {
        conn.execute("INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked,owner_kind,owner_id,current_version,status,base_type_id) VALUES(?1,?2,?3,?4,?5,?5,1,?6,?7,?8,?9,?10)", params![registration.type_id, registration.name, registration.schema_json, registration.ui_schema_json, registration.created_at, registration.owner_kind, registration.owner_id, registration.version, registration.status, registration.base_type_id]).map_err(storage)?;
    } else {
        if is_generated_legacy_definition(conn, registration)? {
            let evidence = archive_evidence(conn, &registration.type_id, &registration.type_id)?;
            match existing_archive(conn, &evidence)? {
                Some(true) => {}
                Some(false) => {
                    return Err(RegistryError::ArchiveConflict {
                        type_id: registration.type_id.clone(),
                    })
                }
                None => {
                    conn.execute("INSERT INTO legacy_type_definition_archive(contract_version,legacy_type_id,canonical_type_id,summary_json,versions_json,inbound_aliases_json,source_hash,archived_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![CONTRACT_VERSION, evidence.legacy_type_id, evidence.canonical_type_id, evidence.summary_json, evidence.versions_json, evidence.inbound_aliases_json, evidence.source_hash, evidence.archived_at]).map_err(storage)?;
                }
            }
            conn.execute("UPDATE object_types SET name=?2,schema_json=?3,ui_schema_json=?4,created_at=?5,updated_at=?5,system_locked=1,owner_kind=?6,owner_id=?7,current_version=?8,status=?9,base_type_id=?10 WHERE id=?1", params![registration.type_id, registration.name, registration.schema_json, registration.ui_schema_json, registration.created_at, registration.owner_kind, registration.owner_id, registration.version, registration.status, registration.base_type_id]).map_err(storage)?;
        } else {
            conn.execute(
                "UPDATE object_types SET system_locked=1 WHERE id=?1",
                [registration.type_id.as_str()],
            )
            .map_err(storage)?;
        }
    }
    let version = registration_version(registration)?;
    let version_exists: Option<(String, String, String, String, String, String)> = conn
        .query_row(
            "SELECT schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
            params![registration.type_id, registration.version],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .optional()
        .map_err(storage)?;
    if let Some(existing) = version_exists {
        if existing
            != (
                version.0.clone(),
                version.1.clone(),
                version.2.clone(),
                version.3.clone(),
                version.4.clone(),
                registration.schema_hash.clone(),
            )
        {
            return Err(RegistryError::CanonicalConflict {
                type_id: registration.type_id.clone(),
            });
        }
        return Ok(());
    }
    conn.execute("INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![registration.type_id, registration.version, version.0, version.1, version.2, version.3, version.4, registration.schema_hash, registration.created_at]).map(|_| ()).map_err(storage)
}

fn archive_and_promote(
    conn: &Connection,
    registration: &TypeRegistration,
    legacy: &str,
) -> Result<ArchiveEvidence, RegistryError> {
    let evidence = archive_evidence(conn, legacy, &registration.type_id)?;
    match existing_archive(conn, &evidence)? {
        Some(true) => {}
        Some(false) => {
            return Err(RegistryError::ArchiveConflict {
                type_id: legacy.into(),
            })
        }
        None => {
            conn.execute("INSERT INTO legacy_type_definition_archive(contract_version,legacy_type_id,canonical_type_id,summary_json,versions_json,inbound_aliases_json,source_hash,archived_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![CONTRACT_VERSION, evidence.legacy_type_id, evidence.canonical_type_id, evidence.summary_json, evidence.versions_json, evidence.inbound_aliases_json, evidence.source_hash, evidence.archived_at]).map_err(storage)?;
        }
    }
    let object_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM objects WHERE type_id=?1",
            [legacy],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if object_count != 0 {
        return Err(RegistryError::LegacyPromotionConflict {
            type_id: legacy.into(),
        });
    }
    conn.execute(
        "UPDATE object_types SET base_type_id=?1 WHERE base_type_id=?2",
        params![registration.type_id, legacy],
    )
    .map_err(storage)?;
    conn.execute(
        "UPDATE object_type_aliases SET canonical_type_id=?1 WHERE canonical_type_id=?2",
        params![registration.type_id, legacy],
    )
    .map_err(storage)?;
    conn.execute(
        "DELETE FROM object_type_versions WHERE type_id=?1",
        [legacy],
    )
    .map_err(storage)?;
    conn.execute("DELETE FROM object_types WHERE id=?1", [legacy])
        .map_err(storage)?;
    Ok(evidence)
}

fn apply_inner(
    conn: &Connection,
    plan: &RegistryPlan,
    fail_after_archives: Option<usize>,
) -> Result<RegistryReport, RegistryError> {
    let registrations = canonical_type_registrations()
        .map_err(|detail| RegistryError::InvariantViolation { detail })?;
    ensure_archive_schema(conn)?;
    for registration in &registrations {
        install_definition(conn, registration)?;
    }
    let mut archived = 0;
    let mut promoted_aliases = Vec::new();
    for registration in &registrations {
        for alias in &registration.aliases {
            let legacy_exists = conn
                .query_row(
                    "SELECT 1 FROM object_types WHERE id=?1",
                    [alias.alias.as_str()],
                    |_| Ok(()),
                )
                .optional()
                .map_err(storage)?
                .is_some();
            if legacy_exists {
                archive_and_promote(conn, registration, &alias.alias)?;
                archived += 1;
                if fail_after_archives == Some(archived) {
                    return Err(RegistryError::InjectedFailure);
                }
                promoted_aliases.push(alias.alias.clone());
            }
            let existing: Option<(String, String)> = conn
                .query_row(
                    "SELECT canonical_type_id,created_at FROM object_type_aliases WHERE alias=?1",
                    [alias.alias.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(storage)?;
            match existing {
                Some((target, _)) if target != registration.type_id => {
                    return Err(RegistryError::AliasTargetConflict {
                        alias: alias.alias.clone(),
                    })
                }
                Some(_) => {}
                None => {
                    conn.execute("INSERT INTO object_type_aliases(alias,canonical_type_id,created_at) VALUES(?1,?2,?3)", params![alias.alias, registration.type_id, alias.created_at]).map_err(storage)?;
                }
            }
        }
    }
    Ok(RegistryReport {
        contract_version: plan.contract_version.clone(),
        installed: registrations.len(),
        archived,
        promoted_aliases,
    })
}

/// Install canonical definitions before object projection; legacy authority is archived
/// only after object rows have been rewritten to canonical identities.
pub fn prepare_registry_for_objects(conn: &Connection) -> Result<(), RegistryError> {
    ensure_archive_schema(conn)?;
    let registrations = canonical_type_registrations()
        .map_err(|detail| RegistryError::InvariantViolation { detail })?;
    for registration in &registrations {
        install_definition(conn, registration)?;
    }
    Ok(())
}

/// Apply a previously read-only plan inside a savepoint. The caller retains transaction ownership.
pub fn apply_registry(
    conn: &Connection,
    plan: &RegistryPlan,
) -> Result<RegistryReport, RegistryError> {
    apply_registry_with_failure(conn, plan, None)
}

/// Test seam for proving that archive/install mutations are atomic under a late failure.
pub fn apply_registry_with_failure(
    conn: &Connection,
    plan: &RegistryPlan,
    fail_after_archives: Option<usize>,
) -> Result<RegistryReport, RegistryError> {
    conn.execute_batch("SAVEPOINT phase3_registry")
        .map_err(storage)?;
    let result = apply_inner(conn, plan, fail_after_archives);
    match result {
        Ok(report) => {
            conn.execute_batch("RELEASE SAVEPOINT phase3_registry")
                .map_err(storage)?;
            Ok(report)
        }
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT phase3_registry; RELEASE SAVEPOINT phase3_registry",
            );
            Err(error)
        }
    }
}
