
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
    // Mirrors the `objects` column order in the SELECT below.
    type ObjectRow = (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
    );
    let actual: Option<ObjectRow> = conn
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
    plan.items.sort_by(|a, b| {
        let rank = |kind: &str| match kind {
            "areas" => 0,
            "projects" => 1,
            "headings" => 2,
            _ => 3,
        };
        (rank(&a.source_kind), &a.source_kind, &a.source_id)
            .cmp(&(rank(&b.source_kind), &b.source_kind, &b.source_id))
    });
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

/// Retire the two legacy planning tables only after the canonical migration
/// archived every source row and materialized the same IDs as project objects.
/// The archive is the rollback source; a count or identity mismatch fails
/// closed and leaves both legacy tables untouched.
pub fn retire_legacy_planning_tables(
    conn: &Connection,
) -> Result<(usize, usize), MigrationError> {
    let completed = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='canonical_migration_runs') AND EXISTS(SELECT 1 FROM canonical_migration_runs WHERE contract_version=?1 AND status='completed')",
            [CONTRACT_VERSION],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    if !completed {
        return Err(MigrationError::Objects(
            "legacy planning retirement requires completed migration".into(),
        ));
    }

    let table_exists = |table: &str| -> Result<bool, MigrationError> {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|e| MigrationError::Storage(e.to_string()))
    };
    let inventory = preflight::inventory_sources(conn).map_err(MigrationError::Storage)?;
    let mut counts = Vec::new();
    for (table, source_kind) in [("areas", "native:areas"), ("headings", "native:headings")] {
        if !table_exists(table)? {
            counts.push(0usize);
            continue;
        }
        let allowed_columns: &[&str] = match table {
            "areas" => &["id", "title", "sort_order", "created_at"],
            "headings" => &["id", "title", "sort_order", "project_id"],
            _ => &[],
        };
        let columns = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|e| MigrationError::Storage(e.to_string()))?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| MigrationError::Storage(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| MigrationError::Storage(e.to_string()))?;
        if columns
            .iter()
            .any(|column| !allowed_columns.contains(&column.as_str()))
        {
            return Err(MigrationError::Objects(format!(
                "legacy planning table {table} has unmigrated columns"
            )));
        }
        let source_count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .map_err(|e| MigrationError::Storage(e.to_string()))?;
        let archive_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canonical_migration_source_archive WHERE contract_version=?1 AND source_kind=?2",
                params![CONTRACT_VERSION, source_kind],
                |row| row.get(0),
            )
            .map_err(|e| MigrationError::Storage(e.to_string()))?;
        if source_count != archive_count {
            return Err(MigrationError::Objects(format!(
                "legacy planning archive incomplete for {table}: {source_count} source rows, {archive_count} archived"
            )));
        }
        let mut ids = conn
            .prepare(&format!("SELECT id FROM {table} ORDER BY id"))
            .map_err(|e| MigrationError::Storage(e.to_string()))?;
        let rows = ids
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| MigrationError::Storage(e.to_string()))?;
        for row in rows {
            let id = row.map_err(|e| MigrationError::Storage(e.to_string()))?;
            let (source_hash, raw_source, planned_json): (String, Vec<u8>, String) = conn
                .query_row(
                    "SELECT source_hash,raw_source,planned_json FROM canonical_migration_source_archive WHERE contract_version=?1 AND source_kind=?2 AND source_id=?3",
                    params![CONTRACT_VERSION, source_kind, id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(|e| MigrationError::Storage(e.to_string()))?
                .ok_or_else(|| {
                    MigrationError::Objects(format!(
                        "legacy planning row {table}/{id} has no archived source"
                    ))
                })?;
            let item: migration_objects::PlannedItem = serde_json::from_str(&planned_json)
                .map_err(|e| MigrationError::Storage(e.to_string()))?;
            if item.source_id != id
                || item.source_hash != source_hash
                || item.raw_source != raw_source
                || format!("{:x}", Sha256::digest(&raw_source)) != source_hash
            {
                return Err(MigrationError::Objects(format!(
                    "legacy planning archive digest mismatch for {table}/{id}"
                )));
            }
            let current = inventory
                .iter()
                .find(|record| record.source_kind.name() == table && record.source_id == id)
                .ok_or_else(|| {
                    MigrationError::Objects(format!(
                        "legacy planning source disappeared for {table}/{id}"
                    ))
                })?;
            if current.source_hash != source_hash || current.raw_source != raw_source {
                return Err(MigrationError::Objects(format!(
                    "legacy planning source changed after migration for {table}/{id}"
                )));
            }
            let expected_hash = canonical_hash(&item)?;
            let stored_hash: Option<String> = conn
                .query_row(
                    "SELECT canonical_hash FROM canonical_migration_items WHERE contract_version=?1 AND source_kind=?2 AND source_id=?3",
                    params![CONTRACT_VERSION, source_kind, id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| MigrationError::Storage(e.to_string()))?;
            if stored_hash.as_deref() != Some(expected_hash.as_str()) {
                return Err(MigrationError::Objects(format!(
                    "legacy planning canonical digest mismatch for {table}/{id}"
                )));
            }
            validate_canonical_state(conn, &item)?;
            let canonical: Option<(String, String)> = conn
                .query_row(
                    "SELECT type_id,type_version FROM objects WHERE id=?1",
                    [id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|e| MigrationError::Storage(e.to_string()))?;
            if !matches!(
                canonical.as_ref(),
                Some((type_id, version))
                    if type_id == "com.kosmos.project"
                        && crate::canonical_types::definitions::is_canonical_version(
                            type_id, version,
                        )
            ) {
                return Err(MigrationError::Objects(format!(
                    "legacy planning row {table}/{id} has no canonical project"
                )));
            }
        }
        counts.push(source_count as usize);
    }

    conn.execute_batch("SAVEPOINT phase9_legacy_planning_retirement")
        .map_err(|e| MigrationError::Storage(e.to_string()))?;
    let result = conn.execute_batch(
        "DROP TABLE IF EXISTS areas;
         DROP TABLE IF EXISTS headings;",
    );
    match result {
        Ok(()) => {
            conn.execute_batch("RELEASE SAVEPOINT phase9_legacy_planning_retirement")
                .map_err(|e| MigrationError::Storage(e.to_string()))?;
            Ok((counts[0], counts[1]))
        }
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT phase9_legacy_planning_retirement; RELEASE SAVEPOINT phase9_legacy_planning_retirement",
            );
            Err(MigrationError::Storage(error.to_string()))
        }
    }
}

#[allow(dead_code)]
fn _ledger_contract_is_stable() -> &'static str {
    migration_ledger::CONTRACT_VERSION
}
