use rusqlite::Connection;

pub use crate::canonical_types::pending::{
    count_pending_for_type, insert_pending_object, replay_pending_for_type,
};

pub fn init_schema(conn: &Connection) -> Result<(), String> {
    ark_store::open::init_schema_prerequisites_for_phase3(conn)?;
    ark_store::canonical_migrations::objects::ensure_object_state_schema(conn).map_err(
        |error| format!("canonical runtime schema during init_schema failed: {error:?}"),
    )?;
    let migration = ark_store::canonical_migrations::migration::migrate_phase3(conn)
        .map_err(|error| format!("phase3 migration during init_schema failed: {error:?}"))?;
    // A blocked legacy-object migration must not prevent new canonical writes.
    // Archive legacy definitions before installing contracts; leave the legacy
    // objects and planning tables untouched until their migration succeeds.
    match ark_store::canonical_migrations::registry::prepare_registry_for_objects(conn) {
        Ok(()) => {}
        Err(error) => {
            return Err(format!(
                "canonical version install during init_schema failed: {error:?}"
            ))
        }
    }
    if migration.status == "completed" {
        ark_store::canonical_migrations::migration::retire_legacy_planning_tables(conn)
            .map(|_| ())
            .map_err(|error| {
                format!("legacy planning retirement during init_schema failed: {error:?}")
            })?;
    }
    Ok(())
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
