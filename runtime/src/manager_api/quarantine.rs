//! `manager.data.quarantine.clear` — очистка `<data_dir>/legacy-quarantine/`
//! по явному действию пользователя (KOS-302).
//!
//! Каталог — наследие 0.9.x: ни один writer в cortex его не создаёт. Мы его
//! только показываем отдельной строкой («Устаревшие данные») и удаляем по
//! кнопке «Очистить» за модалкой подтверждения — никогда молча.
//!
//! Границы удаления как у `db_backup::validated_backups_dir`: ровно этот
//! каталог, без glob'ов; symlink/junction вместо каталога — отказ; reparse
//! point внутри — отказ (не доверяем remove_dir_all резать чужие деревья).

use super::storage::directory_bytes;
use super::ManagerState;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const QUARANTINE_DIR_NAME: &str = "legacy-quarantine";

fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// The directory is only ever deleted if it is a *real* directory named
/// exactly `legacy-quarantine` directly inside the canonical data dir.
fn validated_quarantine_dir(data_dir: &Path) -> Result<Option<PathBuf>, String> {
    let canonical_data_dir = data_dir
        .canonicalize()
        .map_err(|e| format!("canonicalize data directory {data_dir:?}: {e}"))?;
    let dir = data_dir.join(QUARANTINE_DIR_NAME);
    let metadata = match std::fs::symlink_metadata(&dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("stat {dir:?}: {error}")),
    };
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(format!(
            "{QUARANTINE_DIR_NAME} must be a real directory: {dir:?}"
        ));
    }
    let canonical_dir = dir
        .canonicalize()
        .map_err(|e| format!("canonicalize {dir:?}: {e}"))?;
    if canonical_dir.parent() != Some(canonical_data_dir.as_path()) {
        return Err(format!("{dir:?} escapes data directory"));
    }
    Ok(Some(dir))
}

/// Refuse to delete a tree containing links — removing the link itself is
/// safe, but a junction sitting inside legacy-quarantine means something
/// placed it there on purpose; the user can inspect it first.
fn contains_reparse_point(dir: &Path) -> Result<bool, String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if is_reparse_point(&metadata) {
            return Ok(true);
        }
        if metadata.is_dir() && contains_reparse_point(&entry.path())? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Удалить `<data_dir>/legacy-quarantine`. Возвращает освобождённые байты;
/// отсутствующий каталог — Ok(0), не ошибка.
fn clear(data_dir: &Path) -> Result<u64, String> {
    let Some(dir) = validated_quarantine_dir(data_dir)? else {
        return Ok(0);
    };
    if contains_reparse_point(&dir)? {
        return Err(format!(
            "в {QUARANTINE_DIR_NAME} есть ссылки на другие каталоги — очистите её вручную"
        ));
    }
    let freed = directory_bytes(&dir);
    std::fs::remove_dir_all(&dir).map_err(|e| format!("remove {dir:?}: {e}"))?;
    Ok(freed)
}

impl ManagerState {
    /// Directory delete runs on a blocking thread — a large quarantine tree
    /// must not stall the request loop.
    pub async fn clear_legacy_quarantine(&self) -> Result<Value, String> {
        let data_dir = self.data_dir().to_path_buf();
        let freed = tokio::task::spawn_blocking(move || clear(&data_dir))
            .await
            .map_err(|e| e.to_string())??;
        Ok(json!({"freed_bytes": freed}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(dir: &Path, rel: &str, bytes: usize) -> PathBuf {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, vec![0_u8; bytes]).unwrap();
        path
    }

    #[test]
    fn clear_removes_exactly_the_quarantine_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(root, "legacy-quarantine/old.zip", 124);
        put(root, "legacy-quarantine/nested/blob.bin", 32);
        let outside = put(root, "ark.db", 10);
        put(root, "backups/keep.db", 5);

        let freed = clear(root).unwrap();
        assert_eq!(freed, 124 + 32);
        assert!(!root.join("legacy-quarantine").exists());
        assert!(outside.exists(), "nothing outside the dir may be touched");
        assert!(root.join("backups/keep.db").exists());
        // Second clear is a no-op, not an error — the UI can retry freely.
        assert_eq!(clear(root).unwrap(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn clear_refuses_a_junction_pointing_outside() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let real = dir.path().join("real-target");
        std::fs::create_dir_all(&real).unwrap();
        put(&real, "payload.bin", 7);
        crate::test_links::link_dir(&real, &root.join("legacy-quarantine")).expect("junction");

        assert!(clear(root).is_err(), "junction must not be deleted");
        // A link inside the tree is refused too.
        std::fs::remove_dir_all(root.join("legacy-quarantine")).unwrap();
        std::fs::create_dir_all(root.join("legacy-quarantine")).unwrap();
        crate::test_links::link_dir(&real, &root.join("legacy-quarantine/inner"))
            .expect("junction");
        assert!(clear(root).is_err());
        assert!(root.join("legacy-quarantine").exists());
        assert!(real.join("payload.bin").exists());
    }
}
