/// Is the `/v1/rpc` caller the Manager *process*? The bearer token alone is
/// not Manager identity — `engine.lock.json` is readable by any same-user
/// process, and app clients share the credential class (KOS-269 round 3).
/// What proves it is the caller's process image (queried by its already
/// same-user-validated PID, never self-declared) equal to the Manager exe
/// the Engine itself resolves: `resources/components/manager/
/// Mundus Manager.exe`, or the `MUNDUS_MANAGER_EXECUTABLE` dev override —
/// the same resolution the tray uses to launch it. Deny-closed: a missing
/// or unresolvable Manager install means `false`.
#[cfg(windows)]
fn is_manager_process(pid: u32) -> bool {
    let Ok(image) = crate::auth::process_image_path(pid) else {
        return false;
    };
    let Some(manager) = crate::backend_tray::manager_executable() else {
        return false;
    };
    image
        .to_string_lossy()
        .eq_ignore_ascii_case(manager.to_string_lossy().as_ref())
}

/// The gpui Manager ships only on Windows.
#[cfg(not(windows))]
fn is_manager_process(_pid: u32) -> bool {
    false
}

// `mod tests` already exists in the including module — use a distinct name.
#[cfg(test)]
mod manager_identity_tests {
    #[test]
    fn non_manager_process_is_not_authorized() {
        // The test binary is not the installed Manager exe, so the image
        // check must deny even though the PID is real and same-user.
        #[cfg(windows)]
        assert!(!super::is_manager_process(std::process::id()));
        #[cfg(not(windows))]
        assert!(!super::is_manager_process(std::process::id()));
    }
}
