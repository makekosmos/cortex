//! Engine autostart on macOS is a per-user LaunchAgent:
//! `~/Library/LaunchAgents/<label>.plist` running `<exe> --start` at login.
//! launchd reads the directory at the next login, so toggling never has to
//! `launchctl bootstrap` a second supervisor next to the running one.

use std::path::{Path, PathBuf};

use crate::brand::AUTOSTART_LAUNCH_AGENT_LABEL;

fn launch_agents_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(|home| Path::new(&home).join("Library/LaunchAgents"))
        .ok_or_else(|| "HOME is not set".to_string())
}

fn plist_path(dir: &Path) -> PathBuf {
    dir.join(format!("{AUTOSTART_LAUNCH_AGENT_LABEL}.plist"))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// KeepAlive stays off: the supervisor restarts its own core worker, and a
/// user who quits the Engine must not see launchd respawn it.
fn render_plist(executable: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{exe}</string>
		<string>{start}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<false/>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
</dict>
</plist>
"#,
        label = AUTOSTART_LAUNCH_AGENT_LABEL,
        exe = xml_escape(&executable.display().to_string()),
        start = crate::engine_supervisor::START_ARG,
    )
}

fn enabled_in(dir: &Path) -> bool {
    plist_path(dir).is_file()
}

fn set_in(dir: &Path, executable: &Path, enabled: bool) -> Result<(), String> {
    let path = plist_path(dir);
    if enabled {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        std::fs::write(&path, render_plist(executable)).map_err(|e| e.to_string())
    } else {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

pub(super) fn enabled() -> bool {
    launch_agents_dir().is_ok_and(|dir| enabled_in(&dir))
}

pub(super) fn set(enabled: bool) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    set_in(&launch_agents_dir()?, &executable, enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabling_writes_a_login_agent_that_starts_the_supervisor() {
        let dir = tempfile::tempdir().unwrap();
        let agents = dir.path().join("LaunchAgents");
        assert!(!enabled_in(&agents));

        set_in(
            &agents,
            Path::new("/Applications/A & B.app/Contents/MacOS/mundus-engine"),
            true,
        )
        .unwrap();

        assert!(enabled_in(&agents));
        let plist = std::fs::read_to_string(plist_path(&agents)).unwrap();
        assert!(plist.contains("<string>com.kazui.mundus.engine</string>"));
        assert!(plist.contains("<string>/Applications/A &amp; B.app/Contents/MacOS/mundus-engine</string>\n\t\t<string>--start</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(plist.contains("<key>KeepAlive</key>\n\t<false/>"));
        let lint = std::process::Command::new("plutil")
            .arg("-lint")
            .arg(plist_path(&agents))
            .output()
            .unwrap();
        assert!(
            lint.status.success(),
            "{}",
            String::from_utf8_lossy(&lint.stdout)
        );
    }

    #[test]
    fn disabling_removes_the_agent_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let exe = Path::new("/x/mundus-engine");
        set_in(dir.path(), exe, true).unwrap();

        set_in(dir.path(), exe, false).unwrap();
        set_in(dir.path(), exe, false).unwrap();

        assert!(!enabled_in(dir.path()));
        assert!(!plist_path(dir.path()).exists());
    }
}
