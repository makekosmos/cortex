//! Retention policy for `<data_dir>/backups/` (KOS-300).
//!
//! Директорию пишут несколько writer'ов, и до KOS-300 ротация трогала только
//! `ark.db.backup-<ts>` — всё остальное копилось бесконечно:
//!
//! - `ark.db.backup-<ts>` — scheduler + `db_backups.create` (online backup
//!   через Core). Политика: держим `retain` новейших (MUNDUS_BACKUP_RETAIN_COUNT,
//!   default 7); SQLite sidecar'ы (`-shm`/`-wal`/`-journal`) удаляются вместе
//!   с главным файлом — иначе они оставались сиротами навсегда;
//! - `ark.db.pre-restore-failed-<nonce>.db` — Core сохраняет pre-restore
//!   snapshot при двойном провале restore. Политика: держим
//!   [`PRE_RESTORE_RETAIN`] новейших (по mtime — в имени нет timestamp);
//! - `.restore-rollback-*` / `.restore-src-*` — temp файлы атомарного restore
//!   в Core; на успехе удаляются самим restore, остаются только после crash'а
//!   процесса. Политика: удаляем старше [`RESTORE_TEMP_GRACE`], чтобы не
//!   задеть temp файлы restore, идущего прямо сейчас;
//! - `*-shm` / `*-wal` / `*-journal` — SQLite sidecar'ы backup-файлов (WAL
//!   mode хранится в заголовке БД и копируется в backup: любой process,
//!   открывший копию на запись, создаёт их заново). Политика: если главный
//!   файл удалён — sidecar сирота и удаляется.
//!
//! Cleanup работает строго внутри переданной `backups_dir` (caller валидирует
//! её через `validated_backups_dir`): каждый путь строится `join` от basename,
//! живой `ark.db` и файлы вне директории недосягаемы.
//!
//! Concurrency: вызывается под `BACKUP_LOCK` — параллельный backup
//! невозможен. In-progress backup защищён дважды: его главный файл уже
//! существует (значит его sidecar'ы не сироты), а его timestamp делает его
//! новейшей записью в ротации.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::parse_backup_timestamp;

/// Сколько `ark.db.pre-restore-failed-*` держим. Это последняя копия данных
/// перед критическим провалом restore — редкий артефакт, но без потолка он
/// тоже накапливался бы; трёх последних достаточно для ручного восстановления.
const PRE_RESTORE_RETAIN: usize = 3;
const PRE_RESTORE_PREFIX: &str = "ark.db.pre-restore-failed-";
/// Temp-файлы restore моложе этого возраста считаем принадлежащими живому
/// restore и не трогаем — Core не публикует «restore in progress» флаг,
/// а удаление `.restore-rollback-*` посреди restore сломало бы откат.
const RESTORE_TEMP_GRACE: Duration = Duration::from_secs(60 * 60);
const RESTORE_TEMP_PREFIXES: [&str; 2] = [".restore-rollback-", ".restore-src-"];
const SIDECAR_SUFFIXES: [&str; 3] = ["-shm", "-wal", "-journal"];

/// Статистика одного прохода — для логов и тестов.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RetentionReport {
    /// Удалённые `ark.db.backup-*` сверх retain.
    pub rotated_backups: usize,
    /// Удалённые sidecar'ы (вместе с ротацией + сироты).
    pub removed_sidecars: usize,
    /// Удалённые `ark.db.pre-restore-failed-*` сверх потолка.
    pub removed_pre_restore: usize,
    /// Удалённые stale temp файлы crashed restore.
    pub removed_restore_temps: usize,
}

impl RetentionReport {
    pub fn total_removed(&self) -> usize {
        self.rotated_backups
            + self.removed_sidecars
            + self.removed_pre_restore
            + self.removed_restore_temps
    }
}

fn remove_file_logged(path: &Path) -> bool {
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[db-backup] retention failed to remove {path:?}: {e}");
            false
        }
    }
}

/// Имя — sidecar известного нам артефакта (`ark.db.*` или restore temp),
/// чей главный файл отсутствует. Чужие `foo-wal` не считаются нашими сиротами.
fn is_orphaned_sidecar(name: &str, dir: &Path) -> Option<PathBuf> {
    let base = SIDECAR_SUFFIXES
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))?;
    let ours = parse_backup_timestamp(base).is_some()
        || base.starts_with(PRE_RESTORE_PREFIX)
        || RESTORE_TEMP_PREFIXES.iter().any(|p| base.starts_with(p));
    if ours && !dir.join(base).exists() {
        Some(dir.join(name))
    } else {
        None
    }
}

fn is_restore_temp(name: &str) -> bool {
    RESTORE_TEMP_PREFIXES.iter().any(|p| name.starts_with(p))
}

fn sidecar_paths<'a>(dir: &'a Path, base_name: &'a str) -> impl Iterator<Item = PathBuf> + 'a {
    SIDECAR_SUFFIXES
        .iter()
        .map(move |suffix| dir.join(format!("{base_name}{suffix}")))
}

/// Применить retention к `backups_dir`. Вызывается после каждого успешного
/// backup и один раз на старте Engine (из `maybe_backup_on_startup`).
/// Ошибки отдельных удалений логируются, но не прерывают проход — один
/// locked файл не должен блокировать остальную чистку.
pub fn enforce(backups_dir: &Path, retain: usize) -> Result<RetentionReport, String> {
    let mut report = RetentionReport::default();
    let entries: Vec<(String, PathBuf)> = std::fs::read_dir(backups_dir)
        .map_err(|e| format!("read_dir {backups_dir:?}: {e}"))?
        .flatten()
        .filter_map(|entry| {
            // Только regular files: nested dir или link никогда не удаляем.
            if !entry.file_type().is_ok_and(|ft| ft.is_file()) {
                return None;
            }
            Some((entry.file_name().to_str()?.to_string(), entry.path()))
        })
        .collect();

    // 1. Ротация `ark.db.backup-*`: N новейших + их sidecar'ы уходят вместе.
    let mut backups: Vec<(&String, &PathBuf, chrono::DateTime<chrono::Utc>)> = entries
        .iter()
        .filter_map(|(name, path)| Some((name, path, parse_backup_timestamp(name)?)))
        .collect();
    backups.sort_by_key(|entry| std::cmp::Reverse(entry.2));
    for (name, path, _) in backups.iter().skip(retain) {
        if remove_file_logged(path) {
            report.rotated_backups += 1;
        }
        for sidecar in sidecar_paths(backups_dir, name) {
            if sidecar.exists() && remove_file_logged(&sidecar) {
                report.removed_sidecars += 1;
            }
        }
    }

    // 2. Потолок на preserved pre-restore snapshots (по mtime — nonce
    //    в имени не сортируется по времени).
    let mut pre_restore: Vec<(&String, &PathBuf, SystemTime)> = entries
        .iter()
        .filter(|(name, _)| name.starts_with(PRE_RESTORE_PREFIX))
        .map(|(name, path)| {
            let mtime = path
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (name, path, mtime)
        })
        .collect();
    pre_restore.sort_by_key(|entry| std::cmp::Reverse(entry.2));
    for (name, path, _) in pre_restore.iter().skip(PRE_RESTORE_RETAIN) {
        if remove_file_logged(path) {
            report.removed_pre_restore += 1;
        }
        for sidecar in sidecar_paths(backups_dir, name) {
            if sidecar.exists() && remove_file_logged(&sidecar) {
                report.removed_sidecars += 1;
            }
        }
    }

    // 3. Sidecar-сироты: главный файл уже удалён (ротацией или руками).
    for (name, _) in &entries {
        if let Some(path) = is_orphaned_sidecar(name, backups_dir) {
            if remove_file_logged(&path) {
                report.removed_sidecars += 1;
            }
        }
    }

    // 4. Stale temp файлы crashed restore. Живой restore держит их считаные
    //    секунды — grace-окно защищает от гонки без IPC флага в Core.
    let cutoff = SystemTime::now() - RESTORE_TEMP_GRACE;
    for (name, path) in &entries {
        if !is_restore_temp(name) {
            continue;
        }
        let old_enough = path
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|mtime| mtime < cutoff);
        if old_enough && remove_file_logged(path) {
            report.removed_restore_temps += 1;
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_backup::backup_filename;
    use chrono::Utc;

    fn write(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"x").unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    fn backup_name(hours_ago: i64) -> String {
        backup_filename(Utc::now() - chrono::Duration::hours(hours_ago))
    }

    #[test]
    fn keeps_n_newest_backups() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..10 {
            write(dir.path(), &backup_name(i * 24));
        }
        let report = enforce(dir.path(), 7).unwrap();
        assert_eq!(report.rotated_backups, 3);
        assert_eq!(names(dir.path()).len(), 7);
    }

    #[test]
    fn rotated_backup_takes_its_sidecars_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let old = backup_name(10 * 24);
        let fresh = backup_name(0);
        write(dir.path(), &old);
        write(dir.path(), &format!("{old}-wal"));
        write(dir.path(), &format!("{old}-shm"));
        write(dir.path(), &fresh);

        enforce(dir.path(), 1).unwrap();

        assert_eq!(names(dir.path()), vec![fresh]);
    }

    #[test]
    fn removes_orphan_sidecars_whose_main_file_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let kept = backup_name(0);
        write(dir.path(), &kept);
        // Главный файл давно удалён — sidecar'ы висят сиротами.
        let gone = backup_name(500 * 24);
        write(dir.path(), &format!("{gone}-wal"));
        write(dir.path(), &format!("{gone}-shm"));
        write(dir.path(), &format!("{gone}-journal"));

        let report = enforce(dir.path(), 7).unwrap();

        assert_eq!(report.removed_sidecars, 3);
        assert_eq!(names(dir.path()), vec![kept]);
    }

    #[test]
    fn sidecars_of_live_backups_survive() {
        // Backup пишется прямо в финальное имя: пока главный файл есть,
        // его -wal/-shm не сироты и не трогаются.
        let dir = tempfile::tempdir().unwrap();
        let current = backup_name(0);
        write(dir.path(), &current);
        write(dir.path(), &format!("{current}-wal"));
        write(dir.path(), &format!("{current}-shm"));

        enforce(dir.path(), 7).unwrap();

        assert_eq!(names(dir.path()).len(), 3);
    }

    #[test]
    fn unrelated_files_and_foreign_sidecars_survive() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "README.md");
        write(dir.path(), "ark.db");
        write(dir.path(), "notes-wal"); // не наш артефакт — без base не удаляем
        write(dir.path(), &backup_name(0));

        enforce(dir.path(), 7).unwrap();

        assert_eq!(names(dir.path()).len(), 4);
    }

    #[test]
    fn pre_restore_snapshots_are_capped_but_not_empty() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            let p = dir.path().join(format!("{PRE_RESTORE_PREFIX}{i:016x}.db"));
            std::fs::write(&p, b"x").unwrap();
            // Делаем mtime различимым: i=0 — самый старый.
            std::fs::File::options()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(SystemTime::now() - Duration::from_secs(i * 100))
                .unwrap();
        }
        let report = enforce(dir.path(), 7).unwrap();
        assert_eq!(report.removed_pre_restore, 2);
        assert_eq!(names(dir.path()).len(), PRE_RESTORE_RETAIN);
    }

    #[test]
    fn stale_restore_temps_removed_young_ones_survive() {
        let dir = tempfile::tempdir().unwrap();
        let stale = dir.path().join(".restore-rollback-deadbeef.db");
        std::fs::write(&stale, b"x").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(2 * 60 * 60))
            .unwrap();
        write(dir.path(), ".restore-src-live.db"); // только что создан — живой restore
        write(dir.path(), &backup_name(0));

        let report = enforce(dir.path(), 7).unwrap();

        assert_eq!(report.removed_restore_temps, 1);
        assert!(dir.path().join(".restore-src-live.db").exists());
        assert!(!stale.exists());
    }

    #[test]
    fn never_touches_files_outside_backups_dir() {
        let root = tempfile::tempdir().unwrap();
        let backups = root.path().join("backups");
        std::fs::create_dir(&backups).unwrap();
        write(&backups, &format!("{}-wal", backup_name(500)));
        write(root.path(), "ark.db-wal"); // живой WAL рядом — вне нашей dir

        enforce(&backups, 7).unwrap();

        assert!(root.path().join("ark.db-wal").exists());
        assert!(names(&backups).is_empty());
    }
}
