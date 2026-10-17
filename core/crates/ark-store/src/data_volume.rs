use ark_application::data_volume::{DataVolumeEstimate, DataVolumeQuery, DataVolumeReadPort};
use rusqlite::{params_from_iter, Connection};

pub struct SqliteDataVolumeReadSession<'connection> {
    connection: &'connection Connection,
}

impl<'connection> SqliteDataVolumeReadSession<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }
}

impl DataVolumeReadPort for SqliteDataVolumeReadSession<'_> {
    fn estimate_data_volume(
        &self,
        query: &DataVolumeQuery,
    ) -> Result<Vec<DataVolumeEstimate>, String> {
        query
            .scopes()
            .iter()
            .map(|scope| estimate_scope(self.connection, scope))
            .collect()
    }
}

fn estimate_scope(
    connection: &Connection,
    scope: &ark_application::data_volume::DataVolumeScope,
) -> Result<DataVolumeEstimate, String> {
    let type_ids = crate::type_registry::stored_type_ids_for_query(connection, scope.type_id())?;
    let mut values = type_ids
        .iter()
        .cloned()
        .map(rusqlite::types::Value::Text)
        .collect::<Vec<_>>();
    values.push(rusqlite::types::Value::Text(
        scope.type_version().to_owned(),
    ));
    let mut terms = vec![
        "length(CAST(id AS BLOB)) + length(CAST(type_id AS BLOB)) + \
         length(CAST(type_version AS BLOB)) + length(CAST(created_at AS BLOB)) + \
         length(CAST(updated_at AS BLOB)) + 58"
            .to_owned(),
    ];
    for field in scope.fields() {
        match field.as_str() {
            "title" => terms.push("length(CAST(title AS BLOB)) + 10".into()),
            "content" => terms.push("length(CAST(content_json AS BLOB)) + 17".into()),
            field => {
                let property = field.strip_prefix("props.").expect("validated field");
                let quoted_property =
                    serde_json::to_string(property).map_err(|error| error.to_string())?;
                let path = format!("$.{quoted_property}");
                values.push(rusqlite::types::Value::Text(path));
                let parameter = values.len();
                terms.push(format!(
                    "CASE WHEN json_type(props_json, ?{parameter}) IS NULL THEN 0 \
                     ELSE length(CAST(json_extract(props_json, ?{parameter}) AS BLOB)) + 16 END"
                ));
            }
        }
    }
    let placeholders = (1..=type_ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT COUNT(*), COALESCE(SUM({}), 0) FROM objects WHERE type_id IN ({placeholders}) \
         AND type_version = ?{} AND deleted_at IS NULL",
        terms.join(" + "),
        type_ids.len() + 1
    );
    let (object_count, estimated_json_bytes) = connection
        .query_row(&sql, params_from_iter(values.iter()), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|error| error.to_string())?;
    Ok(DataVolumeEstimate {
        type_id: scope.type_id().to_owned(),
        type_version: scope.type_version().to_owned(),
        object_count,
        estimated_json_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn connection(with_alias_registry: bool) -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory SQLite");
        connection
            .execute_batch(
                "CREATE TABLE objects (
                    id TEXT NOT NULL,
                    type_id TEXT NOT NULL,
                    type_version TEXT NOT NULL,
                    title TEXT NOT NULL,
                    content_json TEXT NOT NULL,
                    props_json TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    deleted_at TEXT
                );",
            )
            .expect("create data-volume fixture table");
        if with_alias_registry {
            connection
                .execute_batch(
                    "CREATE TABLE object_types (id TEXT PRIMARY KEY);
                     CREATE TABLE object_type_aliases (
                         alias TEXT PRIMARY KEY,
                         canonical_type_id TEXT NOT NULL
                     );
                     INSERT INTO object_types (id) VALUES ('estimate_type');
                     INSERT INTO object_type_aliases (alias, canonical_type_id)
                         VALUES ('legacy_estimate_type', 'estimate_type');",
                )
                .expect("create type alias fixture tables");
        }
        connection
    }

    fn scope(type_id: &str, type_version: &str, fields: &[&str]) -> DataVolumeQuery {
        DataVolumeQuery::new(vec![ark_application::data_volume::DataVolumeScope::new(
            type_id.into(),
            type_version.into(),
            fields.iter().map(|field| (*field).to_owned()).collect(),
        )
        .expect("valid data-volume scope")])
        .expect("valid data-volume query")
    }

    struct TestObject<'a> {
        id: &'a str,
        type_id: &'a str,
        type_version: &'a str,
        title: &'a str,
        content: &'a str,
        props: &'a str,
        deleted_at: Option<&'a str>,
    }

    fn insert_object(connection: &Connection, object: TestObject<'_>) {
        connection
            .execute(
                "INSERT INTO objects (
                    id, type_id, type_version, title, content_json, props_json,
                    created_at, updated_at, deleted_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '2026-01-01', '2026-01-02', ?7)",
                rusqlite::params![
                    object.id,
                    object.type_id,
                    object.type_version,
                    object.title,
                    object.content,
                    object.props,
                    object.deleted_at
                ],
            )
            .expect("insert data-volume fixture object");
    }

    #[test]
    fn estimate_counts_canonical_and_alias_rows_without_exposing_unselected_values() {
        let connection = connection(true);
        insert_object(
            &connection,
            TestObject {
                id: "estimate-1",
                type_id: "estimate_type",
                type_version: "1.0.0",
                title: "Visible title",
                content: r#"{"text":"private body intentionally much larger than visible fields"}"#,
                props: r#"{"allowed":"yes","secret":"private property"}"#,
                deleted_at: None,
            },
        );
        insert_object(
            &connection,
            TestObject {
                id: "estimate-2",
                type_id: "legacy_estimate_type",
                type_version: "1.0.0",
                title: "Legacy title",
                content: "{}",
                props: "{}",
                deleted_at: None,
            },
        );
        insert_object(
            &connection,
            TestObject {
                id: "estimate-gone",
                type_id: "estimate_type",
                type_version: "1.0.0",
                title: "Gone",
                content: "{}",
                props: "{}",
                deleted_at: Some("2026-01-03"),
            },
        );

        let query = DataVolumeQuery::new(vec![
            ark_application::data_volume::DataVolumeScope::new(
                "legacy_estimate_type".into(),
                "1.0.0".into(),
                vec!["title".into(), "props.allowed".into()],
            )
            .expect("valid data-volume scope"),
            ark_application::data_volume::DataVolumeScope::new(
                "legacy_estimate_type".into(),
                "2.0.0".into(),
                vec!["content".into()],
            )
            .expect("valid data-volume scope"),
        ])
        .expect("valid data-volume query");
        let session = SqliteDataVolumeReadSession::new(&connection);
        let estimate = session
            .estimate_data_volume(&query)
            .expect("estimate selected object fields");
        let preview_json = serde_json::to_string(&estimate).expect("serialize estimate response");

        assert!(!preview_json.contains("private"), "{preview_json}");
        assert!(!preview_json.contains("Visible title"), "{preview_json}");
        assert_eq!(estimate[0].object_count, 2);
        assert_eq!(estimate[0].type_id, "legacy_estimate_type");
        assert!(estimate[0].estimated_json_bytes < 300);
        assert_eq!(estimate[1].object_count, 0);
        assert_eq!(estimate[1].estimated_json_bytes, 0);
    }

    #[test]
    fn estimate_uses_literal_type_id_when_alias_registry_is_absent() {
        let connection = connection(false);
        insert_object(
            &connection,
            TestObject {
                id: "legacy-only",
                type_id: "estimate_type",
                type_version: "1.0.0",
                title: "Title",
                content: "{}",
                props: "{}",
                deleted_at: None,
            },
        );
        let estimate = SqliteDataVolumeReadSession::new(&connection)
            .estimate_data_volume(&scope("estimate_type", "1.0.0", &[]))
            .expect("estimate literal type before registry migration");

        assert_eq!(estimate[0].object_count, 1);
    }
}
