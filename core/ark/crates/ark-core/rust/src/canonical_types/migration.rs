//! Integrated Phase 3 migration orchestration.
//!
//! This facade owns sequencing and the outer rollback boundary.  Inventory,
//! compatibility, ledger, registry, and object persistence remain owned by
//! their dedicated modules.

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
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='canonical_migration_runs')",
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
    conn.execute_batch("CREATE TABLE IF NOT EXISTS canonical_migration_source_archive (contract_version TEXT NOT NULL, source_kind TEXT NOT NULL, source_id TEXT NOT NULL, source_hash TEXT NOT NULL, raw_source BLOB NOT NULL, planned_json TEXT NOT NULL, PRIMARY KEY(contract_version,source_kind,source_id))")
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    for item in items {
        let planned_json =
            serde_json::to_string(item).map_err(|e| MigrationError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT OR IGNORE INTO canonical_migration_source_archive(contract_version,source_kind,source_id,source_hash,raw_source,planned_json) VALUES(?1,?2,?3,?4,?5,?6)",
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
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='canonical_migration_source_archive')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    if !exists {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare("SELECT planned_json FROM canonical_migration_source_archive WHERE contract_version=?1 ORDER BY source_kind,source_id")
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

fn run_status(conn: &Connection) -> Result<Option<String>, MigrationError> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='canonical_migration_runs')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    if !exists {
        return Ok(None);
    }
    conn.query_row(
        "SELECT status FROM canonical_migration_runs WHERE contract_version=?1",
        [CONTRACT_VERSION],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| MigrationError::Storage(e.to_string()))
}

fn json_equal(left: &str, right: &Value) -> Result<bool, MigrationError> {
    let value: Value =
        serde_json::from_str(left).map_err(|e| MigrationError::Storage(e.to_string()))?;
    Ok(canonical_json(&value) == canonical_json(right))
}

fn validate_canonical_state(
    conn: &Connection,
    item: &migration_objects::PlannedItem,
) -> Result<(), MigrationError> {
    let object = &item.mapped.object;
    let actual: Option<(String, String, String, String, String, String, String, Option<String>)> = conn
        .query_row(
            "SELECT type_id,type_version,title,content_json,props_json,created_at,updated_at,deleted_at FROM objects WHERE id=?1",
            [object.id.as_str()],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)),
        )
        .optional()
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    let Some((type_id, version, title, content, props, created, updated, deleted)) = actual else {
        return Err(MigrationError::Objects("CanonicalConflict".into()));
    };
    if type_id != object.type_id
        || version != object.type_version
        || title != object.title
        || !json_equal(&content, &object.content_json)?
        || !json_equal(&props, &object.props_json)?
        || created != object.created_at
        || updated != object.updated_at
        || deleted != object.deleted_at
    {
        return Err(MigrationError::Objects("CanonicalConflict".into()));
    }
    let mut links: Vec<(String, String, String)> = conn
        .prepare("SELECT source_object_id,link_type,target_object_id FROM object_links WHERE source_object_id=?1 ORDER BY link_type,target_object_id")
        .map_err(|e| MigrationError::Storage(e.to_string()))?
        .query_map([object.id.as_str()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))
        .map_err(|e| MigrationError::Storage(e.to_string()))?
        .collect::<Result<_, _>>()
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    let mut expected: Vec<_> = item
        .mapped
        .links
        .iter()
        .map(|link| {
            (
                link.source_object_id.clone(),
                link.link_type.clone(),
                link.target_object_id.clone(),
            )
        })
        .collect();
    links.sort();
    expected.sort();
    if links != expected {
        return Err(MigrationError::Objects("CanonicalConflict".into()));
    }
    Ok(())
}

fn validate_completed(
    conn: &Connection,
    current: &migration_objects::ObjectPlan,
) -> Result<usize, MigrationError> {
    let archived = archived_items(conn)?;
    for item in &archived {
        let current_item = current.items.iter().find(|candidate| {
            ledger_kind(&candidate.source_kind_variant) == ledger_kind(&item.source_kind_variant)
                && candidate.source_id == item.source_id
        });
        if let Some(current_item) = current_item {
            if current_item.source_hash != item.source_hash
                || current_item.raw_source != item.raw_source
            {
                return Err(MigrationError::Objects("CanonicalConflict".into()));
            }
        }
        let stored_hash: Option<String> = conn
            .query_row(
                "SELECT canonical_hash FROM canonical_migration_items WHERE contract_version=?1 AND source_kind=?2 AND source_id=?3",
                params![CONTRACT_VERSION, ledger_kind(&item.source_kind_variant), item.source_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| MigrationError::Storage(e.to_string()))?;
        if stored_hash.as_deref() != Some(canonical_hash(item)?.as_str()) {
            return Err(MigrationError::Objects("CanonicalConflict".into()));
        }
        validate_canonical_state(conn, item)?;
    }
    Ok(archived.len())
}

fn merge_archived_items(
    plan: &mut migration_objects::ObjectPlan,
    archived: Vec<migration_objects::PlannedItem>,
) {
    for item in archived {
        if !plan.items.iter().any(|candidate| {
            ledger_kind(&candidate.source_kind_variant) == ledger_kind(&item.source_kind_variant)
                && candidate.source_id == item.source_id
        }) {
            plan.items.push(item);
        }
    }
    plan.items
        .sort_by(|a, b| (&a.source_kind, &a.source_id).cmp(&(&b.source_kind, &b.source_id)));
}

/// Apply a ready plan under one caller-visible outer savepoint.
pub fn migrate_phase3(conn: &Connection) -> Result<MigrationReport, MigrationError> {
    migrate_phase3_with_options(conn, &MigrationOptions::default())
}

pub fn migrate_phase3_with_options(
    conn: &Connection,
    options: &MigrationOptions,
) -> Result<MigrationReport, MigrationError> {
    let plan = plan_phase3(conn)?;
    if plan.status == "blocked" {
        if let Some(error) = plan
            .errors
            .iter()
            .find(|item| item.source_kind == "registry")
        {
            return Err(MigrationError::Registry(error.code.clone()));
        }
        return Ok(report_from_plan(&plan, "blocked"));
    }
    let prior_items = completed_item_count(conn)?;
    let existing_status = run_status(conn)?;
    let mut objects = plan
        .objects
        .clone()
        .ok_or_else(|| MigrationError::Objects("missing plan".into()))?;
    conn.execute_batch("SAVEPOINT phase3_migration_outer")
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    let result = (|| {
        if existing_status.as_deref() == Some("completed") {
            let unchanged = validate_completed(conn, &objects)?;
            let mut report = report_from_plan(&plan, "completed");
            report.source_inventory_hash = conn
                .query_row(
                    "SELECT source_inventory_hash FROM canonical_migration_runs WHERE contract_version=?1",
                    [CONTRACT_VERSION],
                    |row| row.get(0),
                )
                .map_err(|e| MigrationError::Storage(e.to_string()))?;
            report.unchanged = unchanged;
            return Ok(report);
        }
        pending::migrate_phase2_to_v3(conn).map_err(|e| MigrationError::Pending(e.to_string()))?;
        migration_registry::prepare_registry_for_objects(conn)
            .map_err(|e| MigrationError::Registry(e.to_string()))?;
        ensure_source_archive(conn, &objects.items)?;
        if existing_status.is_some() {
            merge_archived_items(&mut objects, archived_items(conn)?);
        }
        migration_objects::apply_plan_with_failure(
            conn,
            &objects,
            MIGRATION_TIME,
            options.fail_after_objects,
        )
        .map_err(|e| MigrationError::Objects(format!("{e:?}")))?;
        let registry = plan
            .registry
            .as_ref()
            .ok_or_else(|| MigrationError::Registry("missing plan".into()))?;
        migration_registry::apply_registry_with_failure(
            conn,
            registry,
            options.fail_after_registry_archives,
        )
        .map_err(|e| MigrationError::Registry(e.to_string()))?;
        let mut report = report_from_plan(&plan, "completed");
        if prior_items > 0 {
            report.unchanged = prior_items;
        } else {
            report.migrated = objects
                .items
                .iter()
                .filter(|item| item.mapped.quarantine.is_empty())
                .count();
            report.quarantined = objects
                .items
                .iter()
                .filter(|item| !item.mapped.quarantine.is_empty())
                .count();
        }
        Ok(report)
    })();
    match result {
        Ok(report) => {
            conn.execute_batch("RELEASE SAVEPOINT phase3_migration_outer")
                .map_err(|e| MigrationError::Storage(e.to_string()))?;
            Ok(report)
        }
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT phase3_migration_outer; RELEASE SAVEPOINT phase3_migration_outer",
            );
            Err(error)
        }
    }
}

#[allow(dead_code)]
fn _ledger_contract_is_stable() -> &'static str {
    migration_ledger::CONTRACT_VERSION
}
