use super::super::migration_ledger::{self, ItemCheckpoint, ItemStatus};
use super::super::preflight::{SourceKind, SourceRecord};
use super::*;
use rusqlite::Connection;

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
                    migration_ledger::ItemTransition {
                        contract: CONTRACT_VERSION,
                        kind: &ledger_source_kind(&item.source_kind_variant),
                        id: &item.source_id,
                        status: if item.mapped.quarantine.is_empty() {
                            ItemStatus::Migrated
                        } else {
                            ItemStatus::Quarantined
                        },
                        checkpoint: ItemCheckpoint::Committed,
                        canonical_hash: Some(&canonical_hash),
                        result_json: "{}",
                        error_code: None,
                        now,
                    },
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
            let _ = conn.execute_batch(concat!(
                "ROLLBACK TO SAVEPOINT object_migration_outer; RELEASE SAVEPOINT ",
                "object_migration_outer"
            ));
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

/// Additive runtime tables are needed by canonical writes even while legacy
/// object migration is blocked. Creating them does not project legacy rows.
pub fn ensure_object_state_schema(conn: &Connection) -> Result<(), ObjectPlanError> {
    conn.execute_batch(concat!(
        "CREATE TABLE IF NOT EXISTS object_local_state (object_id TEXT NOT NULL,",
        "device_id TEXT NOT NULL,data_json TEXT NOT NULL DEFAULT '{}',updated_at ",
        "TEXT NOT NULL,PRIMARY KEY(object_id,device_id),FOREIGN KEY(object_id) ",
        "REFERENCES objects(id) ON DELETE CASCADE); CREATE TABLE IF NOT EXISTS ",
        "object_sync_versions (object_id TEXT PRIMARY KEY,hlc TEXT NOT NULL,deleted ",
        "INTEGER NOT NULL CHECK(deleted IN (0,1))); CREATE TABLE IF NOT EXISTS ",
        "object_migration_quarantine (object_id TEXT NOT NULL,contract_version TEXT ",
        "NOT NULL,source_type_id TEXT NOT NULL,fields_json TEXT NOT NULL DEFAULT ",
        "'{}',source_hash TEXT NOT NULL,updated_at TEXT NOT NULL,PRIMARY KEY(",
        "object_id,contract_version),FOREIGN KEY(object_id) REFERENCES objects(id) ",
        "ON DELETE CASCADE);"
    ))
    .map_err(|e| ObjectPlanError::Storage(e.to_string()))
}
