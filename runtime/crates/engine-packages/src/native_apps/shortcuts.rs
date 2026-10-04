// Engine-owned Start-menu shortcuts for store-installed apps (KOS-265).
//
// Links live under `<Programs>\Mundus\<Name>.lnk` — inside a product
// subfolder — so the installer's legacy cleanup of the flat bundled-era
// names (`$SMPROGRAMS\Agenda.lnk`, `Kosmos Agenda.lnk`) can never delete a
// live Engine-owned link, and the uninstaller drops the folder wholesale.
//
// Lifecycle: install/update writes the link, uninstall removes it, and
// `reconcile_shortcuts` runs on every Engine start so installs made before
// this existed — or links the user deleted — self-heal.

/// The per-user Start-menu `Programs` root: `MUNDUS_START_MENU_DIR` env
/// override first (tests/dev scratch roots), then the real per-user folder.
/// `None` on hosts with no Start menu — callers skip silently.
pub fn start_menu_dir() -> Option<PathBuf> {
    if let Some(dir) = crate::brand::env_os("START_MENU_DIR") {
        let dir = PathBuf::from(dir);
        if !dir.as_os_str().is_empty() {
            return Some(dir);
        }
    }
    programs_folder()
}

/// `<Programs>\Mundus\<display name>.lnk` — the single location a store
/// app's link may occupy. Descriptor names are unique per table.
fn link_path(programs: &Path, desc: &NativeAppDescriptor) -> PathBuf {
    programs
        .join(crate::brand::PRODUCT_NAME)
        .join(format!("{}.lnk", desc.name))
}

/// Sync one app's link to its install record: installed → create or
/// repoint at the current executable; absent → delete. Idempotent — a link
/// already pointing at the recorded exe is left untouched.
pub fn sync_shortcut(
    store: &NativeAppStore,
    programs: &Path,
    desc: &NativeAppDescriptor,
) -> Result<()> {
    let link = link_path(programs, desc);
    match store.executable_for(desc) {
        Some(target) => {
            if link_points_at(&link, &target) {
                return Ok(());
            }
            write_link(&link, &target)
        }
        None if link.is_file() => Ok(fs::remove_file(&link)?),
        None => Ok(()),
    }
}

/// Reconcile every store app's link at once. Per-app best-effort: one bad
/// link is logged and skipped, never blocking the rest of the table.
pub fn reconcile_shortcuts(store: &NativeAppStore, programs: &Path) {
    for desc in NATIVE_APPS {
        if let Err(error) = sync_shortcut(store, programs, desc) {
            tracing::warn!(
                target: "native_apps",
                id = desc.id,
                %error,
                "start-menu shortcut sync failed"
            );
        }
    }
}

/// True when an existing link already resolves to `target` — avoids a
/// rewrite on every Engine start. An unparseable link answers `false` and
/// gets rewritten.
fn link_points_at(link: &Path, target: &Path) -> bool {
    let Ok(shell_link) = lnk::ShellLink::open(link) else {
        return false;
    };
    let info = shell_link.link_info().as_ref();
    let existing = info
        .and_then(|info| info.local_base_path().clone())
        .or_else(|| info.and_then(|info| info.local_base_path_unicode().clone()));
    existing.is_some_and(|existing| same_path(Path::new(&existing), target))
}

/// Windows paths compare case-insensitively, and an existing path also
/// compares equal to its `\\?\` / 8.3 spelling. `IShellLink::SetPath`
/// stores the long path; the install record keeps the path we were given.
fn same_path(left: &Path, right: &Path) -> bool {
    path_key(left) == path_key(right)
}

fn path_key(path: &Path) -> String {
    #[cfg(windows)]
    {
        return crate::win32::windows_path_key(path);
    }
    #[cfg(not(windows))]
    {
        path.as_os_str().to_string_lossy().to_ascii_lowercase()
    }
}

#[cfg(windows)]
fn write_link(link: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    com_write_link(link, target)
        .map_err(|error| NativeAppError::State(format!("shell link: {error}")))
}

/// IShellLinkW + IPersistFile — the real COM path, not a shell-out. The
/// link's icon is the app executable itself, so an update repoint never
/// leaves a stale icon behind.
#[cfg(windows)]
fn com_write_link(link: &Path, target: &Path) -> windows::core::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    // Balance every successful init (S_OK and S_FALSE both claim one).
    struct ComInit(bool);
    impl Drop for ComInit {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }
    // IShellLink is a Both-threaded coclass — an apartment that already
    // exists (or an MTA forced by the host) still serves it.
    let _com = ComInit(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() });

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let shell_link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }?;
    unsafe { shell_link.SetPath(PCWSTR(wide(target).as_ptr())) }?;
    unsafe { shell_link.SetIconLocation(PCWSTR(wide(target).as_ptr()), 0) }?;
    let persist = shell_link.cast::<IPersistFile>()?;
    unsafe { persist.Save(PCWSTR(wide(link).as_ptr()), true) }
}

#[cfg(not(windows))]
fn write_link(_link: &Path, _target: &Path) -> Result<()> {
    // Store apps only install on Windows targets today; reaching this is a
    // bug in a future port, not a state to silently accept.
    Err(NativeAppError::Invalid("start menu unsupported"))
}

#[cfg(windows)]
fn programs_folder() -> Option<PathBuf> {
    // Known-folder lookup shared with `installer::legacy`.
    crate::win32::known_folder(&windows::Win32::UI::Shell::FOLDERID_Programs).ok()
}

#[cfg(not(windows))]
fn programs_folder() -> Option<PathBuf> {
    None
}
