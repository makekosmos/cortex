use super::{FileIndexError, IndexedFile, Result};
use rusqlite::params;
use std::path::{Path, PathBuf};

pub(super) fn normalize_root(path: &str) -> Result<String> {
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

pub(super) fn root_exists_tx(tx: &rusqlite::Transaction<'_>, path: &str) -> Result<bool> {
    let found: Option<String> = tx
        .query_row(
            "SELECT path FROM file_index_roots WHERE lower(path) = lower(?) LIMIT 1",
            params![path],
            |row| row.get(0),
        )
        .ok();
    Ok(found.is_some())
}

pub(super) fn collect_files(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<IndexedFile>>,
) -> Result<Vec<IndexedFile>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub(super) fn collect_strings(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<String>>,
) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub(super) fn normalize_non_empty(value: &str, label: &str) -> Result<String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(FileIndexError::InvalidSetting(format!(
            "{label} must not be empty"
        )));
    }
    Ok(normalized)
}

pub(super) fn fts_query(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

pub(super) fn escape_like(input: &str) -> String {
    input
        .replace('!', "!!")
        .replace('%', "!%")
        .replace('_', "!_")
}

pub(super) fn wal_path_for(db_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", db_path.to_string_lossy()))
}

pub(super) fn file_len_or_zero(path: &Path) -> Result<u64> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(meta.len()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(err) => Err(err.into()),
    }
}
