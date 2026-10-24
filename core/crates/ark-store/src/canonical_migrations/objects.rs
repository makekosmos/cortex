use rusqlite::Connection;

pub fn ensure_object_state_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(concat!(
            "CREATE TABLE IF NOT EXISTS object_local_state (object_id TEXT NOT NULL,",
            "device_id TEXT NOT NULL,data_json TEXT NOT NULL DEFAULT '{}',updated_at ",
            "TEXT NOT NULL,PRIMARY KEY(object_id,device_id),FOREIGN KEY(object_id) ",
            "REFERENCES objects(id) ON DELETE CASCADE); CREATE TABLE IF NOT EXISTS ",
            "object_sync_versions (object_id TEXT PRIMARY KEY,hlc TEXT NOT NULL,deleted ",
            "INTEGER NOT NULL CHECK(deleted IN (0,1))); CREATE TABLE IF NOT EXISTS ",
            "object_migration_quarantine (object_id TEXT NOT NULL,contract_version TEXT ",
            "NOT NULL,source_type_id TEXT NOT NULL,fields_json TEXT NOT NULL DEFAULT ",
            "'{}',source_hash TEXT NOT NULL,updated_at TEXT NOT NULL,PRIMARY KEY(",
            "object_id,contract_version),FOREIGN KEY(object_id) REFERENCES objects(id) ",
            "ON DELETE CASCADE);"
        ))
        .map_err(|error| error.to_string())
}
