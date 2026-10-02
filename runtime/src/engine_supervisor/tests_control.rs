use super::*;
use crate::engine_control::ControlError;

#[test]
fn restart_and_shutdown_adapters_contain_no_pid_termination_fallback() {
    let source = include_str!("control.rs");
    let restart = source
        .split_once("pub fn restart_core")
        .and_then(|(_, rest)| rest.split_once("pub fn shutdown"))
        .map(|(body, _)| body)
        .expect("restart adapter source");
    let shutdown = source
        .split_once("pub fn shutdown")
        .and_then(|(_, rest)| rest.split_once("fn read_control_state"))
        .map(|(body, _)| body)
        .expect("shutdown adapter source");
    assert!(!restart.contains("kill("));
    assert!(!restart.contains("terminate"));
    assert!(!shutdown.contains("kill("));
    assert!(!shutdown.contains("terminate"));
}

#[tokio::test]
async fn old_core_credential_cannot_authenticate_against_replacement_generation() {
    let (old_server, old_endpoint) = ControlServer::bind(
        "credential-rotation-session".into(),
        1,
        "uid:test".into(),
        b"controller".to_vec(),
        b"old-core-secret".to_vec(),
    )
    .await
    .expect("old control listener");
    old_server.abort();

    let (mut new_server, new_endpoint) = ControlServer::bind(
        "credential-rotation-session".into(),
        2,
        "uid:test".into(),
        b"controller".to_vec(),
        b"new-core-secret".to_vec(),
    )
    .await
    .expect("replacement control listener");
    let replacement = ControlState {
        endpoint: new_endpoint,
        supervisor_session_id: "credential-rotation-session".into(),
        child_generation: 2,
        owner_identity: "uid:test".into(),
        secret: "controller".into(),
    };

    let old_key_result = crate::engine_control::start_core_control_with_secret(
        replacement.clone(),
        b"old-core-secret".to_vec(),
    )
    .await;
    assert!(matches!(old_key_result, Err(ControlError::Unauthorized)));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), new_server.recv())
            .await
            .is_err(),
        "old generation must not reach replacement server"
    );

    let new_commands = crate::engine_control::start_core_control_with_secret(
        replacement,
        b"new-core-secret".to_vec(),
    )
    .await
    .expect("new generation credential");
    drop(new_commands);
    assert_eq!(new_server.recv().await, Some(ControlMessage::CoreReady));
    let _ = old_endpoint;
}

#[tokio::test]
async fn dropping_core_receiver_closes_connection_and_clears_sender() {
    let (mut server, endpoint) = ControlServer::bind(
        "receiver-close-session".into(),
        1,
        "uid:test".into(),
        b"controller".to_vec(),
        b"core".to_vec(),
    )
    .await
    .expect("control listener");
    let state = ControlState {
        endpoint,
        supervisor_session_id: "receiver-close-session".into(),
        child_generation: 1,
        owner_identity: "uid:test".into(),
        secret: "controller".into(),
    };
    let commands = crate::engine_control::start_core_control_with_secret(state, b"core".to_vec())
        .await
        .expect("core ready");
    assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));
    drop(commands);

    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if server
                .send_to_core(ControlMessage::ShutdownRequested)
                .await
                .is_err()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("core connection cleanup");
}

#[tokio::test]
async fn server_abort_closes_core_connection_and_receiver() {
    let (mut server, endpoint) = ControlServer::bind(
        "server-abort-session".into(),
        1,
        "uid:test".into(),
        b"controller".to_vec(),
        b"core".to_vec(),
    )
    .await
    .expect("control listener");
    let state = ControlState {
        endpoint,
        supervisor_session_id: "server-abort-session".into(),
        child_generation: 1,
        owner_identity: "uid:test".into(),
        secret: "controller".into(),
    };
    let mut commands =
        crate::engine_control::start_core_control_with_secret(state, b"core".to_vec())
            .await
            .expect("core ready");
    assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));
    server.abort();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(60), commands.recv())
            .await
            .expect("receiver close"),
        None
    );
}
