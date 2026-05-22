// Start Menu (.lnk) source — Windows.
//
// Сканирует две стандартные директории:
//   - %APPDATA%\Microsoft\Windows\Start Menu\Programs      (per-user)
//   - %PROGRAMDATA%\Microsoft\Windows\Start Menu\Programs  (system-wide)
//
// Рекурсивно обходит подпапки, парсит .lnk через crate `lnk`, извлекает
// target path (с env-var expansion), фильтрует мусор и возвращает Vec<App>.
//
// Поведение:
// - Display name = имя файла без `.lnk` (узнаваемо пользователю), не
//   `ShellLink::name()` (часто пусто или дублирует).
// - id = первые 16 hex символов SHA256(canonical_target.to_lowercase()) —
//   стабильный, дедуплицирует одинаковые таргеты.
// - Skip: uninstall*/deinstall*/удалить* (case-insensitive substring),
//   `.url` и `.appref-ms` (только .lnk), broken targets (path не существует),
//   non-.exe/.bat/.cmd таргеты, zero-byte файлы.
// - Дедуп по id — первый .lnk выигрывает.
// - Errors per-file логируются в `tracing::debug` и пропускаются — never abort
//   the whole scan.

#![cfg(target_os = "windows")]

use crate::app_index::app::{App, AppKind};
use crate::app_index::{AppSource, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct StartMenuSource {
    icon_cache_dir: PathBuf,
}

impl StartMenuSource {
    pub fn new(icon_cache_dir: PathBuf) -> Self {
        Self { icon_cache_dir }
    }
}

impl AppSource for StartMenuSource {
    fn name(&self) -> &'static str {
        "start_menu"
    }

    fn discover(&self) -> Result<Vec<App>> {
        tracing::info!(target: "app_index", "start_menu scan started");

        let roots = start_menu_roots();
        let mut by_id: HashMap<String, App> = HashMap::new();

        for root in &roots {
            if !root.exists() {
                tracing::debug!(
                    target: "app_index",
                    root = %root.display(),
                    "start_menu root missing — skip"
                );
                continue;
            }
            scan_dir(root, &mut by_id, &self.icon_cache_dir);
        }

        let apps: Vec<App> = by_id.into_values().collect();
        tracing::info!(
            target: "app_index",
            count = apps.len(),
            "start_menu scan finished"
        );
        Ok(apps)
    }
}

pub fn launch_win32(exec_path: &str) -> Result<()> {
    // `cmd /c start "" "<path>"` — отрабатывает .exe, .bat, .url, .pdf и т.д.
    // через ShellExecute.
    use std::process::Command;
    Command::new("cmd")
        .args(["/c", "start", "", exec_path])
        .spawn()
        .map_err(|e| crate::app_index::AppIndexError::Launch(format!("{e}")))?;
    Ok(())
}

// --- internals ---

fn start_menu_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        out.push(
            PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs"),
        );
    }
    if let Ok(programdata) = std::env::var("PROGRAMDATA") {
        out.push(
            PathBuf::from(programdata).join("Microsoft\\Windows\\Start Menu\\Programs"),
        );
    }
    out
}

fn scan_dir(root: &Path, by_id: &mut HashMap<String, App>, icon_cache_dir: &Path) {
    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        if ext.as_deref() != Some("lnk") {
            // Skip .url / .appref-ms / прочее.
            continue;
        }

        let file_stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        if is_uninstaller_name(&file_stem) {
            tracing::debug!(
                target: "app_index",
                path = %path.display(),
                "skip: uninstaller filename"
            );
            continue;
        }

        match parse_lnk(path) {
            Ok(Some(target)) => {
                let canonical = canonicalize_target(&target);
                let target_md = match std::fs::metadata(&canonical) {
                    Ok(m) => m,
                    Err(_) => {
                        tracing::debug!(
                            target: "app_index",
                            path = %path.display(),
                            target = %canonical.display(),
                            "skip: broken target"
                        );
                        continue;
                    }
                };
                if target_md.len() == 0 {
                    tracing::debug!(
                        target: "app_index",
                        target = %canonical.display(),
                        "skip: zero-byte target"
                    );
                    continue;
                }
                let target_ext = canonical
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_ascii_lowercase());
                match target_ext.as_deref() {
                    Some("exe") | Some("bat") | Some("cmd") => {}
                    _ => {
                        tracing::debug!(
                            target: "app_index",
                            target = %canonical.display(),
                            "skip: non-executable target"
                        );
                        continue;
                    }
                }

                let exec_path = canonical.to_string_lossy().to_string();
                let id = hash_id(&exec_path.to_lowercase());
                if by_id.contains_key(&id) {
                    tracing::debug!(
                        target: "app_index",
                        target = %exec_path,
                        "skip: duplicate id"
                    );
                    continue;
                }
                let mtime = mtime_secs(path);
                // Извлекаем иконку eagerly с доступом к .lnk path —
                // Squirrel-installer apps (Discord, Slack, Teams) кладут
                // реальный icon location в .lnk, а target указывает на
                // Update.exe без embedded ресурсов.
                let icon_path = crate::app_index::icons::ensure_icon_for_lnk(
                    icon_cache_dir,
                    path,
                    &exec_path,
                )
                .ok();
                let app = App {
                    id: id.clone(),
                    name: file_stem,
                    exec_path,
                    icon_path,
                    kind: AppKind::Win32,
                    source: "start_menu".to_string(),
                    mtime,
                };
                by_id.insert(id, app);
            }
            Ok(None) => {
                tracing::debug!(
                    target: "app_index",
                    path = %path.display(),
                    "skip: lnk has no resolvable target"
                );
            }
            Err(e) => {
                tracing::debug!(
                    target: "app_index",
                    path = %path.display(),
                    error = %e,
                    "skip: lnk parse error"
                );
            }
        }
    }
}

/// Парсит .lnk и пытается извлечь target path несколькими стратегиями:
/// 1) `link_info().local_base_path()` (most reliable)
/// 2) `link_info().local_base_path_unicode()`
/// 3) `working_dir()` + `relative_path()` (RELATIVE_PATH stringdata)
/// Затем — env var expansion.
fn parse_lnk(path: &Path) -> std::result::Result<Option<String>, String> {
    let shell_link = lnk::ShellLink::open(path).map_err(|e| format!("{e:?}"))?;

    // Try link_info first.
    if let Some(li) = shell_link.link_info() {
        if let Some(p) = li.local_base_path() {
            let expanded = expand_env_vars(p);
            if !expanded.is_empty() {
                return Ok(Some(expanded));
            }
        }
        if let Some(p) = li.local_base_path_unicode() {
            let expanded = expand_env_vars(p);
            if !expanded.is_empty() {
                return Ok(Some(expanded));
            }
        }
    }

    // Fallback: relative_path resolved against working_dir or .lnk's parent.
    if let Some(rel) = shell_link.relative_path() {
        let expanded_rel = expand_env_vars(rel);
        let base: PathBuf = if let Some(wd) = shell_link.working_dir() {
            PathBuf::from(expand_env_vars(wd))
        } else {
            path.parent().map(|p| p.to_path_buf()).unwrap_or_default()
        };
        let joined = base.join(&expanded_rel);
        return Ok(Some(joined.to_string_lossy().to_string()));
    }

    Ok(None)
}

fn canonicalize_target(s: &str) -> PathBuf {
    let pb = PathBuf::from(s);
    // Резолвим .. / относительные сегменты если возможно. Если canonicalize
    // падает (broken path), оставляем как есть — выше .exists() решит.
    match std::fs::canonicalize(&pb) {
        Ok(c) => {
            // Срезаем `\\?\` UNC prefix Windows ставит — exec_path выглядит чище.
            let s = c.to_string_lossy().to_string();
            if let Some(stripped) = s.strip_prefix(r"\\?\") {
                PathBuf::from(stripped)
            } else {
                c
            }
        }
        Err(_) => pb,
    }
}

fn mtime_secs(path: &Path) -> i64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn hash_id(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(16);
    for b in &digest[..8] {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Case-insensitive substring match для uninstaller filename heuristic.
fn is_uninstaller_name(stem: &str) -> bool {
    let lower = stem.to_lowercase();
    lower.contains("uninstall") || lower.contains("deinstall") || lower.contains("удалить")
}

/// Раскрывает `%VARNAME%` в строке через `std::env::vars()`. Неизвестные
/// токены остаются как есть (не падаем — лучше отдать «broken» path, который
/// потом отсеется через .exists()).
pub fn expand_env_vars(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            // Ищем закрывающий %.
            if let Some(end_rel) = bytes[i + 1..].iter().position(|&b| b == b'%') {
                let var_name = &s[i + 1..i + 1 + end_rel];
                if !var_name.is_empty() {
                    if let Ok(val) = std::env::var(var_name) {
                        out.push_str(&val);
                        i += 2 + end_rel;
                        continue;
                    }
                    // Case-insensitive fallback (Windows env vars are CI).
                    let lower = var_name.to_ascii_lowercase();
                    if let Some((_, v)) = std::env::vars()
                        .find(|(k, _)| k.to_ascii_lowercase() == lower)
                    {
                        out.push_str(&v);
                        i += 2 + end_rel;
                        continue;
                    }
                }
                // Unknown var — keep literal `%VARNAME%`.
                out.push_str(&s[i..i + 2 + end_rel]);
                i += 2 + end_rel;
                continue;
            }
        }
        let ch = s[i..].chars().next().expect("i < bytes.len()");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}
