use rusqlite::{params, Connection, OptionalExtension};

pub fn resolve_alias(conn: &Connection, alias: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT canonical_type_id FROM object_type_aliases WHERE alias=?1",
        params![alias],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn resolve_type_id(conn: &Connection, input: &str) -> Result<Option<String>, String> {
    if conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![input],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Ok(Some(input.to_string()));
    }
    resolve_alias(conn, input)
}
