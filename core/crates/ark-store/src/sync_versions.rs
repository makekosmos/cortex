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

pub fn record_remote_object_deletion(
    connection: &Connection,
    entity: &ark_domain::types::SyncEntity,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO object_sync_versions(object_id, hlc, deleted)
             VALUES (?1, ?2, 1)
             ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = 1
             WHERE excluded.hlc > object_sync_versions.hlc",
            rusqlite::params![entity.id, entity.hlc],
        )
        .map_err(|error| error.to_string())?;
    crate::sync_tombstones::upsert(connection, entity)
}

pub fn write_object_sync_version(
    connection: &Connection,
    object_id: &str,
    hlc: &str,
    deleted: bool,
) -> Result<usize, String> {
    connection
        .execute(
            "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, ?3)
             ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = excluded.deleted
             WHERE excluded.hlc > object_sync_versions.hlc",
            rusqlite::params![object_id, hlc, deleted],
        )
        .map_err(|error| error.to_string())
}

impl LocalRevisionTarget {
    fn entity_type(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::ObjectLink => "object_link",
        }
    }
}

mod local_revision;
pub use local_revision::bump_sync_version_vector;

pub fn bump_local_revision(
    connection: &Connection,
    target: LocalRevisionTarget,
    entity_id: &str,
    device_id: &str,
) -> Result<String, String> {
    let entity_type = target.entity_type();
    bump_sync_version_vector(connection, entity_type, entity_id, device_id, false)
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
        bump_local_revision, bump_sync_version_vector, object_revision,
        record_remote_object_revision, write_object_sync_version, LocalRevisionTarget,
    };
    use crate::sync_kv::set_sync_kv;
    use ark_domain::types::VersionVector;
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

    #[test]
    fn object_sync_version_write_only_replaces_an_older_revision() {
        let connection = connection();
        connection
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 0)",
                ["item", "2026-10-18T00:00:00.000Z:000001:device-a"],
            )
            .expect("initial revision persists");

        assert_eq!(
            write_object_sync_version(
                &connection,
                "item",
                "2026-10-17T00:00:00.000Z:000001:device-b",
                true,
            )
            .expect("stale revision is ignored"),
            0
        );
        assert_eq!(
            write_object_sync_version(
                &connection,
                "item",
                "2026-10-19T00:00:00.000Z:000001:device-b",
                true,
            )
            .expect("new revision persists"),
            1
        );
        let stored: (String, bool) = connection
            .query_row(
                "SELECT hlc, deleted FROM object_sync_versions WHERE object_id = 'item'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("stored revision loads");
        assert_eq!(
            stored,
            ("2026-10-19T00:00:00.000Z:000001:device-b".to_owned(), true)
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

    #[test]
    fn usage_revision_updates_sequence_version_cursor_and_feed_together() {
        let connection = local_revision_connection();

        let revision =
            bump_sync_version_vector(&connection, "usage_day", "day-1", "device-a", false)
                .expect("usage revision persists");

        let sequence: (i64, String, String, String, i64) = connection
            .query_row(
                "SELECT seq, entity_type, entity_id, hlc, deleted
                 FROM usage_sync_log WHERE device_id = 'device-a'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("usage sequence reads");
        assert_eq!(
            sequence,
            (1, "usage_day".into(), "day-1".into(), revision.clone(), 0)
        );

        let usage_version: String = connection
            .query_row(
                "SELECT hlc FROM usage_sync_versions
                 WHERE entity_type = 'usage_day' AND entity_id = 'day-1'",
                [],
                |row| row.get(0),
            )
            .expect("usage revision reads");
        assert_eq!(usage_version, revision);
        let raw_vector = crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)
            .expect("version vector reads")
            .expect("version vector exists");
        let vector: VersionVector = serde_json::from_str(&raw_vector).expect("vector parses");
        assert_eq!(vector.get("@usage:device-a").map(String::as_str), Some("1"));
        assert!(!vector.contains_key("day-1"));
        let feed_rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM ark_committed_feed
                 WHERE entity_type = 'usage_day' AND entity_id = 'day-1'",
                [],
                |row| row.get(0),
            )
            .expect("usage feed row count reads");
        assert_eq!(feed_rows, 1);
    }
}
