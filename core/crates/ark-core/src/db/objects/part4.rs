
pub fn upsert_object_link(conn: &Connection, link: &ObjectLink) -> Result<(), String> {
    // Do not use SQLite REPLACE here: it deletes the old row first.
    // РЎРј. postmortems.md В§ 2026-06-04.
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
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_object_link(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM object_links WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_object_links(conn: &Connection) -> Result<Vec<ObjectLink>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, source_object_id, target_object_id, link_type, created_at
             FROM object_links
             ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
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
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

