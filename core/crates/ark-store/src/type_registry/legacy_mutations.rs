use ark_domain::canonical_type::{
    definitions::canonical_type_registrations,
    registry::{self, LEGACY_VERSION},
};
use ark_domain::types::ObjectType;
use rusqlite::Connection;

use crate::pending_objects::replay_pending_for_type;

pub fn upsert_object_type(
    connection: &Connection,
    object_type: &ObjectType,
) -> Result<Vec<ark_application::sync_storage::ReplayNotice>, String> {
    if crate::object_types::legacy_alias_conflicts(connection, &object_type.id)? {
        return Err("alias collides with canonical type".into());
    }
    if is_canonical_type(&object_type.id)? {
        return Ok(Vec::new());
    }

    let (version, schema_hash) = registry::legacy_compatibility_version(
        &object_type.schema_json,
        &object_type.ui_schema_json,
    )?;
    let current_version = crate::object_types::upsert_legacy_type(
        connection,
        object_type,
        &crate::object_types::LegacyCompatibilityVersion {
            version,
            schema_hash,
        },
    )?;

    let mut replay_notices =
        replay_pending_for_type(connection, &object_type.id, &current_version)?;
    if current_version != LEGACY_VERSION {
        replay_notices.extend(replay_pending_for_type(
            connection,
            &object_type.id,
            LEGACY_VERSION,
        )?);
    }
    Ok(replay_notices)
}

pub fn delete_object_type(connection: &Connection, id: &str) -> Result<(), String> {
    if is_canonical_type(id)? {
        return Ok(());
    }
    crate::object_types::deprecate_legacy_type(connection, id)
}

fn is_canonical_type(type_id: &str) -> Result<bool, String> {
    Ok(canonical_type_registrations()?
        .iter()
        .any(|registration| registration.type_id == type_id))
}

#[cfg(test)]
mod tests {
    use super::{delete_object_type, upsert_object_type};
    use ark_domain::types::ObjectType;
    use rusqlite::Connection;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().expect("open database");
        crate::open::init_schema(&connection).expect("initialize schema");
        connection
    }

    fn custom_type() -> ObjectType {
        ObjectType {
            id: "custom.note".into(),
            name: "Note".into(),
            schema_json: "{}".into(),
            ui_schema_json: "{}".into(),
            created_at: "2026-10-20T00:00:00.000Z".into(),
            updated_at: "2026-10-20T00:00:00.000Z".into(),
            system_locked: false,
        }
    }

    #[test]
    fn legacy_type_upsert_and_delete_preserve_compatibility_state() {
        let connection = database();
        let object_type = custom_type();

        assert!(upsert_object_type(&connection, &object_type)
            .expect("legacy type upserts")
            .is_empty());
        let current_version: String = connection
            .query_row(
                "SELECT current_version FROM object_types WHERE id=?1",
                [&object_type.id],
                |row| row.get(0),
            )
            .expect("compatibility version persisted");
        let version_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM object_type_versions WHERE type_id=?1 AND version=?2)",
                [&object_type.id, &current_version],
                |row| row.get(0),
            )
            .expect("compatibility version can be read");
        assert!(version_exists);

        delete_object_type(&connection, &object_type.id).expect("legacy type deprecates");
        let status: String = connection
            .query_row(
                "SELECT status FROM object_types WHERE id=?1",
                [&object_type.id],
                |row| row.get(0),
            )
            .expect("deprecated type remains stored");
        assert_eq!(status, "deprecated");
    }
}
