//! KOS-309 regression tests for the waiting process kill (Windows-only:
//! terminate-and-wait rides on Win32 handles).
//!
//! The "process" under test is another instance of this very test binary,
//! filtered to [`sleep_helper`]: it carries a real Win32 process handle, so
//! `terminate_and_wait` exercises the same code path the installer hits —
//! no sleeps in the test itself.
//!
//! The full `kill_by_targets_with` path is exercised only with a fake
//! terminate step: a real one would kill every process sharing this binary's
//! image name, including sibling test processes running in parallel.

use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;

use super::processes::{
    self, is_under, kill_by_targets_with, terminate_and_wait, KillOutcome, KillTarget,
};

/// Re-runs this binary as a sleeping child (see module docs). Without the
/// env marker — i.e. a normal test run — it returns immediately.
#[test]
fn sleep_helper() {
    if std::env::var_os("MUNDUS_TEST_SLEEP_CHILD").is_none() {
        return;
    }
    std::thread::sleep(Duration::from_secs(120));
}

/// Kills the child on drop so a failed assertion can never leak a sleeper.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_sleeping_child() -> ChildGuard {
    let exe = std::env::current_exe().unwrap();
    let child = Command::new(exe)
        .args([
            "--exact",
            "installer::processes_tests::sleep_helper",
            "--test-threads=1",
        ])
        .env("MUNDUS_TEST_SLEEP_CHILD", "1")
        .spawn()
        .unwrap();
    ChildGuard(child)
}

/// The image file name of this test binary — the spawned child shares it.
fn own_image_name() -> String {
    std::env::current_exe()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// A kill target matching this test binary's image name — the spawned
/// sleeping child shares it (the caller's own pid is always excluded).
fn own_image_target(name: &str) -> KillTarget<'_> {
    KillTarget { name, under: None }
}

#[test]
fn terminate_and_wait_reports_gone_only_after_the_process_exits() {
    let mut child = spawn_sleeping_child();
    let outcome = terminate_and_wait(child.0.id()).unwrap();
    assert_eq!(outcome, KillOutcome::Gone);
    // The handle was already signaled — wait() reaps immediately, proving
    // the kill is complete when the function returns (no sleep here).
    assert!(!child.0.wait().unwrap().success());
}

#[test]
fn a_surviving_process_is_reported_and_fails_the_report() {
    let mut child = spawn_sleeping_child();
    let pid = child.0.id();
    // Fake terminate that never kills: every match must land in `survived`,
    // and the aggregate verdict must be non-zero for the subcommand.
    let never_kills = |_pid: u32| Ok(KillOutcome::Survived);
    let image = own_image_name();
    let report = kill_by_targets_with(&[own_image_target(&image)], &never_kills).unwrap();
    assert!(
        report.survived.iter().any(|s| s.contains(&pid.to_string())),
        "survived must name pid {pid}: {report:?}"
    );
    assert!(report.killed.is_empty());
    assert!(!processes::report_ok(&report));
    // Child is still alive — clean it up through the real path.
    assert_eq!(terminate_and_wait(pid).unwrap(), KillOutcome::Gone);
    let _ = child.0.wait();
}

#[test]
fn a_failed_kill_is_reported_with_the_pid() {
    let child = spawn_sleeping_child();
    let pid = child.0.id();
    let fails = |_pid: u32| Err("injected failure".to_string());
    let image = own_image_name();
    let report = kill_by_targets_with(&[own_image_target(&image)], &fails).unwrap();
    assert!(
        report
            .failed
            .iter()
            .any(|s| s.contains(&pid.to_string()) && s.contains("injected failure")),
        "{report:?}"
    );
    assert!(!processes::report_ok(&report));
}

#[test]
fn whisper_server_scope_matches_only_our_tools_dir() {
    let tools = Path::new(r"C:\Users\u\AppData\Roaming\Mundus\tools\dictation");
    assert!(is_under(
        Path::new(
            r"C:\Users\u\AppData\Roaming\Mundus\tools\dictation\whisper.cpp\Release\whisper-server.exe"
        ),
        tools
    ));
    // Another vendor's whisper-server under a foreign root is not ours.
    assert!(!is_under(
        Path::new(r"C:\Program Files\OtherApp\whisper-server.exe"),
        tools
    ));
    assert!(!is_under(
        Path::new(r"C:\Users\u\AppData\Roaming\Mundus\tools\other\whisper-server.exe"),
        tools
    ));
}
