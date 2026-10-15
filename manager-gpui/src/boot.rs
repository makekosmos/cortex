//! The Manager owns its window, never the Engine lifetime. A packaged Engine
//! is started without pipes or a kill-on-drop guard, then authenticated health
//! must report ready before a Manager window is created.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use mundus_gpui_kit::engine::data_dir;
use serde_json::Value;

const ATTEMPTS: usize = 120;
const INTERVAL: Duration = Duration::from_millis(250);

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
    Err("Engine не подтвердил готовность. Manager не открыт. Проверьте журнал Engine и повторите запуск.".into())
}

fn engine_exe_name() -> &'static str {
    if cfg!(windows) {
        "mundus-engine.exe"
    } else {
        "mundus-engine"
    }
}

fn engine_candidates(executable: &Path) -> Vec<PathBuf> {
    let name = engine_exe_name();
    executable
        .parent()
        .into_iter()
        .flat_map(|parent| parent.ancestors().take(4))
        .map(|parent| parent.join(name))
        .collect()
}

/// The installed Engine: `<engine_root>/current.json` (schema 1, written by
/// the Engine installer) points at `<engine_root>/versions/<version>/<exe>`.
/// Only the pointer is validated here — callers check `is_file()` like for
/// the sibling candidates.
fn installed_engine(engine_root: &Path) -> Option<PathBuf> {
    let pointer: Value =
        serde_json::from_slice(&std::fs::read(engine_root.join("current.json")).ok()?).ok()?;
    if pointer.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let version = pointer.get("version").and_then(Value::as_str)?;
    // The version becomes a path segment — reject anything that could
    // escape `versions/` or name a different directory.
    if version.is_empty()
        || version.starts_with('.')
        || !version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'))
    {
        return None;
    }
    Some(
        engine_root
            .join("versions")
            .join(version)
            .join(engine_exe_name()),
    )
}

fn health_ready(directory: &Path) -> bool {
    let Some(lock) = std::fs::read(directory.join("engine.lock.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    else {
        return false;
    };
    let Some(port) = lock["http_port"]
        .as_u64()
        .filter(|p| *p > 0 && *p <= u16::MAX as u64)
    else {
        return false;
    };
    let Some(token) = lock["auth_token"]
        .as_str()
        .filter(|t| t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit()))
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
        .set("X-Kosmos-Client-Class", "manager-gpui")
        .set("X-Kosmos-Client-Version", env!("CARGO_PKG_VERSION"))
        .set("X-Kosmos-Client-Pid", &std::process::id().to_string())
        .call();
    response
        .ok()
        .and_then(|r| r.into_json::<Value>().ok())
        .is_some_and(|v| v["ok"] == true && v["status"] == "ready")
}

pub fn ensure_engine() -> Result<(), String> {
    let directory = data_dir().map_err(|e| e.message())?;
    wait_ready(
        || health_ready(&directory),
        || {
            let executable = std::env::current_exe().map_err(|e| e.to_string())?;
            let path = if let Some(path) = std::env::var_os("MUNDUS_ENGINE_PATH") {
                PathBuf::from(path)
            } else {
                engine_candidates(&executable)
                    .into_iter()
                    .find(|p| p.is_file())
                    // Dev/package siblings keep priority; only then the
                    // installed Engine at %LOCALAPPDATA%\Mundus\Engine.
                    .or_else(|| {
                        std::env::var_os("LOCALAPPDATA")
                            .filter(|local| !local.is_empty())
                            .map(|local| PathBuf::from(local).join("Mundus").join("Engine"))
                            .and_then(|root| installed_engine(&root))
                            .filter(|path| path.is_file())
                    })
                    .ok_or_else(|| "Engine не запущен, а его бинарник не найден. Запустите Engine или задайте MUNDUS_ENGINE_PATH.".to_string())?
            };
            let logs = directory.join("logs");
            std::fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
            let log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(logs.join("manager-engine-start.log"))
                .map_err(|e| e.to_string())?;
            let stderr = log.try_clone().map_err(|e| e.to_string())?;
            let mut command = Command::new(&path);
            command
                // Same supervisor mode the Run autostart entry uses (no-args
                // defaults to it as well — the flag is explicit parity).
                .arg("--start")
                .env("MUNDUS_DATA_DIR", &directory)
                .stdin(Stdio::null())
                .stdout(Stdio::from(log))
                .stderr(Stdio::from(stderr));
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                // Separate process group; do not create a console for Engine.
                command.creation_flags(0x00000200 | 0x08000000);
            }
            // Dropping Child does NOT terminate it. In particular, ⌘Q must
            // leave the supervisor/core alive for Agenda and other clients.
            command
                .spawn()
                .map_err(|e| format!("Не удалось запустить Engine ({}): {e}", path.display()))?;
            Ok(())
        },
        || std::thread::sleep(INTERVAL),
        ATTEMPTS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn healthy_engine_is_reused_without_spawning() {
        assert!(wait_ready(
            || true,
            || panic!("must not start a second Engine"),
            || {},
            3
        )
        .is_ok());
    }

    #[test]
    fn ui_gate_waits_for_health_not_just_successful_spawn() {
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
    fn unavailable_engine_never_passes_ui_gate() {
        assert!(wait_ready(|| false, || Ok(()), || {}, 3).is_err());
        assert_eq!(
            wait_ready(
                || false,
                || Err("spawn failed".into()),
                || panic!("no wait after spawn failure"),
                3
            ),
            Err("spawn failed".into())
        );
    }

    /// Fresh temp dir per test — writes current.json/versions trees for
    /// `installed_engine` without touching the real Engine root.
    fn engine_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "manager-gpui-boot-test-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp engine root");
        dir
    }

    fn write_current(root: &Path, body: &str) {
        std::fs::write(root.join("current.json"), body).expect("current.json");
    }

    #[test]
    fn installed_engine_reads_current_pointer() {
        let root = engine_root("valid");
        write_current(&root, r#"{"schema_version":1,"version":"0.10.15"}"#);
        let exe_dir = root.join("versions").join("0.10.15");
        std::fs::create_dir_all(&exe_dir).unwrap();
        let exe = exe_dir.join(engine_exe_name());
        std::fs::write(&exe, b"exe").unwrap();
        assert_eq!(installed_engine(&root), Some(exe));
    }

    #[test]
    fn installed_engine_rejects_bad_pointer() {
        // No current.json at all.
        let root = engine_root("missing");
        assert_eq!(installed_engine(&root), None);
        // Malformed JSON.
        let root = engine_root("badjson");
        write_current(&root, "{not json");
        assert_eq!(installed_engine(&root), None);
        // Pointer schema from another generation.
        let root = engine_root("schema2");
        write_current(&root, r#"{"schema_version":2,"version":"0.10.15"}"#);
        assert_eq!(installed_engine(&root), None);
        for (i, bad) in ["..", "../x", "a\\b", "a/b", "", " "].iter().enumerate() {
            let root = engine_root(&format!("badver-{i}"));
            write_current(
                &root,
                &format!(r#"{{"schema_version":1,"version":"{bad}"}}"#),
            );
            assert_eq!(
                installed_engine(&root),
                None,
                "version {bad:?} must not resolve"
            );
        }
    }

    #[test]
    fn packaged_engine_is_found_beside_manager_or_at_package_root() {
        let candidates = engine_candidates(Path::new(
            "/package/resources/components/manager/manager-gpui",
        ));
        let name = if cfg!(windows) {
            "mundus-engine.exe"
        } else {
            "mundus-engine"
        };
        assert!(candidates.contains(&Path::new("/package/resources/components/manager").join(name)));
        assert!(candidates.contains(&Path::new("/package").join(name)));
    }
}
