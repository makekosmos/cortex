//! file-index.db growth policy (KOS-302).
//!
//! The index is a rebuildable local cache — losing a row is at worst a
//! rescan, so the policy can be strict:
//!
//! *   a `files` row is pruned when its path no longer exists on disk
//!     (`metadata` → `NotFound`) or when it sits under no configured root —
//!     a root removed without `remove_tree` finishing (crash, kill) would
//!     otherwise leave its whole subtree indexed forever;
//! *   `file_search_fts` rows whose `files` row is gone are pruned
//!     (orphans the chunked deletes above can leave behind);
//! *   `VACUUM` reclaims the freed pages, at most once per
//!     `VACUUM_MIN_INTERVAL_SECS` — the timestamp is persisted in
//!     `file_index_settings` so restarts don't re-vacuum.
//!
//! Everything runs in `PRUNE_CHUNK`-sized transactions with the writer
//! mutex released between chunks (same discipline as `remove_tree`), so
//! readers and the watcher keep slotting in. Call it off the hot path —
//! `FileIndex::run_maintenance` is blocking.

use super::store::FileStore;
use super::{FileIndex, Result};
use rusqlite::params;

/// VACUUM of a multi-GB index is minutes of disk churn — daily is plenty.
pub const VACUUM_MIN_INTERVAL_SECS: i64 = 24 * 60 * 60;
const LAST_VACUUM_KEY: &str = "last_vacuum_at";
const PRUNE_CHUNK: usize = 500;

#[derive(Debug, Default, Clone, Copy)]
pub struct MaintenanceReport {
    /// Rows whose path was not found on disk.
    pub removed_stale: usize,
    /// Rows under no configured root.
    pub removed_out_of_roots: usize,
    /// FTS rows with no matching `files` row.
    pub removed_orphan_fts: usize,
    pub vacuumed: bool,
}

/// Roots lowercased and normalized to end in a separator, so a cheap
/// `starts_with` decides "under this root". The drive root ("c:\") already
/// ends in a separator; anything else gets one appended.
fn normalized_roots(roots: &[String]) -> Vec<String> {
    roots
        .iter()
        .map(|root| {
            let lower = root.to_lowercase();
            if lower.ends_with(['\\', '/']) {
                lower
            } else {
                format!("{lower}\\")
            }
        })
        .collect()
}

fn path_in_roots(path_lower: &str, roots: &[String]) -> bool {
    roots
        .iter()
        .any(|root| path_lower.starts_with(root.as_str()))
}

impl FileStore {
    /// One full maintenance pass. Long-running and blocking by design —
    /// callers wrap it in `spawn_blocking`.
    pub(super) fn maintain(&self, now_unix: i64) -> Result<MaintenanceReport> {
        let mut report = MaintenanceReport::default();
        let roots = normalized_roots(&self.roots()?);
        self.prune_entries(&roots, &mut report)?;
        report.removed_orphan_fts = self.prune_orphan_fts()?;
        report.vacuumed = self.vacuum_if_due(now_unix, VACUUM_MIN_INTERVAL_SECS)?;
        Ok(report)
    }

    /// Pages through `files` and drops rows that fail the retention rules.
    /// Existence is checked outside the lock; the delete re-asserts
    /// `(path, mtime)` so a file recreated between the check and the write
    /// keeps its fresh row.
    fn prune_entries(&self, roots: &[String], report: &mut MaintenanceReport) -> Result<()> {
        let mut cursor = String::new();
        let mut pending: Vec<(String, i64, bool)> = Vec::with_capacity(PRUNE_CHUNK);
        loop {
            let page: Vec<(String, i64)> = {
                let conn = self.lock();
                let mut stmt = conn.prepare(
                    "SELECT path, mtime FROM files WHERE path > ? ORDER BY path LIMIT ?",
                )?;
                let rows = stmt.query_map(params![cursor, PRUNE_CHUNK as i64], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            if page.is_empty() {
                break;
            }
            cursor = page.last().expect("non-empty page").0.clone();
            for (path, mtime) in page {
                let lower = path.to_lowercase();
                let out_of_roots = !path_in_roots(&lower, roots);
                // Anything that is not plainly "not found" counts as
                // existing — ACCESS_DENIED and friends still occupy the path
                // and rescanning will fix a stale row, while a wrongly
                // deleted one would hide the file from search.
                let missing = !out_of_roots
                    && std::fs::metadata(&path)
                        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound);
                if out_of_roots || missing {
                    pending.push((path, mtime, out_of_roots));
                }
            }
            if pending.len() >= PRUNE_CHUNK {
                self.flush_pending(&mut pending, report)?;
            }
            std::thread::yield_now();
        }
        self.flush_pending(&mut pending, report)
    }

    fn flush_pending(
        &self,
        pending: &mut Vec<(String, i64, bool)>,
        report: &mut MaintenanceReport,
    ) -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        let batch = std::mem::take(pending);
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        for (path, mtime, out_of_roots) in &batch {
            let deleted = tx.execute(
                "DELETE FROM files WHERE path = ? AND mtime = ?",
                params![path, mtime],
            )?;
            if deleted == 0 {
                continue; // rewritten by a concurrent upsert — keep it
            }
            tx.execute("DELETE FROM file_search_fts WHERE path = ?", params![path])?;
            if *out_of_roots {
                report.removed_out_of_roots += 1;
            } else {
                report.removed_stale += 1;
            }
        }
        tx.commit()?;
        drop(conn);
        std::thread::yield_now();
        Ok(())
    }

    fn prune_orphan_fts(&self) -> Result<usize> {
        let mut total = 0;
        loop {
            let deleted = {
                let conn = self.lock();
                conn.execute(
                    "DELETE FROM file_search_fts WHERE path IN (
                         SELECT f.path FROM file_search_fts f
                         WHERE NOT EXISTS (SELECT 1 FROM files WHERE files.path = f.path)
                         LIMIT ?1
                     )",
                    params![PRUNE_CHUNK as i64],
                )?
            };
            total += deleted;
            if deleted < PRUNE_CHUNK {
                return Ok(total);
            }
            std::thread::yield_now();
        }
    }

    fn vacuum_if_due(&self, now_unix: i64, min_interval_secs: i64) -> Result<bool> {
        let conn = self.lock();
        let last = conn
            .query_row(
                "SELECT value FROM file_index_settings WHERE key = ?",
                params![LAST_VACUUM_KEY],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|value| value.parse::<i64>().ok());
        if let Some(last) = last {
            if now_unix.saturating_sub(last) < min_interval_secs {
                return Ok(false);
            }
        }
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
        conn.execute(
            "INSERT INTO file_index_settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![LAST_VACUUM_KEY, now_unix.to_string()],
        )?;
        Ok(true)
    }
}

impl FileIndex {
    /// Prune + bounded VACUUM per the module policy. Blocking file-system and
    /// SQLite work — run on `spawn_blocking`, never on the request loop.
    pub fn run_maintenance(&self) -> Result<MaintenanceReport> {
        self.store.maintain(chrono::Utc::now().timestamp())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_index::IndexedFile;
    use std::path::Path;
    use tempfile::tempdir;

    fn indexed(path: &Path) -> IndexedFile {
        IndexedFile {
            path: path.to_string_lossy().into_owned(),
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            mtime: 1,
        }
    }

    #[test]
    fn maintenance_prunes_missing_and_out_of_roots_and_vacuums_once() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        let kept = root.join("kept.txt");
        std::fs::write(&kept, b"x").unwrap();
        let ghost = root.join("ghost.txt"); // under root, absent on disk
        let foreign = dir.path().join("other").join("foreign.txt");
        std::fs::create_dir_all(foreign.parent().unwrap()).unwrap();
        std::fs::write(&foreign, b"y").unwrap(); // exists, but outside any root

        let store = FileStore::open(&dir.path().join("file-index.db")).unwrap();
        store.add_root(&root.to_string_lossy()).unwrap();
        for file in [&kept, &ghost, &foreign] {
            store.upsert(&indexed(file)).unwrap();
        }
        // An FTS orphan that survived a crashed remove.
        {
            let conn = store.lock();
            conn.execute(
                "INSERT INTO file_search_fts (path, name) VALUES ('orphan', 'orphan')",
                [],
            )
            .unwrap();
        }

        let report = store.maintain(1_000_000).unwrap();
        assert_eq!(report.removed_stale, 1);
        assert_eq!(report.removed_out_of_roots, 1);
        assert_eq!(report.removed_orphan_fts, 1);
        assert!(report.vacuumed);
        assert_eq!(store.stats_snapshot().unwrap().total, 1);
        assert_eq!(
            store.search("kept", 10).unwrap()[0].path,
            indexed(&kept).path
        );
        assert!(store.search("ghost", 10).unwrap().is_empty());
        assert!(store.search("foreign", 10).unwrap().is_empty());

        // The persisted vacuum stamp rate-limits the next pass.
        let report = store.maintain(1_000_000 + 60).unwrap();
        assert!(!report.vacuumed);
        let report = store
            .maintain(1_000_000 + VACUUM_MIN_INTERVAL_SECS + 1)
            .unwrap();
        assert!(report.vacuumed);
    }

    #[test]
    fn maintenance_rewrites_do_not_eat_fresh_rows() {
        // A file whose mtime changed between the metadata check and the
        // delete keeps its row: the delete is guarded by (path, mtime).
        let dir = tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        let store = FileStore::open(&dir.path().join("file-index.db")).unwrap();
        store.add_root(&root.to_string_lossy()).unwrap();
        let file = root.join("fresh.txt");
        store.upsert(&indexed(&file)).unwrap();
        std::fs::write(&file, b"z").unwrap();
        let mut fresh = indexed(&file);
        fresh.mtime = 2;
        store.upsert(&fresh).unwrap();

        let report = store.maintain(2_000_000).unwrap();
        assert_eq!(report.removed_stale, 0);
        assert_eq!(store.stats_snapshot().unwrap().total, 1);
    }
}
