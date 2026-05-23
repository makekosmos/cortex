use super::{IndexedFile, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

const EXCLUDE_NOISY_FOLDERS_KEY: &str = "exclude_noisy_folders";

pub struct FileStore {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    path TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    mtime INTEGER NOT NULL,
    last_indexed_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS files_name_ci ON files(name COLLATE NOCASE);
CREATE VIRTUAL TABLE IF NOT EXISTS file_search_fts USING fts5(
    path,
    name,
    tokenize = 'trigram'
);
CREATE TABLE IF NOT EXISTS file_index_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

impl FileStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn replace_all(&self, files: &[IndexedFile]) -> Result<()> {
        // NTFS fast scan может вернуть один и тот же абсолютный path дважды
        // (junctions / symlinks / mount points видимые из нескольких roots).
        // Без дедупа первый дубликат валит транзакцию по `files.path PRIMARY KEY`
        // и весь rescan возвращает Err. См. postmortems.md § 2026-05-23.
        let mut seen: std::collections::HashSet<&str> =
            std::collections::HashSet::with_capacity(files.len());
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM files", [])?;
        tx.execute("DELETE FROM file_search_fts", [])?;
        let now = chrono::Utc::now().timestamp();
        {
            let mut file_stmt = tx.prepare(
                "INSERT INTO files (path, name, mtime, last_indexed_at) VALUES (?, ?, ?, ?)",
            )?;
            let mut fts_stmt =
                tx.prepare("INSERT INTO file_search_fts (path, name) VALUES (?, ?)")?;
            for file in files {
                if !seen.insert(file.path.as_str()) {
                    continue;
                }
                file_stmt.execute(params![file.path, file.name, file.mtime, now])?;
                fts_stmt.execute(params![file.path, file.name])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn upsert(&self, file: &IndexedFile) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO files (path, name, mtime, last_indexed_at) VALUES (?, ?, ?, ?)
             ON CONFLICT(path) DO UPDATE SET
                 name = excluded.name,
                 mtime = excluded.mtime,
                 last_indexed_at = excluded.last_indexed_at",
            params![
                file.path,
                file.name,
                file.mtime,
                chrono::Utc::now().timestamp()
            ],
        )?;
        tx.execute(
            "DELETE FROM file_search_fts WHERE path = ?",
            params![file.path],
        )?;
        tx.execute(
            "INSERT INTO file_search_fts (path, name) VALUES (?, ?)",
            params![file.path, file.name],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn remove_tree(&self, path: &str) -> Result<()> {
        let mut conn = self.lock();
        let base = path.trim_end_matches(['\\', '/']);
        let backslash_tree = format!("{}\\%", escape_like(base));
        let slash_tree = format!("{}/%", escape_like(base));
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM files
             WHERE path = ?
                OR path LIKE ? ESCAPE '!'
                OR path LIKE ? ESCAPE '!'",
            params![base, backslash_tree, slash_tree],
        )?;
        tx.execute(
            "DELETE FROM file_search_fts
             WHERE path = ?
                OR path LIKE ? ESCAPE '!'
                OR path LIKE ? ESCAPE '!'",
            params![base, backslash_tree, slash_tree],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<IndexedFile>> {
        let q = query.trim().to_lowercase();
        if q.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        if q.chars().count() >= 3 {
            return self.search_fts(&q, limit);
        }
        self.search_like(&q, limit)
    }

    fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<IndexedFile>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT path, name, 0 FROM file_search_fts
             WHERE file_search_fts MATCH ?
             LIMIT ?",
        )?;
        let rows = stmt.query_map(params![fts_query(query), limit as i64], |row| {
            Ok(IndexedFile {
                path: row.get(0)?,
                name: row.get(1)?,
                mtime: row.get(2)?,
            })
        })?;
        collect_files(rows)
    }

    fn search_like(&self, query: &str, limit: usize) -> Result<Vec<IndexedFile>> {
        let pattern = format!("%{}%", escape_like(query));
        let prefix_pattern = format!("{}%", escape_like(query));
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT path, name, mtime FROM files
             WHERE lower(name) LIKE ? ESCAPE '!' OR lower(path) LIKE ? ESCAPE '!'
             ORDER BY CASE
                 WHEN lower(name) LIKE ? ESCAPE '!' THEN 0
                 WHEN lower(name) LIKE ? ESCAPE '!' THEN 1
                 ELSE 2
             END,
             length(name) ASC,
             name COLLATE NOCASE ASC
             LIMIT ?",
        )?;
        let rows = stmt.query_map(
            params![pattern, pattern, prefix_pattern, pattern, limit as i64],
            |row| {
                Ok(IndexedFile {
                    path: row.get(0)?,
                    name: row.get(1)?,
                    mtime: row.get(2)?,
                })
            },
        )?;
        collect_files(rows)
    }

    pub fn exclude_noisy_folders(&self) -> Result<bool> {
        let conn = self.lock();
        let mut stmt =
            conn.prepare("SELECT value FROM file_index_settings WHERE key = ? LIMIT 1")?;
        let mut rows = stmt.query(params![EXCLUDE_NOISY_FOLDERS_KEY])?;
        Ok(match rows.next()? {
            Some(row) => row.get::<_, String>(0)? != "0",
            None => true,
        })
    }

    pub fn set_exclude_noisy_folders(&self, exclude: bool) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO file_index_settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![EXCLUDE_NOISY_FOLDERS_KEY, if exclude { "1" } else { "0" }],
        )?;
        Ok(())
    }
}

fn collect_files(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<IndexedFile>>,
) -> Result<Vec<IndexedFile>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn fts_query(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

fn escape_like(input: &str) -> String {
    input
        .replace('!', "!!")
        .replace('%', "!%")
        .replace('_', "!_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn remove_tree_keeps_paths_that_only_share_a_prefix() {
        let data = tempdir().unwrap();
        let store = FileStore::open(&data.path().join("files.db")).unwrap();
        for path in [r"C:\foo\note.txt", r"C:\foobar\note.txt"] {
            store
                .upsert(&IndexedFile {
                    path: path.to_string(),
                    name: "note.txt".to_string(),
                    mtime: 1,
                })
                .unwrap();
        }

        store.remove_tree(r"C:\foo").unwrap();

        assert_eq!(store.search("note", 10).unwrap().len(), 1);
        assert_eq!(
            store.search("note", 10).unwrap()[0].path,
            r"C:\foobar\note.txt"
        );
    }

    #[test]
    fn replace_all_dedupes_duplicate_paths() {
        // Regression: 2026-05-23. NTFS fast scan возвращал один и тот же path
        // дважды (junction / symlink / mount point), `INSERT` валил всю
        // транзакцию по `files.path PRIMARY KEY` → backend в зависе.
        let data = tempdir().unwrap();
        let store = FileStore::open(&data.path().join("files.db")).unwrap();
        let files = vec![
            IndexedFile {
                path: r"C:\junction\note.txt".to_string(),
                name: "note.txt".to_string(),
                mtime: 1,
            },
            IndexedFile {
                path: r"C:\junction\note.txt".to_string(),
                name: "note.txt".to_string(),
                mtime: 2,
            },
            IndexedFile {
                path: r"C:\real\note.txt".to_string(),
                name: "note.txt".to_string(),
                mtime: 3,
            },
        ];

        store
            .replace_all(&files)
            .expect("replace_all не должен падать на дубликатах");

        let results = store.search("note", 10).unwrap();
        assert_eq!(results.len(), 2, "должно остаться 2 уникальных path");
        let paths: std::collections::HashSet<_> = results.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(r"C:\junction\note.txt"));
        assert!(paths.contains(r"C:\real\note.txt"));
    }

    #[test]
    fn fts_search_matches_filename_and_path_substrings() {
        let data = tempdir().unwrap();
        let store = FileStore::open(&data.path().join("files.db")).unwrap();
        for (path, name) in [
            (r"C:\notes\roadmap-final.md", "roadmap-final.md"),
            (r"D:\reports\q4\invoice.txt", "invoice.txt"),
        ] {
            store
                .upsert(&IndexedFile {
                    path: path.to_string(),
                    name: name.to_string(),
                    mtime: 1,
                })
                .unwrap();
        }

        assert_eq!(
            store.search("map-f", 10).unwrap()[0].name,
            "roadmap-final.md"
        );
        assert_eq!(store.search("reports", 10).unwrap()[0].name, "invoice.txt");
    }
}
