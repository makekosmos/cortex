// Game launcher.
//
// Резолвит typed Game DTO → способ запуска:
//   * source == "steam" + source_app_id → `steam://rungameid/<id>` через
//     платформенный ShellExecute adapter.
//   * source == "epic" | "gog" | "manual" + trusted exe_path → spawn напрямую
//     с cwd = install_dir.
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
    /// ShellExecuteW launches through the Windows shell broker and does not
    /// expose the child process PID.
    pub pid: Option<u32>,
    pub started_at: String,
    pub method: String, // "steam_url" | "direct_exe"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchCommand {
    SteamUrl(String),
    DirectExe {
        program: std::path::PathBuf,
        cwd: std::path::PathBuf,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("local launcher state missing 'source'")]
    MissingSource,
    #[error("local launcher state missing source_app_id")]
    MissingSteamAppId,
    #[error("local launcher state missing exe_path")]
    MissingExePath,
    #[error("invalid Steam AppID")]
    InvalidSteamAppId,
    #[error("invalid provider launcher id")]
    InvalidProviderId,
    #[error("unsupported launcher source")]
    UnsupportedSource,
    #[error("invalid local launcher state: {0}")]
    InvalidLocalState(&'static str),
    #[error("Steam protocol launch failed: {0}")]
    ShellExecuteFailed(String),
    #[error("spawn failed: {0}")]
    SpawnFailed(#[from] std::io::Error),
}

impl LaunchError {
    pub fn quarantine_code(&self) -> Option<&'static str> {
        match self {
            Self::MissingSource => Some("missing-source"),
            Self::MissingSteamAppId => Some("missing-steam-app-id"),
            Self::MissingExePath => Some("missing-exe-path"),
            Self::InvalidSteamAppId => Some("invalid-steam-app-id"),
            Self::InvalidProviderId => Some("invalid-provider-id"),
            Self::UnsupportedSource => Some("unsupported-source"),
            Self::InvalidLocalState(reason) => Some(reason),
            Self::ShellExecuteFailed(_) | Self::SpawnFailed(_) => None,
        }
    }
}

/// Резолвит запуск без spawn'а. Возвращает typed plan, а не shell command string.
/// Чистая функция — тестируется без процесс-spawn'а.
pub fn resolve_launch_command(
    _game: &LaunchGame,
    local: &LocalGameState,
) -> Result<LaunchCommand, LaunchError> {
    let source = local.source.as_deref().ok_or(LaunchError::MissingSource)?;

    match source {
        "steam" => {
            let app_id = local
                .source_app_id
                .as_deref()
                .ok_or(LaunchError::MissingSteamAppId)?;
            validate_steam_app_id(app_id)?;
            Ok(LaunchCommand::SteamUrl(format!(
                "steam://rungameid/{app_id}"
            )))
        }
        "epic" | "gog" | "manual" => {
            let (program, cwd) = trusted_direct_executable(local)?;
            if matches!(source, "epic" | "gog") {
                let provider_id = local
                    .source_app_id
                    .as_deref()
                    .ok_or(LaunchError::InvalidProviderId)?;
                validate_provider_id(provider_id)?;
            }
            Ok(LaunchCommand::DirectExe { program, cwd })
        }
        _ => Err(LaunchError::UnsupportedSource),
    }
}

fn validate_steam_app_id(app_id: &str) -> Result<(), LaunchError> {
    if app_id.is_empty()
        || app_id.len() > 10
        || !app_id.bytes().all(|byte| byte.is_ascii_digit())
        || app_id.parse::<u32>().ok().filter(|id| *id > 0).is_none()
    {
        return Err(LaunchError::InvalidSteamAppId);
    }
    Ok(())
}

fn validate_provider_id(provider_id: &str) -> Result<(), LaunchError> {
    if provider_id.is_empty()
        || provider_id.len() > 256
        || !provider_id.is_ascii()
        || provider_id.chars().any(char::is_control)
    {
        return Err(LaunchError::InvalidProviderId);
    }
    Ok(())
}

fn trusted_direct_executable(
    local: &LocalGameState,
) -> Result<(std::path::PathBuf, std::path::PathBuf), LaunchError> {
    let exe_path = local
        .exe_path
        .as_deref()
        .ok_or(LaunchError::MissingExePath)?;
    let install_dir = local
        .install_dir
        .as_deref()
        .ok_or(LaunchError::InvalidLocalState("missing-install-dir"))?;
    let exe_path = std::path::Path::new(exe_path);
    let install_dir = std::path::Path::new(install_dir);
    if !exe_path.is_absolute() || !install_dir.is_absolute() {
        return Err(LaunchError::InvalidLocalState("paths-must-be-absolute"));
    }
    if exe_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
    {
        return Err(LaunchError::InvalidLocalState("executable-must-be-exe"));
    }

    let install_dir = std::fs::canonicalize(install_dir)
        .map_err(|_| LaunchError::InvalidLocalState("install-dir-not-found"))?;
    if !install_dir.is_dir() {
        return Err(LaunchError::InvalidLocalState("install-dir-not-directory"));
    }
    let exe_path = std::fs::canonicalize(exe_path)
        .map_err(|_| LaunchError::InvalidLocalState("executable-not-found"))?;
    if !exe_path.is_file() {
        return Err(LaunchError::InvalidLocalState("executable-not-file"));
    }
    if !exe_path.starts_with(&install_dir) {
        return Err(LaunchError::InvalidLocalState(
            "executable-outside-install-dir",
        ));
    }
    Ok((exe_path, install_dir))
}

pub fn launch(game: &LaunchGame, local: &LocalGameState) -> Result<LaunchResult, LaunchError> {
    let source = local.source.as_deref().unwrap_or("unknown");
    let plan = match resolve_launch_command(game, local) {
        Ok(plan) => plan,
        Err(error) => {
            tracing::warn!(
                target: "arrancador.launch",
                game_id = %game.id,
                source,
                method = "validation",
                result = "rejected",
                error = %error,
                "game launch"
            );
            return Err(error);
        }
    };
    let method = match &plan {
        LaunchCommand::SteamUrl(_) => "steam_url",
        LaunchCommand::DirectExe { .. } => "direct_exe",
    };
    let pid = match plan {
        LaunchCommand::SteamUrl(url) => {
            let app_id = url
                .strip_prefix("steam://rungameid/")
                .ok_or(LaunchError::InvalidSteamAppId)?;
            launch_steam_url_with(app_id, invoke_steam_url)
        }
        LaunchCommand::DirectExe { program, cwd } => {
            let current_program = std::fs::canonicalize(&program)
                .map_err(|_| LaunchError::InvalidLocalState("executable-changed-before-launch"))?;
            if current_program != program || !current_program.is_file() {
                return Err(LaunchError::InvalidLocalState(
                    "executable-changed-before-launch",
                ));
            }
            Ok(Some(
                std::process::Command::new(current_program)
                    .current_dir(cwd)
                    .spawn()?
                    .id(),
            ))
        }
    };
    match pid {
        Ok(pid) => {
            tracing::info!(
                target: "arrancador.launch",
                game_id = %game.id,
                source,
                method,
                result = "started",
                pid = ?pid,
                "game launch"
            );
            Ok(LaunchResult {
                pid,
                started_at: chrono::Utc::now().to_rfc3339(),
                method: method.to_string(),
            })
        }
        Err(error) => {
            tracing::warn!(
                target: "arrancador.launch",
                game_id = %game.id,
                source,
                method,
                result = "failed",
                error = %error,
                "game launch"
            );
            Err(error)
        }
    }
}

fn launch_steam_url_with<F>(app_id: &str, invoke: F) -> Result<Option<u32>, LaunchError>
where
    F: FnOnce(&str) -> Result<Option<u32>, LaunchError>,
{
    validate_steam_app_id(app_id)?;
    invoke(&format!("steam://rungameid/{app_id}"))
}

fn invoke_steam_url(url: &str) -> Result<Option<u32>, LaunchError> {
    #[cfg(windows)]
    {
        crate::app_index::platform::windows::shell_execute_open(url)
            .map_err(|error| LaunchError::ShellExecuteFailed(error.to_string()))?;
        // ShellExecuteW is an async broker and does not provide the child PID.
        Ok(None)
    }
    #[cfg(not(windows))]
    {
        Ok(Some(
            std::process::Command::new("xdg-open")
                .arg(url)
                .spawn()?
                .id(),
        ))
    }
}

#[cfg(test)]
mod tests;
