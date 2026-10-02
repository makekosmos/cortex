// Integrated Phase 3 migration orchestration.
//
// This facade owns sequencing and the outer rollback boundary.  Inventory,
// compatibility, ledger, registry, and object persistence remain owned by
// their dedicated modules.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use super::{migration_ledger, migration_objects, migration_registry, pending, preflight};

pub const CONTRACT_VERSION: &str = "phase3-canonical-v1";
const MIGRATION_TIME: &str = "1970-01-01T00:00:00.000Z";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationReport {
    pub contract_version: String,
    pub status: String,
    pub source_inventory_hash: String,
    pub migrated: usize,
    pub unchanged: usize,
    pub quarantined: usize,
    pub blocked: Vec<preflight::BlockedItem>,
    pub transformations: Vec<Value>,
    pub errors: Vec<preflight::BlockedItem>,
}

#[derive(Debug, Clone)]
pub struct MigrationPlan {
    pub contract_version: String,
    pub status: String,
    pub source_inventory_hash: String,
    pub blocked: Vec<preflight::BlockedItem>,
    pub transformations: Vec<Value>,
    pub errors: Vec<preflight::BlockedItem>,
    pub registry: Option<migration_registry::RegistryPlan>,
    pub objects: Option<migration_objects::ObjectPlan>,
}

#[derive(Debug, Clone, Default)]
pub struct MigrationOptions {
    /// Deterministic fault seam used by real-SQLite rollback tests.
    pub fail_after_registry_archives: Option<usize>,
    pub fail_after_objects: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationError {
    Storage(String),
    Preflight(String),
    Registry(String),
    Objects(String),
    Pending(String),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(_) => f.write_str("Storage"),
            Self::Preflight(_) => f.write_str("Preflight"),
            Self::Registry(_) => f.write_str("Registry"),
            Self::Objects(_) => f.write_str("Objects"),
            Self::Pending(_) => f.write_str("Pending"),
        }
    }
}
impl std::error::Error for MigrationError {}

fn error_item(code: String, detail: String) -> preflight::BlockedItem {
    preflight::BlockedItem {
        source_kind: "registry".into(),
        source_id: detail,
        pointer: "".into(),
        code,
        raw_source: Vec::new(),
    }
}

/// Compose every read-only checkpoint without creating migration tables or rows.
pub fn plan_phase3(conn: &Connection) -> Result<MigrationPlan, MigrationError> {
    let source = preflight::preflight_phase3(conn).map_err(MigrationError::Preflight)?;
    let registry = migration_registry::preflight_registry(conn);
    let objects = migration_objects::plan_objects(conn, MIGRATION_TIME);

    let mut blocked = source.blocked.clone();
    let mut errors = source.errors.clone();
    let registry_plan = match registry {
        Ok(plan) => Some(plan),
        Err(error) => {
            let item = error_item(error.code().into(), error.to_string());
            blocked.push(item.clone());
            errors.push(item);
            None
        }
    };
    let object_plan = match objects {
        Ok(plan) => {
            blocked.extend(plan.blocked.clone());
            errors.extend(plan.blocked.clone());
            Some(plan)
        }
        Err(error) => return Err(MigrationError::Objects(error.to_string())),
    };
    blocked.sort_by(|a, b| {
        (&a.source_kind, &a.source_id, &a.pointer, &a.code).cmp(&(
            &b.source_kind,
            &b.source_id,
            &b.pointer,
            &b.code,
        ))
    });
    errors.sort_by(|a, b| {
        (&a.source_kind, &a.source_id, &a.pointer, &a.code).cmp(&(
            &b.source_kind,
            &b.source_id,
            &b.pointer,
            &b.code,
        ))
    });
    let status = if source.status == "blocked" || !blocked.is_empty() {
        "blocked"
    } else {
        "ready"
    };
    Ok(MigrationPlan {
        contract_version: CONTRACT_VERSION.into(),
        status: status.into(),
        source_inventory_hash: source.source_inventory_hash,
        blocked,
        transformations: source.transformations,
        errors,
        registry: registry_plan,
        objects: object_plan,
    })
}

fn report_from_plan(plan: &MigrationPlan, status: &str) -> MigrationReport {
    MigrationReport {
        contract_version: plan.contract_version.clone(),
        status: status.into(),
        source_inventory_hash: plan.source_inventory_hash.clone(),
        migrated: 0,
        unchanged: 0,
        quarantined: 0,
        blocked: plan.blocked.clone(),
        transformations: plan.transformations.clone(),
        errors: plan.errors.clone(),
    }
}

fn completed_item_count(conn: &Connection) -> Result<usize, MigrationError> {
    let exists: bool = conn
        .query_row(
            concat!("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND ","name='canonical_migration_runs')"),
            [],
            |row| row.get(0),
        )
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    if !exists {
        return Ok(0);
    }
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM canonical_migration_items WHERE contract_version=?1",
            [CONTRACT_VERSION],
            |row| row.get(0),
        )
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    Ok(count as usize)
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

fn canonical_hash(item: &migration_objects::PlannedItem) -> Result<String, MigrationError> {
    let value = serde_json::json!({
        "object": item.mapped.object,
        "links": item.mapped.links,
        "local": item.mapped.local_state,
        "quarantine": item.mapped.quarantine,
    });
    let bytes = serde_json::to_vec(&canonical_json(&value))
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn ledger_kind(kind: &preflight::SourceKind) -> String {
    match kind {
        preflight::SourceKind::Legacy(name) => name.clone(),
        preflight::SourceKind::Native(name) => format!("native:{name}"),
    }
}

fn ensure_source_archive(
    conn: &Connection,
    items: &[migration_objects::PlannedItem],
) -> Result<(), MigrationError> {
    conn.execute_batch(concat!("CREATE TABLE IF NOT EXISTS canonical_migration_source_archive (","contract_version TEXT NOT NULL, source_kind TEXT NOT NULL, source_id TEXT ","NOT NULL, source_hash TEXT NOT NULL, raw_source BLOB NOT NULL, planned_json ","TEXT NOT NULL, PRIMARY KEY(contract_version,source_kind,source_id))"))
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    for item in items {
        let planned_json =
            serde_json::to_string(item).map_err(|e| MigrationError::Storage(e.to_string()))?;
        conn.execute(
            concat!("INSERT OR IGNORE INTO canonical_migration_source_archive(contract_version,","source_kind,source_id,source_hash,raw_source,planned_json) VALUES(?1,?2,?3,","?4,?5,?6)"),
            params![CONTRACT_VERSION, ledger_kind(&item.source_kind_variant), item.source_id, item.source_hash, item.raw_source, planned_json],
        )
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    }
    Ok(())
}

fn archived_items(
    conn: &Connection,
) -> Result<Vec<migration_objects::PlannedItem>, MigrationError> {
    let exists: bool = conn
        .query_row(
            concat!("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND ","name='canonical_migration_source_archive')"),
            [],
            |row| row.get(0),
        )
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    if !exists {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare(concat!("SELECT planned_json FROM canonical_migration_source_archive WHERE ","contract_version=?1 ORDER BY source_kind,source_id"))
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    let rows = stmt
        .query_map([CONTRACT_VERSION], |row| row.get::<_, String>(0))
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    rows.map(|row| {
        let json = row.map_err(|e| MigrationError::Storage(e.to_string()))?;
        serde_json::from_str(&json).map_err(|e| MigrationError::Storage(e.to_string()))
    })
    .collect()
}
