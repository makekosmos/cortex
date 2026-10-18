use rusqlite::Connection;

use crate::canonical_types::migration;
pub use crate::canonical_types::pending::{
    count_pending_for_type, insert_pending_object, replay_pending_for_type,
};
use crate::type_registry;
use ark_store::schema::CREATE_TABLES;

use super::objects::{ensure_object_search_fts, rebuild_object_search_fts};

pub(crate) use ark_store::VERSION_VECTOR_KEY;
pub(in crate::db) const USAGE_SEQUENCE_MIGRATION_KEY: &str = "usage_sync.sequence_v1";

pub fn is_sequenced_usage_entity(entity_type: &str) -> bool {
    matches!(entity_type, "usage_session" | "usage_event" | "usage_day")
}

pub(in crate::db) fn usage_cursor_key(device_id: &str) -> String {
    format!("@usage:{device_id}")
}

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

/// Initializes the schema and all Phase 2 prerequisites, but does not run Phase 3.
#[doc(hidden)]
pub fn init_schema_prerequisites_for_phase3(conn: &Connection) -> Result<(), String> {
    ark_store::open::check_integrity(conn)?;
    // Prevent repeated replacement of the large sync version-vector value
    // from leaving permanent freelist growth. Existing databases adopt this
    // mode after the explicit maintenance VACUUM.
    conn.pragma_update(None, "auto_vacuum", "FULL")
        .map_err(|e| e.to_string())?;
    conn.execute_batch(CREATE_TABLES)
        .map_err(|e| e.to_string())?;
    ensure_task_merge_delivery_journal(conn)?;
    ensure_device_registry_position(conn)?;
    ensure_device_registry_updated_hlc(conn)?;
    super::committed_feed::bootstrap_committed_feed_heads(conn)?;
    ensure_integration_credential_fence_column(conn)?;
    ensure_authorized_node_transport_key(conn)?;
    ensure_sync_tombstone_type_id(conn)?;
    ensure_usage_runtime_ms(conn)?;
    conn.execute_batch("SAVEPOINT ark_phase2_init")
        .map_err(|e| e.to_string())?;
    let phase2_result = (|| {
        type_registry::migrate_phase2(conn)?;
        ark_store::data_platform::ensure_schema(conn)?;
        let object_search_fts_enabled = ensure_object_search_fts(conn).is_ok();
        if object_search_fts_enabled {
            rebuild_object_search_fts(conn)?;
        }
        Ok::<bool, String>(object_search_fts_enabled)
    })();
    match phase2_result {
        Ok(_) => conn
            .execute_batch("RELEASE SAVEPOINT ark_phase2_init")
            .map_err(|e| e.to_string())?,
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_phase2_init; RELEASE SAVEPOINT ark_phase2_init",
            );
            return Err(error);
        }
    }
    Ok(())
}

pub(in crate::db) fn ensure_task_merge_delivery_journal(conn: &Connection) -> Result<(), String> {
    let columns = conn
        .prepare("PRAGMA table_info(task_merge_journal)")
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    if columns.iter().any(|column| column == "operation_id") {
        conn.execute_batch("SAVEPOINT ark_task_merge_delivery_journal")
            .map_err(|error| error.to_string())?;
        let migration = conn.execute_batch(
            "ALTER TABLE task_merge_journal RENAME TO task_merge_operation_journal_legacy;
             CREATE TABLE task_merge_journal (
                 position INTEGER PRIMARY KEY AUTOINCREMENT,
                 record_kind TEXT NOT NULL CHECK (record_kind IN ('bootstrap', 'operation')),
                 record_id TEXT NOT NULL,
                 UNIQUE(record_kind, record_id)
             );
             INSERT INTO task_merge_journal(position, record_kind, record_id)
             SELECT position, 'operation', operation_id
             FROM task_merge_operation_journal_legacy ORDER BY position;
             DROP TABLE task_merge_operation_journal_legacy;
             RELEASE SAVEPOINT ark_task_merge_delivery_journal;",
        );
        if let Err(error) = migration {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_task_merge_delivery_journal;
                 RELEASE SAVEPOINT ark_task_merge_delivery_journal;",
            );
            return Err(error.to_string());
        }
    }

    conn.execute_batch(
        "INSERT OR IGNORE INTO task_merge_journal(record_kind, record_id)
         SELECT 'operation', operation_id FROM task_merge_operations ORDER BY rowid;",
    )
    .map_err(|error| error.to_string())
}

fn ensure_device_registry_position(conn: &Connection) -> Result<(), String> {
    let has_position = conn
        .prepare("PRAGMA table_info(device_registry_events)")
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|name| name == "registry_position");
    if !has_position {
        conn.execute_batch(
            "ALTER TABLE device_registry_events
             ADD COLUMN registry_position INTEGER NOT NULL DEFAULT 0",
        )
        .map_err(|error| error.to_string())?;
        conn.execute_batch(
            "WITH ordered AS (
                 SELECT actor_device_id, actor_sequence,
                        row_number() OVER (ORDER BY actor_device_id, actor_sequence) AS position
                 FROM device_registry_events
             )
             UPDATE device_registry_events
             SET registry_position = (
                 SELECT position FROM ordered
                 WHERE ordered.actor_device_id = device_registry_events.actor_device_id
                   AND ordered.actor_sequence = device_registry_events.actor_sequence
             )",
        )
        .map_err(|error| error.to_string())?;
    }
    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_device_registry_events_position
         ON device_registry_events(registry_position)",
    )
    .map_err(|error| error.to_string())
}

fn ensure_device_registry_updated_hlc(conn: &Connection) -> Result<(), String> {
    let has_updated_hlc = conn
        .prepare("PRAGMA table_info(device_registry_projection)")
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|name| name == "updated_hlc");
    if !has_updated_hlc {
        conn.execute_batch("ALTER TABLE device_registry_projection ADD COLUMN updated_hlc TEXT")
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn ensure_authorized_node_transport_key(conn: &Connection) -> Result<(), String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('authorized_nodes')
             WHERE name = 'transport_public_key')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !exists {
        conn.execute(
            "ALTER TABLE authorized_nodes ADD COLUMN transport_public_key TEXT",
            [],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn ensure_integration_credential_fence_column(conn: &Connection) -> Result<(), String> {
    let has_column = conn
        .prepare("PRAGMA table_info(integration_credential_envelopes)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|name| name == "refresh_fencing_token");
    if !has_column {
        conn.execute_batch(
            "ALTER TABLE integration_credential_envelopes
             ADD COLUMN refresh_fencing_token INTEGER NOT NULL DEFAULT 0",
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn init_schema(conn: &Connection) -> Result<(), String> {
    init_schema_prerequisites_for_phase3(conn)?;
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

fn ensure_usage_runtime_ms(conn: &Connection) -> Result<(), String> {
    let has_runtime_ms = conn
        .prepare("PRAGMA table_info(usage_sessions)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|name| name == "runtime_ms");

    if !has_runtime_ms {
        conn.execute_batch(
            "ALTER TABLE usage_sessions ADD COLUMN runtime_ms INTEGER NOT NULL DEFAULT 0;",
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "UPDATE usage_sessions
         SET runtime_ms = COALESCE(foreground_ms, 0) + COALESCE(idle_ms, 0)
         WHERE runtime_ms = 0 AND (COALESCE(foreground_ms, 0) > 0 OR COALESCE(idle_ms, 0) > 0)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_sync_tombstone_type_id(conn: &Connection) -> Result<(), String> {
    let has_type_id = conn
        .prepare("PRAGMA table_info(sync_tombstones)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|name| name == "type_id");
    if !has_type_id {
        conn.execute_batch("ALTER TABLE sync_tombstones ADD COLUMN type_id TEXT;")
            .map_err(|e| e.to_string())?;
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
