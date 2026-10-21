use ark_application::source_snapshot::{
    AreaRow, GenericObjectRow, HeadingRow, ObjectIdentityRow, ProjectRow, SourceSnapshot, TagRow,
    TodoRow, GENERIC_SOURCE_TYPE_IDS,
};
use rusqlite::{params_from_iter, Connection, Params, Row};

const GENERIC_OBJECTS_SQL: &str = concat!(
    "SELECT id,type_id,type_version,title,content_json,props_json,created_at,",
    "updated_at,deleted_at FROM objects WHERE type_id IN ",
    "(?1,?2,?3,?4,?5,?6,?7,?8,?9) ORDER BY type_id,id"
);
const TODOS_SQL: &str = concat!(
    "SELECT id,title,notes,priority,scheduled_date,deadline,reminder_date,",
    "is_today,is_evening,is_someday,is_completed,completed_at,is_cancelled,",
    "cancelled_at,is_trashed,sort_order,heading_id,project_id,area_id,tag_ids,",
    "checklist_items,recurrence_rule,created_at FROM todos ORDER BY id"
);
const PROJECTS_SQL: &str = concat!(
    "SELECT id,title,notes,status,scheduled_date,deadline,sort_order,color_tag,",
    "area_id,created_at FROM projects ORDER BY id"
);
const TAGS_SQL: &str = "SELECT id,title,color,created_at FROM tags ORDER BY id";
const AREAS_SQL: &str = "SELECT id,title,sort_order,created_at FROM areas ORDER BY sort_order,id";
const HEADINGS_SQL: &str =
    "SELECT id,title,sort_order,project_id FROM headings ORDER BY sort_order,id";
const SNAPSHOT_SAVEPOINT: &str = "ark_phase3_source_snapshot";

pub fn read_source_snapshot(connection: &Connection) -> Result<SourceSnapshot, String> {
    connection
        .execute_batch(&format!("SAVEPOINT {SNAPSHOT_SAVEPOINT}"))
        .map_err(|error| error.to_string())?;
    match read_source_snapshot_in_savepoint(connection) {
        Ok(snapshot) => {
            connection
                .execute_batch(&format!("RELEASE SAVEPOINT {SNAPSHOT_SAVEPOINT}"))
                .map_err(|error| error.to_string())?;
            Ok(snapshot)
        }
        Err(error) => {
            let _ = connection.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {SNAPSHOT_SAVEPOINT}; RELEASE SAVEPOINT {SNAPSHOT_SAVEPOINT}"
            ));
            Err(error)
        }
    }
}

fn read_source_snapshot_in_savepoint(connection: &Connection) -> Result<SourceSnapshot, String> {
    let object_identities = crate::objects::list_object_type_identities(connection)?
        .into_iter()
        .map(|(id, identity)| ObjectIdentityRow {
            id,
            type_id: identity.type_id,
            type_version: identity.type_version,
        })
        .collect();
    Ok(SourceSnapshot {
        object_identities,
        object_links: crate::objects::list_object_links(connection)?,
        objects: read_rows(
            connection,
            GENERIC_OBJECTS_SQL,
            params_from_iter(GENERIC_SOURCE_TYPE_IDS),
            generic_object_row,
        )?,
        todos: read_rows(connection, TODOS_SQL, [], todo_row)?,
        projects: read_rows(connection, PROJECTS_SQL, [], project_row)?,
        tags: read_rows(connection, TAGS_SQL, [], tag_row)?,
        areas: read_optional(connection, "areas", AREAS_SQL, area_row)?,
        headings: read_optional(connection, "headings", HEADINGS_SQL, heading_row)?,
    })
}

fn read_rows<T, P: Params>(
    connection: &Connection,
    sql: &str,
    params: P,
    map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, String> {
    let mut statement = connection.prepare(sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params, map)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn read_optional<T>(
    connection: &Connection,
    table: &str,
    sql: &str,
    map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Option<Vec<T>>, String> {
    let exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| error.to_string())?;
    if !exists {
        return Ok(None);
    }
    read_rows(connection, sql, [], map).map(Some)
}

fn generic_object_row(row: &Row<'_>) -> rusqlite::Result<GenericObjectRow> {
    Ok(GenericObjectRow {
        id: row.get(0)?,
        type_id: row.get(1)?,
        type_version: row.get(2)?,
        title: row.get(3)?,
        content_json: row.get(4)?,
        props_json: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
    })
}

fn todo_row(row: &Row<'_>) -> rusqlite::Result<TodoRow> {
    Ok(TodoRow {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        priority: row.get(3)?,
        scheduled_date: row.get(4)?,
        deadline: row.get(5)?,
        reminder_date: row.get(6)?,
        is_today: row.get(7)?,
        is_evening: row.get(8)?,
        is_someday: row.get(9)?,
        is_completed: row.get(10)?,
        completed_at: row.get(11)?,
        is_cancelled: row.get(12)?,
        cancelled_at: row.get(13)?,
        is_trashed: row.get(14)?,
        sort_order: row.get(15)?,
        heading_id: row.get(16)?,
        project_id: row.get(17)?,
        area_id: row.get(18)?,
        tag_ids: row.get(19)?,
        checklist_items: row.get(20)?,
        recurrence_rule: row.get(21)?,
        created_at: row.get(22)?,
    })
}

fn project_row(row: &Row<'_>) -> rusqlite::Result<ProjectRow> {
    Ok(ProjectRow {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        status: row.get(3)?,
        scheduled_date: row.get(4)?,
        deadline: row.get(5)?,
        sort_order: row.get(6)?,
        color_tag: row.get(7)?,
        area_id: row.get(8)?,
        created_at: row.get(9)?,
    })
}

fn tag_row(row: &Row<'_>) -> rusqlite::Result<TagRow> {
    Ok(TagRow {
        id: row.get(0)?,
        title: row.get(1)?,
        color: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn area_row(row: &Row<'_>) -> rusqlite::Result<AreaRow> {
    Ok(AreaRow {
        id: row.get(0)?,
        title: row.get(1)?,
        sort_order: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn heading_row(row: &Row<'_>) -> rusqlite::Result<HeadingRow> {
    Ok(HeadingRow {
        id: row.get(0)?,
        title: row.get(1)?,
        sort_order: row.get(2)?,
        project_id: row.get(3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_source_schema() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory database opens");
        connection
            .execute_batch(
                "CREATE TABLE objects(
                    id TEXT, type_id TEXT, type_version TEXT, title TEXT,
                    content_json TEXT, props_json TEXT, created_at TEXT,
                    updated_at TEXT, deleted_at TEXT
                );
                CREATE TABLE object_links(
                    id TEXT, source_object_id TEXT, target_object_id TEXT,
                    link_type TEXT, created_at TEXT
                );
                CREATE TABLE todos(
                    id TEXT, title TEXT, notes TEXT, priority INTEGER,
                    scheduled_date TEXT, deadline TEXT, reminder_date TEXT,
                    is_today INTEGER, is_evening INTEGER, is_someday INTEGER,
                    is_completed INTEGER, completed_at TEXT, is_cancelled INTEGER,
                    cancelled_at TEXT, is_trashed INTEGER, sort_order INTEGER,
                    heading_id TEXT, project_id TEXT, area_id TEXT, tag_ids TEXT,
                    checklist_items TEXT, recurrence_rule TEXT, created_at TEXT
                );
                CREATE TABLE projects(
                    id TEXT, title TEXT, notes TEXT, status TEXT,
                    scheduled_date TEXT, deadline TEXT, sort_order INTEGER,
                    color_tag TEXT, area_id TEXT, created_at TEXT
                );
                CREATE TABLE tags(id TEXT, title TEXT, color TEXT, created_at TEXT);",
            )
            .expect("required source tables create");
        connection
    }

    #[test]
    fn optional_tables_distinguish_absent_from_present_empty() {
        let connection = empty_source_schema();
        let snapshot = read_source_snapshot(&connection).expect("empty source schema reads");
        assert_eq!(snapshot.areas, None);
        assert_eq!(snapshot.headings, None);

        connection
            .execute_batch(
                "CREATE TABLE areas(id TEXT,title TEXT,sort_order INTEGER,created_at TEXT);
                CREATE TABLE headings(id TEXT,title TEXT,sort_order INTEGER,project_id TEXT);",
            )
            .expect("optional tables create");
        let snapshot = read_source_snapshot(&connection).expect("empty optional tables read");
        assert_eq!(snapshot.areas, Some(vec![]));
        assert_eq!(snapshot.headings, Some(vec![]));
    }

    #[test]
    fn generic_json_is_returned_as_exact_persisted_text() {
        let connection = empty_source_schema();
        let raw = r#"{ "z" : 1, "a": [ 2 ] }"#;
        connection
            .execute(
                "INSERT INTO objects VALUES('o','note_obj','v','title',?1,'{ }','c','u',NULL)",
                [raw],
            )
            .expect("object inserts");

        let snapshot = read_source_snapshot(&connection).expect("source snapshot reads");

        assert_eq!(snapshot.objects[0].content_json, raw);
        assert_eq!(snapshot.objects[0].props_json, "{ }");
    }

    #[test]
    fn generic_snapshot_excludes_non_legacy_object_types() {
        let connection = empty_source_schema();
        connection
            .execute_batch(
                "INSERT INTO objects VALUES
                    ('legacy','note_obj','0.0.0-legacy','legacy','{}','{}','c','u',NULL),
                    ('canonical','com.kosmos.note','1.0.0','canonical','{}','{}','c','u',NULL)",
            )
            .expect("source objects insert");

        let snapshot = read_source_snapshot(&connection).expect("source snapshot reads");

        assert_eq!(snapshot.objects.len(), 1);
        assert_eq!(snapshot.objects[0].id, "legacy");
    }

    #[test]
    fn snapshot_includes_all_object_identities_and_links_in_store_order() {
        let connection = empty_source_schema();
        connection
            .execute_batch(
                "INSERT INTO objects VALUES
                    ('legacy','note_obj','0.0.0-legacy','legacy','{}','{}','c','u',NULL),
                    ('canonical','com.kosmos.note','1.0.0','canonical','{}','{}','c','u',NULL);
                 INSERT INTO object_links VALUES
                    ('link-z','source','target-z','related','2026-01-02'),
                    ('link-a','source','target-a','related','2026-01-01');",
            )
            .expect("objects and links insert");

        let snapshot = read_source_snapshot(&connection).expect("source snapshot reads");

        assert_eq!(
            snapshot
                .object_identities
                .iter()
                .map(|row| (
                    row.id.as_str(),
                    row.type_id.as_str(),
                    row.type_version.as_str()
                ))
                .collect::<Vec<_>>(),
            [
                ("canonical", "com.kosmos.note", "1.0.0"),
                ("legacy", "note_obj", "0.0.0-legacy"),
            ]
        );
        assert_eq!(
            snapshot
                .object_links
                .iter()
                .map(|link| link.id.as_str())
                .collect::<Vec<_>>(),
            ["link-a", "link-z"]
        );
    }

    #[test]
    fn snapshot_read_preserves_a_callers_outer_savepoint() {
        let connection = empty_source_schema();
        connection
            .execute_batch("SAVEPOINT caller_transaction")
            .expect("caller savepoint begins");

        read_source_snapshot(&connection).expect("source snapshot reads");
        connection
            .execute(
                "INSERT INTO objects VALUES (
                    'pending','note_obj','legacy','title','{}','{}','c','u',NULL
                )",
                [],
            )
            .expect("caller writes after snapshot");
        connection
            .execute_batch(
                "ROLLBACK TO SAVEPOINT caller_transaction; RELEASE SAVEPOINT caller_transaction",
            )
            .expect("caller transaction rolls back");

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM objects", [], |row| row.get(0))
            .expect("object count reads");
        assert_eq!(count, 0);
    }
}
