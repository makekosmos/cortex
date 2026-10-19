use ark_domain::hlc::HLC;
use ark_domain::types::VersionVector;
use rusqlite::{Connection, OptionalExtension};

pub fn object_revision(connection: &Connection, entity_id: &str) -> Result<Option<String>, String> {
    let vector_revision = crate::sync_kv::get_sync_kv(connection, crate::VERSION_VECTOR_KEY)?
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            serde_json::from_str::<VersionVector>(&value).map_err(|error| error.to_string())
        })
        .transpose()?
        .and_then(|vector| vector.get(entity_id).cloned());
    let object_revision = connection
        .query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id = ?1",
            [entity_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let tombstone_revision = connection
        .query_row(
            "SELECT hlc FROM sync_tombstones WHERE id = ?1 AND entity_type = 'object'",
            [entity_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok([vector_revision, object_revision, tombstone_revision]
        .into_iter()
        .flatten()
        .max_by(|left, right| HLC::compare(&HLC::from_string(left), &HLC::from_string(right))))
}

#[cfg(test)]
mod tests {
    use super::object_revision;
    use crate::sync_kv::set_sync_kv;
    use rusqlite::Connection;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(
                "CREATE TABLE sync_kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE object_sync_versions (
                    object_id TEXT PRIMARY KEY, hlc TEXT NOT NULL, deleted INTEGER NOT NULL
                 );
                 CREATE TABLE sync_tombstones (
                    id TEXT PRIMARY KEY, entity_type TEXT NOT NULL, hlc TEXT NOT NULL
                 );",
            )
            .expect("revision schema creates");
        connection
    }

    #[test]
    fn chooses_the_newest_persisted_object_revision() {
        let connection = connection();
        set_sync_kv(
            &connection,
            crate::VERSION_VECTOR_KEY,
            r#"{"task":"2026-10-17T00:00:00.000Z:000001:device-a"}"#,
        )
        .expect("version vector persists");
        connection
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 0)",
                ["task", "2026-10-17T00:00:01.000Z:000001:device-a"],
            )
            .expect("object revision persists");
        connection
            .execute(
                "INSERT INTO sync_tombstones(id, entity_type, hlc) VALUES (?1, 'object', ?2)",
                ["task", "2026-10-17T00:00:02.000Z:000001:device-a"],
            )
            .expect("tombstone revision persists");

        assert_eq!(
            object_revision(&connection, "task")
                .expect("revision loads")
                .as_deref(),
            Some("2026-10-17T00:00:02.000Z:000001:device-a")
        );
    }

    #[test]
    fn missing_revision_returns_none() {
        let connection = connection();

        assert_eq!(
            object_revision(&connection, "task").expect("revision loads"),
            None
        );
    }
}
