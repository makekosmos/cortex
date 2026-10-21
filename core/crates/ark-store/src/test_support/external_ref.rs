use std::path::Path;

use rusqlite::OptionalExtension;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRefSnapshot {
    pub connector_id: String,
    pub account_id: String,
    pub external_type: String,
    pub external_id: String,
    pub object_id: String,
    pub revision: Option<String>,
    pub content_hash: Option<String>,
    pub conflict_state: String,
}

/// Seeds commit failure and reads persisted state for Core external-ref tests.
pub struct ExternalRefFixture;

impl ExternalRefFixture {
    pub fn reject_update_commit(path: &Path) -> Result<(), String> {
        let path = database_path(path)?;
        let connection = crate::open::open_db(path)?;
        connection
            .execute_batch(
                "CREATE TABLE tx_parent (id INTEGER PRIMARY KEY);
                 CREATE TABLE tx_child (
                     parent_id INTEGER REFERENCES tx_parent(id) DEFERRABLE INITIALLY DEFERRED
                 );
                 CREATE TRIGGER reject_external_ref_commit AFTER UPDATE ON external_refs
                 BEGIN INSERT INTO tx_child(parent_id) VALUES (404); END;",
            )
            .map_err(|error| error.to_string())
    }

    pub fn inspect(path: &Path) -> Result<Option<ExternalRefSnapshot>, String> {
        let path = database_path(path)?;
        let connection = crate::open::open_db(path)?;
        connection
            .query_row(
                "SELECT connector_id, account_id, external_type, external_id, object_id,
                        external_revision, content_hash, conflict_state
                 FROM external_refs
                 WHERE connector_id = 'drive' AND account_id = 'personal'
                   AND external_type = 'file' AND external_id = 'file-1'",
                [],
                |row| {
                    Ok(ExternalRefSnapshot {
                        connector_id: row.get(0)?,
                        account_id: row.get(1)?,
                        external_type: row.get(2)?,
                        external_id: row.get(3)?,
                        object_id: row.get(4)?,
                        revision: row.get(5)?,
                        content_hash: row.get(6)?,
                        conflict_state: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(|error| error.to_string())
    }
}

fn database_path(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "external-ref database path is not UTF-8".to_string())
}
