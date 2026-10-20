use rusqlite::Connection;

/// Reconstructs the four legacy authorities that existed before Phase 3
/// stopped seeding them during fresh initialization.
pub fn seed_historical_legacy_authorities(conn: &Connection) {
    for (legacy_id, name) in [
        ("note_obj", "Заметка"),
        ("game_obj", "Игра"),
        ("time_entry_obj", "Запись времени"),
        ("tag_obj", "Тег"),
    ] {
        conn.execute(
            concat!(
                "INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,",
                "updated_at,system_locked,owner_kind,owner_id,current_version,status,",
                "base_type_id) VALUES (?1,?2,'{}','{}','1970-01-01T00:00:00.000Z',",
                "'1970-01-01T00:00:00.000Z',0,'core','com.kosmos.core','0.0.0-legacy',",
                "'active',NULL)"
            ),
            [legacy_id, name],
        )
        .unwrap();
        conn.execute(
            concat!(
                "INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,",
                "content_contract_json,relations_json,sync_policy_json,schema_hash,",
                "created_at) VALUES (?1,'0.0.0-legacy','{}','{}','{}','[]','{}',",
                "'historical-legacy-hash','1970-01-01T00:00:00.000Z')"
            ),
            [legacy_id],
        )
        .unwrap();
    }
}
