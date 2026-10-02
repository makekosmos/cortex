// ---------------------------------------------------------------------------
// Bulk operations
// ---------------------------------------------------------------------------

pub fn clear_all(conn: &Connection) -> Result<(), String> {
    if object_search_fts_exists(conn).unwrap_or(false) {
        conn.execute("DELETE FROM object_search_fts", [])
            .map_err(|e| e.to_string())?;
    }
    let phase3_completed = phase3_migration_completed(conn)?;
    for table in [
        "object_migration_quarantine",
        "object_local_state",
        "canonical_migration_items",
        "canonical_migration_runs",
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
        } else if phase3_completed {
            return Err(format!("clear_all missing owned table: {table}"));
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
         DELETE FROM todos;
         DELETE FROM projects;
         DELETE FROM tags;
         DELETE FROM sync_tombstones;
         DELETE FROM sync_kv;",
    )
    .map_err(|e| e.to_string())
}

pub fn delete_trashed(conn: &Connection, device_id: &str) -> Result<usize, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM todos WHERE is_trashed = 1")
        .map_err(|e| e.to_string())?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    // Hard-deleted rows must leave sync tombstones + version-vector bumps,
    // otherwise peers keep their copies and resurrect them on the next
    // sync round.
    conn.execute_batch("SAVEPOINT ark_delete_trashed")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<usize, String> {
        let deleted = conn
            .execute("DELETE FROM todos WHERE is_trashed = 1", [])
            .map_err(|e| e.to_string())?;
        for id in &ids {
            let hlc = bump_sync_version_vector(conn, "todo", id, device_id, true)?;
            record_sync_tombstone(conn, "todo", id, &hlc)?;
        }
        Ok(deleted)
    })();
    match result {
        Ok(count) => {
            conn.execute_batch("RELEASE SAVEPOINT ark_delete_trashed")
                .map_err(|e| e.to_string())?;
            Ok(count)
        }
        Err(error) => {
            let _ =
                conn.execute_batch("ROLLBACK TO ark_delete_trashed; RELEASE ark_delete_trashed");
            Err(error)
        }
    }
}

// ---------------------------------------------------------------------------
// Load all
// ---------------------------------------------------------------------------

pub fn load_all(conn: &Connection) -> Result<LoadAllData, String> {
    let mut todos = load_all_todos(conn)?;
    let mut projects = load_all_projects(conn)?;
    let mut tags = load_all_tags(conn)?;
    let mut tracked_apps = load_all_tracked_apps(conn)?;
    let mut usage_sessions = load_all_usage_sessions(conn)?;
    let mut usage_events = load_all_usage_events(conn)?;
    let mut objects = list_objects(conn)?;
    let (mut areas, mut headings) = compatibility_planning_views(&objects);
    let mut object_types = list_object_types(conn)?;
    let mut object_type_summaries = type_registry::list_type_summaries(conn)?;
    let mut object_type_versions = type_registry::list_all_type_versions(conn)?;
    let mut object_type_aliases = type_registry::list_aliases(conn)?;
    let mut object_links = list_object_links(conn)?;

    // The export contract is deterministic even when SQLite's query planner or
    // insertion order changes.  Keep the legacy collections and the additive
    // registry collections stable by their primary-key tuples.
    todos.sort_by(|a, b| a.id.cmp(&b.id));
    projects.sort_by(|a, b| a.id.cmp(&b.id));
    areas.sort_by(|a, b| a.id.cmp(&b.id));
    tags.sort_by(|a, b| a.id.cmp(&b.id));
    headings.sort_by(|a, b| a.id.cmp(&b.id));
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
        todos,
        projects,
        areas,
        tags,
        headings,
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

fn compatibility_planning_views(objects: &[ArkObject]) -> (Vec<Area>, Vec<Heading>) {
    let mut areas = Vec::new();
    let mut headings = Vec::new();
    for object in objects {
        if object.type_id != "com.kosmos.project"
            || !crate::canonical_types::definitions::is_canonical_version(
                &object.type_id,
                &object.type_version,
            )
        {
            continue;
        }
        let Some(compatibility) = object
            .props_json
            .get("extensions")
            .and_then(|value| value.get("compatibility"))
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        let Some(kind) = compatibility
            .get("legacyKind")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let sort_order = compatibility
            .get("sortOrder")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        match kind {
            "area" => areas.push(Area {
                id: object.id.clone(),
                title: object.title.clone(),
                sort_order,
                created_at: object.created_at.clone(),
            }),
            "heading" => {
                let Some(project_id) = compatibility
                    .get("legacyParentProjectId")
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                headings.push(Heading {
                    id: object.id.clone(),
                    title: object.title.clone(),
                    sort_order,
                    project_id: project_id.to_string(),
                });
            }
            _ => {}
        }
    }
    (areas, headings)
}
