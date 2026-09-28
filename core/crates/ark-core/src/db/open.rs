use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{Duration, Utc};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};

use crate::canonical_types::migration;
pub use crate::canonical_types::pending::{
    count_pending_for_type, insert_pending_object, replay_pending_for_type,
};
use crate::hlc::HLC;
use crate::schema::{CREATE_OBJECT_SEARCH_FTS, CREATE_TABLES};
use crate::sync_server::StorageBackend;
use crate::type_registry;
use crate::types::*;

const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";
const USAGE_SEQUENCE_MIGRATION_KEY: &str = "usage_sync.sequence_v1";

pub fn is_sequenced_usage_entity(entity_type: &str) -> bool {
    matches!(entity_type, "usage_session" | "usage_event" | "usage_day")
}

fn usage_cursor_key(device_id: &str) -> String {
    format!("@usage:{device_id}")
}

// ---------------------------------------------------------------------------
// Open and init
// ---------------------------------------------------------------------------

pub fn open_db(path: &str) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// Online SQLite integrity check. Возвращает Err если DB corrupted.
/// Вызывается из init_schema до миграций — silent corruption хуже чем
/// fail-loud при старте (см. 2026-05-18 hardening loop AC2).
pub fn check_integrity(conn: &Connection) -> Result<(), String> {
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| format!("integrity_check query failed: {e}"))?;
    if result != "ok" {
        return Err(format!("ARK DB integrity check failed: {result}"));
    }
    Ok(())
}

fn phase3_migration_completed(conn: &Connection) -> Result<bool, String> {
    let table_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='canonical_migration_runs')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !table_exists {
        return Ok(false);
    }
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM canonical_migration_runs WHERE contract_version='phase3-canonical-v1' AND status='completed')",
        [],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

/// Initializes the schema and all Phase 2 prerequisites, but does not run Phase 3.
#[doc(hidden)]
pub fn init_schema_prerequisites_for_phase3(conn: &Connection) -> Result<(), String> {
    check_integrity(conn)?;
    // Prevent repeated replacement of the large sync version-vector value
    // from leaving permanent freelist growth. Existing databases adopt this
    // mode after the explicit maintenance VACUUM.
    conn.pragma_update(None, "auto_vacuum", "FULL")
        .map_err(|e| e.to_string())?;
    conn.execute_batch(CREATE_TABLES)
        .map_err(|e| e.to_string())?;
    ensure_integration_credential_fence_column(conn)?;
    ensure_authorized_node_transport_key(conn)?;
    ensure_sync_tombstone_type_id(conn)?;
    ensure_usage_runtime_ms(conn)?;
    conn.execute_batch("SAVEPOINT ark_phase2_init")
        .map_err(|e| e.to_string())?;
    let phase2_result = (|| {
        type_registry::migrate_phase2(conn)?;
        crate::data_platform::ensure_schema(conn)?;
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
    migration::migrate_phase3(conn)
        .map(|_| ())
        .map_err(|error| format!("phase3 migration during init_schema failed: {error:?}"))?;
    if phase3_migration_completed(conn)? {
        migration::retire_legacy_planning_tables(conn)
            .map(|_| ())
            .map_err(|error| format!("legacy planning retirement during init_schema failed: {error:?}"))?;
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
/// readers/writer). Вызывается из db_backup scheduler в kepler-backend
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
    let src = open_db(src_db_path)?;
    let mut dst = Connection::open(dest_path)
        .map_err(|e| format!("backup_to_file_chunked open dest failed: {e}"))?;
    let backup = rusqlite::backup::Backup::new(&src, &mut dst)
        .map_err(|e| format!("backup_to_file_chunked init failed: {e}"))?;
    backup
        .run_to_completion(
            pages_per_step,
            pause,
            None::<fn(rusqlite::backup::Progress)>,
        )
        .map_err(|e| format!("backup_to_file_chunked run failed: {e}"))
}

// Legacy registry definitions are owned by canonical_types::definitions.
// Historical rows remain readable through the Phase 3 migration/archive path.
