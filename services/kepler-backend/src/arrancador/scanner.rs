// Steam / Epic / GOG game scanner.
//
// Steam discovery:
//   1. Найти Steam root → `libraryfolders.vdf` → список library paths.
//   2. Per library → `steamapps/appmanifest_*.acf` → name + installdir + size.
//   3. Walk install dir → подобрать `.exe` candidate (приоритет: имя совпадает
//      с name, иначе наибольший .exe).
//
// Epic:
//   * Per manifest в `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` —
//     JSON с DisplayName / InstallLocation / LaunchExecutable / CatalogItemId.
//
// GOG: skip в MVP (см. spec.md OOS) — TODO follow-up через galaxy-2.0.db SQLite.
//
// VDF / ACF: один и тот же KeyValues text format. Парсер ниже работает в обоих
// случаях — он не валидирует строгий VDF, но успешно извлекает простые
// `"key" "value"` пары и nested object'ы.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredGame {
    pub source: String,         // "steam" | "epic" | "gog"
    pub source_app_id: String,  // Steam app_id, Epic CatalogItemId, GOG product id
    pub name: String,
    pub install_dir: PathBuf,
    pub exe_candidate: Option<PathBuf>,
    pub install_size_bytes: u64,
}

// ---------------- VDF / ACF parser ----------------

/// Простое keyvalues-дерево.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum VdfNode {
    #[default]
    Null,
    String(String),
    Object(Vec<(String, VdfNode)>),
}

impl VdfNode {
    pub fn as_str(&self) -> Option<&str> {
        if let VdfNode::String(s) = self {
            Some(s)
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&VdfNode> {
        if let VdfNode::Object(entries) = self {
            for (k, v) in entries {
                if k.eq_ignore_ascii_case(key) {
                    return Some(v);
                }
            }
        }
        None
    }

    pub fn entries(&self) -> &[(String, VdfNode)] {
        if let VdfNode::Object(e) = self {
            e
        } else {
            &[]
        }
    }
}

/// Парсер VDF/ACF. Толерантен к комментариям `// ...` и пустым строкам.
/// Реализация state-machine на char-by-char.
pub fn parse_vdf(text: &str) -> VdfNode {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    parse_object_body(&chars, &mut i, /* top */ true, 0)
}

fn skip_ws_and_comments(chars: &[char], i: &mut usize) {
    loop {
        while *i < chars.len() && chars[*i].is_whitespace() {
            *i += 1;
        }
        if *i + 1 < chars.len() && chars[*i] == '/' && chars[*i + 1] == '/' {
            while *i < chars.len() && chars[*i] != '\n' {
                *i += 1;
            }
            continue;
        }
        break;
    }
}

fn parse_quoted(chars: &[char], i: &mut usize) -> Option<String> {
    if *i >= chars.len() || chars[*i] != '"' {
        return None;
    }
    *i += 1;
    let mut out = String::new();
    while *i < chars.len() {
        let c = chars[*i];
        if c == '\\' && *i + 1 < chars.len() {
            let nxt = chars[*i + 1];
            match nxt {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                other => out.push(other),
            }
            *i += 2;
            continue;
        }
        if c == '"' {
            *i += 1;
            return Some(out);
        }
        out.push(c);
        *i += 1;
    }
    Some(out)
}

fn parse_object_body(chars: &[char], i: &mut usize, top: bool, depth: usize) -> VdfNode {
    if depth > 64 {
        return VdfNode::Null;
    }
    let mut entries: Vec<(String, VdfNode)> = Vec::new();
    loop {
        skip_ws_and_comments(chars, i);
        if *i >= chars.len() {
            break;
        }
        if chars[*i] == '}' {
            if !top {
                *i += 1;
            }
            break;
        }
        let key = match parse_quoted(chars, i) {
            Some(k) => k,
            None => {
                // Defensive: skip char to avoid infinite loop on malformed input.
                *i += 1;
                continue;
            }
        };
        skip_ws_and_comments(chars, i);
        if *i >= chars.len() {
            entries.push((key, VdfNode::Null));
            break;
        }
        if chars[*i] == '{' {
            *i += 1;
            let nested = parse_object_body(chars, i, false, depth + 1);
            entries.push((key, nested));
            continue;
        }
        if let Some(value) = parse_quoted(chars, i) {
            entries.push((key, VdfNode::String(value)));
            continue;
        }
        entries.push((key, VdfNode::Null));
    }
    VdfNode::Object(entries)
}

// ---------------- Steam ----------------

/// Парсит libraryfolders.vdf → пути библиотек (включая root Steam).
pub fn parse_libraryfolders(text: &str) -> Vec<PathBuf> {
    let root = parse_vdf(text);
    let mut out: Vec<PathBuf> = Vec::new();
    // Format: top-level "libraryfolders" → numeric children → "path" "<...>".
    let lib = root.get("libraryfolders").unwrap_or(&root);
    for (_idx, node) in lib.entries() {
        if let Some(path) = node.get("path").and_then(|v| v.as_str()) {
            out.push(PathBuf::from(path));
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct AppManifest {
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub size_on_disk: u64,
}

pub fn parse_appmanifest(text: &str) -> Option<AppManifest> {
    let root = parse_vdf(text);
    let state = root.get("AppState")?;
    let app_id = state.get("appid")?.as_str()?.to_string();
    let name = state.get("name")?.as_str()?.to_string();
    let install_dir = state.get("installdir")?.as_str()?.to_string();
    let size_on_disk = state
        .get("SizeOnDisk")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    Some(AppManifest {
        app_id,
        name,
        install_dir,
        size_on_disk,
    })
}

/// Подобрать .exe candidate в install dir. Приоритет:
///   1. `<name>.exe` (case-insensitive contains после lowercase + non-alnum strip)
///   2. Наибольший .exe в дереве (depth <= 4 чтобы не walk'ать по полному installdir
///      с миллионом asset'ов).
pub fn pick_exe(install_dir: &Path, name: &str) -> Option<PathBuf> {
    if !install_dir.exists() {
        return None;
    }
    let mut exes: Vec<(PathBuf, u64)> = Vec::new();
    walk_exes(install_dir, 0, 4, &mut exes);
    if exes.is_empty() {
        return None;
    }
    let key = normalize_for_match(name);
    let by_name = exes
        .iter()
        .find(|(p, _)| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| normalize_for_match(s).contains(&key) || key.contains(&normalize_for_match(s)))
                .unwrap_or(false)
        })
        .map(|(p, _)| p.clone());
    if let Some(p) = by_name {
        return Some(p);
    }
    exes.sort_by_key(|(_, sz)| std::cmp::Reverse(*sz));
    exes.into_iter().next().map(|(p, _)| p)
}

fn normalize_for_match(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn walk_exes(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<(PathBuf, u64)>) {
    if depth > max_depth {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if ft.is_dir() {
            walk_exes(&path, depth + 1, max_depth, out);
        } else if ft.is_file()
            && path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
        {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push((path, size));
        }
    }
}

/// Discover Steam root. Override → ENV `STEAM_LIBRARY_FOLDERS` → registry (Windows)
/// → стандартный путь `C:\Program Files (x86)\Steam`.
pub fn steam_root(override_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = override_path {
        return Some(p.to_path_buf());
    }
    if let Ok(env) = std::env::var("STEAM_LIBRARY_FOLDERS") {
        if !env.is_empty() {
            return Some(PathBuf::from(env));
        }
    }
    #[cfg(windows)]
    {
        // Registry: HKCU\Software\Valve\Steam\SteamPath — TODO в follow-up.
        // Пока — стандартный путь.
        let default = PathBuf::from("C:\\Program Files (x86)\\Steam");
        if default.exists() {
            return Some(default);
        }
    }
    None
}

/// Сканировать одну Steam library (path указывает либо на Steam root, либо
/// на сторонюю library — в обоих случаях ожидается subdir `steamapps`).
fn scan_steam_library(library: &Path) -> Vec<DiscoveredGame> {
    let steamapps = library.join("steamapps");
    if !steamapps.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&steamapps) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let manifest = match parse_appmanifest(&text) {
            Some(m) => m,
            None => continue,
        };
        let install_dir = steamapps.join("common").join(&manifest.install_dir);
        let exe = pick_exe(&install_dir, &manifest.name);
        out.push(DiscoveredGame {
            source: "steam".into(),
            source_app_id: manifest.app_id,
            name: manifest.name,
            install_dir,
            exe_candidate: exe,
            install_size_bytes: manifest.size_on_disk,
        });
    }
    out
}

pub fn scan_steam(override_path: Option<&Path>) -> Vec<DiscoveredGame> {
    let root = match steam_root(override_path) {
        Some(r) => r,
        None => return Vec::new(),
    };
    // Читаем libraryfolders.vdf (в root\steamapps\libraryfolders.vdf).
    let vdf_path = root.join("steamapps").join("libraryfolders.vdf");
    let mut libraries: Vec<PathBuf> = if let Ok(text) = std::fs::read_to_string(&vdf_path) {
        parse_libraryfolders(&text)
    } else {
        Vec::new()
    };
    // Root всегда добавляем как library (даже если в .vdf он не упомянут — это
    // справедливо для свежих установок Steam).
    if !libraries.iter().any(|p| p == &root) {
        libraries.insert(0, root.clone());
    }
    let mut games = Vec::new();
    for lib in libraries {
        games.extend(scan_steam_library(&lib));
    }
    games
}

// ---------------- Epic ----------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EpicManifest {
    display_name: Option<String>,
    install_location: Option<String>,
    catalog_item_id: Option<String>,
    launch_executable: Option<String>,
    app_name: Option<String>,
}

pub fn parse_epic_manifest(text: &str) -> Option<DiscoveredGame> {
    let mf: EpicManifest = serde_json::from_str(text).ok()?;
    let name = mf.display_name?;
    let install_location = mf.install_location?;
    let install_dir = PathBuf::from(&install_location);
    let exe = mf.launch_executable.as_deref().filter(|s| !s.is_empty()).map(|s| install_dir.join(s));
    let app_id = mf.catalog_item_id.or(mf.app_name).unwrap_or_else(|| name.clone());
    Some(DiscoveredGame {
        source: "epic".into(),
        source_app_id: app_id,
        name,
        install_dir,
        exe_candidate: exe,
        install_size_bytes: 0,
    })
}

pub fn scan_epic(manifests_dir_override: Option<&Path>) -> Vec<DiscoveredGame> {
    let dir = match manifests_dir_override {
        Some(p) => p.to_path_buf(),
        None => {
            #[cfg(windows)]
            {
                PathBuf::from("C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests")
            }
            #[cfg(not(windows))]
            {
                return Vec::new();
            }
        }
    };
    if !dir.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("item") {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if let Some(game) = parse_epic_manifest(&text) {
            out.push(game);
        }
    }
    out
}

// ---------------- Combined ----------------

pub fn scan_all(steam_library_override: Option<&Path>) -> Vec<DiscoveredGame> {
    let mut games = scan_steam(steam_library_override);
    games.extend(scan_epic(None));
    // Dedupe по (source, source_app_id) на всякий случай.
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    let mut deduped: Vec<DiscoveredGame> = Vec::new();
    for g in games {
        let key = (g.source.clone(), g.source_app_id.clone());
        if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(key) {
            e.insert(deduped.len());
            deduped.push(g);
        }
    }
    deduped.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const LIBRARYFOLDERS_VDF: &str = r#"
"libraryfolders"
{
    "0"
    {
        "path"      "C:\\Program Files (x86)\\Steam"
        "apps"
        {
            "570"   "30000000000"
        }
    }
    "1"
    {
        "path"      "D:\\SteamLibrary"
    }
}
"#;

    const APPMANIFEST_DOTA: &str = r#"
"AppState"
{
    "appid"     "570"
    "name"      "Dota 2"
    "installdir"    "dota 2 beta"
    "SizeOnDisk"    "30000000000"
}
"#;

    const APPMANIFEST_CAIRN: &str = r#"
"AppState"
{
    "appid"     "1810770"
    "name"      "Cairn"
    "installdir"    "Cairn"
    "SizeOnDisk"    "8000000000"
}
"#;

    const APPMANIFEST_OUTLAST: &str = r#"
"AppState"
{
    "appid"     "238320"
    "name"      "Outlast"
    "installdir"    "Outlast"
    "SizeOnDisk"    "4000000000"
}
"#;

    #[test]
    fn scanner_steam_parses_libraryfolders_vdf() {
        let paths = parse_libraryfolders(LIBRARYFOLDERS_VDF);
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|p| p.to_string_lossy().contains("Steam")));
        assert!(paths.iter().any(|p| p.to_string_lossy().contains("SteamLibrary")));
    }

    #[test]
    fn scanner_steam_parses_appmanifest_acf() {
        let dota = parse_appmanifest(APPMANIFEST_DOTA).unwrap();
        assert_eq!(dota.app_id, "570");
        assert_eq!(dota.name, "Dota 2");
        assert_eq!(dota.install_dir, "dota 2 beta");
        assert_eq!(dota.size_on_disk, 30_000_000_000);

        let cairn = parse_appmanifest(APPMANIFEST_CAIRN).unwrap();
        assert_eq!(cairn.app_id, "1810770");
        assert_eq!(cairn.name, "Cairn");

        let outlast = parse_appmanifest(APPMANIFEST_OUTLAST).unwrap();
        assert_eq!(outlast.app_id, "238320");
        assert_eq!(outlast.name, "Outlast");
    }

    #[test]
    fn scanner_appmanifest_handles_missing_size() {
        let text = r#"
"AppState"
{
    "appid"     "1"
    "name"      "X"
    "installdir"    "x"
}
"#;
        let mf = parse_appmanifest(text).unwrap();
        assert_eq!(mf.size_on_disk, 0);
    }

    #[test]
    fn scanner_epic_parses_manifest_item() {
        let json = r#"{
            "DisplayName": "Hades",
            "InstallLocation": "D:\\EpicGames\\Hades",
            "CatalogItemId": "abc123",
            "LaunchExecutable": "x64\\Hades.exe",
            "AppName": "Min"
        }"#;
        let game = parse_epic_manifest(json).unwrap();
        assert_eq!(game.source, "epic");
        assert_eq!(game.name, "Hades");
        assert_eq!(game.source_app_id, "abc123");
        assert!(game.exe_candidate.is_some());
        assert!(game
            .exe_candidate
            .unwrap()
            .to_string_lossy()
            .ends_with("Hades.exe"));
    }

    #[test]
    fn scanner_scan_steam_finds_three_synthetic_games() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let steamapps = root.join("steamapps");
        std::fs::create_dir_all(&steamapps).unwrap();
        std::fs::write(
            steamapps.join("libraryfolders.vdf"),
            format!(
                r#"
"libraryfolders"
{{
    "0"
    {{
        "path"      "{}"
    }}
}}
"#,
                root.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        std::fs::write(steamapps.join("appmanifest_570.acf"), APPMANIFEST_DOTA).unwrap();
        std::fs::write(steamapps.join("appmanifest_1810770.acf"), APPMANIFEST_CAIRN).unwrap();
        std::fs::write(steamapps.join("appmanifest_238320.acf"), APPMANIFEST_OUTLAST).unwrap();

        // Создаём install dirs + dummy exe для pick_exe.
        let common = steamapps.join("common");
        std::fs::create_dir_all(common.join("dota 2 beta")).unwrap();
        std::fs::write(common.join("dota 2 beta").join("dota2.exe"), [0u8; 16]).unwrap();
        std::fs::create_dir_all(common.join("Cairn")).unwrap();
        std::fs::write(common.join("Cairn").join("Cairn.exe"), [0u8; 16]).unwrap();
        std::fs::create_dir_all(common.join("Outlast")).unwrap();
        std::fs::write(common.join("Outlast").join("OLGame.exe"), [0u8; 16]).unwrap();

        let games = scan_steam(Some(root));
        assert_eq!(games.len(), 3, "expected 3 games, got {games:?}");
        let names: Vec<_> = games.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"Dota 2"));
        assert!(names.contains(&"Cairn"));
        assert!(names.contains(&"Outlast"));
        let ids: Vec<_> = games.iter().map(|g| g.source_app_id.as_str()).collect();
        assert!(ids.contains(&"570"));
        assert!(ids.contains(&"1810770"));
        assert!(ids.contains(&"238320"));
    }

    #[test]
    fn scan_all_returns_sorted_by_name() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let steamapps = root.join("steamapps");
        std::fs::create_dir_all(&steamapps).unwrap();
        std::fs::write(
            steamapps.join("libraryfolders.vdf"),
            format!(
                r#"
"libraryfolders"
{{
    "0" {{ "path" "{}" }}
}}
"#,
                root.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        std::fs::write(steamapps.join("appmanifest_570.acf"), APPMANIFEST_DOTA).unwrap();
        std::fs::write(steamapps.join("appmanifest_238320.acf"), APPMANIFEST_OUTLAST).unwrap();
        std::fs::create_dir_all(steamapps.join("common").join("dota 2 beta")).unwrap();
        std::fs::create_dir_all(steamapps.join("common").join("Outlast")).unwrap();

        let games = scan_all(Some(root));
        assert_eq!(games.len(), 2);
        // Cairn isn't here, so just Dota 2 < Outlast alphabetically.
        assert_eq!(games[0].name, "Dota 2");
        assert_eq!(games[1].name, "Outlast");
    }

    #[test]
    fn pick_exe_prefers_name_match() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("launcher.exe"), [0u8; 4096]).unwrap();
        std::fs::write(dir.join("Cairn.exe"), [0u8; 64]).unwrap();
        let pick = pick_exe(dir, "Cairn").unwrap();
        assert!(pick.file_name().unwrap().to_string_lossy().contains("Cairn"));
    }

    #[test]
    fn pick_exe_falls_back_to_largest() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("redist.exe"), [0u8; 16]).unwrap();
        std::fs::write(dir.join("big_thing.exe"), [0u8; 4096]).unwrap();
        let pick = pick_exe(dir, "completely-different-name").unwrap();
        assert!(pick
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("big_thing"));
    }

    #[test]
    fn vdf_depth_limit_prevents_stack_overflow() {
        // 200 levels of nesting — well beyond the 64-level cap.
        let mut vdf = String::new();
        for _ in 0..200 {
            vdf.push_str("\"k\" {\n");
        }
        for _ in 0..200 {
            vdf.push('}');
        }
        // Should not panic/stack-overflow; just returns a truncated tree.
        let result = parse_vdf(&vdf);
        assert!(matches!(result, VdfNode::Object(_) | VdfNode::Null));
    }
}
