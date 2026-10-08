// Dictation's process lifecycle is Engine-owned: an installed
// `com.kosmos.dictation` means a running background app. The Engine
// launches it with `--background` (no window — the status window is a
// disposable view, dictation itself runs windowless), stops it before
// install/uninstall so the Store's running-app refusal never deadlocks the
// Engine's own update, and relaunches after every install attempt — the
// pointer flip keeps the previous version live until the new one verifies,
// so `ensure` after a failure restores the old version rather than leaving
// dictation dead.
//
// The app self-dedupes with its single-instance mutex, so a redundant spawn
// is cheap and harmless; the running-probe exists only to skip that waste.

const DICTATION_APP_ID: &str = "com.kosmos.dictation";

/// Wait budget for the app to exit after terminate — long enough for a
/// normal exit, short enough that a wedged process still fails the
/// mutation instead of hanging the job.
const DICTATION_STOP_TIMEOUT: Duration = Duration::from_secs(10);
const DICTATION_STOP_POLL: Duration = Duration::from_millis(100);

impl PackageService {
    /// Launch the installed dictation app in background mode. Not installed
    /// or already running → no-op. Best-effort: lifecycle must never break
    /// the caller (Engine start, install completion).
    pub fn ensure_dictation_running(&self) {
        self.ensure_dictation_running_with(spawn_dictation_background);
    }

    fn ensure_dictation_running_with(&self, spawn: impl FnOnce(&Path) -> std::io::Result<()>) {
        let Some(desc) = crate::native_apps::app_descriptor(DICTATION_APP_ID) else {
            return;
        };
        let Ok(store) = self.native_store() else {
            return;
        };
        // A probe error must not decide "not running" — the app's own
        // single-instance mutex dedupes a redundant spawn anyway.
        if matches!(store.app_is_running(desc.id), Ok(true)) {
            return;
        }
        let executable = match self.native_app_executable(desc) {
            Ok(path) => path,
            // Not installed — nothing to run.
            Err(_) => return,
        };
        if let Err(error) = spawn(&executable) {
            tracing::warn!(
                target: "native_apps",
                executable = %executable.display(),
                %error,
                "dictation background launch failed"
            );
        }
    }

    /// Жив ли background-процесс dictation app? Используется trigger
    /// fallback'ом в engine main: пока приложение запущено, оно владеет
    /// обработкой `dictation.trigger`; нет процесса — хоткей обслуживает
    /// сам Engine (`handle_engine_trigger`).
    pub fn dictation_app_running(&self) -> bool {
        let Ok(store) = self.native_store() else {
            return false;
        };
        matches!(store.app_is_running(DICTATION_APP_ID), Ok(true))
    }

    /// Stop the running dictation app so the Store can mutate its install.
    /// `false` = still running after the bounded wait — the caller then
    /// reports `app-running` instead of racing a live exe. `true` also when
    /// nothing is installed or running.
    pub fn stop_dictation_app(&self) -> bool {
        self.stop_dictation_app_with(terminate_dictation_process, DICTATION_STOP_TIMEOUT)
    }

    fn stop_dictation_app_with(
        &self,
        terminate: impl FnOnce(&Path) -> std::io::Result<()>,
        timeout: Duration,
    ) -> bool {
        let Ok(store) = self.native_store() else {
            return true;
        };
        match store.app_is_running(DICTATION_APP_ID) {
            // An unreadable record can't tell us what is live — don't kill
            // blind and don't proceed as if stopped.
            Err(_) => return false,
            Ok(false) => return true,
            Ok(true) => {}
        }
        let Some(executable) = store.executable_path(DICTATION_APP_ID) else {
            // Record exists but the exe is gone — nothing can be running.
            return true;
        };
        if terminate(&executable).is_err() {
            return false;
        }
        let deadline = Instant::now() + timeout;
        loop {
            match store.app_is_running(DICTATION_APP_ID) {
                Ok(false) => return true,
                Ok(true) if Instant::now() < deadline => std::thread::sleep(DICTATION_STOP_POLL),
                _ => return false,
            }
        }
    }

    /// `run_native_install_inner` under lifecycle ownership. After the
    /// attempt — success or failure — the currently installed version runs
    /// again; on success that is the new one.
    async fn run_dictation_native_install(
        &self,
        probe: &ReleaseProbe,
        desc: &'static NativeAppDescriptor,
        version: Option<&str>,
    ) -> Result<NativeAppSummary, PackageError> {
        if !self.stop_dictation_app() {
            return Err(PackageError::AppRunning);
        }
        let result = self.run_native_install_inner(probe, desc, version).await;
        self.ensure_dictation_running();
        result
    }
}

/// `<exe> --background`, detached with no console window — mirrors
/// `open_native_app`'s spawn, plus the background flag. Never compiled into
/// test builds: unit tests simulate "running" via a file lock, and a real
/// spawn/kill could hit a developer's live dictation process.
#[cfg(all(windows, not(test)))]
fn spawn_dictation_background(executable: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new(executable)
        .arg("--background")
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

/// Image-name termination — the same `taskkill /IM` the installer already
/// uses for this app (`desktop/build/installer.nsi`). `/F` is required: in
/// background mode the app owns no window, so a graceful WM_CLOSE pass
/// would leave the process running.
#[cfg(all(windows, not(test)))]
fn terminate_dictation_process(executable: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Some(image) = executable.file_name() else {
        return Ok(());
    };
    let status = std::process::Command::new("taskkill.exe")
        .args(["/F", "/IM"])
        .arg(image)
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    // Exit 128 = "process not found" — already gone is the desired state.
    if status.success() || status.code() == Some(128) {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("taskkill exit {status}")))
    }
}

/// `<exe> --background` detached — the unix twin of the Windows spawn. The
/// executable points inside the `.app` bundle; spawning it directly (rather
/// than `open` on the bundle) keeps the Engine's process accounting exact.
/// Process termination stays inert: unix unlink/rename semantics let the
/// installer replace a live tree safely, so `app_is_running` is never true
/// there and `terminate` is unreachable.
#[cfg(all(unix, not(test)))]
fn spawn_dictation_background(executable: &Path) -> std::io::Result<()> {
    std::process::Command::new(executable)
        .arg("--background")
        .spawn()
        .map(|_| ())
}

// Test builds must never spawn or kill real processes, and hosts outside
// windows/unix have no store installs at all — the inert path covers both.
#[cfg(any(test, not(any(windows, unix))))]
fn spawn_dictation_background(_: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "native apps are unsupported here",
    ))
}

#[cfg(any(not(windows), test))]
fn terminate_dictation_process(_: &Path) -> std::io::Result<()> {
    Ok(())
}

// MIGRATION(KOS-267): remove after 2026-11-01
//
// dictation-gpui ≤0.3.x persisted its own sign-in autostart as the HKCU Run
// value `LEGACY_AUTOSTART_VALUE` (`"<exe>" --background`). The Engine-owned
// lifecycle replaces it. Deleting an absent value is a single cheap
// registry call answering ERROR_FILE_NOT_FOUND, so this runs on every
// Engine start without a "done" marker.
#[cfg(all(windows, not(test)))]
const LEGACY_AUTOSTART_VALUE: &str = "KosmosDictation"; // MIGRATION(KOS-267)

/// Remove the app's legacy Run value. Best-effort: a failure is logged and
/// retried on the next Engine start. Inert in test builds so the suite can
/// never touch the developer's real Run key.
#[cfg(all(windows, not(test)))]
pub fn cleanup_legacy_dictation_autostart() {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
    use windows::Win32::System::Registry::{RegDeleteKeyValueW, HKEY_CURRENT_USER};
    let wide = |value: &str| {
        value
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let subkey = wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let name = wide(LEGACY_AUTOSTART_VALUE);
    let status = unsafe {
        RegDeleteKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
        )
    };
    if status != WIN32_ERROR(0) && status != ERROR_FILE_NOT_FOUND {
        tracing::warn!(
            target: "native_apps",
            ?status,
            "legacy dictation autostart delete failed; will retry next start"
        );
    }
}

#[cfg(any(not(windows), test))]
pub fn cleanup_legacy_dictation_autostart() {}
