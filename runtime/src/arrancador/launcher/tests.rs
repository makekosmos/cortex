use super::*;
use serde_json::json;

fn make_game(_props: serde_json::Value) -> LaunchGame {
    LaunchGame {
        id: "g1".into(),
        title: "Test".into(),
    }
}

#[test]
fn launcher_steam_url_format() {
    let game = make_game(json!({}));
    let local = LocalGameState {
        source: Some("steam".into()),
        source_app_id: Some("570".into()),
        ..Default::default()
    };
    assert_eq!(
        resolve_launch_command(&game, &local).unwrap(),
        LaunchCommand::SteamUrl("steam://rungameid/570".into())
    );
}

#[test]
fn launcher_accepts_camelcase_app_id() {
    let game = make_game(json!({}));
    let local = LocalGameState {
        source: Some("steam".into()),
        source_app_id: Some("238320".into()),
        ..Default::default()
    };
    assert_eq!(
        resolve_launch_command(&game, &local).unwrap(),
        LaunchCommand::SteamUrl("steam://rungameid/238320".into())
    );
}

#[test]
fn launcher_direct_exe_resolves() {
    let game = make_game(json!({}));
    let dir = tempfile::tempdir().unwrap();
    let install_dir = dir.path().join("Hades");
    std::fs::create_dir_all(&install_dir).unwrap();
    let exe_path = install_dir.join("Hades.exe");
    std::fs::write(&exe_path, b"MZ").unwrap();
    let local = LocalGameState {
        source: Some("epic".into()),
        source_app_id: Some("hades".into()),
        exe_path: Some(exe_path.to_string_lossy().into_owned()),
        install_dir: Some(install_dir.to_string_lossy().into_owned()),
        ..Default::default()
    };
    assert!(matches!(
        resolve_launch_command(&game, &local).unwrap(),
        LaunchCommand::DirectExe { .. }
    ));
}

#[test]
fn launcher_missing_source_errors() {
    let game = make_game(json!({}));
    let local = LocalGameState::default();
    assert!(matches!(
        resolve_launch_command(&game, &local),
        Err(LaunchError::MissingSource)
    ));
}

#[test]
fn launcher_steam_without_app_id_errors() {
    let game = make_game(json!({}));
    let local = LocalGameState {
        source: Some("steam".into()),
        ..Default::default()
    };
    assert!(matches!(
        resolve_launch_command(&game, &local),
        Err(LaunchError::MissingSteamAppId)
    ));
}

#[test]
fn launcher_rejects_steam_shell_metacharacters_and_unicode_lookalikes() {
    for app_id in [
        "",
        "0",
        "1&whoami",
        "1|whoami",
        "1<",
        "1>",
        "1(",
        "1)",
        "1%PATH%",
        "1!PATH!",
        "1^",
        "1\"",
        "1'",
        "1 2",
        "1\t2",
        "１２３",
        "١٢٣",
        "12345678901",
    ] {
        assert!(matches!(
            validate_steam_app_id(app_id),
            Err(LaunchError::InvalidSteamAppId)
        ));
    }
    assert!(validate_steam_app_id("1").is_ok());
    assert!(validate_steam_app_id("4294967295").is_ok());
}

#[test]
fn steam_adapter_receives_exact_native_url_without_spawning() {
    let mut received = None;
    let pid = launch_steam_url_with("570", |url| {
        received = Some(url.to_owned());
        Ok(42)
    })
    .unwrap();
    assert_eq!(pid, 42);
    assert_eq!(received.as_deref(), Some("steam://rungameid/570"));
}

#[test]
fn direct_executable_must_stay_inside_install_dir() {
    let install = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let exe = outside.path().join("payload.exe");
    std::fs::write(&exe, b"MZ").unwrap();
    let local = LocalGameState {
        source: Some("manual".into()),
        exe_path: Some(exe.to_string_lossy().into_owned()),
        install_dir: Some(install.path().to_string_lossy().into_owned()),
        ..Default::default()
    };
    assert!(matches!(
        resolve_launch_command(&make_game(json!({})), &local),
        Err(LaunchError::InvalidLocalState(
            "executable-outside-install-dir"
        ))
    ));
}

#[test]
fn launcher_non_steam_without_exe_errors() {
    let game = make_game(json!({}));
    let local = LocalGameState {
        source: Some("epic".into()),
        ..Default::default()
    };
    assert!(matches!(
        resolve_launch_command(&game, &local),
        Err(LaunchError::MissingExePath)
    ));
}
