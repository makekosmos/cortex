// Native-app process launch. The Engine is a background process — on
// Windows it owns no foreground window, so a spawned GUI app inherits no
// foreground rights: `SetForegroundWindow` calls it makes are swallowed
// by the foreground lock and its window opens *behind* the Manager, while
// the Manager keeps focus and appears to flicker/reopen (KOS-354). After
// spawn we hand the child explicit foreground permission via
// `AllowSetForegroundWindow(pid)` — the child (or, on relaunch, the
// already-running instance it forwards to) can then raise its own window.
// The Engine never calls `SetForegroundWindow` itself and never
// re-activates the Manager.

use std::ffi::OsStr;

/// `CREATE_NO_WINDOW`: the Engine is console-less; without the flag a
/// console-subsystem child would pop a conhost window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Which freshly spawned process gets foreground rights. `Some(pid)` for a
/// real child id; `None` for pid 0 (`System Idle Process` — granting it is
/// meaningless). `ASFW_ANY` is deliberately *not* used: it would let any
/// process steal foreground until the next user input, not just the app we
/// launched.
fn foreground_grant_target(child_id: u32) -> Option<u32> {
    (child_id != 0).then_some(child_id)
}

/// Spawn the app detached; on Windows additionally grant the child
/// foreground rights (see module doc). Grant failure is logged, not
/// fatal — the app already runs, it is just possibly behind.
pub fn spawn_detached(executable: &Path, args: &[&OsStr]) -> io::Result<()> {
    let child = spawn(executable, args)?;
    grant_foreground(child.id());
    Ok(())
}

#[cfg(windows)]
fn spawn(executable: &Path, args: &[&OsStr]) -> io::Result<std::process::Child> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(executable)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
}

#[cfg(not(windows))]
fn spawn(executable: &Path, args: &[&OsStr]) -> io::Result<std::process::Child> {
    std::process::Command::new(executable).args(args).spawn()
}

fn grant_foreground(child_id: u32) {
    let Some(pid) = foreground_grant_target(child_id) else {
        return;
    };
    platform_grant_foreground(pid);
}

#[cfg(windows)]
fn platform_grant_foreground(pid: u32) {
    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
    if let Err(error) = unsafe { AllowSetForegroundWindow(pid) } {
        tracing::warn!(
            target: "native_apps",
            pid,
            %error,
            "AllowSetForegroundWindow failed; app may open behind"
        );
    }
}

#[cfg(not(windows))]
fn platform_grant_foreground(_: u32) {}
