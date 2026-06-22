// Game launcher.
//
// Резолвит ArkObject (game_obj) → способ запуска:
//   * source == "steam" + source_app_id → `steam://rungameid/<id>` через ShellExecute
//     (на Windows — `cmd /c start <url>`, на других платформах — TODO).
//   * exe_path задан → spawn напрямую с cwd = install_dir.
//   * иначе — error.
//
// Hook в usage_tracker: пока fire-and-forget. Tracker уже polls running processes
// и сам отметит usage_session по имени exe. Прямая game_id → pid привязка —
// TODO follow-up (требует registry shared между tracker'ом и launcher'ом).

use ark_core::types::ArkObject;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub pid: u32,
    pub started_at: String,
    pub method: String, // "steam_url" | "direct_exe"
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("game_obj missing 'source' in propsJson")]
    MissingSource,
    #[error("steam source missing source_app_id in propsJson")]
    MissingSteamAppId,
    #[error("non-steam source missing exe_path in propsJson")]
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
pub fn resolve_launch_command(game: &ArkObject) -> Result<LaunchCommand, LaunchError> {
    let props = &game.props_json;
    let source = props
        .get("source")
        .and_then(|v| v.as_str())
        .ok_or(LaunchError::MissingSource)?;

    if source == "steam" {
        let app_id = props
            .get("source_app_id")
            .and_then(|v| v.as_str())
            .or_else(|| props.get("sourceAppId").and_then(|v| v.as_str()))
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
    let exe_path = props
        .get("exe_path")
        .and_then(|v| v.as_str())
        .or_else(|| props.get("exePath").and_then(|v| v.as_str()))
        .ok_or(LaunchError::MissingExePath)?;
    let cwd = props
        .get("install_dir")
        .and_then(|v| v.as_str())
        .or_else(|| props.get("installDir").and_then(|v| v.as_str()))
        .map(std::path::PathBuf::from);
    Ok((exe_path.to_string(), Vec::new(), cwd, "direct_exe"))
}

pub fn launch(game: &ArkObject) -> Result<LaunchResult, LaunchError> {
    let (program, args, cwd, method) = resolve_launch_command(game)?;
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

    fn make_game(props: serde_json::Value) -> ArkObject {
        ArkObject {
            id: "g1".into(),
            type_id: "game_obj".into(),
            title: "Test".into(),
            content_json: json!({}),
            props_json: props,
            created_at: "2026-05-18T00:00:00Z".into(),
            updated_at: "2026-05-18T00:00:00Z".into(),
            deleted_at: None,
        }
    }

    #[test]
    fn launcher_steam_url_format() {
        let game = make_game(json!({
            "source": "steam",
            "source_app_id": "570",
        }));
        let (program, args, _cwd, method) = resolve_launch_command(&game).unwrap();
        assert_eq!(method, "steam_url");
        let joined = format!("{program} {}", args.join(" "));
        assert!(
            joined.contains("steam://rungameid/570"),
            "command should contain steam URL: {joined}"
        );
    }

    #[test]
    fn launcher_accepts_camelcase_app_id() {
        let game = make_game(json!({
            "source": "steam",
            "sourceAppId": "238320",
        }));
        let (_program, args, _cwd, _) = resolve_launch_command(&game).unwrap();
        assert!(args.iter().any(|a| a.contains("238320")));
    }

    #[test]
    fn launcher_direct_exe_resolves() {
        let game = make_game(json!({
            "source": "epic",
            "exe_path": "C:\\Games\\Hades\\Hades.exe",
            "install_dir": "C:\\Games\\Hades",
        }));
        let (program, args, cwd, method) = resolve_launch_command(&game).unwrap();
        assert_eq!(method, "direct_exe");
        assert!(program.contains("Hades.exe"));
        assert!(args.is_empty());
        assert!(cwd.is_some());
    }

    #[test]
    fn launcher_missing_source_errors() {
        let game = make_game(json!({}));
        assert!(matches!(
            resolve_launch_command(&game),
            Err(LaunchError::MissingSource)
        ));
    }

    #[test]
    fn launcher_steam_without_app_id_errors() {
        let game = make_game(json!({ "source": "steam" }));
        assert!(matches!(
            resolve_launch_command(&game),
            Err(LaunchError::MissingSteamAppId)
        ));
    }

    #[test]
    fn launcher_non_steam_without_exe_errors() {
        let game = make_game(json!({ "source": "epic" }));
        assert!(matches!(
            resolve_launch_command(&game),
            Err(LaunchError::MissingExePath)
        ));
    }
}
