use super::{FileIndexError, IndexedFile, Result};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const EXCLUDE_NOISY_FOLDERS_KEY: &str = "exclude_noisy_folders";
const RESPECT_GITIGNORE_KEY: &str = "respect_gitignore";
const INCLUDE_HIDDEN_KEY: &str = "include_hidden";
const NTFS_ACCELERATED_KEY: &str = "ntfs_accelerated";

pub struct FileStore {
    conn: Mutex<Connection>,
    db_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StatsSnapshot {
    pub total: usize,
    pub roots: Vec<String>,
    pub exclude_noisy_folders: bool,
    pub respect_gitignore: bool,
    pub include_hidden: bool,
    pub ntfs_accelerated: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DatabaseSizeSnapshot {
    pub db_size_bytes: u64,
    pub wal_size_bytes: u64,
    pub total_size_bytes: u64,
}

fn bool_setting_with_conn(conn: &Connection, key: &str, default: bool) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT value FROM file_index_settings WHERE key = ? LIMIT 1")?;
    let mut rows = stmt.query(params![key])?;
    Ok(match rows.next()? {
        Some(row) => row.get::<_, String>(0)? != "0",
        None => default,
    })
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
CREATE TABLE IF NOT EXISTS file_index_roots (
    path TEXT PRIMARY KEY,
    added_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS file_index_ignore_patterns (
    pattern TEXT PRIMARY KEY,
    added_at INTEGER NOT NULL
);
"#;

const FTS_SCHEMA: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS file_search_fts USING fts5(
    path,
    name,
    tokenize = 'trigram'
);
"#;

impl FileStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
            db_path: path.to_path_buf(),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    // Regression 2026-05-24-evening: read_conn() was deleted. Opening a fresh
    // SQLite connection per read costs ~ms on Windows; current_stats() did 7
    // back-to-back read_conn() calls, each with busy_timeout(5s). Under load
    // (chunked remove_tree holding the writer mutex with brief release windows)
    // these chained 5-second waits compounded into the 30s scope_remove timeout
    // visible in the UI. All reads now go through the single Mutex<Connection>;
    // chunked writes call thread::yield_now() between chunks so readers slot in
    // within microseconds, not seconds.

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

    pub fn clear_index_cache(&self) -> Result<()> {
        let conn = self.lock();
        conn.execute_batch(
            r#"
            PRAGMA wal_checkpoint(TRUNCATE);
            DELETE FROM files;
            DROP TABLE IF EXISTS file_search_fts;
            "#,
        )?;
        conn.execute_batch(FTS_SCHEMA)?;
        conn.execute_batch(
            r#"
            VACUUM;
            PRAGMA wal_checkpoint(TRUNCATE);
            "#,
        )?;
        Ok(())
    }

    pub fn checkpoint_truncate_wal(&self) -> Result<()> {
        let conn = self.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    pub fn database_size_snapshot(&self) -> Result<DatabaseSizeSnapshot> {
        let db_size_bytes = file_len_or_zero(&self.db_path)?;
        let wal_size_bytes = file_len_or_zero(&wal_path_for(&self.db_path))?;
        Ok(DatabaseSizeSnapshot {
            db_size_bytes,
            wal_size_bytes,
            total_size_bytes: db_size_bytes.saturating_add(wal_size_bytes),
        })
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
        // Regression L3 (2026-05-24): old impl held the writer mutex for the
        // entire DELETE (could be hundreds of thousands of rows on D:\). Settings
        // reads through Mutex<Connection> were blocked the whole time. Now we
        // delete in chunks of CHUNK rows, releasing the mutex between batches.
        const CHUNK: i64 = 5_000;
        let base = path.trim_end_matches(['\\', '/']).to_string();
        let backslash_tree = format!("{}\\%", escape_like(&base));
        let slash_tree = format!("{}/%", escape_like(&base));
        loop {
            let mut conn = self.lock();
            let tx = conn.transaction()?;
            // Use a subselect with LIMIT — SQLite does not support DELETE...LIMIT
            // unless built with SQLITE_ENABLE_UPDATE_DELETE_LIMIT.
            let deleted = tx.execute(
                "DELETE FROM files WHERE path IN (
                     SELECT path FROM files
                     WHERE path = ?
                        OR path LIKE ? ESCAPE '!'
                        OR path LIKE ? ESCAPE '!'
                     LIMIT ?
                 )",
                params![base, backslash_tree, slash_tree, CHUNK],
            )?;
            tx.execute(
                "DELETE FROM file_search_fts WHERE path IN (
                     SELECT path FROM file_search_fts
                     WHERE path = ?
                        OR path LIKE ? ESCAPE '!'
                        OR path LIKE ? ESCAPE '!'
                     LIMIT ?
                 )",
                params![base, backslash_tree, slash_tree, CHUNK],
            )?;
            tx.commit()?;
            drop(conn); // release writer mutex so readers/other writers can squeeze in
            if deleted == 0 {
                break;
            }
            if (deleted as i64) < CHUNK {
                // last chunk — orphaned fts rows already gone above
                break;
            }
            std::thread::yield_now();
        }
        Ok(())
    }

    pub fn remove_root_record(&self, path: &str) -> Result<String> {
        let normalized = normalize_root(path)?;
        let conn = self.lock();
        // Regression M4 (2026-05-24): case-insensitive match on Windows. Picker
        // may return "D:\Personal" while the row was stored as "d:\\personal".
        conn.execute(
            "DELETE FROM file_index_roots WHERE lower(path) = lower(?)",
            params![normalized],
        )?;
        Ok(normalized)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<IndexedFile>> {
        let q = query.trim().to_lowercase();
        if q.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        // См. postmortems.md § 2026-06-08: короткие substring queries
        // превращали hidden-launcher polling в повторяющийся LIKE scan.
        if q.chars().count() < 3 {
            return Ok(Vec::new());
        }
        self.search_fts(&q, limit)
    }

    // Regression 2026-05-24-evening: scope_remove was timing out at 30s because
    // current_stats() did 7 separate lock() acquisitions; each had to wait for
    // the background remove_tree chunk to yield. Snapshot reads ALL stats fields
    // under a single lock — one acquisition, in-and-out.
    pub fn stats_snapshot(&self) -> Result<StatsSnapshot> {
        let conn = self.lock();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        let mut roots = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT path FROM file_index_roots ORDER BY path ASC")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                roots.push(row?);
            }
        }
        let exclude_noisy_folders = bool_setting_with_conn(&conn, EXCLUDE_NOISY_FOLDERS_KEY, true)?;
        let respect_gitignore = bool_setting_with_conn(&conn, RESPECT_GITIGNORE_KEY, true)?;
        let include_hidden = bool_setting_with_conn(&conn, INCLUDE_HIDDEN_KEY, false)?;
        let ntfs_accelerated = bool_setting_with_conn(&conn, NTFS_ACCELERATED_KEY, false)?;
        Ok(StatsSnapshot {
            total: total.max(0) as usize,
            roots,
            exclude_noisy_folders,
            respect_gitignore,
            include_hidden,
            ntfs_accelerated,
        })
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

    pub fn exclude_noisy_folders(&self) -> Result<bool> {
        self.bool_setting(EXCLUDE_NOISY_FOLDERS_KEY, true)
    }

    pub fn set_exclude_noisy_folders(&self, exclude: bool) -> Result<()> {
        self.set_bool_setting(EXCLUDE_NOISY_FOLDERS_KEY, exclude)
    }

    pub fn respect_gitignore(&self) -> Result<bool> {
        self.bool_setting(RESPECT_GITIGNORE_KEY, true)
    }

    pub fn set_respect_gitignore(&self, respect: bool) -> Result<()> {
        self.set_bool_setting(RESPECT_GITIGNORE_KEY, respect)
    }

    pub fn include_hidden(&self) -> Result<bool> {
        self.bool_setting(INCLUDE_HIDDEN_KEY, false)
    }

    pub fn set_include_hidden(&self, include: bool) -> Result<()> {
        self.set_bool_setting(INCLUDE_HIDDEN_KEY, include)
    }

    pub fn ntfs_accelerated(&self) -> Result<bool> {
        self.bool_setting(NTFS_ACCELERATED_KEY, false)
    }

    pub fn set_ntfs_accelerated(&self, enabled: bool) -> Result<()> {
        self.set_bool_setting(NTFS_ACCELERATED_KEY, enabled)
    }

    fn bool_setting(&self, key: &str, default: bool) -> Result<bool> {
        let conn = self.lock();
        let mut stmt =
            conn.prepare("SELECT value FROM file_index_settings WHERE key = ? LIMIT 1")?;
        let mut rows = stmt.query(params![key])?;
        Ok(match rows.next()? {
            Some(row) => row.get::<_, String>(0)? != "0",
            None => default,
        })
    }

    fn set_bool_setting(&self, key: &str, enabled: bool) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO file_index_settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, if enabled { "1" } else { "0" }],
        )?;
        Ok(())
    }

    pub fn roots(&self) -> Result<Vec<String>> {
        let conn = self.lock();
        let mut stmt = conn.prepare("SELECT path FROM file_index_roots ORDER BY path ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        collect_strings(rows)
    }

    pub fn seed_roots_if_empty(&self, roots: &[std::path::PathBuf]) -> Result<()> {
        if !self.roots()?.is_empty() || roots.is_empty() {
            return Ok(());
        }
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().timestamp();
        for root in roots {
            let normalized = normalize_root(&root.to_string_lossy())?;
            if !root_exists_tx(&tx, &normalized)? {
                tx.execute(
                    "INSERT INTO file_index_roots (path, added_at) VALUES (?, ?)",
                    params![normalized, now],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn add_root(&self, path: &str) -> Result<()> {
        let normalized = normalize_root(path)?;
        // Regression M4 (2026-05-24): explicit case-insensitive dedup. SQLite
        // PRIMARY KEY is case-sensitive by default — "D:\\Personal" and
        // "d:\\personal" would coexist as two rows and double-watch the same tree.
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        if !root_exists_tx(&tx, &normalized)? {
            tx.execute(
                "INSERT INTO file_index_roots (path, added_at) VALUES (?, ?)",
                params![normalized, chrono::Utc::now().timestamp()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn ignore_patterns(&self) -> Result<Vec<String>> {
        let conn = self.lock();
        let mut stmt =
            conn.prepare("SELECT pattern FROM file_index_ignore_patterns ORDER BY pattern ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        collect_strings(rows)
    }

    pub fn add_ignore_pattern(&self, pattern: &str) -> Result<()> {
        let normalized = normalize_non_empty(pattern, "ignore pattern")?;
        // Regression M3 (2026-05-24): case-insensitive dedup. "*.TMP" and "*.tmp"
        // would coexist as two rows because SQLite PK is case-sensitive.
        let conn = self.lock();
        let existing: Option<String> = conn
            .query_row(
                "SELECT pattern FROM file_index_ignore_patterns WHERE lower(pattern) = lower(?) LIMIT 1",
                params![normalized],
                |row| row.get(0),
            )
            .ok();
        if existing.is_some() {
            return Err(FileIndexError::InvalidSetting(format!(
                "pattern уже есть: {normalized}"
            )));
        }
        conn.execute(
            "INSERT INTO file_index_ignore_patterns (pattern, added_at) VALUES (?, ?)",
            params![normalized, chrono::Utc::now().timestamp()],
        )?;
        Ok(())
    }

    pub fn remove_ignore_pattern(&self, pattern: &str) -> Result<()> {
        let normalized = normalize_non_empty(pattern, "ignore pattern")?;
        let conn = self.lock();
        conn.execute(
            "DELETE FROM file_index_ignore_patterns WHERE lower(pattern) = lower(?)",
            params![normalized],
        )?;
        Ok(())
    }
}

fn normalize_root(path: &str) -> Result<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(FileIndexError::InvalidSetting(
            "root must not be empty".to_string(),
        ));
    }
    // Strip trailing slashes (except drive root "D:\\" which must keep one).
    let no_trailing = trimmed.trim_end_matches(['\\', '/']);
    let canonical = if no_trailing.len() == 2 && no_trailing.as_bytes()[1] == b':' {
        format!("{no_trailing}\\")
    } else {
        no_trailing.to_string()
    };
    Ok(canonical)
}

fn root_exists_tx(tx: &rusqlite::Transaction<'_>, path: &str) -> Result<bool> {
    let found: Option<String> = tx
        .query_row(
            "SELECT path FROM file_index_roots WHERE lower(path) = lower(?) LIMIT 1",
            params![path],
            |row| row.get(0),
        )
        .ok();
    Ok(found.is_some())
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

fn collect_strings(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<String>>,
) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn normalize_non_empty(value: &str, label: &str) -> Result<String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(FileIndexError::InvalidSetting(format!(
            "{label} must not be empty"
        )));
    }
    Ok(normalized)
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

fn wal_path_for(db_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", db_path.to_string_lossy()))
}

fn file_len_or_zero(path: &Path) -> Result<u64> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(meta.len()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(err) => Err(err.into()),
    }
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
    fn short_queries_do_not_scan_files_table() {
        // Regression: 2026-06-08. Launcher polling with "a"/"do" hit the
        // substring LIKE path forever after the window was hidden.
        let data = tempdir().unwrap();
        let store = FileStore::open(&data.path().join("files.db")).unwrap();
        store
            .upsert(&IndexedFile {
                path: r"C:\docs\alpha.txt".to_string(),
                name: "alpha.txt".to_string(),
                mtime: 1,
            })
            .unwrap();

        assert!(store.search("a", 10).unwrap().is_empty());
        assert!(store.search("al", 10).unwrap().is_empty());
        assert_eq!(store.search("alp", 10).unwrap().len(), 1);
    }

    #[test]
    fn roots_and_ignore_patterns_are_persisted_and_deduped() {
        let data = tempdir().unwrap();
        let db = data.path().join("files.db");
        let store = FileStore::open(&db).unwrap();

        store.add_root(r"D:\Personal").unwrap();
        store.add_root(r"D:\Personal").unwrap();
        store.add_ignore_pattern("*.tmp").unwrap();
        // Second insert is a duplicate (case-insensitive) and must error, not
        // silently swallow — UI shows the message.
        assert!(store.add_ignore_pattern("*.tmp").is_err());

        let reopened = FileStore::open(&db).unwrap();
        assert_eq!(reopened.roots().unwrap(), vec![r"D:\Personal".to_string()]);
        assert_eq!(
            reopened.ignore_patterns().unwrap(),
            vec!["*.tmp".to_string()]
        );
    }

    #[test]
    fn roots_dedup_is_case_and_trailing_slash_insensitive() {
        // Regression M4 (2026-05-24): "D:\\Personal", "d:\\personal\\" and
        // "D:\\Personal\\" used to insert as three distinct rows because
        // SQLite PRIMARY KEY is case-sensitive — watcher then double-watched.
        let data = tempdir().unwrap();
        let store = FileStore::open(&data.path().join("files.db")).unwrap();
        store.add_root(r"D:\Personal").unwrap();
        store.add_root(r"d:\personal\").unwrap();
        store.add_root(r"D:\Personal\").unwrap();
        assert_eq!(store.roots().unwrap().len(), 1);
        // remove_root must match regardless of casing/trailing-slash.
        store.remove_root_record(r"d:\PERSONAL").unwrap();
        assert!(store.roots().unwrap().is_empty());
    }

    #[test]
    fn ignore_pattern_dedup_is_case_insensitive() {
        // Regression M3 (2026-05-24): "*.TMP" and "*.tmp" used to coexist —
        // double-matching slows scans and bloats settings.
        let data = tempdir().unwrap();
        let store = FileStore::open(&data.path().join("files.db")).unwrap();
        store.add_ignore_pattern("*.tmp").unwrap();
        let err = store.add_ignore_pattern("*.TMP").unwrap_err();
        assert!(matches!(err, FileIndexError::InvalidSetting(_)));
        assert_eq!(store.ignore_patterns().unwrap().len(), 1);
    }

    #[test]
    fn drive_root_normalization_keeps_trailing_backslash() {
        let data = tempdir().unwrap();
        let store = FileStore::open(&data.path().join("files.db")).unwrap();
        // Picker can return "D:" — must be stored as "D:\\" so watcher gets a
        // real directory path.
        store.add_root("D:").unwrap();
        assert_eq!(store.roots().unwrap(), vec![r"D:\".to_string()]);
    }

    #[test]
    fn new_file_search_settings_have_safe_defaults() {
        let data = tempdir().unwrap();
        let store = FileStore::open(&data.path().join("files.db")).unwrap();

        assert!(store.respect_gitignore().unwrap());
        assert!(!store.include_hidden().unwrap());
        assert!(!store.ntfs_accelerated().unwrap());
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

    #[test]
    fn checkpoint_truncate_wal_clears_wal_bytes() {
        let data = tempdir().unwrap();
        let db = data.path().join("files.db");
        let store = FileStore::open(&db).unwrap();
        for n in 0..4_000 {
            store
                .upsert(&IndexedFile {
                    path: format!(r"C:\docs\checkpoint-{n}.txt"),
                    name: format!("checkpoint-{n}.txt"),
                    mtime: n,
                })
                .unwrap();
        }

        store.checkpoint_truncate_wal().unwrap();

        let sizes = store.database_size_snapshot().unwrap();
        assert_eq!(sizes.wal_size_bytes, 0);
        assert!(sizes.db_size_bytes > 0);
        assert_eq!(sizes.total_size_bytes, sizes.db_size_bytes);
    }
}
