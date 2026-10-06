use super::*;

#[test]
fn real_child_id_gets_foreground_grant() {
    assert_eq!(foreground_grant_target(4242), Some(4242));
}

#[test]
fn pid_zero_never_gets_foreground_grant() {
    // pid 0 is System Idle Process — no foreground rights, and ASFW_ANY
    // must never be the fallback.
    assert_eq!(foreground_grant_target(0), None);
}

#[cfg(unix)]
#[test]
fn spawn_detached_runs_executable() {
    spawn_detached(Path::new("/bin/true"), &[]).expect("spawn /bin/true");
}

#[test]
fn spawn_detached_reports_spawn_errors() {
    let result = spawn_detached(Path::new("/definitely/not/a/real/exe"), &[]);
    assert!(result.is_err());
}
