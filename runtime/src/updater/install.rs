//! Silent installer launch. Windows-only for now: the feed this module reads
//! (`feed::DEFAULT_FEED_BASE`) only ever names an NSIS `.exe`, so there is no
//! macOS/Linux installer to launch here yet — see the PR description for the
//! mac follow-up.
//!
//! The installer is spawned **detached** and this call does not wait for it
//! or terminate the current process: Engine is not the process that owns
//! Mundus's lifetime (Manager + Host are), so who quits and relaunches
//! around the silent install is an integration decision left to the caller
//! (`updater.install` RPC — see `runtime/src/updater/ops.rs` doc comment).
use std::path::Path;

use super::UpdaterError;

#[cfg(windows)]
pub(crate) fn launch_silent_detached(installer_path: &Path) -> Result<(), UpdaterError> {
    use std::os::windows::process::CommandExt;
    // Same no-console guard the rest of the crate uses for spawned Windows
    // processes (see e.g. `manager_api.rs::windows_registry_command`),
    // combined with DETACHED_PROCESS so the installer keeps running after
    // this Engine process exits or is itself replaced by the install.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    std::process::Command::new(installer_path)
        .arg("/S")
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map_err(|error| UpdaterError::Io(error.to_string()))?;
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn launch_silent_detached(_installer_path: &Path) -> Result<(), UpdaterError> {
    Err(UpdaterError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn non_windows_reports_unsupported_platform() {
        let result = launch_silent_detached(Path::new("installer.exe"));
        assert!(matches!(result, Err(UpdaterError::UnsupportedPlatform)));
    }

    #[test]
    #[cfg(windows)]
    fn missing_installer_surfaces_as_io_error() {
        let missing = Path::new("C:/mundus-updater-tests/definitely-missing.exe");
        let result = launch_silent_detached(missing);
        assert!(matches!(result, Err(UpdaterError::Io(_))));
    }
}
