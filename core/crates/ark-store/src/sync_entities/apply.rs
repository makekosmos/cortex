use ark_domain::hlc::HLC;
use ark_domain::types::{ArkObject, ObjectLink};
use rusqlite::Connection;

/// Selects the revision source used by the existing inbound compatibility path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboundObjectRevision {
    /// Canonical object sync considers the object row, sync vector, and tombstone.
    Canonical,
    /// Legacy compatibility sync historically compares only object_sync_versions.
    LegacyCompatibility,
}

/// Applies one fresh object snapshot and its links within the caller's transaction.
///
/// A stale snapshot is a no-op. The object, links, revision, and tombstone update
/// are kept together so a caller transaction can roll back the complete change.
pub fn apply_inbound_object_snapshot(
    connection: &Connection,
    object: &ArkObject,
    links: &[ObjectLink],
    source_hlc: &str,
    revision: InboundObjectRevision,
) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
    let current = match revision {
        InboundObjectRevision::Canonical => {
            crate::sync_versions::object_revision(connection, &object.id)?
        }
        InboundObjectRevision::LegacyCompatibility => {
            crate::sync_versions::stored_object_revision(connection, &object.id)?
        }
    };
    if current
        .as_deref()
        .is_some_and(|current| HLC::compare_str(source_hlc, current) != std::cmp::Ordering::Greater)
    {
        return Ok(ark_application::sync_storage::InboundApplyOutcome::Stale);
    }

    crate::objects::upsert_object_compat(connection, object)?;
    for link in links {
        crate::objects::upsert_object_link(connection, link)?;
    }
    connection
        .execute(
            "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON \
             CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE \
             excluded.hlc > object_sync_versions.hlc",
            rusqlite::params![object.id, source_hlc],
        )
        .map_err(|error| error.to_string())?;
    crate::sync_tombstones::delete(connection, &object.id)?;
    Ok(ark_application::sync_storage::InboundApplyOutcome::Applied)
}

#[cfg(test)]
mod tests {
    use super::{apply_inbound_object_snapshot, InboundObjectRevision};
    use crate::schema::CREATE_TABLES;
    use ark_domain::types::{ArkObject, ObjectLink};
    use rusqlite::Connection;
    use serde_json::json;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(CREATE_TABLES)
            .expect("schema creates");
        connection
            .execute_batch(
                "CREATE TABLE object_sync_versions (
                    object_id TEXT PRIMARY KEY,
                    hlc TEXT NOT NULL,
                    deleted INTEGER NOT NULL DEFAULT 0
                );",
            )
            .expect("object sync versions table creates");
        connection
            .execute(
                concat!(
                    "INSERT INTO object_types(id, name, schema_json, ui_schema_json, ",
                    "created_at, updated_at)\n",
                    "VALUES ('note', 'Note', '{}', '{}', 'now', 'now')",
                ),
                [],
            )
            .expect("object type creates");
        for id in ["note-1", "note-2"] {
            crate::objects::upsert_object_compat(&connection, &object(id, "Original"))
                .expect("fixture object persists");
        }
        connection
    }

    fn object(id: &str, title: &str) -> ArkObject {
        ArkObject {
            id: id.into(),
            type_id: "note".into(),
            type_version: "1.0.0".into(),
            title: title.into(),
            content_json: json!({"type": "doc", "content": []}),
            props_json: json!({}),
            created_at: "created".into(),
            updated_at: "updated".into(),
            deleted_at: None,
        }
    }

    fn link() -> ObjectLink {
        ObjectLink {
            id: "link-1".into(),
            source_object_id: "note-1".into(),
            target_object_id: "note-2".into(),
            link_type: "related".into(),
            created_at: "created".into(),
        }
    }

    #[test]
    fn fresh_snapshot_atomically_persists_object_links_revision_and_clears_tombstone() {
        let connection = connection();
        crate::sync_tombstones::record(
            &connection,
            "object",
            "note-1",
            Some("note"),
            "2026-01-01T00:00:00.000Z:000001:peer-a",
        )
        .expect("tombstone records");
        let transaction = connection
            .unchecked_transaction()
            .expect("transaction begins");

        let outcome = apply_inbound_object_snapshot(
            &transaction,
            &object("note-1", "Updated"),
            &[link()],
            "2026-01-02T00:00:00.000Z:000001:peer-a",
            InboundObjectRevision::Canonical,
        )
        .expect("snapshot applies");
        assert_eq!(
            outcome,
            ark_application::sync_storage::InboundApplyOutcome::Applied
        );
        transaction.commit().expect("transaction commits");

        let title: String = connection
            .query_row("SELECT title FROM objects WHERE id='note-1'", [], |row| {
                row.get(0)
            })
            .expect("object reads");
        let revision: String = connection
            .query_row(
                "SELECT hlc FROM object_sync_versions WHERE object_id='note-1'",
                [],
                |row| row.get(0),
            )
            .expect("revision reads");
        let links: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM object_links WHERE id='link-1'",
                [],
                |row| row.get(0),
            )
            .expect("link count reads");
        let tombstones: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id='note-1'",
                [],
                |row| row.get(0),
            )
            .expect("tombstone count reads");
        assert_eq!(title, "Updated");
        assert_eq!(revision, "2026-01-02T00:00:00.000Z:000001:peer-a");
        assert_eq!(links, 1);
        assert_eq!(tombstones, 0);
    }

    #[test]
    fn stale_snapshot_does_not_mutate_object_or_links() {
        let connection = connection();
        crate::sync_versions::record_remote_object_revision(
            &connection,
            "note-1",
            "2026-01-02T00:00:00.000Z:000001:peer-a",
            false,
        )
        .expect("revision records");

        let outcome = apply_inbound_object_snapshot(
            &connection,
            &object("note-1", "Stale"),
            &[link()],
            "2026-01-01T00:00:00.000Z:000001:peer-a",
            InboundObjectRevision::Canonical,
        )
        .expect("stale snapshot is ignored");
        assert_eq!(
            outcome,
            ark_application::sync_storage::InboundApplyOutcome::Stale
        );
        let title: String = connection
            .query_row("SELECT title FROM objects WHERE id='note-1'", [], |row| {
                row.get(0)
            })
            .expect("object reads");
        let links: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM object_links WHERE id='link-1'",
                [],
                |row| row.get(0),
            )
            .expect("link count reads");
        assert_eq!(title, "Original");
        assert_eq!(links, 0);
    }
}
