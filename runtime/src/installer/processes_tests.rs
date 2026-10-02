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

use std::io::{BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
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

/// Helper: spawn B (sleep_helper), write its pid to the file named by
/// MUNDUS_TEST_PIPE_PIDFILE, exit. B keeps running — dropping `Child`
/// detaches, it never kills. (A pid file, not stdout: libtest captures test
/// output, so the pipe would carry harness noise, not the pid.)
#[test]
fn pipe_spawn_helper() {
    let Some(pidfile) = std::env::var_os("MUNDUS_TEST_PIPE_PIDFILE") else {
        return;
    };
    // Deliberately NOT ChildGuard: B must outlive this process — the drop
    // guard would kill it. The zombie is reaped by the calling test's
    // PidGuard.
    let exe = std::env::current_exe().unwrap();
    #[allow(clippy::zombie_processes)] // B must outlive us — that's the test.
    let child = Command::new(exe)
        .args([
            "--exact",
            "installer::processes_tests::sleep_helper",
            "--test-threads=1",
        ])
        .env("MUNDUS_TEST_SLEEP_CHILD", "1")
        .spawn()
        .unwrap();
    std::fs::write(&pidfile, child.id().to_string()).unwrap();
}

/// Proves why `--start-engine` must run via ExecWait, not nsExec::ExecToLog:
/// B — spawned with the default (inherit) stdio — inherits A's stdout, which
/// is our pipe's write end, so the pipe stays open while B lives even after
/// A exits. ExecToLog waits for pipe EOF: under an installer whose last step
/// spawns a long-lived Engine it would block forever.
#[test]
fn grandchildren_hold_our_piped_stdout_open() {
    let pidfile = tempfile::NamedTempFile::new().unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut a = Command::new(exe)
        .args([
            "--exact",
            "installer::processes_tests::pipe_spawn_helper",
            "--test-threads=1",
        ])
        .env("MUNDUS_TEST_PIPE_PIDFILE", pidfile.path())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    a.wait().unwrap();
    let b_pid: u32 = std::fs::read_to_string(pidfile.path())
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let _b_guard = PidGuard(b_pid);
    let mut reader = BufReader::new(a.stdout.take().unwrap());

    // A is gone, B still runs — B inherited A's stdout (the pipe's write
    // end), so the pipe must NOT reach EOF. The read runs on a thread and
    // reports back through a channel; no sleep needed.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = reader.read_to_end(&mut sink);
        let _ = tx.send(());
    });
    assert!(
        rx.recv_timeout(Duration::from_secs(3)).is_err(),
        "pipe reached EOF while grandchild B (pid {b_pid}) still runs — \
         B did not inherit A's stdout, so ExecToLog would have been safe"
    );

    // Only B's death releases the handle — killed through the same
    // terminate-and-wait path the installer uses; only then EOF.
    assert_eq!(terminate_and_wait(b_pid).unwrap(), KillOutcome::Gone);
    rx.recv_timeout(Duration::from_secs(30))
        .expect("pipe did not reach EOF after the grandchild exited");
}

/// Kills a not-our-child pid on drop — the spawn belongs to the helper
/// process, so `ChildGuard` cannot reach it.
struct PidGuard(u32);

impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = terminate_and_wait(self.0);
    }
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
