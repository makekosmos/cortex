use domain::canonical_type::registry::legacy_compatibility_version;
use domain::types::ObjectType;
use rusqlite::Connection;
use std::path::Path;

/// Seed a type the way the pre-canonical Engine did: any id, `{}` schema, and
/// the legacy compatibility version as `current_version`.
///
/// Tests need to write objects of an arbitrary type before a canonical type
/// exists. The RPC that used to create such a type is gone, so they seed the
/// row directly instead of reviving it.
pub fn seed_legacy_type(db_path: &Path, id: &str, name: &str) -> Result<(), String> {
    let connection = Connection::open(db_path).map_err(|error| error.to_string())?;
    seed_legacy_type_in(&connection, id, name)
}

pub fn seed_legacy_type_in(connection: &Connection, id: &str, name: &str) -> Result<(), String> {
    let (version, schema_hash) = legacy_compatibility_version("{}", "{}")?;
    crate::object_types::upsert_legacy_type(
        connection,
        &ObjectType {
            id: id.to_string(),
            name: name.to_string(),
            schema_json: "{}".to_string(),
            ui_schema_json: "{}".to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            system_locked: false,
        },
        &crate::object_types::LegacyCompatibilityVersion {
            version,
            schema_hash,
        },
    )
    .map(|_| ())
}
