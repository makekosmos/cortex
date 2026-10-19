use ark_domain::hlc::HLC;
use ark_domain::types::VersionVector;
use rusqlite::{Connection, OptionalExtension};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalRevisionTarget {
    Object,
    ObjectLink,
}

pub fn record_remote_object_revision(
    connection: &Connection,
    entity_id: &str,
    incoming_hlc: &str,
    deleted: bool,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO object_sync_versions(object_id, hlc, deleted)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc,
                 deleted = excluded.deleted
             WHERE excluded.hlc > object_sync_versions.hlc",
            rusqlite::params![entity_id, incoming_hlc, deleted],
        )
        .map_err(|error| error.to_string())?;

    let raw = crate::sync_kv::get_sync_kv(connection, crate::VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(value).map_err(|error| error.to_string()))
        .transpose()?
        .unwrap_or_default();
    if vector
        .get(entity_id)
        .is_none_or(|current| HLC::is_newer(incoming_hlc, current))
    {
        vector.insert(entity_id.to_owned(), incoming_hlc.to_owned());
        crate::sync_kv::set_sync_kv(
            connection,
            crate::VERSION_VECTOR_KEY,
            &serde_json::to_string(&vector).map_err(|error| error.to_string())?,
        )?;
        crate::committed_feed::append_committed_change(
            connection,
            "object",
            entity_id,
            incoming_hlc,
            false,
            Some(&HLC::from_string(incoming_hlc).device_id),
            None,
        )?;
    }
    if !deleted {
        connection
            .execute("DELETE FROM sync_tombstones WHERE id = ?1", [entity_id])
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

impl LocalRevisionTarget {
    fn entity_type(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::ObjectLink => "object_link",
        }
    }
}

pub fn bump_local_revision(
    connection: &Connection,
    target: LocalRevisionTarget,
    entity_id: &str,
    device_id: &str,
) -> Result<String, String> {
    let entity_type = target.entity_type();
    let raw = crate::sync_kv::get_sync_kv(connection, crate::VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(value).map_err(|error| error.to_string()))
        .transpose()?
        .unwrap_or_default();

    let mut previous = vector.get(entity_id).cloned();
    if target == LocalRevisionTarget::Object {
        if let Some(candidate) = object_revision(connection, entity_id)? {
            previous = Some(candidate);
        }
    }

    let now = HLC::now(device_id);
    let next = match previous {
        Some(existing) => {
            let previous = HLC::from_string(&existing);
            if HLC::compare(&now, &previous).is_gt() {
                now
            } else {
                if previous.counter == u64::MAX {
                    return Err("hlc_counter_overflow".to_string());
                }
                HLC::new(
                    previous.wall_time,
                    previous.counter + 1,
                    device_id.to_string(),
                )
            }
        }
        None => now,
    };
    let revision = next.to_string();
    vector.insert(entity_id.to_string(), revision.clone());
    crate::sync_kv::set_sync_kv(
        connection,
        crate::VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|error| error.to_string())?,
    )?;
    crate::committed_feed::append_committed_change(
        connection,
        entity_type,
        entity_id,
        &revision,
        false,
        Some(device_id),
        None,
    )?;

    if target == LocalRevisionTarget::Object {
        connection
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 0)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = 0
                 WHERE excluded.hlc > object_sync_versions.hlc",
                rusqlite::params![entity_id, revision],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute("DELETE FROM sync_tombstones WHERE id = ?1", [entity_id])
            .map_err(|error| error.to_string())?;
    }

    Ok(revision)
}

pub fn object_revision(connection: &Connection, entity_id: &str) -> Result<Option<String>, String> {
    let vector_revision = crate::sync_kv::get_sync_kv(connection, crate::VERSION_VECTOR_KEY)?
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            serde_json::from_str::<VersionVector>(&value).map_err(|error| error.to_string())
        })
        .transpose()?
        .and_then(|vector| vector.get(entity_id).cloned());
    let object_revision = stored_object_revision(connection, entity_id)?;
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

pub fn stored_object_revision(
    connection: &Connection,
    entity_id: &str,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id = ?1",
            [entity_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        bump_local_revision, object_revision, record_remote_object_revision, LocalRevisionTarget,
    };
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

    fn local_revision_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("application schema creates");
        crate::data_platform::ensure_schema(&connection).expect("data platform schema creates");
        connection
            .execute_batch(
                "CREATE TABLE object_sync_versions (
                    object_id TEXT PRIMARY KEY,
                    hlc TEXT NOT NULL,
                    deleted INTEGER NOT NULL DEFAULT 0
                );",
            )
            .expect("object revision schema creates");
        connection
    }

    #[test]
    fn local_object_revision_updates_feed_and_clears_tombstone() {
        let connection = local_revision_connection();
        set_sync_kv(
            &connection,
            crate::VERSION_VECTOR_KEY,
            r#"{"task":"2999-01-01T00:00:00.000Z:000001:device-a"}"#,
        )
        .expect("previous revision persists");
        connection
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 1)",
                ["task", "2999-01-01T00:00:00.000Z:000001:device-a"],
            )
            .expect("object revision persists");
        connection
            .execute(
                "INSERT INTO sync_tombstones(id, entity_type, hlc, deleted_at)
                 VALUES (?1, 'object', ?2, ?2)",
                ["task", "2999-01-01T00:00:00.000Z:000001:device-a"],
            )
            .expect("tombstone persists");

        let revision =
            bump_local_revision(&connection, LocalRevisionTarget::Object, "task", "device-b")
                .expect("local object revision persists");

        assert_eq!(revision, "2999-01-01T00:00:00.000Z:000002:device-b");
        assert_eq!(
            object_revision(&connection, "task").expect("object revision reads"),
            Some(revision)
        );
        let tombstones: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = 'task'",
                [],
                |row| row.get(0),
            )
            .expect("tombstone count reads");
        assert_eq!(tombstones, 0);
        let feed_rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM ark_committed_feed
                 WHERE entity_type = 'object' AND entity_id = 'task'",
                [],
                |row| row.get(0),
            )
            .expect("committed feed row count reads");
        assert_eq!(feed_rows, 1);
    }

    #[test]
    fn remote_object_revision_updates_vector_feed_and_clears_tombstone() {
        let connection = local_revision_connection();
        connection
            .execute(
                "INSERT INTO sync_tombstones(id, entity_type, hlc, deleted_at)
                 VALUES ('task', 'object', '2026-10-01T00:00:00.000Z:000001:old',
                     '2026-10-01T00:00:00.000Z:000001:old')",
                [],
            )
            .expect("old tombstone persists");

        record_remote_object_revision(
            &connection,
            "task",
            "2026-10-02T00:00:00.000Z:000001:remote",
            false,
        )
        .expect("remote revision persists");

        let object_revision: String = connection
            .query_row(
                "SELECT hlc FROM object_sync_versions WHERE object_id = 'task'",
                [],
                |row| row.get(0),
            )
            .expect("object revision reads");
        assert_eq!(object_revision, "2026-10-02T00:00:00.000Z:000001:remote");
        let raw_vector = crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)
            .expect("version vector reads")
            .expect("version vector exists");
        let vector: ark_domain::types::VersionVector =
            serde_json::from_str(&raw_vector).expect("vector decodes");
        assert_eq!(
            vector.get("task").map(String::as_str),
            Some("2026-10-02T00:00:00.000Z:000001:remote")
        );
        let feed_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM ark_committed_feed
                 WHERE entity_type = 'object' AND entity_id = 'task'",
                [],
                |row| row.get(0),
            )
            .expect("feed count reads");
        assert_eq!(feed_count, 1);
        let tombstone_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = 'task'",
                [],
                |row| row.get(0),
            )
            .expect("tombstone count reads");
        assert_eq!(tombstone_count, 0);
    }

    #[test]
    fn local_link_revision_updates_only_its_sync_vector_and_feed() {
        let connection = local_revision_connection();

        let revision = bump_local_revision(
            &connection,
            LocalRevisionTarget::ObjectLink,
            "link-1",
            "device-a",
        )
        .expect("local link revision persists");

        assert_eq!(
            object_revision(&connection, "link-1").expect("link revision reads"),
            Some(revision.clone())
        );
        let object_rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM object_sync_versions WHERE object_id = 'link-1'",
                [],
                |row| row.get(0),
            )
            .expect("object revision row count reads");
        assert_eq!(object_rows, 0);
        let feed_entity: (String, String, String) = connection
            .query_row(
                "SELECT entity_type, entity_id, revision FROM ark_committed_feed",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("committed feed row reads");
        assert_eq!(
            feed_entity,
            ("object_link".into(), "link-1".into(), revision)
        );
    }
}
