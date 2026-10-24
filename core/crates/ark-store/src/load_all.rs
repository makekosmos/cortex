// ---------------------------------------------------------------------------
// Bulk operations
// ---------------------------------------------------------------------------

use ark_application::load_all_data::LoadAllData;
use rusqlite::Connection;

pub fn clear_all(conn: &Connection) -> Result<(), String> {
    if crate::objects::object_search_fts_exists(conn).unwrap_or(false) {
        conn.execute("DELETE FROM object_search_fts", [])
            .map_err(|e| e.to_string())?;
    }
    for table in [
        "device_registry_events",
        "device_registry_projection",
        "object_migration_quarantine",
        "object_local_state",
        "entity_write_receipts",
        "ark_operation_receipts",
        "object_sync_versions",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists {
            conn.execute(&format!("DELETE FROM {table}"), [])
                .map_err(|e| e.to_string())?;
        }
    }
    conn.execute_batch(
        "DELETE FROM object_links;
         DELETE FROM objects;
         DELETE FROM sync_pending_objects;
         DELETE FROM object_type_aliases WHERE canonical_type_id IN (SELECT id FROM \
         object_types WHERE system_locked = 0);
         DELETE FROM object_type_versions WHERE type_id IN (SELECT id FROM object_types WHERE \
         system_locked = 0);
         DELETE FROM object_types WHERE system_locked = 0;
         DELETE FROM usage_days;
         DELETE FROM usage_events;
         DELETE FROM usage_sessions;
         DELETE FROM usage_sync_log;
         DELETE FROM usage_sync_versions;
         DELETE FROM tracked_apps;
         DELETE FROM sync_tombstones;
         DELETE FROM sync_kv;",
    )
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Load all
// ---------------------------------------------------------------------------

pub fn load_all(conn: &Connection) -> Result<LoadAllData, String> {
    let mut tracked_apps = crate::usage::load_all_tracked_apps(conn)?;
    let mut usage_sessions = crate::usage_history::load_all_usage_sessions(conn)?;
    let mut usage_events = crate::usage_history::load_all_usage_events(conn)?;
    let mut objects = crate::objects::list_objects(conn)?;
    let mut object_types = crate::object_types::list(conn)?;
    let mut object_type_summaries = crate::type_registry::list_type_summaries(conn)?;
    let mut object_type_versions = crate::type_registry::list_all_type_versions(conn)?;
    let mut object_type_aliases = crate::type_registry::list_aliases(conn)?;
    let mut object_links = crate::objects::list_object_links(conn)?;

    // Keep exported collections stable when SQLite's query plan or insertion
    // order changes.
    tracked_apps.sort_by(|a, b| a.id.cmp(&b.id));
    usage_sessions.sort_by(|a, b| a.id.cmp(&b.id));
    usage_events.sort_by(|a, b| a.id.cmp(&b.id));
    objects.sort_by(|a, b| a.id.cmp(&b.id));
    object_types.sort_by(|a, b| a.id.cmp(&b.id));
    object_type_summaries.sort_by(|a, b| a.type_id.cmp(&b.type_id));
    object_type_versions.sort_by(|a, b| {
        a.type_id
            .cmp(&b.type_id)
            .then_with(|| a.version.cmp(&b.version))
    });
    object_type_aliases.sort_by(|a, b| a.alias.cmp(&b.alias));
    object_links.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(LoadAllData {
        tracked_apps,
        usage_sessions,
        usage_events,
        objects,
        object_types,
        object_type_summaries,
        object_type_versions,
        object_type_aliases,
        object_links,
    })
}
