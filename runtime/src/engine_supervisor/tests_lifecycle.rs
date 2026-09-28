use super::*;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[tokio::test]
#[cfg(unix)]
async fn graceful_restart_requests_core_stop_before_owned_child_exit() {
    let dir = tempfile::tempdir().expect("control fixture");
    let marker = dir.path().join("stop");
    let mut child = Command::new("sh")
        .args([
            "-c",
            "while [ ! -f \"$1\" ]; do sleep 0.01; done",
            "controlled-core",
            marker.to_str().expect("marker path"),
        ])
        .spawn()
        .expect("controlled child");
    let (mut server, endpoint) = ControlServer::bind(
        "lifecycle-session".into(),
        11,
        "uid:test".into(),
        b"lifecycle-controller-secret".to_vec(),
        b"lifecycle-core-secret".to_vec(),
    )
    .await
    .expect("control listener");
    let state = ControlState {
        endpoint,
        supervisor_session_id: "lifecycle-session".into(),
        child_generation: 11,
        owner_identity: "uid:test".into(),
        secret: "lifecycle-controller-secret".into(),
    };
    let mut core_commands = crate::engine_control::start_core_control_with_secret(
        state.clone(),
        b"lifecycle-core-secret".to_vec(),
    )
    .await
    .expect("core ready");
    assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));
    let waiter = tokio::spawn(async move {
        let result = wait_for_child(&mut child, &mut server).await;
        assert_eq!(
            result,
            ChildResult::Control(ControlMessage::RestartRequested)
        );
        assert_eq!(
            core_commands.recv().await,
            Some(ControlMessage::RestartRequested)
        );
        std::fs::write(&marker, "stopping").expect("graceful marker");
        stop_owned_child(&mut child).await;
        child
            .try_wait()
            .expect("child status")
            .expect("child exited")
    });
    crate::engine_control::send_control(&state, ControlMessage::RestartRequested)
        .await
        .expect("restart request");
    let status = waiter.await.expect("supervisor waiter");
    assert!(status.success());
}

#[tokio::test]
#[cfg(unix)]
async fn graceful_shutdown_requests_core_stop_and_leaves_no_child() {
    let dir = tempfile::tempdir().expect("control fixture");
    let marker = dir.path().join("stop");
    let mut child = Command::new("sh")
        .args([
            "-c",
            "while [ ! -f \"$1\" ]; do sleep 0.01; done",
            "controlled-core",
            marker.to_str().expect("marker path"),
        ])
        .spawn()
        .expect("controlled child");
    let (mut server, endpoint) = ControlServer::bind(
        "shutdown-session".into(),
        12,
        "uid:test".into(),
        b"shutdown-controller-secret".to_vec(),
        b"shutdown-core-secret".to_vec(),
    )
    .await
    .expect("control listener");
    let state = ControlState {
        endpoint,
        supervisor_session_id: "shutdown-session".into(),
        child_generation: 12,
        owner_identity: "uid:test".into(),
        secret: "shutdown-controller-secret".into(),
    };
    let mut core_commands = crate::engine_control::start_core_control_with_secret(
        state.clone(),
        b"shutdown-core-secret".to_vec(),
    )
    .await
    .expect("core ready");
    assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));
    let waiter = tokio::spawn(async move {
        let result = wait_for_child(&mut child, &mut server).await;
        assert_eq!(
            result,
            ChildResult::Control(ControlMessage::ShutdownRequested)
        );
        assert_eq!(
            core_commands.recv().await,
            Some(ControlMessage::ShutdownRequested)
        );
        std::fs::write(&marker, "stopping").expect("graceful marker");
        stop_owned_child(&mut child).await;
        child
            .try_wait()
            .expect("child status")
            .expect("child exited")
    });
    crate::engine_control::send_control(&state, ControlMessage::ShutdownRequested)
        .await
        .expect("shutdown request");
    let status = waiter.await.expect("supervisor waiter");
    assert!(status.success());
}

#[tokio::test]
#[cfg(unix)]
async fn closed_core_channel_waits_for_deadline_before_force_kill() {
    let mut child = Command::new("sleep")
        .arg("10")
        .spawn()
        .expect("controlled child");
    let (mut server, _) = ControlServer::bind(
        "closed-channel-session".into(),
        1,
        "uid:test".into(),
        b"controller".to_vec(),
        b"core".to_vec(),
    )
    .await
    .expect("control listener");
    server.abort();
    let started = Instant::now();
    stop_owned_child_gracefully_with_deadline(&mut child, &mut server, Duration::from_millis(40))
        .await;
    assert!(started.elapsed() >= Duration::from_millis(30));
    assert!(child.try_wait().expect("child status").is_some());
}

#[tokio::test]
async fn deadline_force_kills_only_the_owned_child_handle() {
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new("sleep");
        command.arg("10");
        command
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd.exe");
        command.args(["/D", "/C", "ping -n 11 127.0.0.1 >NUL"]);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };
    let mut child = command.spawn().expect("controlled child");
    stop_owned_child_with_deadline(&mut child, Duration::from_millis(10)).await;
    assert!(child.try_wait().expect("child status").is_some());
}

#[test]
fn sync_env_loads_only_runtime_keys() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("sync.env"),
        "MUNDUS_SPACE_ID = personal\nIGNORED_SECRET=nope\n# MUNDUS_IROH=0\n",
    )
    .expect("write sync.env");

    assert_eq!(
        read_sync_env(dir.path()),
        vec![("MUNDUS_SPACE_ID".to_string(), "personal".to_string())]
    );
}

#[test]
#[cfg(unix)]
fn raw_control_state_is_owner_only_and_removed_on_terminal_drop() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("control fixture");
    let path = dir.path().join(CONTROL_STATE_FILE);
    let state = ControlState {
        endpoint: "127.0.0.1:1".into(),
        supervisor_session_id: "session".into(),
        child_generation: 1,
        owner_identity: "uid:test".into(),
        secret: "raw-secret-must-not-leak".into(),
    };
    lock_file::write_owner_only_json(&path, &state).expect("protected control state");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains(&state.secret));
    {
        let _cleanup = ControlStateCleanup(&path);
    }
    assert!(!path.exists());
}

#[tokio::test]
#[cfg(unix)]
async fn spontaneous_success_exit_uses_unexpected_backoff_path() {
    let mut child = Command::new("sh")
        .args(["-c", "exit 0"])
        .spawn()
        .expect("controlled core");
    let started = Instant::now();
    let status = child.wait().await.expect("child exit");
    assert!(status.success());
    assert_eq!(
        crash_streak_after_exit(0, started.elapsed(), ExitKind::Unexpected),
        1
    );
}

#[test]
fn spontaneous_success_exit_is_unexpected() {
    assert_eq!(
        crash_streak_after_exit(0, Duration::from_millis(1), ExitKind::Unexpected),
        1
    );
}

#[tokio::test]
async fn desktop_lease_waits_for_slow_core_ready() {
    let (mut server, endpoint) = ControlServer::bind(
        "slow-lease-session".into(),
        1,
        "uid:test".into(),
        b"controller-secret".to_vec(),
        b"core-secret".to_vec(),
    )
    .await
    .expect("control listener");
    let state = ControlState {
        endpoint,
        supervisor_session_id: "slow-lease-session".into(),
        child_generation: 1,
        owner_identity: "uid:test".into(),
        secret: engine_control::encode_secret(b"controller-secret"),
    };
    let core_state = state.clone();
    let core = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(2100)).await;
        let mut receiver = engine_control::start_core_control_with_secret(
            core_state.clone(),
            b"core-secret".to_vec(),
        )
        .await
        .expect("core control");
        assert!(matches!(
            receiver.recv().await,
            Some(ControlMessage::DesktopLease { .. })
        ));
        let mut stream = TcpStream::connect(&core_state.endpoint)
            .await
            .expect("control");
        let mut envelope = engine_control::ControlEnvelope {
            protocol_version: engine_control::CONTROL_PROTOCOL_VERSION,
            supervisor_session_id: core_state.supervisor_session_id,
            child_generation: 1,
            request_id: uuid::Uuid::new_v4().to_string(),
            owner_identity: core_state.owner_identity,
            channel: engine_control::ControlChannel::CoreEvents,
            message: ControlMessage::DesktopLeaseInstalled {
                generation: 1,
                electron_pid: 42,
            },
            authentication: String::new(),
        };
        envelope.authentication = engine_control::authentication_tag(b"core-secret", &envelope);
        let bytes = serde_json::to_vec(&envelope).expect("control envelope");
        stream
            .write_u32(bytes.len() as u32)
            .await
            .expect("frame size");
        stream.write_all(&bytes).await.expect("control frame");
    });
    send_desktop_lease_when_ready(&mut server, 42, "desktop-credential".into())
        .await
        .expect("slow core should still receive the lease");
    core.await.expect("core task");
}
