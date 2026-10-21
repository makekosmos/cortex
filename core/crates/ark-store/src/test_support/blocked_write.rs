use rusqlite::{params, Connection};
use std::path::Path;

/// Creates the legacy database state that blocks normal startup because an
/// object refers to a missing related object.
pub fn create_blocked_write_legacy_fixture(path: &Path) -> Result<(), String> {
    let connection = Connection::open(path).map_err(|error| error.to_string())?;
    crate::open::init_schema_prerequisites(&connection)?;
    for (legacy_id, name) in [
        ("note_obj", "Заметка"),
        ("game_obj", "Игра"),
        ("time_entry_obj", "Запись времени"),
        ("tag_obj", "Тег"),
    ] {
        connection
            .execute(
                concat!(
                    "INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,",
                    "updated_at,system_locked,owner_kind,owner_id,current_version,status,",
                    "base_type_id) VALUES (?1,?2,'{}','{}','1970-01-01T00:00:00.000Z',",
                    "'1970-01-01T00:00:00.000Z',0,'core','com.kosmos.core','0.0.0-legacy',",
                    "'active',NULL)"
                ),
                params![legacy_id, name],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                concat!(
                    "INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,",
                    "content_contract_json,relations_json,sync_policy_json,schema_hash,",
                    "created_at) VALUES (?1,'0.0.0-legacy','{}','{}','{}','[]','{}',",
                    "'historical-legacy-hash','1970-01-01T00:00:00.000Z')"
                ),
                [legacy_id],
            )
            .map_err(|error| error.to_string())?;
    }
    connection
        .execute(
            concat!(
                "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
                "created_at,updated_at) VALUES('dangling-note','note_obj','0.0.0-legacy','n',",
                "'{}','{\"relatedNotes\":[\"missing-target\"]}',",
                "'2026-01-01T00:00:00.000Z','2026-01-01T00:00:00.000Z')"
            ),
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}
