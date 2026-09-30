use super::process_tree::ProcessTree;
use super::*;

#[tokio::test]
async fn terminate_and_wait_is_idempotent() {
    let mut command = long_running_command();
    let mut tree = ProcessTree::spawn(&mut command).await.unwrap();

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
    let child_script = format!(
        "Start-Sleep 0.5; Set-Content -LiteralPath '{}' leaked",
        marker.display()
    );
    let parent_script = format!(
        "Start-Process powershell -ArgumentList '-NoProfile','-Command','{}'; Start-Sleep 30",
        child_script.replace('\'', "''")
    );
    let mut command = Command::new("powershell");
    command.args(["-NoProfile", "-Command", &parent_script]);
    let mut tree = ProcessTree::spawn(&mut command).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;
    tree.terminate_and_wait(Duration::from_secs(5))
        .await
        .unwrap();
    // A surviving grandchild would write the marker ~0.8 s after spawn; 1.5 s
    // of silence is conclusive without a multi-second wait.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(
        !marker.exists(),
        "grandchild escaped the Windows Job Object"
    );
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
