use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectTypeIdentity {
    pub type_id: String,
    pub type_version: String,
}

pub fn object_type_identity(
    connection: &Connection,
    object_id: &str,
) -> Result<Option<ObjectTypeIdentity>, String> {
    connection
        .query_row(
            "SELECT type_id, type_version FROM objects WHERE id = ?1",
            [object_id],
            |row| {
                Ok(ObjectTypeIdentity {
                    type_id: row.get(0)?,
                    type_version: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{object_type_identity, ObjectTypeIdentity};
    use rusqlite::Connection;

    #[test]
    fn reads_only_the_persisted_object_type_identity() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute(
                "CREATE TABLE objects (
                    id TEXT PRIMARY KEY, type_id TEXT NOT NULL, type_version TEXT NOT NULL
                )",
                [],
            )
            .expect("object identity schema creates");
        connection
            .execute(
                "INSERT INTO objects(id, type_id, type_version)
                 VALUES ('object-1', 'note', '2.0.0')",
                [],
            )
            .expect("object identity inserts");

        assert_eq!(
            object_type_identity(&connection, "object-1").expect("identity reads"),
            Some(ObjectTypeIdentity {
                type_id: "note".into(),
                type_version: "2.0.0".into(),
            })
        );
        assert_eq!(
            object_type_identity(&connection, "missing").expect("absent identity reads"),
            None
        );
    }
}
