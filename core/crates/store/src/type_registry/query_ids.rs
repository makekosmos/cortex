use rusqlite::{params, Connection};

pub fn stored_type_ids_for_query(conn: &Connection, type_id: &str) -> Result<Vec<String>, String> {
    let registry_ready: bool = conn
        .query_row(
            concat!(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND ",
                "name='object_type_aliases')"
            ),
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?
        != 0;
    if !registry_ready {
        return Ok(vec![type_id.to_owned()]);
    }

    let canonical = super::resolve_type_id(conn, type_id)?.unwrap_or_else(|| type_id.to_owned());
    let mut ids = vec![canonical.clone()];
    let mut statement = conn
        .prepare("SELECT alias FROM object_type_aliases WHERE canonical_type_id=?1 ORDER BY alias")
        .map_err(|error| error.to_string())?;
    let aliases = statement
        .query_map(params![canonical], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    ids.extend(aliases);
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_type_query_includes_sorted_stored_aliases() {
        let connection = Connection::open_in_memory().expect("open in-memory SQLite");
        connection
            .execute_batch(
                "CREATE TABLE object_types (id TEXT PRIMARY KEY);
                 CREATE TABLE object_type_aliases (alias TEXT, canonical_type_id TEXT);
                 INSERT INTO object_types (id) VALUES ('canonical');
                 INSERT INTO object_type_aliases VALUES
                    ('z_alias', 'canonical'), ('a_alias', 'canonical');",
            )
            .expect("create type alias fixture tables");

        let ids = stored_type_ids_for_query(&connection, "a_alias")
            .expect("resolve canonical type and aliases");

        assert_eq!(ids, ["canonical", "a_alias", "z_alias"]);
    }

    #[test]
    fn type_query_uses_literal_id_before_alias_schema_exists() {
        let connection = Connection::open_in_memory().expect("open in-memory SQLite");

        let ids = stored_type_ids_for_query(&connection, "legacy")
            .expect("resolve literal type without alias registry");

        assert_eq!(ids, ["legacy"]);
    }
}
