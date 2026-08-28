
pub fn register_type(conn: &Connection, registration: &TypeRegistration) -> Result<(), String> {
    validate_id(&registration.type_id, "type id")?;
    if registration.owner_kind.trim().is_empty() {
        return Err("invalid owner kind".into());
    }
    if registration.name.trim().is_empty() {
        return Err("invalid type name".into());
    }
    validate_version(&registration.version)?;
    if !matches!(
        registration.status.as_str(),
        "active" | "deprecated" | "pending"
    ) {
        return Err("invalid status".into());
    }
    if let Some(base) = &registration.base_type_id {
        validate_id(base, "base type id")?;
    }
    for alias in &registration.aliases {
        if alias.canonical_type_id != registration.type_id {
            return Err("alias canonical type mismatch".into());
        }
    }
    if conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![registration.type_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        // Package definitions are replayed on every runtime start. A replay
        // is safe only when the canonical summary and requested version are
        // identical; conflicting definitions remain errors.
        let existing_name: String = conn
            .query_row(
                "SELECT name FROM object_types WHERE id=?1",
                params![registration.type_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let existing = get_type(conn, &registration.type_id, Some(&registration.version))?
            .ok_or_else(|| "canonical type already exists".to_string())?;
        let definition = &existing.definition;
        let canonical = canonical_definition(&TypeVersion {
            type_id: registration.type_id.clone(),
            version: registration.version.clone(),
            schema_json: registration.schema_json.clone(),
            ui_schema_json: registration.ui_schema_json.clone(),
            content_contract_json: registration.content_contract_json.clone(),
            relations_json: registration.relations_json.clone(),
            sync_policy_json: registration.sync_policy_json.clone(),
            schema_hash: registration.schema_hash.clone(),
            created_at: registration.created_at.clone(),
        })?;
        let canonical_hash = canonical
            .4
            .split('|')
            .next()
            .ok_or_else(|| "canonical hash metadata missing".to_string())?;
        if existing_name == registration.name
            && existing.summary.owner_kind == registration.owner_kind
            && existing.summary.owner_id == registration.owner_id
            && existing.summary.status == registration.status
            && existing.summary.base_type_id == registration.base_type_id
            && definition.type_id == registration.type_id
            && definition.version == registration.version
            && definition.schema_json == canonical.0
            && definition.ui_schema_json == canonical.1
            && definition.content_contract_json == canonical.2
            && definition.relations_json == canonical.3
            && definition.sync_policy_json
                == serde_json::to_string(&canonical_value(&parse_json(
                    &registration.sync_policy_json,
                    "sync_policy_json",
                )?))
                .map_err(|e| e.to_string())?
            && definition.schema_hash == canonical_hash
            && definition.created_at == registration.created_at
        {
            return Ok(());
        }
        return Err("canonical type already exists".into());
    }

    if conn
        .query_row(
            "SELECT 1 FROM object_type_aliases WHERE alias=?1",
            params![registration.type_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err("canonical type id collides with alias".into());
    }

    conn.execute_batch("SAVEPOINT ark_registry_registration")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        conn.execute(
                "INSERT INTO object_types (id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked,owner_kind,owner_id,current_version,status,base_type_id) VALUES (?1,?2,?3,?4,?5,?5,0,?6,?7,?8,?9,?10)",
                params![registration.type_id, registration.name, registration.schema_json, registration.ui_schema_json, registration.created_at, registration.owner_kind, registration.owner_id, registration.version, registration.status, registration.base_type_id],
            )
            .map_err(|e| e.to_string())?;
        insert_type_version(
            conn,
            &TypeVersion {
                type_id: registration.type_id.clone(),
                version: registration.version.clone(),
                schema_json: registration.schema_json.clone(),
                ui_schema_json: registration.ui_schema_json.clone(),
                content_contract_json: registration.content_contract_json.clone(),
                relations_json: registration.relations_json.clone(),
                sync_policy_json: registration.sync_policy_json.clone(),
                schema_hash: registration.schema_hash.clone(),
                created_at: registration.created_at.clone(),
            },
            &registration.created_at,
        )?;
        for alias in &registration.aliases {
            register_alias(conn, alias)?;
        }
        set_current_version(conn, &registration.type_id, &registration.version)?;
        set_status(conn, &registration.type_id, &registration.status)?;
        set_base_type(
            conn,
            &registration.type_id,
            registration.base_type_id.as_deref(),
        )?;
        crate::canonical_types::pending::replay_pending_for_type(
            conn,
            &registration.type_id,
            &registration.version,
        )?;
        Ok::<(), String>(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT ark_registry_registration")
            .map_err(|e| e.to_string()),
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT ark_registry_registration; RELEASE SAVEPOINT ark_registry_registration");
            Err(error)
        }
    }
}

pub fn migrate_phase2(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ark_phase2_registry")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        conn.execute_batch("CREATE TABLE IF NOT EXISTS sync_pending_objects (id TEXT PRIMARY KEY, payload TEXT NOT NULL, awaited_type_id TEXT NOT NULL, received_at TEXT NOT NULL); CREATE INDEX IF NOT EXISTS idx_sync_pending_awaited_type ON sync_pending_objects(awaited_type_id); CREATE TABLE IF NOT EXISTS object_type_versions (type_id TEXT NOT NULL, version TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL DEFAULT '{}', content_contract_json TEXT NOT NULL DEFAULT '{}', relations_json TEXT NOT NULL DEFAULT '[]', sync_policy_json TEXT NOT NULL DEFAULT '{}', schema_hash TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(type_id,version), FOREIGN KEY(type_id) REFERENCES object_types(id)); CREATE TABLE IF NOT EXISTS object_type_aliases (alias TEXT PRIMARY KEY, canonical_type_id TEXT NOT NULL, created_at TEXT NOT NULL, FOREIGN KEY(canonical_type_id) REFERENCES object_types(id));").map_err(|e| e.to_string())?;
        for (table, column, definition) in [
            (
                "object_types",
                "owner_kind",
                "TEXT NOT NULL DEFAULT 'system'",
            ),
            ("object_types", "owner_id", "TEXT"),
            (
                "object_types",
                "current_version",
                "TEXT NOT NULL DEFAULT '0.0.0-legacy'",
            ),
            ("object_types", "status", "TEXT NOT NULL DEFAULT 'active'"),
            ("object_types", "base_type_id", "TEXT"),
            (
                "objects",
                "type_version",
                "TEXT NOT NULL DEFAULT '0.0.0-legacy'",
            ),
            (
                "sync_pending_objects",
                "awaited_type_version",
                "TEXT NOT NULL DEFAULT '0.0.0-legacy'",
            ),
        ] {
            let exists = conn
                .query_row(
                    &format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name=?1"),
                    params![column],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .is_some();
            if !exists {
                conn.execute_batch(&format!(
                    "ALTER TABLE {table} ADD COLUMN {column} {definition}"
                ))
                .map_err(|e| e.to_string())?;
            }
        }
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_object_type_versions_lookup ON object_type_versions(type_id,version); CREATE INDEX IF NOT EXISTS idx_object_type_aliases_canonical ON object_type_aliases(canonical_type_id); CREATE INDEX IF NOT EXISTS idx_objects_type_version ON objects(type_id,type_version); CREATE INDEX IF NOT EXISTS idx_sync_pending_awaited_type_version ON sync_pending_objects(awaited_type_id,awaited_type_version);").map_err(|e|e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id,schema_json,ui_schema_json,created_at,current_version FROM object_types",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        for (id, schema, ui, created, current_version) in rows {
            if current_version != LEGACY_VERSION {
                continue;
            }
            let v = TypeVersion {
                type_id: id.clone(),
                version: LEGACY_VERSION.into(),
                schema_json: schema,
                ui_schema_json: ui,
                content_contract_json: "{}".into(),
                relations_json: "[]".into(),
                sync_policy_json: "{}".into(),
                schema_hash: String::new(),
                created_at: created.clone(),
            };
            let schema_v = parse_json(&v.schema_json, "schema_json")?;
            let ui_v = parse_json(&v.ui_schema_json, "ui_schema_json")?;
            let hash = canonical_schema_hash(
                &schema_v,
                &ui_v,
                &json_empty(),
                &json_empty_array(),
                &json_empty(),
            )?;
            conn.execute("INSERT OR IGNORE INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES (?1,?2,?3,?4,'{}','[]','{}',?5,?6)",params![id,LEGACY_VERSION,serde_json::to_string(&canonical_value(&schema_v)).map_err(|e|e.to_string())?,serde_json::to_string(&canonical_value(&ui_v)).map_err(|e|e.to_string())?,hash,created]).map_err(|e|e.to_string())?;
        }
        Ok::<(), String>(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT ark_phase2_registry")
            .map_err(|e| e.to_string()),
        Err(e) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_phase2_registry; RELEASE SAVEPOINT ark_phase2_registry",
            );
            Err(e)
        }
    }
}
fn json_empty() -> Value {
    Value::Object(Map::new())
}
fn json_empty_array() -> Value {
    Value::Array(Vec::new())
}
