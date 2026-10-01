//! Retention for `<engine_root>/versions/` (KOS-261).
//!
//! The installer (`install-engine.ps1`) extracts every new Engine into
//! `versions/<semver>/` and rewrites `current.json`; nothing ever removed the
//! old dirs, so each update leaked ~70 MB. This module owns the single
//! selection rule shared by both runners — the `prune-versions` CLI the
//! installer calls after switching `current.json`, and the startup pass in
//! `setup()` that cleans what existing users already accumulated:
//!
//!   * keep the version `current.json` names plus exactly one previous
//!     (the highest semver strictly below it) for a manual rollback;
//!   * versions newer than current are left alone — a newer install followed
//!     by a rollback must not delete the newer payload;
//!   * names that are not strict `X.Y.Z` are never touched;
//!   * the installer's own temp shape `<v>.<pid>.tmp` is removed only past
//!     [`LEFTOVER_GRACE`], same reasoning as KOS-301 — a live extract holds
//!     its temp for seconds, so the grace protects an in-flight install;
//!   * `.tombstone-*` dirs are this pass's own crash leftovers and are
//!     removed unconditionally.
//!
//! A version dir is never deleted outright: it is first renamed to a
//! `.tombstone-<v>-<n>` sibling. On Windows the rename fails while any file
//! inside is mapped or open (a still-shutting-down Engine, a lingering
//! `ark-core-rpc`), which is the in-use guard — no process-name matching.
//! Only a successfully renamed dir is then `remove_dir_all`'d. A failed
//! rename skips the dir until the next pass; a crash between rename and
//! removal leaves a stale tombstone the next pass removes.
//!
//! If `current.json` is missing, invalid, or points at a missing dir, the
//! pass deletes *nothing* — tombstones included.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use semver::Version;
use serde::Serialize;

use crate::brand;
use crate::data_dir::temp_sweep::LEFTOVER_GRACE;

/// Sibling name a victim dir is renamed to before removal.
const TOMBSTONE_PREFIX: &str = ".tombstone-";

/// Статистика одного прохода — для логов и тестов (style of
/// `db_backup::retention::RetentionReport`).
#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Report {
    /// Имя версии, на которую указывает `current.json`.
    pub current: String,
    /// Оставленная предыдущая версия (для отката), если была.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    /// Имена версий новее текущей — оставлены, залогированы.
    pub kept_newer: Vec<String>,
    /// Имена, которые не являются ни semver, ни нашими temp/tombstone.
    pub kept_unknown: Vec<String>,
    /// Temp-директории установщика моложе grace — живой install.
    pub kept_young_temps: usize,
    /// Удалённые версии.
    pub removed: Vec<String>,
    /// Освобождённые байты (best-effort сумма размеров файлов).
    pub removed_bytes: u64,
    /// Версии, пропущенные потому что rename упал (каталог занят).
    pub skipped: Vec<String>,
    /// Удалённые stale tombstone'ы crashed-прохода.
    pub removed_tombstones: usize,
    /// Удалённые stale temp-директории установщика.
    pub removed_temps: usize,
    /// Ошибки удаления после успешного rename (tombstone остаётся до
    /// следующего прохода) и прочие per-dir ошибки — залогированы.
    pub failed: usize,
}

impl Report {
    /// Проход ничего не удалил и не пропустил — стартовый лог можно опустить.
    pub fn is_idle(&self) -> bool {
        self.removed.is_empty()
            && self.skipped.is_empty()
            && self.failed == 0
            && self.removed_tombstones == 0
            && self.removed_temps == 0
    }
}

/// Строгая `X.Y.Z` версия — тот же shape, что `$semver` в
/// `install-engine.ps1` (`semver::Version` шире: принимает prerelease/build
/// суффиксы, которых installer никогда не пишет).
fn strict_version(name: &str) -> Option<Version> {
    let version = Version::parse(name).ok()?;
    (version.pre.is_empty() && version.build.is_empty()).then_some(version)
}

/// `<v>.<pid>.tmp` — temp-директория `install-engine.ps1`
/// (`versions/<version>.<pid>.tmp` между Expand-Archive и Move-Item).
fn is_install_temp(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".tmp") else {
        return false;
    };
    let Some((version, pid)) = stem.rsplit_once('.') else {
        return false;
    };
    !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()) && strict_version(version).is_some()
}

/// Лучшая оценка размера директории для отчёта об освобождённом месте.
/// Ошибки чтения и symlinks пропускаются — это только статистика.
fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            total += dir_size(&entry.path());
        } else if file_type.is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

/// Директория-кандидат внутри `versions/` с уже распознанным именем.
struct Entry {
    name: String,
    path: PathBuf,
}

/// Первое свободное имя `.tombstone-<name>-<n>` рядом с жертвой.
fn free_tombstone(versions_dir: &Path, name: &str) -> PathBuf {
    let mut n = 0u32;
    loop {
        let candidate = versions_dir.join(format!("{TOMBSTONE_PREFIX}{name}-{n}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Прочитать `current.json`: Ok(None) если файла нет или он невалиден —
/// в обоих случаях чистить нельзя (непонятно, что «текущее»).
fn read_current(engine_root: &Path) -> Result<Option<Version>, String> {
    let pointer_path = engine_root.join("current.json");
    let bytes = match std::fs::read(&pointer_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {pointer_path:?}: {e}")),
    };
    let Ok(pointer) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Ok(None);
    };
    let valid = pointer.get("schema_version").and_then(|v| v.as_u64()) == Some(1);
    let version = pointer
        .get("version")
        .and_then(|v| v.as_str())
        .and_then(strict_version);
    Ok(match (valid, version) {
        (true, Some(version)) => Some(version),
        _ => None,
    })
}

/// Применить retention к `engine_root` (`%LOCALAPPDATA%\Mundus\Engine`).
/// `Ok(None)` — `current.json` отсутствует/невалиден/указывает на несуществующую
/// директорию: удалять ничего нельзя. `Err` — только ошибка, помешавшая
/// построить выборку; ошибки отдельных удалений идут в `report.failed`.
pub fn prune(engine_root: &Path) -> Result<Option<Report>, String> {
    prune_with_grace(engine_root, LEFTOVER_GRACE)
}

fn prune_with_grace(engine_root: &Path, temp_grace: Duration) -> Result<Option<Report>, String> {
    let Some(current) = read_current(engine_root)? else {
        return Ok(None);
    };
    let versions_dir = engine_root.join("versions");
    if !versions_dir.join(current.to_string()).is_dir() {
        // Указатель на отсутствующую версию — установка сломана; не чистим,
        // предыдущая версия может быть единственным рабочим payload'ом.
        return Ok(None);
    }

    let mut report = Report {
        current: current.to_string(),
        ..Report::default()
    };

    let mut versions: Vec<(Version, Entry)> = Vec::new();
    let mut tombstones: Vec<PathBuf> = Vec::new();
    let mut stale_temps: Vec<PathBuf> = Vec::new();
    let entries =
        std::fs::read_dir(&versions_dir).map_err(|e| format!("read_dir {versions_dir:?}: {e}"))?;
    let cutoff = SystemTime::now() - temp_grace;
    for entry in entries.flatten() {
        // Никогда не трогаем не-директории и не спускаемся за пределы уровня.
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name.starts_with(TOMBSTONE_PREFIX) {
            tombstones.push(entry.path());
        } else if let Some(version) = strict_version(&name) {
            versions.push((
                version,
                Entry {
                    name,
                    path: entry.path(),
                },
            ));
        } else if is_install_temp(&name) {
            let old_enough = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .is_ok_and(|mtime| mtime < cutoff);
            if old_enough {
                stale_temps.push(entry.path());
            } else {
                report.kept_young_temps += 1;
            }
        } else {
            report.kept_unknown.push(name);
        }
    }

    for tombstone in tombstones {
        match std::fs::remove_dir_all(&tombstone) {
            Ok(()) => report.removed_tombstones += 1,
            Err(e) => {
                report.failed += 1;
                eprintln!("[engine-versions] failed to remove tombstone {tombstone:?}: {e}");
            }
        }
    }
    for temp in stale_temps {
        match std::fs::remove_dir_all(&temp) {
            Ok(()) => report.removed_temps += 1,
            Err(e) => {
                report.failed += 1;
                eprintln!("[engine-versions] failed to remove install temp {temp:?}: {e}");
            }
        }
    }

    // Предыдущая = высший semver строго ниже текущего — остаётся для отката.
    let previous = versions
        .iter()
        .map(|(v, _)| v)
        .filter(|v| *v < &current)
        .max()
        .cloned();
    report.previous = previous.as_ref().map(ToString::to_string);

    for (version, entry) in versions {
        if version == current {
            continue;
        }
        if version > current {
            // Был установлен более новый Engine, потом current.json откатили —
            // новый payload не трогаем.
            report.kept_newer.push(entry.name);
            continue;
        }
        if Some(&version) == previous.as_ref() {
            continue;
        }
        remove_version_dir(&versions_dir, entry, &mut report);
    }

    Ok(Some(report))
}

/// Rename-guard + удаление жертвы. Rename падает, пока хоть один файл внутри
/// открыт/замаплен — это и есть проверка «версия сейчас используется».
fn remove_version_dir(versions_dir: &Path, entry: Entry, report: &mut Report) {
    let Entry { name, path } = entry;
    let bytes = dir_size(&path);
    let tombstone = free_tombstone(versions_dir, &name);
    if let Err(e) = std::fs::rename(&path, &tombstone) {
        eprintln!("[engine-versions] skipping {name} (in use): rename failed: {e}");
        report.skipped.push(name);
        return;
    }
    match std::fs::remove_dir_all(&tombstone) {
        Ok(()) => {
            report.removed.push(name);
            report.removed_bytes += bytes;
        }
        Err(e) => {
            report.failed += 1;
            eprintln!("[engine-versions] failed to remove {tombstone:?}: {e}");
        }
    }
}

/// Корень установки Engine (`<root>` в `<root>/versions/<v>/mundus-engine.exe`),
/// выведенный из пути запущенного exe. Возвращает None при любом отклонении
/// от формата — в частности, для dev-сборки из `target/` (`debug`/`release`
/// не являются semver), поэтому dev-run никогда ничего не чистит.
pub fn engine_root_of_exe(exe_path: &Path) -> Option<PathBuf> {
    let file_name = exe_path.file_name()?.to_str()?;
    if !file_name.eq_ignore_ascii_case(&format!("{}.exe", brand::ENGINE_BINARY_STEM)) {
        return None;
    }
    let version_dir = exe_path.parent()?;
    strict_version(version_dir.file_name()?.to_str()?)?;
    let versions_dir = version_dir.parent()?;
    if versions_dir.file_name()?.to_str()? != "versions" {
        return None;
    }
    Some(versions_dir.parent()?.to_path_buf())
}

#[cfg(test)]
#[path = "prune/tests.rs"]
mod tests;
