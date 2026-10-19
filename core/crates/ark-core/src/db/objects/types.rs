pub fn upsert_object_type(
    conn: &Connection,
    object_type: &ObjectType,
) -> Result<Vec<crate::canonical_types::pending::ReplayNotice>, String> {
    if ark_store::object_types::legacy_alias_conflicts(conn, &object_type.id)? {
        return Err("alias collides with canonical type".into());
    }
    if is_canonical_type(&object_type.id) {
        return Ok(Vec::new());
    }

    let (version, schema_hash) = type_registry::legacy_compatibility_version(
        &object_type.schema_json,
        &object_type.ui_schema_json,
    )?;
    let current_version = ark_store::object_types::upsert_legacy_type(
        conn,
        object_type,
        &ark_store::object_types::LegacyCompatibilityVersion {
            version,
            schema_hash,
        },
    )?;

    let mut replay_notices = replay_pending_for_type(conn, &object_type.id, &current_version)?;
    if current_version != type_registry::LEGACY_VERSION {
        replay_notices.extend(replay_pending_for_type(
            conn,
            &object_type.id,
            type_registry::LEGACY_VERSION,
        )?);
    }
    Ok(replay_notices)
}

pub fn delete_object_type(conn: &Connection, id: &str) -> Result<(), String> {
    if is_canonical_type(id) {
        return Ok(());
    }
    ark_store::object_types::deprecate_legacy_type(conn, id)
}

pub fn is_object_type_known(conn: &Connection, type_id: &str) -> Result<bool, String> {
    ark_store::object_types::is_type_known(conn, type_id)
}

pub fn is_object_definition_known(
    conn: &Connection,
    type_id: &str,
    type_version: &str,
) -> Result<bool, String> {
    ark_store::object_types::is_definition_known(conn, type_id, type_version)
}

pub fn list_object_types(conn: &Connection) -> Result<Vec<ObjectType>, String> {
    ark_store::object_types::list(conn)
}

pub fn get_object_type(conn: &Connection, id: &str) -> Result<Option<ObjectType>, String> {
    ark_store::object_types::get(conn, id)
}

fn is_canonical_type(type_id: &str) -> bool {
    crate::canonical_types::definitions::canonical_type_registrations()
        .map(|registrations| {
            registrations
                .iter()
                .any(|registration| registration.type_id == type_id)
        })
        .unwrap_or(false)
}
