#[tokio::test]
async fn macos_stop_and_drop_kill_owned_group() {
    let executable = Path::new("/usr/bin/yes");
    let mut worker = launch_macos(executable, None).expect("launch controlled test executable");
    let pid = worker.id().expect("pid") as i32;
    assert_eq!(unsafe { libc::getpgid(pid) }, pid);
    worker
        .stop_until(Instant::now() + Duration::from_secs(2))
        .await
        .expect("stop");
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);

    let worker = launch_macos(executable, None).expect("launch second controlled test executable");
    let pid = worker.id().expect("pid") as i32;
    drop(worker);
    let deadline = Instant::now() + Duration::from_secs(2);
    while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "Drop must kill the worker"
    );
}

#[test]
fn macos_rejects_uninstalled_macho() {
    assert!(matches!(
        validate_executable(Path::new("/usr/bin/yes")),
        Err(WorkerProcessError::InvalidExecutable)
    ));
}

#[tokio::test]
async fn macos_launch_scrubs_parent_environment() {
    use tokio::io::AsyncReadExt;
    let mut worker = launch_macos(
        Path::new("/usr/bin/env"),
        Some(Path::new("/tmp/mundus-worker-state")),
    )
    .unwrap();
    let (_, mut stdout, _) = worker.take_all_pipes().unwrap();
    let mut output = Vec::new();
    stdout.read_to_end(&mut output).await.unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("PATH=/usr/bin:/bin\n"));
    assert!(output.contains("MUNDUS_DATA_DIR=/tmp/mundus-worker-state\n"));
    assert!(!output.contains("HOME="));
    assert!(!output.contains("MUNDUS_WORKER_FIXTURE="));
    worker
        .stop_until(Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn expired_stop_still_kills_group_on_drop() {
    let mut worker = launch_macos(Path::new("/usr/bin/yes"), None).unwrap();
    let pid = worker.id().unwrap() as i32;
    assert!(worker.stop_until(Instant::now()).await.is_err());
    drop(worker);
    let deadline = Instant::now() + Duration::from_secs(2);
    while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
}

#[tokio::test]
async fn macos_stop_kills_descendants_in_worker_group() {
    use tokio::io::AsyncBufReadExt;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("worker.c");
    let binary = root.path().join("worker.exe");
    std::fs::write(
        &source,
        concat!(
            "#include <stdio.h>\n#include <unistd.h>\n",
            "int main(void) { pid_t child=fork(); if(child<0) return 1; ",
            "if(child==0) { for(;;) pause(); } printf(\"%d\\n\", child); ",
            "fflush(stdout); for(;;) pause(); }\n",
        ),
    )
    .unwrap();
    let built = std::process::Command::new("/usr/bin/cc")
        .args(["-x", "c"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .status()
        .unwrap();
    assert!(built.success());
    let mut worker = launch_macos(&binary, None).unwrap();
    let group = worker.id().unwrap() as i32;
    let (_, stdout, _) = worker.take_all_pipes().unwrap();
    let mut stdout = tokio::io::BufReader::new(stdout);
    let mut line = String::new();
    stdout.read_line(&mut line).await.unwrap();
    let descendant: i32 = line.trim().parse().unwrap();
    assert_eq!(unsafe { libc::getpgid(descendant) }, group);
    worker
        .stop_until(Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !group_is_empty(group) && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        group_is_empty(group),
        "worker process group still has members"
    );
}
