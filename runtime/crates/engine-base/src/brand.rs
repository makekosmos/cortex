//! Single source of truth for the Mundus product identity.
//!
//! All brand strings, install/data paths and OS-level ids that name the
//! product live here so a rename never has to hunt literals through the
//! tree again. Internal code names stay neutral (`engine`, `focus-svc`,
//! `watcher`) — only user/OS-visible identifiers carry the brand.
//!
//! Persisted/legacy identifiers that must keep working (wire contract with
//! already-shipped component builds, on-disk state written by older
//! releases) are listed in `docs/brand-legacy-identifiers.md`.

/// Product name shown to users and used in OS-facing identifiers.
pub const PRODUCT_NAME: &str = "Mundus";

/// Engine binary file stem (`mundus-engine.exe` on Windows).
pub const ENGINE_BINARY_STEM: &str = "mundus-engine";

/// Log/stderr tag emitted by the Engine process.
pub const PROCESS_TAG: &str = "[mundus-engine]";

/// Roaming config root name: `%APPDATA%\Mundus` (Windows),
/// `~/Library/Application Support/Mundus` (macOS),
/// `$XDG_CONFIG_HOME/Mundus` or `~/.config/Mundus` (other Unix).
pub const CONFIG_DIR_NAME: &str = "Mundus";

/// Local data root name: `%LOCALAPPDATA%\Mundus` (Engine installs under
/// `%LOCALAPPDATA%\Mundus\Engine`).
pub const LOCAL_DIR_NAME: &str = "Mundus";

/// Engine install subdir inside the local data root.
pub const ENGINE_DIR_NAME: &str = "Engine";

/// Singleton lock DB file name inside the data dir.
pub const SINGLETON_LOCK_NAME: &str = "mundus-singleton.lock.db";

/// Rolling log file prefix inside `<data_dir>/logs/`.
pub const LOG_FILE_PREFIX: &str = "mundus-engine";

/// HKCU `...\Run` value name for Engine autostart.
pub const AUTOSTART_RUN_VALUE: &str = "Mundus Engine";

/// launchd label (and `~/Library/LaunchAgents/<label>.plist` stem) for
/// Engine autostart on macOS.
pub const AUTOSTART_LAUNCH_AGENT_LABEL: &str = "com.kazui.mundus.engine";

/// HKCU Uninstall key name (`...\Uninstall\Mundus`).
pub const UNINSTALL_KEY_NAME: &str = "Mundus";

/// Windows service name for the privileged system service.
pub const SYSTEM_SERVICE_NAME: &str = "MundusSystemSvc";

/// Named pipe the privileged system service listens on.
pub const SYSTEM_SERVICE_PIPE: &str = r"\\.\pipe\mundus-system-service";

/// Wire header naming the calling client's role. Neutral on purpose: the
/// product may be renamed again, and the wire should not have to follow.
pub const CLIENT_CLASS_HEADER: &str = "x-client-class";

/// The pre-rename class header, still accepted from already-shipped clients
/// (Atlas, agenda, memoria, dictation, imago's kit). Persisted contract — see
/// `docs/brand-legacy-identifiers.md`.
pub const LEGACY_CLIENT_CLASS_HEADER: &str = "x-kosmos-client-class";

/// Classes that speak for the Manager, latest first. A class is a role, not a
/// framework: `manager` is what the new Manager sends, `manager-gpui` is what
/// the one it replaces sends, and the Engine honours both until the old
/// Manager is gone.
pub const MANAGER_CLIENT_CLASSES: &[&str] = &["manager", "mundus-manager", "manager-gpui"];

/// Whether a client class may perform Manager-only operations.
pub fn is_manager_client(class: Option<&str>) -> bool {
    class.is_some_and(|class| MANAGER_CLIENT_CLASSES.contains(&class))
}

/// OS keyring (Windows Credential Manager) service holding user API keys
/// and package integration secrets. Persisted legacy identifier: renaming it
/// orphans every stored credential. See docs/brand-legacy-identifiers.md.
pub const KEYRING_SERVICE: &str = "kosmos-kepler";

/// New environment-variable prefix for everything the product reads.
pub const ENV_PREFIX: &str = "MUNDUS_";

/// Read an environment variable by suffix, trying the current `MUNDUS_`
/// prefix first and the legacy `KOSMOS_`/`KEPLER_` prefixes as fallbacks.
/// The legacy fallbacks exist because users/devs may still have the old
/// names exported; they do not change on-disk state.
// MIGRATION(KOS-267): remove the legacy-prefix fallbacks after 2026-11-01.
pub fn env(suffix: &str) -> Option<String> {
    env_os(suffix).and_then(|v| v.into_string().ok())
}

/// [`env`] for values that may not be valid UTF-8.
pub fn env_os(suffix: &str) -> Option<std::ffi::OsString> {
    for prefix in [ENV_PREFIX, "KOSMOS_", "KEPLER_"] {
        if let Some(value) = std::env::var_os(format!("{prefix}{suffix}")) {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}
