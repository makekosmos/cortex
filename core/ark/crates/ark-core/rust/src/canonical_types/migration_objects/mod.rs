//! Checkpoint D: read-only all-source planning and narrow per-item application.
mod apply;
mod native;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use super::compatibility::{
    map_legacy_with_context, CanonicalIdentity, LegacySource, MappedRecord, MappingContext,
};
use super::migration_ledger::{self, ItemCheckpoint, ItemStatus};
use super::preflight::{inventory_sources, BlockedItem, SourceKind, SourceRecord};

pub const CONTRACT_VERSION: &str = "phase3-canonical-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedItem {
    pub source_kind: String,
    pub source_kind_variant: SourceKind,
    pub source_id: String,
    pub source_hash: String,
    pub raw_source: Vec<u8>,
    pub mapped: MappedRecord,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPlan {
    pub contract_version: String,
    pub source_inventory_hash: String,
    pub items: Vec<PlannedItem>,
    pub blocked: Vec<BlockedItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObjectPlanError {
    Blocked {
        source_kind: String,
        source_id: String,
        pointer: String,
        code: String,
        raw_source: Vec<u8>,
    },
    Storage(String),
    CanonicalConflict {
        source_kind: String,
        source_id: String,
    },
}
impl std::fmt::Display for ObjectPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Blocked { .. } => "Blocked",
            Self::Storage(_) => "Storage",
            Self::CanonicalConflict { .. } => "CanonicalConflict",
        })
    }
}
impl std::error::Error for ObjectPlanError {}

fn hash(v: &[u8]) -> String {
    format!("{:x}", Sha256::digest(v))
}
fn inventory_hash(records: &[SourceRecord]) -> Result<String, ObjectPlanError> {
    let rows: Vec<Value> = records.iter().map(|r| serde_json::json!({"sourceKind":r.source_kind.name(),"sourceId":r.source_id,"sourceHash":r.source_hash})).collect();
    serde_json::to_vec(&rows)
        .map(|bytes| hash(&bytes))
        .map_err(|e| ObjectPlanError::Storage(e.to_string()))
}
fn aliases() -> Result<BTreeMap<String, CanonicalIdentity>, ObjectPlanError> {
    let result = super::definitions::canonical_type_registrations()
        .map_err(ObjectPlanError::Storage)?
        .into_iter()
        .flat_map(|r| {
            r.aliases.into_iter().map(move |a| {
                (
                    a.alias,
                    CanonicalIdentity::new(r.type_id.clone(), r.version.clone()),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    Ok(result)
}
fn context(conn: &Connection, records: &[SourceRecord]) -> Result<MappingContext, ObjectPlanError> {
    let alias = aliases()?;
    let mut ids = BTreeMap::new();
    let mut stmt = conn
        .prepare("SELECT id,type_id,type_version FROM objects")
        .map_err(|e| ObjectPlanError::Storage(e.to_string()))?;
    for row in stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| ObjectPlanError::Storage(e.to_string()))?
    {
        let (id, t, v) = row.map_err(|e| ObjectPlanError::Storage(e.to_string()))?;
        ids.insert(
            id,
            alias
                .get(&t)
                .cloned()
                .unwrap_or_else(|| CanonicalIdentity::new(t, v)),
        );
    }
    // All source identities are known before mapping: this makes forward and back references symmetric.
    for r in records {
        let canonical = match r.source_kind.name() {
            "todos" => "com.kosmos.task",
            "projects" | "areas" | "headings" => "com.kosmos.project",
            "tags" => "com.kosmos.tag",
            k => alias.get(k).map(|x| x.type_id.as_str()).unwrap_or(k),
        };
        ids.insert(
            r.source_id.clone(),
            CanonicalIdentity::new(canonical, "1.0.0"),
        );
    }
    Ok(MappingContext {
        existing_object_ids: ids,
        existing_links: crate::db::list_object_links(conn)
            .map_err(|e| ObjectPlanError::Storage(e.to_string()))?,
    })
}
fn blocked(e: &super::compatibility::CompatibilityError) -> BlockedItem {
    BlockedItem {
        source_kind: e.source_kind().into(),
        source_id: e.source_id().into(),
        pointer: e.pointer().into(),
        code: e.code().into(),
        raw_source: e.raw_source().to_vec(),
    }
}

/// Build a deterministic plan without creating schema, ledger rows, or mutating SQLite.
pub fn plan_objects(conn: &Connection, _now: &str) -> Result<ObjectPlan, ObjectPlanError> {
    let records = inventory_sources(conn).map_err(ObjectPlanError::Storage)?;
    let ctx = context(conn, &records)?;
    let mut items = Vec::new();
    let mut blocked_items = Vec::new();
    for r in &records {
        if matches!(r.source_kind, SourceKind::Native(ref kind) if kind == "areas" || kind == "headings")
        {
            continue;
        }
        if r.canonical_bytes.is_empty() {
            blocked_items.push(BlockedItem {
                source_kind: r.source_kind.name().into(),
                source_id: r.source_id.clone(),
                pointer: "/content_json".into(),
                code: "MALFORMED_JSON".into(),
                raw_source: r.raw_source.clone(),
            });
            continue;
        }
        let (kind, raw) = native::adapter(r);
        match map_legacy_with_context(
            LegacySource {
                source_kind: &kind,
                source_id: &r.source_id,
                raw_json: &raw,
            },
            &ctx,
        ) {
            Ok(mut m) => {
                m.raw_source = Some(r.raw_source.clone());
                items.push(PlannedItem {
                    source_kind: r.source_kind.name().into(),
                    source_kind_variant: r.source_kind.clone(),
                    source_id: r.source_id.clone(),
                    source_hash: r.source_hash.clone(),
                    raw_source: r.raw_source.clone(),
                    mapped: m,
                });
            }
            Err(e) => blocked_items.push(blocked(&e)),
        }
    }
    blocked_items.sort_by(|a, b| {
        (&a.source_kind, &a.source_id, &a.pointer, &a.code).cmp(&(
            &b.source_kind,
            &b.source_id,
            &b.pointer,
            &b.code,
        ))
    });
    items.sort_by(|a, b| (&a.source_kind, &a.source_id).cmp(&(&b.source_kind, &b.source_id)));
    Ok(ObjectPlan {
        contract_version: CONTRACT_VERSION.into(),
        source_inventory_hash: inventory_hash(&records)?,
        items,
        blocked: blocked_items,
    })
}

pub fn apply_plan(conn: &Connection, plan: &ObjectPlan, now: &str) -> Result<(), ObjectPlanError> {
    apply_plan_with_failure(conn, plan, now, None)
}

pub fn apply_plan_with_failure(
    conn: &Connection,
    plan: &ObjectPlan,
    now: &str,
    fail_after_objects: Option<usize>,
) -> Result<(), ObjectPlanError> {
    if !plan.blocked.is_empty() {
        return Err(ObjectPlanError::Blocked {
            source_kind: plan.blocked[0].source_kind.clone(),
            source_id: plan.blocked[0].source_id.clone(),
            pointer: plan.blocked[0].pointer.clone(),
            code: plan.blocked[0].code.clone(),
            raw_source: plan.blocked[0].raw_source.clone(),
        });
    }
    ensure_object_state_schema(conn)?;
    migration_ledger::ensure_ledger_schema(conn)
        .map_err(|e| ObjectPlanError::Storage(e.to_string()))?;
    conn.execute_batch("SAVEPOINT object_migration_outer")
        .map_err(|e| ObjectPlanError::Storage(e.to_string()))?;
    let result = (|| {
        let records: Vec<SourceRecord> = plan
            .items
            .iter()
            .map(|i| SourceRecord {
                source_kind: SourceKind::Legacy(ledger_source_kind(&i.source_kind_variant)),
                source_id: i.source_id.clone(),
                source_hash: i.source_hash.clone(),
                canonical_bytes: vec![],
                raw_source: i.raw_source.clone(),
            })
            .collect();
        let run = migration_ledger::begin_or_resume(conn, CONTRACT_VERSION, &records, now)
            .map_err(|e| ObjectPlanError::Storage(e.to_string()))?;
        if run.status == migration_ledger::MigrationRunStatus::Completed {
            return Ok(());
        }
        let mut applied_objects = 0usize;
        for item in &plan.items {
            let ledger_item = run
                .items
                .iter()
                .find(|x| {
                    x.source_kind == ledger_source_kind(&item.source_kind_variant)
                        && x.source_id == item.source_id
                })
                .ok_or_else(|| ObjectPlanError::Storage("missing ledger item".into()))?;
            if ledger_item.checkpoint == ItemCheckpoint::Committed {
                continue;
            }
            let mut sp = migration_ledger::ChildSavepoint::begin(conn, ledger_item)
                .map_err(|e| ObjectPlanError::Storage(e.to_string()))?;
            let result = apply::apply_one(conn, item, now).and_then(|canonical_hash| {
                migration_ledger::transition_item(
                    conn,
                    CONTRACT_VERSION,
                    &ledger_source_kind(&item.source_kind_variant),
                    &item.source_id,
                    if item.mapped.quarantine.is_empty() {
                        ItemStatus::Migrated
                    } else {
                        ItemStatus::Quarantined
                    },
                    ItemCheckpoint::Committed,
                    Some(&canonical_hash),
                    "{}",
                    None,
                    now,
                )
                .map(|_| canonical_hash)
                .map_err(|e| e.to_string())
                .map_err(ObjectPlanError::Storage)
            });
            match result {
                Ok(_) => {
                    sp.commit()
                        .map_err(|e| ObjectPlanError::Storage(e.to_string()))?;
                    applied_objects += 1;
                    if fail_after_objects == Some(applied_objects) {
                        return Err(ObjectPlanError::Storage(
                            "injected object migration failure".into(),
                        ));
                    }
                }
                Err(e) => {
                    let _ = sp.rollback(migration_ledger::LedgerError::Storage(e.to_string()));
                    return Err(e);
                }
            }
        }
        migration_ledger::complete_run(conn, CONTRACT_VERSION, "{}", now)
            .map_err(|e| ObjectPlanError::Storage(e.to_string()))
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT object_migration_outer")
            .map_err(|e| ObjectPlanError::Storage(e.to_string())),
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT object_migration_outer; RELEASE SAVEPOINT object_migration_outer");
            Err(error)
        }
    }
}

fn ledger_source_kind(kind: &SourceKind) -> String {
    match kind {
        SourceKind::Legacy(name) => name.clone(),
        SourceKind::Native(name) => format!("native:{name}"),
    }
}

fn ensure_object_state_schema(conn: &Connection) -> Result<(), ObjectPlanError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS object_local_state (object_id TEXT NOT NULL,device_id TEXT NOT NULL,data_json TEXT NOT NULL DEFAULT '{}',updated_at TEXT NOT NULL,PRIMARY KEY(object_id,device_id),FOREIGN KEY(object_id) REFERENCES objects(id) ON DELETE CASCADE); CREATE TABLE IF NOT EXISTS object_sync_versions (object_id TEXT PRIMARY KEY,hlc TEXT NOT NULL,deleted INTEGER NOT NULL CHECK(deleted IN (0,1))); CREATE TABLE IF NOT EXISTS object_migration_quarantine (object_id TEXT NOT NULL,contract_version TEXT NOT NULL,source_type_id TEXT NOT NULL,fields_json TEXT NOT NULL DEFAULT '{}',source_hash TEXT NOT NULL,updated_at TEXT NOT NULL,PRIMARY KEY(object_id,contract_version),FOREIGN KEY(object_id) REFERENCES objects(id) ON DELETE CASCADE);").map_err(|e| ObjectPlanError::Storage(e.to_string()))
}

pub use apply::stable_link_id;
