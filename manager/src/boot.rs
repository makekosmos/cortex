//! Starting the Engine, and stopping the one we started.
//!
//! The Manager owns its window, not the Engine's lifetime: an Engine already
//! answering is reused as is (another app may be mid-write), and one we spawn
//! is left to the supervisor — a closed Manager must not take the backend of
//! every other app down with it.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use serde_json::Value;

use crate::engine;

const ATTEMPTS: usize = 120;
/// How long the supervisor may take to bring its core down after `--shutdown`.
const GRACE_ATTEMPTS: usize = 40;
const INTERVAL: Duration = Duration::from_millis(250);

fn engine_exe_name() -> &'static str {
    if cfg!(windows) {
        "mundus-engine.exe"
    } else {
        "mundus-engine"
    }
}

/// Where a dev or packaged Manager finds the Engine: next to itself first (a
/// `cargo build` tree, an unpacked archive), then the installed one.
fn engine_candidates(executable: &Path) -> Vec<PathBuf> {
    let name = engine_exe_name();
    let mut candidates: Vec<PathBuf> = executable
        .parent()
        .into_iter()
        .flat_map(|parent| parent.ancestors().take(5))
        .flat_map(|parent| {
            [
                parent.join(name),
                parent.join("target").join("debug").join(name),
            ]
        })
        .collect();
    if let Some(root) = installed_root() {
        candidates.extend(installed_engine(&root));
    }
    candidates
}

/// `%LOCALAPPDATA%\Mundus\Engine`, where the installer keeps its versions.
fn installed_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|local| !local.is_empty())
        .map(|local| PathBuf::from(local).join("Mundus").join("Engine"))
}

/// The installer writes `<root>/current.json` (schema 1) naming the version it
/// points at; only a plain version segment is followed, so a hand-edited
/// pointer cannot walk out of `versions/`.
fn installed_engine(root: &Path) -> Option<PathBuf> {
    let pointer: Value =
        serde_json::from_slice(&std::fs::read(root.join("current.json")).ok()?).ok()?;
    if pointer.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let version = pointer.get("version").and_then(Value::as_str)?;
    if version.is_empty()
        || version.starts_with('.')
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-'))
    {
        return None;
    }
    Some(root.join("versions").join(version).join(engine_exe_name()))
}

/// Whether an Engine answers `/v1/health` with `ready` right now.
fn health_ready() -> bool {
    let Some(directory) = engine::data_dir() else {
        return false;
    };
    let Ok(bytes) = std::fs::read(directory.join("engine.lock.json")) else {
        return false;
    };
    let Ok(lock) = serde_json::from_slice::<Value>(&bytes) else {
        return false;
    };
    let Some(port) = lock["http_port"]
        .as_u64()
        .filter(|port| *port > 0 && *port <= u16::MAX as u64)
    else {
        return false;
    };
    let Some(token) = lock["auth_token"]
        .as_str()
        .filter(|token| token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
    else {
        return false;
    };
    if lock["format_version"] != 1 || lock["api_version"]["major"] != 1 {
        return false;
    }
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(1))
        .redirects(0)
        .build()
        .get(&format!("http://127.0.0.1:{port}/v1/health"))
        .set("Authorization", &format!("Bearer {token}"))
        .set("X-Kosmos-Api-Version", "1.0.0")
        .set(
            crate::identity::CLIENT_CLASS_HEADER,
            crate::identity::CLIENT_CLASS,
        )
        .set("X-Kosmos-Client-Version", env!("CARGO_PKG_VERSION"))
        .set("X-Kosmos-Client-Pid", &std::process::id().to_string())
        .call();
    response
        .ok()
        .and_then(|response| response.into_json::<Value>().ok())
        .is_some_and(|value| value["ok"] == true && value["status"] == "ready")
}

/// Waits for `health` to turn true, starting the Engine once in between.
fn wait_ready(
    mut health: impl FnMut() -> bool,
    start: impl FnOnce() -> Result<(), String>,
    mut pause: impl FnMut(),
    attempts: usize,
) -> Result<(), String> {
    if health() {
        return Ok(());
    }
    start()?;
    for _ in 0..attempts {
        pause();
        if health() {
            return Ok(());
        }
    }
    Err("Engine не подтвердил готовность. Проверьте его журнал и повторите запуск.".into())
}

/// Starts the Engine when none answers, and returns once it reports ready.
pub fn ensure_engine() -> Result<(), String> {
    wait_ready(
        health_ready,
        || {
            // After a Stop the Engine has deleted `engine.lock.json`, so the
            // dir is found by the rules the Engine itself uses, not by the lock.
            let directory = engine::data_home()
                .ok_or_else(|| "Не найден каталог данных Engine.".to_string())?;
            let executable = std::env::current_exe().map_err(|error| error.to_string())?;
            let path = match std::env::var_os("MUNDUS_ENGINE_PATH") {
                Some(path) => PathBuf::from(path),
                None => engine_candidates(&executable)
                    .into_iter()
                    .find(|path| path.is_file())
                    .ok_or_else(|| {
                        concat!(
                            "Engine не запущен, а его бинарник не найден. ",
                            "Запустите Engine или задайте MUNDUS_ENGINE_PATH."
                        )
                        .to_string()
                    })?,
            };
            let logs = directory.join("logs");
            std::fs::create_dir_all(&logs).map_err(|error| error.to_string())?;
            let log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(logs.join("manager-engine-start.log"))
                .map_err(|error| error.to_string())?;
            let stderr = log.try_clone().map_err(|error| error.to_string())?;
            let mut command = Command::new(&path);
            command
                // The supervisor mode the Run entry uses; no-args defaults to
                // it too, and the flag says so out loud.
                .arg("--start")
                .env("MUNDUS_DATA_DIR", &directory)
                .stdin(Stdio::null())
                .stdout(Stdio::from(log))
                .stderr(Stdio::from(stderr));
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                // Separate process group, no console of its own.
                command.creation_flags(0x00000200 | 0x08000000);
            }
            // Dropping the child does not end it: the Engine outlives the
            // window on purpose.
            command.spawn().map_err(|error| {
                format!("Не удалось запустить Engine ({}): {error}", path.display())
            })?;
            Ok(())
        },
        || std::thread::sleep(INTERVAL),
        ATTEMPTS,
    )
}

/// The Engine binary this Manager would start, if it can find one.
fn engine_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MUNDUS_ENGINE_PATH") {
        return Some(PathBuf::from(path));
    }
    let executable = std::env::current_exe().ok()?;
    engine_candidates(&executable)
        .into_iter()
        .find(|path| path.is_file())
}

/// Stops the Engine through the supervisor's own `--shutdown`, whichever
/// process started it: the Manager never kills a backend another app is using.
/// Waits for the supervisor to acknowledge (and for the lock to go quiet).
pub fn stop_engine() -> Result<(), String> {
    let path = engine_path().ok_or_else(|| {
        "Бинарник Engine не найден: остановить сервер из Manager нельзя.".to_string()
    })?;
    let directory = engine::data_home();
    let mut command = Command::new(&path);
    command
        .arg("--shutdown")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(directory) = directory {
        command.env("MUNDUS_DATA_DIR", directory);
    }
    let status = command
        .status()
        .map_err(|error| format!("Не удалось попросить Engine остановиться: {error}"))?;
    if !status.success() {
        return Err("Engine отказался останавливаться (см. его журнал).".to_string());
    }
    // The supervisor answers before its core is gone: wait for the health
    // surface to stop answering, up to the supervisor's own grace period.
    for _ in 0..(GRACE_ATTEMPTS) {
        if !health_ready() {
            return Ok(());
        }
        std::thread::sleep(INTERVAL);
    }
    Err("Engine принял запрос, но продолжает отвечать. Проверьте его журнал.".to_string())
}

/* (running_pid removed: the shutdown flag is the supported way to stop it) */

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn a_healthy_engine_is_reused_without_spawning() {
        assert!(wait_ready(
            || true,
            || panic!("must not start a second Engine"),
            || {},
            3
        )
        .is_ok());
    }

    #[test]
    fn the_gate_waits_for_health_not_for_a_successful_spawn() {
        let polls = Cell::new(0);
        let starts = Cell::new(0);
        let result = wait_ready(
            || {
                polls.set(polls.get() + 1);
                polls.get() == 4
            },
            || {
                starts.set(starts.get() + 1);
                Ok(())
            },
            || {},
            5,
        );
        assert!(result.is_ok());
        assert_eq!(starts.get(), 1);
        assert_eq!(polls.get(), 4);
    }

    #[test]
    fn an_engine_that_never_answers_never_passes_the_gate() {
        assert!(wait_ready(|| false, || Ok(()), || {}, 3).is_err());
        assert_eq!(
            wait_ready(
                || false,
                || Err("spawn failed".into()),
                || panic!("no wait"),
                3
            ),
            Err("spawn failed".into())
        );
    }

    #[test]
    fn a_pointer_to_a_stray_version_is_ignored() {
        let root = std::env::temp_dir().join(format!("manager-boot-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for version in ["../escape", ".hidden", "/absolute", ""] {
            let pointer = serde_json::json!({ "schema_version": 1, "version": version });
            std::fs::write(root.join("current.json"), pointer.to_string()).unwrap();
            assert!(
                installed_engine(&root).is_none(),
                "{version} must not resolve"
            );
        }
        let good = serde_json::json!({ "schema_version": 1, "version": "0.10.26" });
        std::fs::write(root.join("current.json"), good.to_string()).unwrap();
        assert!(installed_engine(&root).is_some_and(|path| path.ends_with(
            Path::new("versions")
                .join("0.10.26")
                .join(engine_exe_name())
        )));
        let _ = std::fs::remove_dir_all(&root);
    }
}
