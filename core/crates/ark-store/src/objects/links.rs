use ark_domain::types::ObjectLink;
use rusqlite::{params, Connection};

pub fn upsert_object_link(conn: &Connection, link: &ObjectLink) -> Result<(), String> {
    conn.execute(
        "INSERT INTO object_links
            (id, source_object_id, target_object_id, link_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            source_object_id = excluded.source_object_id,
            target_object_id = excluded.target_object_id,
            link_type = excluded.link_type,
            created_at = excluded.created_at",
        params![
            link.id,
            link.source_object_id,
            link.target_object_id,
            link.link_type,
            link.created_at,
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn delete_object_link(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM object_links WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn list_other_object_link_ids(
    conn: &Connection,
    source_id: &str,
    link_type: &str,
    keep_id: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id FROM object_links
             WHERE source_object_id = ?1 AND link_type = ?2 AND id <> ?3
             ORDER BY id",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![source_id, link_type, keep_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn list_object_links(conn: &Connection) -> Result<Vec<ObjectLink>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, source_object_id, target_object_id, link_type, created_at
             FROM object_links
             ORDER BY created_at ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ObjectLink {
                id: row.get(0)?,
                source_object_id: row.get(1)?,
                target_object_id: row.get(2)?,
                link_type: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn list_object_links_for_source(
    conn: &Connection,
    source_id: &str,
) -> Result<Vec<ObjectLink>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, source_object_id, target_object_id, link_type, created_at
             FROM object_links WHERE source_object_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![source_id], |row| {
            Ok(ObjectLink {
                id: row.get(0)?,
                source_object_id: row.get(1)?,
                target_object_id: row.get(2)?,
                link_type: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_queries_and_upsert_preserve_rows() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(
                "CREATE TABLE object_links (
                    id TEXT PRIMARY KEY,
                    source_object_id TEXT NOT NULL,
                    target_object_id TEXT NOT NULL,
                    link_type TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )",
            )
            .expect("link schema creates");
        let first = ObjectLink {
            id: "first-link".into(),
            source_object_id: "first".into(),
            target_object_id: "second".into(),
            link_type: "related".into(),
            created_at: "2026-01-04".into(),
        };
        upsert_object_link(&connection, &first).expect("link persists");
        let mut updated = first;
        updated.target_object_id = "third".into();
        updated.created_at = "2026-01-05".into();
        upsert_object_link(&connection, &updated).expect("link updates in place");

        assert_eq!(
            list_object_links(&connection).expect("links load"),
            [updated.clone()]
        );
        assert_eq!(
            list_object_links_for_source(&connection, "first").expect("source links load"),
            [updated.clone()]
        );
        assert!(list_object_links_for_source(&connection, "missing")
            .expect("missing source links load")
            .is_empty());
        assert!(
            list_other_object_link_ids(&connection, "first", "related", "first-link")
                .expect("other IDs load")
                .is_empty()
        );
        delete_object_link(&connection, "first-link").expect("link deletes");
        assert!(list_object_links(&connection)
            .expect("links load after delete")
            .is_empty());
    }
}
