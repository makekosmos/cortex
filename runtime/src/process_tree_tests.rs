use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::process_tree::{resolve_command, ProcessTree};

#[tokio::test]
async fn terminate_and_wait_is_idempotent() {
    let mut command = long_running_command();
    let mut tree = ProcessTree::spawn(&mut command, 0).await.unwrap();

    tree.terminate_and_wait(Duration::from_secs(2))
        .await
        .unwrap();
    tree.terminate_and_wait(Duration::from_secs(2))
        .await
        .unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn terminate_kills_grandchild() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("grandchild-survived");
    // The grandchild sleeps ~1 s, then writes the marker if it escaped the
    // job. cmd.exe, not PowerShell: PowerShell drops __PSScriptPolicyTest_*
    // probe files into %TEMP% when killed mid-startup, which is exactly the
    // window this test exercises.
    let grandchild = dir.path().join("grandchild.cmd");
    std::fs::write(
        &grandchild,
        format!(
            "@ping -n 2 127.0.0.1 >nul & @echo leaked>\"{}\"\r\n",
            marker.display()
        ),
    )
    .unwrap();
    let mut command = Command::new("cmd.exe");
    command.args([
        "/d",
        "/s",
        "/c",
        &format!(
            "start /b \"\" \"{}\" & ping -n 30 127.0.0.1 >nul",
            grandchild.display()
        ),
    ]);
    let mut tree = ProcessTree::spawn(&mut command, 0).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;
    tree.terminate_and_wait(Duration::from_secs(5))
        .await
        .unwrap();
    // A surviving grandchild would write the marker ~1 s after spawn; 1.5 s
    // of silence is conclusive without a multi-second wait.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(
        !marker.exists(),
        "grandchild escaped the Windows Job Object"
    );
}

/// A bare program name resolves over PATH × PATHEXT, so an npm-style `.cmd`
/// shim on the command's PATH spawns through ProcessTree. The shim forwards
/// `%*` verbatim — the way npm's shims hand arguments to `node.exe` — and the
/// consumer prints the argv it actually received, so a metacharacter that
/// survives only inside cmd's own quoting (e.g. `echo(%~N`) cannot hide here.
#[cfg(windows)]
#[tokio::test]
async fn spawn_runs_cmd_shim_from_path() {
    use tokio::io::AsyncReadExt;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("argv.js"),
        "process.stdout.write(JSON.stringify(process.argv.slice(2)))",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("probe.cmd"),
        "@node \"%~dp0argv.js\" %*\r\n",
    )
    .unwrap();
    let args = [
        "plain",
        "with space",
        "a&b|c>d",
        "x^y!(z)",
        "50%",
        "%PATH%",
        "",
        "trailing\\",
        "say \"hi\"",
        "a\"&b",
    ];
    let mut command = Command::new("probe");
    // dir goes first so it wins the lookup; the inherited PATH stays so the
    // shim can still find node.
    command.args(args).env(
        "PATH",
        format!(
            "{};{}",
            dir.path().display(),
            std::env::var("PATH").unwrap()
        ),
    );
    let mut command = resolve_command(command).unwrap();
    command.stdout(Stdio::piped()).stderr(Stdio::inherit());
    let mut tree = ProcessTree::spawn(&mut command, 0).await.unwrap();

    let mut stdout = tree.child_mut().stdout.take().unwrap();
    let mut output = Vec::new();
    stdout.read_to_end(&mut output).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
        serde_json::json!(args),
        "node printed {}",
        String::from_utf8_lossy(&output)
    );
    tree.terminate_and_wait(Duration::from_secs(5))
        .await
        .unwrap();
}

/// Holder half of `parent_death_kills_job_child` (KOS-312): run as a separate
/// process — the same test binary with `MUNDUS_TEST_JOB_HOLDER_DIR` set — it
/// spawns a sleeper child inside the KILL_ON_JOB_CLOSE job, publishes the
/// child pid, and then simply waits to be killed. In a normal suite run the
/// env var is absent and this test returns instantly.
#[cfg(windows)]
#[tokio::test]
async fn job_holder_mode() {
    let Ok(dir) = std::env::var("MUNDUS_TEST_JOB_HOLDER_DIR") else {
        return;
    };
    let mut command = Command::new("cmd.exe");
    command.args(["/C", "ping -n 300 127.0.0.1 > nul"]);
    let tree = ProcessTree::spawn(&mut command, 0).await.unwrap();
    let pid = tree.child().id().unwrap();
    std::fs::write(
        std::path::Path::new(&dir).join("child.pid"),
        pid.to_string(),
    )
    .unwrap();
    // `tree` must stay alive for the job to exist; park until the harness
    // kills us.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

/// KOS-312 regression: the process holding the job is killed → the child in
/// the job is gone within a bounded deadline. `terminate_kills_grandchild`
/// covers an explicit `terminate_and_wait`; this covers the *involuntary*
/// path — process death closes the job handle and the kernel applies
/// KILL_ON_JOB_CLOSE. `job_holder_mode` stands in for the Engine process and
/// `cmd /c ping` for whisper-server.
#[cfg(windows)]
#[test]
fn parent_death_kills_job_child() {
    use std::time::Instant;

    let dir = tempfile::tempdir().unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut holder = std::process::Command::new(exe)
        .args([
            "--exact",
            "process_tree_tests::job_holder_mode",
            "--nocapture",
        ])
        .env("MUNDUS_TEST_JOB_HOLDER_DIR", dir.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let holder_pid = holder.id();
    let mut child_pid = None;

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // The holder process needs a moment to boot and spawn its child.
        let deadline = Instant::now() + Duration::from_secs(30);
        let pid = loop {
            if let Ok(text) = std::fs::read_to_string(dir.path().join("child.pid")) {
                if let Ok(pid) = text.trim().parse::<u32>() {
                    break pid;
                }
            }
            assert!(
                Instant::now() < deadline,
                "holder never published a child pid"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        child_pid = Some(pid);
        assert!(process_alive(pid), "job child {pid} died before the parent");

        kill_process(holder_pid);
        let deadline = Instant::now() + Duration::from_secs(10);
        while process_alive(pid) {
            assert!(
                Instant::now() < deadline,
                "job child {pid} survived the death of the job owner"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }));

    // Cleanup even on assertion failure: kill only the two pids this test
    // started.
    kill_process(holder_pid);
    let _ = holder.wait();
    if let Some(pid) = child_pid {
        if process_alive(pid) {
            kill_process(pid);
        }
    }
    outcome.unwrap();
}

/// STILL_ACTIVE (259): the exit code of a process that has not exited. The
/// `windows` crate exports it only as an NTSTATUS constant, so spell it out.
#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        if handle.is_invalid() {
            return false;
        }
        let mut code = 0u32;
        let alive = GetExitCodeProcess(handle, &mut code).is_ok() && code == 259;
        let _ = CloseHandle(handle);
        alive
    }
}

/// Terminate a process this test started. A missing or dead pid is fine.
#[cfg(windows)]
fn kill_process(pid: u32) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
            if !handle.is_invalid() {
                let _ = TerminateProcess(handle, 1);
                let _ = CloseHandle(handle);
            }
        }
    }
}

/// Failure paths fail closed: a pid that owns no threads must not be resumed,
/// and a nonexistent process cannot join the job. These exercise the same
/// errors `spawn` hits when job assignment or resume goes wrong.
#[cfg(windows)]
#[test]
fn spawn_primitives_fail_closed_on_bad_pid() {
    use crate::process_tree::win32;
    let error = win32::resume_primary_thread(0).unwrap_err();
    assert!(
        error.to_string().contains("expected exactly one"),
        "unexpected error: {error}"
    );
    assert!(win32::JobHandle::for_process(0).is_err());
}

#[cfg(unix)]
fn long_running_command() -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30"]);
    command
}

#[cfg(windows)]
fn long_running_command() -> Command {
    let mut command = Command::new("cmd.exe");
    command.args(["/C", "ping -n 30 127.0.0.1 > nul"]);
    command
}
