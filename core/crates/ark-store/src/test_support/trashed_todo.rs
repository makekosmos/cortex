use std::path::Path;

use rusqlite::params;

/// Owns legacy todo setup and inspection for the Core delete-trashed command test.
pub struct TrashedTodoFixture;

impl TrashedTodoFixture {
    pub fn insert(path: &Path, id: &str) -> Result<(), String> {
        let path = database_path(path)?;
        let connection = crate::open::open_db(path)?;
        connection
            .execute(
                "INSERT INTO todos (id, title, created_at, is_trashed)
                 VALUES (?1, 'Trashed', '2026-01-01T00:00:00Z', 1)",
                [id],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn exists(path: &Path, id: &str) -> Result<bool, String> {
        let path = database_path(path)?;
        let connection = crate::open::open_db(path)?;
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM todos WHERE id = ?1)",
                params![id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())
    }
}

fn database_path(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "todo fixture database path is not UTF-8".to_string())
}
