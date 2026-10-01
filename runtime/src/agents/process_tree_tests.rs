use super::process_tree::{resolve_command, ProcessTree};
use super::*;

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
/// shim on the command's PATH spawns through ProcessTree — and every
/// metacharacter in its arguments survives the cmd.exe hand-off unchanged.
#[cfg(windows)]
#[tokio::test]
async fn spawn_runs_cmd_shim_from_path() {
    use tokio::io::AsyncReadExt;

    let dir = tempfile::tempdir().unwrap();
    // echo(%~N prints argument N verbatim on its own line.
    std::fs::write(
        dir.path().join("probe.cmd"),
        "@echo off\r\necho(%~1\r\necho(%~2\r\necho(%~3\r\necho(%~4\r\necho(%~5\r\necho(%~6\r\necho(%~7\r\necho(%~8\r\necho(%~9\r\n",
    )
    .unwrap();
    // Pairs of (argument sent, line echo(%~N prints). A `"` arrives in the
    // batch-level doubled form `""` — cmd's quote escape, which a real .exe
    // behind the shim collapses back to `"` when it parses argv.
    let cases = [
        ("plain", "plain"),
        ("with space", "with space"),
        ("a&b|c>d", "a&b|c>d"),
        ("x^y!(z)", "x^y!(z)"),
        ("50%", "50%"),
        ("", ""),
        ("trailing\\", "trailing\\"),
        ("say \"hi\"", "say \"\"hi\"\""),
        ("a\"&b", "a\"\"&b"),
    ];
    let mut command = Command::new("probe");
    command
        .args(cases.map(|(arg, _)| arg))
        .env("PATH", dir.path());
    let mut command = resolve_command(command).unwrap();
    command.stdout(Stdio::piped()).stderr(Stdio::null());
    let mut tree = ProcessTree::spawn(&mut command, 0).await.unwrap();

    let mut stdout = tree.child_mut().stdout.take().unwrap();
    let mut output = Vec::new();
    stdout.read_to_end(&mut output).await.unwrap();
    let expected = cases.map(|(_, line)| format!("{line}\r\n")).concat();
    assert_eq!(String::from_utf8_lossy(&output), expected);
    tree.terminate_and_wait(Duration::from_secs(5))
        .await
        .unwrap();
}

/// Failure paths fail closed: a pid that owns no threads must not be resumed,
/// and a nonexistent process cannot join the job. These exercise the same
/// errors `spawn` hits when job assignment or resume goes wrong.
#[cfg(windows)]
#[test]
fn spawn_primitives_fail_closed_on_bad_pid() {
    use super::process_tree::win32;
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
