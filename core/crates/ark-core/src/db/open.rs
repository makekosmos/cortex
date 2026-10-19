use rusqlite::Connection;

use crate::canonical_types::migration;
pub use crate::canonical_types::pending::{
    count_pending_for_type, insert_pending_object, replay_pending_for_type,
};

pub(in crate::db) fn phase3_migration_completed(conn: &Connection) -> Result<bool, String> {
    let table_exists: bool = conn
        .query_row(
            concat!(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND ",
                "name='canonical_migration_runs')"
            ),
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !table_exists {
        return Ok(false);
    }
    conn.query_row(
        concat!(
            "SELECT EXISTS(SELECT 1 FROM canonical_migration_runs WHERE ",
            "contract_version='phase3-canonical-v1' AND status='completed')"
        ),
        [],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

pub fn init_schema(conn: &Connection) -> Result<(), String> {
    ark_store::open::init_schema_prerequisites_for_phase3(conn)?;
    crate::canonical_types::migration_objects::ensure_object_state_schema(conn).map_err(
        |error| format!("canonical runtime schema during init_schema failed: {error:?}"),
    )?;
    migration::migrate_phase3(conn)
        .map(|_| ())
        .map_err(|error| format!("phase3 migration during init_schema failed: {error:?}"))?;
    // A blocked legacy-object migration must not prevent new canonical writes.
    // Archive legacy definitions before installing contracts; leave the legacy
    // objects and planning tables untouched until their migration succeeds.
    conn.execute_batch("SAVEPOINT ark_canonical_contracts")
        .map_err(|error| error.to_string())?;
    match crate::canonical_types::migration_registry::prepare_registry_for_objects(conn) {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT ark_canonical_contracts")
            .map_err(|error| error.to_string())?,
        Err(error) => {
            let _ = conn.execute_batch(concat!(
                "ROLLBACK TO SAVEPOINT ark_canonical_contracts; ",
                "RELEASE SAVEPOINT ark_canonical_contracts",
            ));
            return Err(format!(
                "canonical version install during init_schema failed: {error:?}"
            ));
        }
    }
    if phase3_migration_completed(conn)? {
        migration::retire_legacy_planning_tables(conn)
            .map(|_| ())
            .map_err(|error| {
                format!("legacy planning retirement during init_schema failed: {error:?}")
            })?;
    }
    Ok(())
}

/// Online backup ARK DB в указанный destination path. Source connection
/// может оставаться live (SQLite Online Backup API safe для concurrent
/// readers/writer). Вызывается из db_backup scheduler в mundus-engine
/// через `Request::DbBackup`.
pub fn backup_to_file(conn: &Connection, dest_path: &str) -> Result<(), String> {
    conn.backup(rusqlite::DatabaseName::Main, dest_path, None)
        .map_err(|e| format!("backup_to_file failed: {e}"))
}

/// Online backup через ОТДЕЛЬНЫЙ read-коннекшн к `src_db_path` — НЕ держит
/// глобальный DB mutex, поэтому обычные ARK ops продолжают работать во время
/// копирования. Копирование идёт chunk'ами по `pages_per_step` страниц с паузой
/// `pause` между шагами, чтобы тяжёлый I/O не насыщал диск (стабильность ПК
/// важнее скорости бэкапа). WAL + busy_timeout source DB дают консистентный
/// снапшот при concurrent writer. Вызывается из `Request::DbBackup` на отдельном
/// background-priority потоке (см. ark-core service worker).
pub fn backup_to_file_chunked(
    src_db_path: &str,
    dest_path: &str,
    pages_per_step: i32,
    pause: std::time::Duration,
) -> Result<(), String> {
    ark_store::snapshot::backup_to_file_chunked_verified(
        src_db_path,
        dest_path,
        pages_per_step,
        pause,
    )
}

// Legacy registry definitions are owned by canonical_types::definitions.
// Historical rows remain readable through the Phase 3 migration/archive path.
