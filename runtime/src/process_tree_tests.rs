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
