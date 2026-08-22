use hmac::{Hmac, Mac};
use kepler_backend::engine_control::{
    authentication_tag, ControlChannel, ControlEnvelope, ControlError, ControlMessage,
    ControlPhase, ControlServer, ControlSession, ControlState, CONTROL_PROTOCOL_VERSION,
};
use sha2::Sha256;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn session() -> ControlSession {
    ControlSession::new(
        "session-1",
        7,
        "owner-1",
        b"controller-secret",
        b"core-secret",
    )
}

fn envelope(message: ControlMessage) -> ControlEnvelope {
    let mut value = ControlEnvelope {
        protocol_version: CONTROL_PROTOCOL_VERSION,
        supervisor_session_id: "session-1".into(),
        child_generation: 7,
        request_id: uuid::Uuid::new_v4().to_string(),
        owner_identity: "owner-1".into(),
        channel: match message {
            ControlMessage::RestartRequested | ControlMessage::ShutdownRequested => {
                ControlChannel::Controller
            }
            ControlMessage::CoreReady
            | ControlMessage::CoreStopping
            | ControlMessage::DesktopLeaseInstalled { .. } => ControlChannel::CoreEvents,
            ControlMessage::DesktopLease { .. } | ControlMessage::DesktopLeaseRevoked { .. } => {
                ControlChannel::Controller
            }
        },
        message,
        authentication: String::new(),
    };
    value.authentication = authentication_tag(
        match value.channel {
            ControlChannel::Controller => b"controller-secret",
            ControlChannel::CoreEvents => b"core-secret",
        },
        &value,
    );
    value
}

#[derive(serde::Deserialize)]
struct RawResponse {
    accepted: bool,
}

async fn send_raw(endpoint: &str, mut envelope: ControlEnvelope, key: &[u8]) -> bool {
    envelope.authentication = authentication_tag(key, &envelope);
    let bytes = serde_json::to_vec(&envelope).expect("envelope json");
    let mut stream = TcpStream::connect(endpoint).await.expect("control socket");
    stream
        .write_u32(bytes.len() as u32)
        .await
        .expect("request size");
    stream.write_all(&bytes).await.expect("request body");
    let size = stream.read_u32().await.expect("response size") as usize;
    let mut response = vec![0; size];
    stream
        .read_exact(&mut response)
        .await
        .expect("response body");
    serde_json::from_slice::<RawResponse>(&response)
        .expect("response json")
        .accepted
}

#[tokio::test]
async fn real_socket_capabilities_cannot_cross_roles_or_preempt_core_channel() {
    let (mut server, endpoint) = ControlServer::bind(
        "socket-session".into(),
        9,
        "owner-1".into(),
        b"controller-secret".to_vec(),
        b"core-secret".to_vec(),
    )
    .await
    .expect("control listener");
    let mut controller_event = envelope(ControlMessage::CoreReady);
    controller_event.supervisor_session_id = "socket-session".into();
    controller_event.child_generation = 9;
    controller_event.owner_identity = "owner-1".into();
    controller_event.request_id = "controller-preemption".into();
    assert!(!send_raw(&endpoint, controller_event, b"controller-secret").await);

    let mut core_ready = envelope(ControlMessage::CoreReady);
    core_ready.supervisor_session_id = "socket-session".into();
    core_ready.child_generation = 9;
    core_ready.owner_identity = "owner-1".into();
    core_ready.request_id = "core-ready".into();
    assert!(send_raw(&endpoint, core_ready, b"core-secret").await);
    assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));

    let mut core_command = envelope(ControlMessage::RestartRequested);
    core_command.supervisor_session_id = "socket-session".into();
    core_command.child_generation = 9;
    core_command.owner_identity = "owner-1".into();
    core_command.request_id = "core-command".into();
    assert!(!send_raw(&endpoint, core_command, b"core-secret").await);

    let mut controller_command = envelope(ControlMessage::RestartRequested);
    controller_command.supervisor_session_id = "socket-session".into();
    controller_command.child_generation = 9;
    controller_command.owner_identity = "owner-1".into();
    controller_command.request_id = "controller-command".into();
    assert!(send_raw(&endpoint, controller_command, b"controller-secret").await);
    assert_eq!(server.recv().await, Some(ControlMessage::RestartRequested));

    let mut stale_stopping = envelope(ControlMessage::CoreStopping);
    stale_stopping.supervisor_session_id = "socket-session".into();
    stale_stopping.child_generation = 8;
    stale_stopping.owner_identity = "owner-1".into();
    stale_stopping.request_id = "stale-stopping".into();
    assert!(!send_raw(&endpoint, stale_stopping, b"core-secret").await);
    server.abort();
}

#[test]
fn ready_then_stop_requests_are_authenticated_and_ordered() {
    let mut state = session();
    assert_eq!(
        state.validate(&envelope(ControlMessage::CoreReady)),
        Ok(ControlMessage::CoreReady)
    );
    assert_eq!(state.phase(), ControlPhase::Running);
    assert_eq!(
        state.validate(&envelope(ControlMessage::RestartRequested)),
        Ok(ControlMessage::RestartRequested)
    );
    assert_eq!(state.phase(), ControlPhase::StopRequested);
    assert_eq!(
        state.validate(&envelope(ControlMessage::CoreStopping)),
        Ok(ControlMessage::CoreStopping)
    );
}

#[test]
fn wrong_secret_session_generation_and_replay_fail_closed() {
    let mut state = session();
    let mut wrong_secret = envelope(ControlMessage::CoreReady);
    wrong_secret.authentication = authentication_tag(b"wrong", &wrong_secret);
    assert_eq!(
        state.validate(&wrong_secret),
        Err(ControlError::Unauthorized)
    );

    let mut wrong_session = envelope(ControlMessage::CoreReady);
    wrong_session.supervisor_session_id = "other".into();
    wrong_session.authentication = authentication_tag(b"controller-secret", &wrong_session);
    assert_eq!(
        state.validate(&wrong_session),
        Err(ControlError::Unauthorized)
    );

    let mut stale = envelope(ControlMessage::CoreReady);
    stale.child_generation = 6;
    stale.authentication = authentication_tag(b"core-secret", &stale);
    assert_eq!(state.validate(&stale), Err(ControlError::StaleGeneration));

    let ready = envelope(ControlMessage::CoreReady);
    assert_eq!(state.validate(&ready), Ok(ControlMessage::CoreReady));
    assert_eq!(state.validate(&ready), Err(ControlError::Replayed));
}

#[test]
fn malformed_and_out_of_order_messages_are_rejected() {
    let mut state = session();
    let mut malformed = envelope(ControlMessage::CoreReady);
    malformed.request_id.clear();
    assert_eq!(state.validate(&malformed), Err(ControlError::Malformed));

    let stopping = envelope(ControlMessage::CoreStopping);
    assert_eq!(state.validate(&stopping), Err(ControlError::OutOfOrder));
    assert_eq!(state.validate(&stopping), Err(ControlError::Replayed));
}

#[test]
fn authentication_tag_matches_rfc_4231_hmac_sha256_vector() {
    let mut mac = Hmac::<Sha256>::new_from_slice(b"Jefe").expect("valid HMAC key");
    mac.update(b"what do ya want for nothing?");
    let actual: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        actual,
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

#[test]
fn authentication_tag_is_hmac_sha256_and_tamper_evident() {
    let mut value = envelope(ControlMessage::CoreReady);
    value.request_id = "fixed-request-id".into();
    value.authentication = authentication_tag(b"controller-secret", &value);
    assert_eq!(
        authentication_tag(b"Jefe", &value),
        "a8ffa7c39e3bb2082cc63379afdf9e07c9bb80ee01fbdfb981ddcfb2ad0f46e2"
    );
    let mut tampered = value.clone();
    tampered.message = ControlMessage::CoreStopping;
    assert_ne!(
        authentication_tag(b"Jefe", &tampered),
        authentication_tag(b"Jefe", &value)
    );
}

#[test]
fn channel_roles_fence_commands_and_core_events() {
    let mut injected_event = envelope(ControlMessage::CoreReady);
    injected_event.authentication = authentication_tag(b"controller-secret", &injected_event);
    let mut controller = session();
    assert_eq!(
        controller.validate(&injected_event),
        Err(ControlError::Unauthorized)
    );

    let mut injected_command = envelope(ControlMessage::RestartRequested);
    injected_command.authentication = authentication_tag(b"core-secret", &injected_command);
    let mut fresh = session();
    assert_eq!(
        fresh.validate(&injected_command),
        Err(ControlError::Unauthorized)
    );
}

#[tokio::test]
async fn loopback_server_delivers_only_authenticated_bounded_commands() {
    let (mut server, endpoint) = ControlServer::bind(
        "session-1".into(),
        7,
        "owner-1".into(),
        b"controller-secret".to_vec(),
        b"core-secret".to_vec(),
    )
    .await
    .expect("loopback listener");
    let state = ControlState {
        endpoint,
        supervisor_session_id: "session-1".into(),
        child_generation: 7,
        owner_identity: "owner-1".into(),
        secret: "controller-secret".into(),
    };

    let _core_commands = kepler_backend::engine_control::start_core_control_with_secret(
        state.clone(),
        b"core-secret".to_vec(),
    )
    .await
    .expect("core ready");
    assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));
    let mut wrong = state.clone();
    wrong.secret = "wrong".into();
    assert_eq!(
        kepler_backend::engine_control::send_control(&wrong, ControlMessage::RestartRequested)
            .await,
        Err(ControlError::Unauthorized)
    );
    server.abort();
}

#[tokio::test]
async fn core_control_connection_receives_one_forwarded_stop_and_concurrent_duplicate_is_rejected()
{
    let (mut server, endpoint) = ControlServer::bind(
        "session-2".into(),
        3,
        "owner-2".into(),
        b"controller-secret".to_vec(),
        b"core-secret".to_vec(),
    )
    .await
    .expect("loopback listener");
    let state = ControlState {
        endpoint,
        supervisor_session_id: "session-2".into(),
        child_generation: 3,
        owner_identity: "owner-2".into(),
        secret: "controller-secret".into(),
    };
    let mut core_commands = kepler_backend::engine_control::start_core_control_with_secret(
        state.clone(),
        b"core-secret".to_vec(),
    )
    .await
    .expect("core ready connection");
    assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));

    let first_state = state.clone();
    let second_state = state.clone();
    let first = tokio::spawn(async move {
        kepler_backend::engine_control::send_control(&first_state, ControlMessage::RestartRequested)
            .await
    });
    let second = tokio::spawn(async move {
        kepler_backend::engine_control::send_control(
            &second_state,
            ControlMessage::RestartRequested,
        )
        .await
    });
    let first_result = first.await.expect("first controller");
    let second_result = second.await.expect("second controller");
    assert!(matches!(
        [first_result, second_result],
        [Ok(()), Err(ControlError::OutOfOrder)] | [Err(ControlError::OutOfOrder), Ok(())]
    ));
    assert_eq!(server.recv().await, Some(ControlMessage::RestartRequested));
    assert_eq!(
        core_commands.recv().await,
        Some(ControlMessage::RestartRequested)
    );
    server.abort();
}

#[tokio::test]
async fn core_ready_handshake_times_out_on_stalled_peer() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("stalled listener");
    let endpoint = listener.local_addr().expect("listener address").to_string();
    let peer = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("accept stalled peer");
        tokio::time::sleep(kepler_backend::engine_control::CONTROL_IO_TIMEOUT * 2).await;
    });
    let state = ControlState {
        endpoint,
        supervisor_session_id: "stalled-session".into(),
        child_generation: 1,
        owner_identity: "owner-1".into(),
        secret: "controller-secret".into(),
    };
    let started = Instant::now();
    let result = kepler_backend::engine_control::start_core_control_with_secret(
        state,
        b"core-secret".to_vec(),
    )
    .await;
    assert!(matches!(result, Err(ControlError::Timeout)));
    assert!(started.elapsed() < kepler_backend::engine_control::CONTROL_IO_TIMEOUT * 2);
    peer.abort();
}
