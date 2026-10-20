pub fn upsert_object_type(
    conn: &Connection,
    object_type: &ObjectType,
) -> Result<Vec<ark_application::sync_storage::ReplayNotice>, String> {
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

    let mut replay_notices = ark_store::pending_objects::replay_pending_for_type(
        conn,
        &object_type.id,
        &current_version,
    )?;
    if current_version != type_registry::LEGACY_VERSION {
        replay_notices.extend(ark_store::pending_objects::replay_pending_for_type(
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

fn is_canonical_type(type_id: &str) -> bool {
    crate::canonical_types::definitions::canonical_type_registrations()
        .map(|registrations| {
            registrations
                .iter()
                .any(|registration| registration.type_id == type_id)
        })
        .unwrap_or(false)
}
