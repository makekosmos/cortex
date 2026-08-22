// Game launcher.
//
// Резолвит typed Game DTO → способ запуска:
//   * source == "steam" + source_app_id → `steam://rungameid/<id>` через ShellExecute
//     (на Windows — `cmd /c start <url>`, на других платформах — TODO).
//   * exe_path задан → spawn напрямую с cwd = install_dir.
//   * иначе — error.
//
// Hook в usage_tracker: пока fire-and-forget. Tracker уже polls running processes
// и сам отметит usage_session по имени exe. Прямая game_id → pid привязка —
// TODO follow-up (требует registry shared между tracker'ом и launcher'ом).

use ark_core::canonical_types::game::GameLocalState;
use serde::Serialize;

pub type LocalGameState = GameLocalState;

#[derive(Debug, Clone)]
pub struct LaunchGame {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub pid: u32,
    pub started_at: String,
    pub method: String, // "steam_url" | "direct_exe"
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("local launcher state missing 'source'")]
    MissingSource,
    #[error("local launcher state missing source_app_id")]
    MissingSteamAppId,
    #[error("local launcher state missing exe_path")]
    MissingExePath,
    #[error("spawn failed: {0}")]
    SpawnFailed(#[from] std::io::Error),
}

type LaunchCommand = (
    String,
    Vec<String>,
    Option<std::path::PathBuf>,
    &'static str,
);

/// Резолвит команду без spawn'а. Возвращает (program, args, cwd_optional, method).
/// Чистая функция — тестируется без процесс-spawn'а.
pub fn resolve_launch_command(
    _game: &LaunchGame,
    local: &LocalGameState,
) -> Result<LaunchCommand, LaunchError> {
    let source = local.source.as_deref().ok_or(LaunchError::MissingSource)?;

    if source == "steam" {
        let app_id = local
            .source_app_id
            .as_deref()
            .ok_or(LaunchError::MissingSteamAppId)?;
        let url = format!("steam://rungameid/{app_id}");
        #[cfg(windows)]
        {
            return Ok((
                "cmd".to_string(),
                vec!["/c".into(), "start".into(), "".into(), url],
                None,
                "steam_url",
            ));
        }
        #[cfg(not(windows))]
        {
            return Ok(("xdg-open".to_string(), vec![url], None, "steam_url"));
        }
    }

    // Direct exe path.
    let exe_path = local.exe_path.as_ref().ok_or(LaunchError::MissingExePath)?;
    let cwd = local.install_dir.as_ref().map(std::path::PathBuf::from);
    Ok((exe_path.clone(), Vec::new(), cwd, "direct_exe"))
}

pub fn launch(game: &LaunchGame, local: &LocalGameState) -> Result<LaunchResult, LaunchError> {
    let (program, args, cwd, method) = resolve_launch_command(game, local)?;
    let mut cmd = std::process::Command::new(&program);
    cmd.args(&args);
    if let Some(dir) = cwd.as_ref() {
        if dir.exists() {
            cmd.current_dir(dir);
        }
    }
    let child = cmd.spawn()?;
    Ok(LaunchResult {
        pid: child.id(),
        started_at: chrono::Utc::now().to_rfc3339(),
        method: method.to_string(),
    })
}

#[cfg(test)]
mod tests {
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
        let (program, args, _cwd, method) = resolve_launch_command(&game, &local).unwrap();
        assert_eq!(method, "steam_url");
        let joined = format!("{program} {}", args.join(" "));
        assert!(
            joined.contains("steam://rungameid/570"),
            "command should contain steam URL: {joined}"
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
        let (_program, args, _cwd, _) = resolve_launch_command(&game, &local).unwrap();
        assert!(args.iter().any(|a| a.contains("238320")));
    }

    #[test]
    fn launcher_direct_exe_resolves() {
        let game = make_game(json!({}));
        let local = LocalGameState {
            source: Some("epic".into()),
            exe_path: Some("C:\\Games\\Hades\\Hades.exe".into()),
            install_dir: Some("C:\\Games\\Hades".into()),
            ..Default::default()
        };
        let (program, args, cwd, method) = resolve_launch_command(&game, &local).unwrap();
        assert_eq!(method, "direct_exe");
        assert!(program.contains("Hades.exe"));
        assert!(args.is_empty());
        assert!(cwd.is_some());
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
}
