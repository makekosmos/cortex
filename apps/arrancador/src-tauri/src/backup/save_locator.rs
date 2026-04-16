use crate::backup::sqoba_manifest::{normalize_name, similarity_score, SqobaGame, SqobaManifest};
use glob::glob;
use std::collections::{HashSet, VecDeque};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct SaveRoot {
    pub label: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SaveFile {
    pub path: PathBuf,
    pub root_label: String,
    pub relative_path: PathBuf,
    #[allow(dead_code)]
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct SaveDiscovery {
    pub roots: Vec<SaveRoot>,
    pub files: Vec<SaveFile>,
    pub total_size: u64,
}

pub fn locate_game_saves(
    game_name: &str,
    manifest: Option<&SqobaManifest>,
    override_path: Option<&str>,
) -> Result<Option<SaveDiscovery>, String> {
    let roots = locate_game_save_roots(game_name, manifest, override_path)?;
    if roots.is_empty() {
        return Ok(None);
    }

    let discovery = collect_files(&roots)?;
    if discovery.files.is_empty() {
        return Ok(None);
    }

    Ok(Some(discovery))
}

pub fn locate_game_save_roots(
    game_name: &str,
    manifest: Option<&SqobaManifest>,
    override_path: Option<&str>,
) -> Result<Vec<SaveRoot>, String> {
    let mut roots = Vec::new();

    if let Some(path) = override_path {
        let path = PathBuf::from(path);
        if path.exists() {
            roots.push(path);
        } else {
            return Err(format!(
                "Путь к сохранениям не существует: {}",
                path.display()
            ));
        }
    }

    if roots.is_empty() {
        if let Some(manifest) = manifest {
            if let Some((_, entry)) = manifest.find_game_entry(game_name) {
                roots = manifest_roots(&entry);
            }
        }
    }

    if roots.is_empty() {
        roots = heuristic_roots(game_name);
    }

    let roots = build_roots(roots);
    Ok(roots)
}

fn build_roots(paths: Vec<PathBuf>) -> Vec<SaveRoot> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            let label = format!("root-{}", out.len());
            out.push(SaveRoot { label, path });
        }
    }
    out
}

fn manifest_roots(entry: &SqobaGame) -> Vec<PathBuf> {
    let context = PathResolutionContext::new();
    let mut roots = Vec::new();
    if let Some(files_map) = &entry.files {
        for paths in files_map.values() {
            for raw_path in paths {
                roots.extend(resolve_path(raw_path, &context));
            }
        }
    }
    roots
}

fn collect_files(roots: &[SaveRoot]) -> Result<SaveDiscovery, String> {
    let mut files = Vec::new();
    let mut total_size = 0u64;
    let mut seen = HashSet::new();

    for root in roots {
        if root.path.is_file() {
            let size = fs::metadata(&root.path).map(|m| m.len()).unwrap_or(0);
            let name = root
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string());
            let relative = PathBuf::from(name);
            if seen.insert(root.path.clone()) {
                files.push(SaveFile {
                    path: root.path.clone(),
                    root_label: root.label.clone(),
                    relative_path: relative,
                    size,
                });
                total_size += size;
            }
        } else if root.path.is_dir() {
            for entry in WalkDir::new(&root.path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let relative = entry
                        .path()
                        .strip_prefix(&root.path)
                        .unwrap_or(entry.path())
                        .to_path_buf();
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let entry_path = entry.path().to_path_buf();
                    if seen.insert(entry_path.clone()) {
                        files.push(SaveFile {
                            path: entry_path,
                            root_label: root.label.clone(),
                            relative_path: relative,
                            size,
                        });
                        total_size += size;
                    }
                }
            }
        }
    }

    Ok(SaveDiscovery {
        roots: roots.to_vec(),
        files,
        total_size,
    })
}

struct PathResolutionContext {
    home: Option<PathBuf>,
    documents: Option<PathBuf>,
    appdata: Option<PathBuf>,
    local_appdata: Option<PathBuf>,
    local_low: Option<PathBuf>,
    saved_games: Option<PathBuf>,
    public: Option<PathBuf>,
    public_documents: Option<PathBuf>,
    program_data: Option<PathBuf>,
    steam: Option<PathBuf>,
    steam_userdata: Option<PathBuf>,
}

impl PathResolutionContext {
    fn new() -> Self {
        let home = dirs::home_dir();
        let documents = dirs::document_dir();
        let appdata = dirs::data_dir();
        let local_appdata = dirs::data_local_dir();
        let local_low = local_appdata
            .as_ref()
            .and_then(|local| local.parent().map(|p| p.join("LocalLow")));
        let saved_games = home.as_ref().map(|h| h.join("Saved Games"));
        let public = env::var("PUBLIC").ok().map(PathBuf::from);
        let public_documents = public.as_ref().map(|p| p.join("Documents"));
        let program_data = env::var("ProgramData").ok().map(PathBuf::from);
        let steam = find_steam_path();
        let steam_userdata = steam.as_ref().map(|p| p.join("userdata"));

        Self {
            home,
            documents,
            appdata,
            local_appdata,
            local_low,
            saved_games,
            public,
            public_documents,
            program_data,
            steam,
            steam_userdata,
        }
    }
}

fn resolve_path(raw_path: &str, context: &PathResolutionContext) -> Vec<PathBuf> {
    let mut path = raw_path.to_string();
    let mut missing = false;

    path = replace_token(path, "<home>", &context.home, &mut missing);
    path = replace_token(path, "<winDocuments>", &context.documents, &mut missing);
    path = replace_token(path, "<documents>", &context.documents, &mut missing);
    path = replace_token(path, "<winAppData>", &context.appdata, &mut missing);
    path = replace_token(
        path,
        "<winLocalAppData>",
        &context.local_appdata,
        &mut missing,
    );
    path = replace_token(
        path,
        "<winLocalAppDataLow>",
        &context.local_low,
        &mut missing,
    );
    path = replace_token(path, "<winLocalLow>", &context.local_low, &mut missing);
    path = replace_token(path, "<winSavedGames>", &context.saved_games, &mut missing);
    path = replace_token(path, "<winPublic>", &context.public, &mut missing);
    path = replace_token(
        path,
        "<winPublicDocuments>",
        &context.public_documents,
        &mut missing,
    );
    path = replace_token(
        path,
        "<winProgramData>",
        &context.program_data,
        &mut missing,
    );
    path = replace_token(path, "<steam>", &context.steam, &mut missing);
    path = replace_token(
        path,
        "<steamUserData>",
        &context.steam_userdata,
        &mut missing,
    );
    path = replace_token(
        path,
        "<steamuserdata>",
        &context.steam_userdata,
        &mut missing,
    );

    // Ludusavi manifest uses <storeUserId> for per-user steam subfolders.
    // In this app we do not have the numeric user id, so match all ids.
    path = path.replace("<storeUserId>", "*");

    if missing {
        return Vec::new();
    }

    let path = expand_env_vars(&path);
    let path = expand_tilde(&path, context.home.as_deref());

    if path.contains('*') || path.contains('?') {
        let mut out = Vec::new();
        if let Ok(paths) = glob(&path) {
            for item in paths.flatten() {
                out.push(item);
            }
        }
        return out;
    }

    let resolved = PathBuf::from(path);
    if resolved.exists() {
        vec![resolved]
    } else {
        Vec::new()
    }
}

fn replace_token(
    mut base: String,
    token: &str,
    value: &Option<PathBuf>,
    missing: &mut bool,
) -> String {
    if base.contains(token) {
        if let Some(val) = value {
            base = base.replace(token, &val.to_string_lossy());
        } else {
            *missing = true;
        }
    }
    base
}

fn expand_env_vars(path: &str) -> String {
    let mut out = String::new();
    let mut chars = path.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let mut key = String::new();
            for next in chars.by_ref() {
                if next == '%' {
                    break;
                }
                key.push(next);
            }
            if key.is_empty() {
                out.push('%');
            } else if let Ok(val) = env::var(&key) {
                out.push_str(&val);
            } else {
                out.push('%');
                out.push_str(&key);
                out.push('%');
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn expand_tilde(path: &str, home: Option<&Path>) -> String {
    if let Some(home) = home {
        if let Some(stripped) = path.strip_prefix("~") {
            let mut out = home.to_string_lossy().to_string();
            out.push_str(stripped);
            return out;
        }
    }
    path.to_string()
}

fn heuristic_roots(game_name: &str) -> Vec<PathBuf> {
    let context = PathResolutionContext::new();
    let variants = candidate_names(game_name);
    let mut roots = Vec::new();

    if let Some(documents) = &context.documents {
        roots.extend(find_named_paths(&documents.join("My Games"), &variants));
        roots.extend(find_named_paths(&documents.join("Saved Games"), &variants));
        roots.extend(find_named_paths(documents, &variants));
    }

    if let Some(saved_games) = &context.saved_games {
        roots.extend(find_named_paths(saved_games, &variants));
    }

    if let Some(appdata) = &context.appdata {
        roots.extend(find_named_paths(appdata, &variants));
    }

    if let Some(local) = &context.local_appdata {
        roots.extend(find_named_paths(local, &variants));
        roots.extend(find_windows_store_paths(local, game_name));
    }

    if let Some(local_low) = &context.local_low {
        roots.extend(find_named_paths(local_low, &variants));
    }

    roots.extend(find_steam_save_paths(game_name));
    roots
}

fn candidate_names(game_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    let base = sanitize_name(game_name);
    push_unique(&mut out, &mut seen, base);

    let normalized = sanitize_name(&normalize_name(game_name));
    push_unique(&mut out, &mut seen, normalized);

    let collapsed = out
        .iter()
        .map(|s| s.replace(' ', ""))
        .collect::<Vec<String>>();
    for item in collapsed {
        push_unique(&mut out, &mut seen, item);
    }

    out.retain(|name| !name.is_empty());
    out
}

fn sanitize_name(name: &str) -> String {
    let invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let cleaned: String = name.chars().filter(|c| !invalid.contains(c)).collect();
    cleaned.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn push_unique(out: &mut Vec<String>, seen: &mut HashSet<String>, value: String) {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    if seen.insert(trimmed.clone()) {
        out.push(trimmed);
    }
}

fn find_named_paths(base: &Path, names: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for name in names {
        let path = base.join(name);
        if path.exists() {
            out.push(path);
        }
    }
    out
}

fn find_windows_store_paths(local_appdata: &Path, game_name: &str) -> Vec<PathBuf> {
    let packages_root = local_appdata.join("Packages");
    if !packages_root.exists() {
        return Vec::new();
    }

    let normalized = normalize_name(game_name).replace(' ', "");
    if normalized.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    if let Ok(entries) = fs::read_dir(packages_root) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if !file_type.is_dir() {
                    continue;
                }
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !name.contains(&normalized) {
                continue;
            }
            let root = entry.path();
            let candidates = [
                root.join("SystemAppData").join("wgs"),
                root.join("SystemAppData").join("xgs"),
                root.join("LocalState"),
            ];
            for candidate in candidates {
                if candidate.exists() {
                    matches.push(candidate);
                }
            }
        }
    }

    matches
}

fn find_steam_save_paths(game_name: &str) -> Vec<PathBuf> {
    let steam_path = match find_steam_path() {
        Some(path) => path,
        None => return Vec::new(),
    };

    let library_paths = find_steam_library_paths(&steam_path);
    let app_ids = find_steam_app_ids(game_name, &library_paths);
    if app_ids.is_empty() {
        return Vec::new();
    }

    let userdata_root = steam_path.join("userdata");
    if !userdata_root.exists() {
        return Vec::new();
    }

    let mut out = Vec::new();
    if let Ok(users) = fs::read_dir(userdata_root) {
        for user in users.flatten() {
            let user_path = user.path();
            if !user_path.is_dir() {
                continue;
            }
            for app_id in &app_ids {
                let app_root = user_path.join(app_id);
                if !app_root.exists() {
                    continue;
                }
                let remote = app_root.join("remote");
                if remote.exists() {
                    out.push(remote);
                    continue;
                }
                let local = app_root.join("local");
                if local.exists() {
                    out.push(local);
                    continue;
                }
                out.push(app_root.clone());
            }
        }
    }

    out
}

fn find_steam_library_paths(steam_path: &Path) -> Vec<PathBuf> {
    let mut paths = VecDeque::new();
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    paths.push_back(steam_path.to_path_buf());
    if let Some(value) = read_libraryfolders_value(steam_path) {
        for path in value {
            paths.push_back(path);
        }
    }

    while let Some(path) = paths.pop_front() {
        if seen.insert(path.clone()) {
            out.push(path);
        }
    }

    out
}

fn read_libraryfolders_value(steam_path: &Path) -> Option<Vec<PathBuf>> {
    let library_file = steam_path.join("steamapps").join("libraryfolders.vdf");
    let text = fs::read_to_string(library_file).ok()?;
    let mut paths = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() < 4 {
            continue;
        }
        if parts[1] == "path" {
            let raw = parts[3].replace("\\\\", "\\");
            paths.push(PathBuf::from(raw));
        }
    }
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

fn find_steam_app_ids(game_name: &str, library_paths: &[PathBuf]) -> Vec<String> {
    let target = normalize_name(game_name);
    let mut app_ids = Vec::new();
    let mut seen = HashSet::new();

    for library in library_paths {
        let steamapps = library.join("steamapps");
        if !steamapps.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&steamapps) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("acf") {
                    continue;
                }
                let stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(s) => s,
                    None => continue,
                };
                let app_id = match stem.strip_prefix("appmanifest_") {
                    Some(id) => id.to_string(),
                    None => continue,
                };
                let text = fs::read_to_string(&path).ok();
                let name = text
                    .as_deref()
                    .and_then(|contents| find_acf_value(contents, "name"));
                let Some(name) = name else {
                    continue;
                };
                let normalized = normalize_name(&name);
                let score = similarity_score(&target, &normalized);
                if score >= 0.7 && seen.insert(app_id.clone()) {
                    app_ids.push(app_id);
                }
            }
        }
    }

    app_ids
}

fn find_acf_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() >= 4 && parts[1] == key {
            return Some(parts[3].to_string());
        }
    }
    None
}

fn find_steam_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(key) = hklm.open_subkey("SOFTWARE\\Wow6432Node\\Valve\\Steam") {
            if let Ok(path) = key.get_value::<String, _>("InstallPath") {
                return Some(PathBuf::from(path));
            }
        }
        if let Ok(key) = hklm.open_subkey("SOFTWARE\\Valve\\Steam") {
            if let Ok(path) = key.get_value::<String, _>("InstallPath") {
                return Some(PathBuf::from(path));
            }
        }
    }

    let paths = vec!["C:\\Program Files (x86)\\Steam", "C:\\Program Files\\Steam"];
    for path in paths {
        let pb = PathBuf::from(path);
        if pb.exists() {
            return Some(pb);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn locate_game_saves_with_override_directory() {
        let dir = tempdir().expect("tempdir");
        let save_dir = dir.path().join("saves");
        fs::create_dir_all(&save_dir).expect("create save dir");

        let file_a = save_dir.join("slot1.sav");
        let file_b = save_dir.join("slot2.sav");
        fs::write(&file_a, b"alpha").expect("write file a");
        fs::write(&file_b, b"beta").expect("write file b");

        let save_path = save_dir.to_string_lossy().to_string();
        let discovery = locate_game_saves("Test Game", None, Some(&save_path))
            .expect("locate saves")
            .expect("discovery present");

        assert_eq!(discovery.files.len(), 2);
        assert!(discovery.total_size > 0);
        assert_eq!(discovery.roots.len(), 1);
    }

    #[test]
    fn locate_game_saves_with_manifest_entry() {
        let dir = tempdir().expect("tempdir");
        let save_dir = dir.path().join("manifest_saves");
        fs::create_dir_all(&save_dir).expect("create save dir");
        let file_a = save_dir.join("slot1.sav");
        fs::write(&file_a, b"alpha").expect("write file");

        let mut files = HashMap::new();
        files.insert(
            "save".to_string(),
            vec![save_dir.to_string_lossy().to_string()],
        );
        let mut games = HashMap::new();
        games.insert(
            "Manifest Game".to_string(),
            SqobaGame {
                files: Some(files),
                registry: None,
            },
        );
        let manifest = SqobaManifest::from_games(games);

        let discovery = locate_game_saves("Manifest Game", Some(&manifest), None)
            .expect("locate saves")
            .expect("discovery present");

        assert_eq!(discovery.files.len(), 1);
        assert_eq!(discovery.roots.len(), 1);
        assert_eq!(discovery.roots[0].path, save_dir);
    }

    #[test]
    fn locate_game_saves_reports_missing_override_path() {
        let dir = tempdir().expect("tempdir");
        let missing_path = dir.path().join("missing");
        let missing_path = missing_path.to_string_lossy().to_string();
        let result = locate_game_saves("Missing", None, Some(&missing_path));
        assert!(result.is_err());
    }
}
