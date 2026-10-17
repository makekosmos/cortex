use ark_domain::canonical_type::registry::{
    canonicalize_type_version, validate_type_id, validate_version,
};
use ark_store::type_registry::{insert_type_record, type_name};
use rusqlite::{params, Connection, OptionalExtension};

use super::{
    get_type, insert_type_version, register_alias, resolve_alias, set_base_type,
    set_current_version, set_status, TypeRegistration, TypeVersion,
};

pub fn register_type(
    conn: &Connection,
    registration: &TypeRegistration,
) -> Result<Vec<crate::canonical_types::pending::ReplayNotice>, String> {
    validate_type_id(&registration.type_id, "type id")?;
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
        validate_type_id(base, "base type id")?;
    }
    for alias in &registration.aliases {
        if alias.canonical_type_id != registration.type_id {
            return Err("alias canonical type mismatch".into());
        }
    }
    if let Some(existing_name) = type_name(conn, &registration.type_id)? {
        // Package definitions are replayed on every runtime start. A replay
        // is safe only when the canonical summary and requested version are
        // identical; conflicting definitions remain errors.
        let existing = get_type(conn, &registration.type_id, Some(&registration.version))?
            .ok_or_else(|| "canonical type already exists".to_string())?;
        let definition = &existing.definition;
        let canonical = canonicalize_type_version(&TypeVersion {
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
        if existing_name == registration.name
            && existing.summary.owner_kind == registration.owner_kind
            && existing.summary.owner_id == registration.owner_id
            && existing.summary.status == registration.status
            && existing.summary.base_type_id == registration.base_type_id
            && definition.type_id == registration.type_id
            && definition.version == registration.version
            && definition.schema_json == canonical.schema_json
            && definition.ui_schema_json == canonical.ui_schema_json
            && definition.content_contract_json == canonical.content_contract_json
            && definition.relations_json == canonical.relations_json
            && definition.sync_policy_json == canonical.sync_policy_json
            && definition.schema_hash == canonical.schema_hash
            && definition.created_at == registration.created_at
        {
            return Ok(Vec::new());
        }
        return Err("canonical type already exists".into());
    }

    if resolve_alias(conn, &registration.type_id)?.is_some() {
        return Err("canonical type id collides with alias".into());
    }

    conn.execute_batch("SAVEPOINT ark_registry_registration")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        insert_type_record(conn, registration)?;
        let mut notices = insert_type_version(
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
        notices.extend(crate::canonical_types::pending::replay_pending_for_type(
            conn,
            &registration.type_id,
            &registration.version,
        )?);
        Ok::<_, String>(notices)
    })();
    match result {
        Ok(notices) => {
            conn.execute_batch("RELEASE SAVEPOINT ark_registry_registration")
                .map_err(|e| e.to_string())?;
            Ok(notices)
        }
        Err(error) => {
            let _ = conn.execute_batch(concat!(
                "ROLLBACK TO SAVEPOINT ark_registry_registration; RELEASE SAVEPOINT ",
                "ark_registry_registration"
            ));
            Err(error)
        }
    }
}

pub fn migrate_phase2(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ark_phase2_registry")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        ark_store::type_registry::migrate_schema(conn)?;
        conn.execute_batch(concat!(
            "CREATE TABLE IF NOT EXISTS sync_pending_objects (id TEXT PRIMARY KEY, ",
            "payload TEXT NOT NULL, awaited_type_id TEXT NOT NULL, received_at TEXT NOT ",
            "NULL); CREATE INDEX IF NOT EXISTS idx_sync_pending_awaited_type ON ",
            "sync_pending_objects(awaited_type_id);"
        ))
        .map_err(|e| e.to_string())?;
        for (table, column, definition) in [
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
        conn.execute_batch(concat!(
            "CREATE INDEX IF NOT EXISTS idx_objects_type_version ON objects(type_id,",
            "type_version); CREATE INDEX IF NOT EXISTS ",
            "idx_sync_pending_awaited_type_version ON sync_pending_objects(",
            "awaited_type_id,awaited_type_version);"
        ))
        .map_err(|e| e.to_string())?;
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
