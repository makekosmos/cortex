//! Storage lifecycle for canonical data migrations.

mod object_projection;

pub use object_projection::stable_link_id;

/// Persist a planned canonical object, its links and local migration state.
pub fn apply_object_projection(
    connection: &rusqlite::Connection,
    item: &ark_application::migration_source_projection::PlannedItem,
) -> Result<String, ark_application::migration_source_projection::ObjectPlanError> {
    object_projection::apply_one(connection, item)
}

use ark_domain::canonical_type::legacy_task::{
    map_legacy_task, LegacyTask, CORE_TASK_TYPE, CORE_TASK_VERSION, LEGACY_TASK_TYPE,
    LEGACY_TASK_VERSION,
};
use rusqlite::{params, Connection, Transaction};

use crate::{
    sync_versions::bump_sync_version_vector,
    writer::{SqliteWriter, TransactionFailure},
};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CoreTaskRewriteReport {
    pub rewritten: usize,
    pub live: usize,
    pub trashed: usize,
}

/// Migrate legacy task objects before the database actor publishes readiness.
/// All rows are validated before a no-clobber backup is created; updates and
/// sync-version changes then commit together in one writer transaction.
pub fn rewrite_legacy_tasks(
    writer: &mut SqliteWriter,
    backup_path: &str,
    device_id: &str,
) -> Result<CoreTaskRewriteReport, String> {
    let rows = writer.with_connection_mut(load_legacy_tasks)?;
    if rows.is_empty() {
        return Ok(CoreTaskRewriteReport::default());
    }

    let mapped = rows
        .iter()
        .map(|row| map_legacy_task(row).map(|props| (row, props)))
        .collect::<Result<Vec<_>, _>>()?;

    writer.with_connection_mut(|connection| {
        if std::path::Path::new(backup_path).exists() {
            return Err(format!("backup target already exists: {backup_path}"));
        }
        backup_to_file(connection, backup_path)
    })?;

    let report = writer
        .immediate(|transaction| apply_rewrite(transaction, &mapped, device_id))
        .map_err(format_transaction_failure)?;
    Ok(report)
}

fn load_legacy_tasks(connection: &mut Connection) -> Result<Vec<LegacyTask>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, type_version, content_json, props_json, deleted_at
             FROM objects WHERE type_id = ?1 ORDER BY id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![LEGACY_TASK_TYPE], |row| {
            Ok(LegacyTask {
                id: row.get(0)?,
                type_version: row.get(1)?,
                content_json: row.get(2)?,
                props_json: row.get(3)?,
                deleted_at: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn apply_rewrite(
    transaction: &Transaction<'_>,
    mapped: &[(&LegacyTask, serde_json::Value)],
    device_id: &str,
) -> Result<CoreTaskRewriteReport, String> {
    let mut report = CoreTaskRewriteReport::default();
    for (row, props) in mapped {
        let changed = transaction
            .execute(
                "UPDATE objects SET type_id = ?1, type_version = ?2, props_json = ?3
                 WHERE id = ?4 AND type_id = ?5 AND type_version = ?6",
                params![
                    CORE_TASK_TYPE,
                    CORE_TASK_VERSION,
                    props.to_string(),
                    row.id,
                    LEGACY_TASK_TYPE,
                    LEGACY_TASK_VERSION,
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err(format!("task {} changed during rewrite", row.id));
        }
        bump_sync_version_vector(transaction, "object", &row.id, device_id, false)?;
        report.rewritten += 1;
        if row.deleted_at.is_some() {
            report.trashed += 1;
        } else {
            report.live += 1;
        }
    }
    Ok(report)
}

fn backup_to_file(connection: &Connection, destination: &str) -> Result<(), String> {
    connection
        .backup(rusqlite::DatabaseName::Main, destination, None)
        .map_err(|error| format!("legacy task backup failed: {error}"))
}

fn format_transaction_failure(error: TransactionFailure<String>) -> String {
    match error {
        TransactionFailure::Begin(error) => format!("legacy task migration begin failed: {error}"),
        TransactionFailure::Operation(error) => error,
        TransactionFailure::Commit(error) => {
            format!("legacy task migration commit failed: {error}")
        }
    }
}
