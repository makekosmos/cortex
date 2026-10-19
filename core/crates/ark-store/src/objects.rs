use ark_domain::types::{ArkObject, ArkObjectSummary};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde_json::{json, Value};

mod search;
mod search_text;

pub use ark_application::object_read::SearchResult;
pub use search::search_objects;

fn parse_json_or_default(raw: String) -> Value {
    serde_json::from_str(&raw).unwrap_or_else(|_| json!({}))
}

fn map_object_row(row: &Row<'_>) -> rusqlite::Result<ArkObject> {
    Ok(ArkObject {
        id: row.get(0)?,
        type_id: row.get(1)?,
        type_version: row.get(2)?,
        title: row.get(3)?,
        content_json: parse_json_or_default(row.get(4)?),
        props_json: parse_json_or_default(row.get(5)?),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
    })
}

fn map_object_summary_row(row: &Row<'_>) -> rusqlite::Result<ArkObjectSummary> {
    Ok(ArkObjectSummary {
        id: row.get(0)?,
        type_id: row.get(1)?,
        type_version: row.get(2)?,
        title: row.get(3)?,
        props_json: parse_json_or_default(row.get(4)?),
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        deleted_at: row.get(7)?,
    })
}

fn type_id_in_clause(ids: &[String]) -> String {
    let placeholders = (1..=ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("type_id IN ({placeholders})")
}

pub fn list_objects(conn: &Connection) -> Result<Vec<ArkObject>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, content_json, props_json, created_at,
             updated_at, deleted_at
             FROM objects
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], map_object_row)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn list_object_summaries(conn: &Connection) -> Result<Vec<ArkObjectSummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, props_json, created_at, updated_at,
             deleted_at FROM objects ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], map_object_summary_row)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn list_objects_by_type(conn: &Connection, type_id: &str) -> Result<Vec<ArkObject>, String> {
    let type_ids = crate::type_registry::stored_type_ids_for_query(conn, type_id)?;
    let sql = format!(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at,
         deleted_at FROM objects WHERE {} ORDER BY updated_at DESC, created_at DESC",
        type_id_in_clause(&type_ids)
    );
    let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(type_ids.iter()), map_object_row)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn list_object_summaries_by_type(
    conn: &Connection,
    type_id: &str,
) -> Result<Vec<ArkObjectSummary>, String> {
    let type_ids = crate::type_registry::stored_type_ids_for_query(conn, type_id)?;
    let sql = format!(
        "SELECT id, type_id, type_version, title, props_json, created_at, updated_at, deleted_at
         FROM objects WHERE {} ORDER BY updated_at DESC, created_at DESC",
        type_id_in_clause(&type_ids)
    );
    let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(type_ids.iter()), map_object_summary_row)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn list_running_time_entries(
    conn: &Connection,
    source_filter: Option<&str>,
) -> Result<Vec<ArkObject>, String> {
    let type_ids = crate::type_registry::stored_type_ids_for_query(conn, "time_entry_obj")?;
    let base_sql = format!(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at,
         deleted_at FROM objects WHERE {} AND deleted_at IS NULL
         AND json_extract(props_json, '$.endedAt') IS NULL",
        type_id_in_clause(&type_ids)
    );
    let order = " ORDER BY json_extract(props_json, '$.startedAt') DESC";
    let mut values: Vec<&dyn rusqlite::ToSql> = type_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();
    let source_value;
    let sql = if let Some(source) = source_filter {
        source_value = source;
        values.push(&source_value);
        format!(
            "{base_sql} AND json_extract(props_json, '$.source') = ?{}{order}",
            type_ids.len() + 1
        )
    } else {
        format!("{base_sql}{order}")
    };
    let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(values.as_slice(), map_object_row)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn get_objects_by_ids(conn: &Connection, ids: &[String]) -> Result<Vec<ArkObject>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = (1..=ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at,
         deleted_at FROM objects WHERE id IN ({placeholders})
         ORDER BY updated_at DESC, created_at DESC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(ids.iter()), map_object_row)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn get_object(conn: &Connection, id: &str) -> Result<Option<ArkObject>, String> {
    conn.query_row(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at,
         deleted_at FROM objects WHERE id = ?1",
        params![id],
        map_object_row,
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn ensure_object_search_fts(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(crate::schema::CREATE_OBJECT_SEARCH_FTS)
        .map_err(|error| error.to_string())
}

pub fn object_search_fts_exists(conn: &Connection) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master \
         WHERE type='table' AND name='object_search_fts')",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(|error| error.to_string())
}

pub fn rebuild_object_search_fts(conn: &Connection) -> Result<(), String> {
    if !object_search_fts_exists(conn)? {
        return Ok(());
    }
    conn.execute("DELETE FROM object_search_fts", [])
        .map_err(|error| error.to_string())?;
    for object in list_objects(conn)? {
        index_object_for_search(conn, &object)?;
    }
    Ok(())
}

pub fn index_object_for_search(conn: &Connection, object: &ArkObject) -> Result<(), String> {
    if !object_search_fts_exists(conn)? {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM object_search_fts WHERE object_id = ?1",
        params![object.id],
    )
    .map_err(|error| error.to_string())?;
    if object.deleted_at.is_some() {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO object_search_fts (object_id, title, body, props) VALUES (?1, ?2, ?3, ?4)",
        params![
            object.id,
            object.title,
            search_text::extract_plain_text_from_value(&object.content_json),
            search_text::extract_plain_text_from_value(&object.props_json),
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn remove_object_from_search(conn: &Connection, id: &str) -> Result<(), String> {
    if !object_search_fts_exists(conn)? {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM object_search_fts WHERE object_id = ?1",
        params![id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("schema creates");
        crate::type_registry::migrate_schema(&connection).expect("type schema migrates");
        connection
            .execute(
                concat!(
                    "INSERT INTO object_types ",
                    "(id,name,schema_json,ui_schema_json,created_at,updated_at) ",
                    "VALUES ('note','Note','{}','{}','2026-01-01','2026-01-01'), ",
                    "('book','Book','{}','{}','2026-01-01','2026-01-01')"
                ),
                [],
            )
            .expect("object types insert");
        connection
            .execute(
                concat!(
                    "INSERT INTO objects ",
                    "(id,type_id,type_version,title,content_json,props_json,",
                    "created_at,updated_at,deleted_at) ",
                    "VALUES ('first','note','1.0.0','First','{}','{\"rank\":1}',",
                    "'2026-01-01','2026-01-01',NULL), ",
                    "('second','note','1.0.0','Second','{}','{\"rank\":2}',",
                    "'2026-01-02','2026-01-02',NULL), ",
                    "('book','book','1.0.0','Book','{}','{}','2026-01-03','2026-01-03',NULL)"
                ),
                [],
            )
            .expect("objects insert");
        connection
    }

    #[test]
    fn object_read_queries_filter_order_and_project_summaries() {
        let connection = database();
        let objects = list_objects_by_type(&connection, "note").expect("type query succeeds");
        assert_eq!(
            objects
                .iter()
                .map(|object| object.id.as_str())
                .collect::<Vec<_>>(),
            ["second", "first"]
        );
        let summaries =
            list_object_summaries_by_type(&connection, "note").expect("summary query succeeds");
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].props_json, json!({"rank": 2}));
        assert_eq!(summaries[0].id, "second");
    }

    #[test]
    fn object_reads_by_id_and_running_time_filter_keep_empty_behavior() {
        let connection = database();
        let ids = vec!["first".to_string(), "book".to_string()];
        assert_eq!(
            get_objects_by_ids(&connection, &ids)
                .expect("IDs query succeeds")
                .len(),
            2
        );
        assert!(get_objects_by_ids(&connection, &[])
            .expect("empty ID query succeeds")
            .is_empty());
        assert_eq!(
            get_object(&connection, "first")
                .expect("object lookup succeeds")
                .expect("object exists")
                .title,
            "First"
        );
        assert!(list_running_time_entries(&connection, None)
            .expect("running time query succeeds")
            .is_empty());
        assert!(list_running_time_entries(&connection, Some("manual"))
            .expect("source-filtered running time query succeeds")
            .is_empty());
    }

    #[test]
    fn object_search_supports_fallback_fts_and_index_removal() {
        let connection = database();
        let fallback = search_objects(&connection, "First").expect("fallback search succeeds");
        assert_eq!(fallback[0].entry_id, "first");

        ensure_object_search_fts(&connection).expect("FTS schema creates");
        let first = get_object(&connection, "first")
            .expect("object lookup succeeds")
            .expect("object exists");
        index_object_for_search(&connection, &first).expect("object indexes");
        let indexed = search_objects(&connection, "First").expect("FTS search succeeds");
        assert_eq!(indexed[0].entry_id, "first");

        remove_object_from_search(&connection, "first").expect("index entry removes");
        assert!(search_objects(&connection, "First")
            .expect("search with no indexed match succeeds")
            .is_empty());
    }
}
