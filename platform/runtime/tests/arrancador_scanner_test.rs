#![allow(clippy::unwrap_used)]

// Integration test: синтетическая Steam library с тремя играми (Dota 2 / Cairn /
// Outlast). Покрывает AC3 (scanner-часть): scanner находит 3 game_obj с
// корректными именами + source_app_id. ARK upsert через WS dispatcher — не
// здесь, а в `arrancador_full_flow.rs` (subagent B/C добавят), потому что
// требует поднятого ark-core-rpc child'а.

use kepler_backend::arrancador::scanner;
use tempfile::TempDir;

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

fn build_synthetic_library() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let steamapps = root.join("steamapps");
    std::fs::create_dir_all(&steamapps).unwrap();

    let escaped = root.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        steamapps.join("libraryfolders.vdf"),
        format!(
            r#"
"libraryfolders"
{{
    "0"
    {{
        "path"      "{escaped}"
        "apps"
        {{
            "570"   "30000000000"
            "1810770" "8000000000"
            "238320" "4000000000"
        }}
    }}
}}
"#
        ),
    )
    .unwrap();
    std::fs::write(steamapps.join("appmanifest_570.acf"), APPMANIFEST_DOTA).unwrap();
    std::fs::write(steamapps.join("appmanifest_1810770.acf"), APPMANIFEST_CAIRN).unwrap();
    std::fs::write(
        steamapps.join("appmanifest_238320.acf"),
        APPMANIFEST_OUTLAST,
    )
    .unwrap();

    let common = steamapps.join("common");
    std::fs::create_dir_all(common.join("dota 2 beta")).unwrap();
    std::fs::write(common.join("dota 2 beta").join("dota2.exe"), [0u8; 256]).unwrap();
    std::fs::create_dir_all(common.join("Cairn")).unwrap();
    std::fs::write(common.join("Cairn").join("Cairn.exe"), [0u8; 256]).unwrap();
    std::fs::create_dir_all(common.join("Outlast")).unwrap();
    std::fs::write(common.join("Outlast").join("OLGame.exe"), [0u8; 256]).unwrap();

    tmp
}

#[test]
fn arrancador_scanner_finds_three_synthetic_games() {
    let tmp = build_synthetic_library();
    let games = scanner::scan_all(Some(tmp.path()));
    assert_eq!(games.len(), 3, "discovered: {games:?}");

    let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
    assert!(names.contains(&"Dota 2"), "Dota 2 missing: {names:?}");
    assert!(names.contains(&"Cairn"), "Cairn missing: {names:?}");
    assert!(names.contains(&"Outlast"), "Outlast missing: {names:?}");

    let ids: Vec<&str> = games.iter().map(|g| g.source_app_id.as_str()).collect();
    assert!(ids.contains(&"570"));
    assert!(ids.contains(&"1810770"));
    assert!(ids.contains(&"238320"));

    // Все игры должны быть source="steam" с непустым install_dir.
    for g in &games {
        assert_eq!(g.source, "steam");
        assert!(
            g.install_dir.exists(),
            "install dir doesn't exist: {:?}",
            g.install_dir
        );
        assert!(g.exe_candidate.is_some(), "exe not picked for {}", g.name);
    }

    // Размеры распарсены.
    let dota = games.iter().find(|g| g.name == "Dota 2").unwrap();
    assert_eq!(dota.install_size_bytes, 30_000_000_000);
}

#[test]
fn arrancador_scanner_returns_empty_when_steam_missing() {
    let tmp = TempDir::new().unwrap();
    // Нет steamapps/ subdir → пустой результат.
    let games = scanner::scan_all(Some(tmp.path()));
    assert!(games.is_empty());
}

#[test]
fn arrancador_scanner_sorted_by_name() {
    let tmp = build_synthetic_library();
    let games = scanner::scan_all(Some(tmp.path()));
    assert_eq!(games[0].name, "Cairn");
    assert_eq!(games[1].name, "Dota 2");
    assert_eq!(games[2].name, "Outlast");
}
