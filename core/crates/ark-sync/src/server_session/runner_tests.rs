use super::*;
use std::{num::NonZeroUsize, time::Duration};
use tokio::sync::oneshot;

#[tokio::test]
async fn transport_failure_is_reported_by_runner() {
    use crate::server_session::ServerSessionHandle;

    let (handle, mut actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    let mut session_io = SessionIo::start_transport(1, |driver| {
        tokio::spawn(async move {
            let _ = driver
                .incoming
                .send(SessionIoEvent::Failed("read failed".to_owned()))
                .await;
            driver.worker_completion.complete();
        })
    })
    .unwrap();

    assert_eq!(
        run_connected_server_session_controlled(&mut session_io, &mut actor).await,
        DirectServerSessionEnd::Failed("read failed".to_owned())
    );
    session_io.finish().await.unwrap();
    handle.revoke();
}

#[tokio::test]
async fn full_control_queue_fails_closed_on_ping() {
    use crate::server_session::ServerSessionHandle;

    let (handle, mut actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    let (send_ping_tx, send_ping_rx) = oneshot::channel();
    let (finish_tx, finish_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(1, |driver| {
        let _control_outgoing = driver.control_outgoing;
        tokio::spawn(async move {
            let _control_outgoing = _control_outgoing;
            let _ = send_ping_rx.await;
            let _ = driver
                .incoming
                .send(SessionIoEvent::Message(Box::new(LanSyncMessage::Ping {
                    ts: 42,
                })))
                .await;
            let _ = finish_rx.await;
            driver.worker_completion.complete();
        })
    })
    .unwrap();
    session_io
        .sender()
        .try_send(LanSyncMessage::Pong { ts: 1 })
        .unwrap();
    send_ping_tx.send(()).unwrap();

    let result = run_connected_server_session_controlled(&mut session_io, &mut actor).await;
    finish_tx.send(()).unwrap();
    assert_eq!(
        result,
        DirectServerSessionEnd::Failed("OutboundFull".to_owned())
    );
    session_io.finish().await.unwrap();
    handle.revoke();
}

#[tokio::test]
async fn revoke_wakes_runner_when_intent_mailbox_is_full() {
    use crate::server_session::{ServerSessionHandle, ServerSessionIntent};

    let (handle, mut actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    handle
        .try_send(ServerSessionIntent::WakeCommittedFeed)
        .unwrap();

    let (close_seen_tx, close_seen_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(1, |mut driver| {
        tokio::spawn(async move {
            let _ = driver.close_after_flush.recv().await;
            let _ = close_seen_tx.send(());
            let _ = driver.close_result.send(Some(Ok(())));
            driver.worker_completion.complete();
        })
    })
    .unwrap();
    handle.close_after_flush();

    assert_eq!(
        run_connected_server_session_controlled(&mut session_io, &mut actor).await,
        DirectServerSessionEnd::Closed
    );
    close_seen_rx.await.unwrap();
    session_io.finish().await.unwrap();
}

#[tokio::test]
async fn delayed_peer_list_waits_until_due_and_sends_once() {
    use crate::server_session::ServerSessionHandle;

    let (handle, mut actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    actor.schedule_peer_list_at(
        tokio::time::Instant::now() + Duration::from_millis(50),
        vec![test_peer_record()],
    );

    let (peer_list_tx, mut peer_list_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(1, |mut driver| {
        tokio::spawn(async move {
            let first = driver.control_outgoing.recv().await;
            let _ = driver.incoming.send(SessionIoEvent::Closed).await;
            let second =
                tokio::time::timeout(Duration::from_millis(40), driver.control_outgoing.recv())
                    .await
                    .ok()
                    .flatten();
            let _ = peer_list_tx.send((first, second));
            driver.worker_completion.complete();
        })
    })
    .unwrap();

    let run = tokio::spawn(async move {
        let end = run_connected_server_session_controlled(&mut session_io, &mut actor).await;
        session_io.finish().await.unwrap();
        end
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(15), &mut peer_list_rx)
            .await
            .is_err()
    );
    let (first, second) = peer_list_rx.await.unwrap();
    assert!(matches!(first, Some(LanSyncMessage::PeerList { .. })));
    assert!(second.is_none());
    assert_eq!(run.await.unwrap(), DirectServerSessionEnd::Closed);
    handle.revoke();
}

#[tokio::test]
async fn full_peer_list_queue_does_not_block_inbound_close() {
    use crate::server_session::ServerSessionHandle;

    let (handle, mut actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    actor.schedule_peer_list_at(tokio::time::Instant::now(), vec![test_peer_record()]);
    let mut session_io = SessionIo::start_transport(1, |driver| {
        tokio::spawn(async move {
            let _control_outgoing = driver.control_outgoing;
            let _bulk_outgoing = driver.outgoing;
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = driver.incoming.send(SessionIoEvent::Closed).await;
            driver.worker_completion.complete();
        })
    })
    .unwrap();
    session_io
        .sender()
        .try_send(LanSyncMessage::Pong { ts: 1 })
        .unwrap();

    let end = tokio::time::timeout(
        Duration::from_millis(100),
        run_connected_server_session_controlled(&mut session_io, &mut actor),
    )
    .await
    .expect("inbound close must be selected while peer-list retry is pending");
    assert_eq!(end, DirectServerSessionEnd::Closed);
    session_io.finish().await.unwrap();
    handle.revoke();
}

#[tokio::test]
async fn full_peer_list_queue_services_malformed_input_then_retries_once() {
    use crate::server_session::{ConnectedServerSessionHost, ServerSessionHandle};
    use ark_domain::{device_registry::RegistryEvent, types::SyncEntity};
    use std::sync::Arc;

    struct MalformedNotifier(tokio::sync::mpsc::UnboundedSender<()>);

    #[async_trait::async_trait]
    impl ConnectedServerSessionHost for MalformedNotifier {
        async fn broadcast_registry_events(
            &self,
            _: crate::server_session::ServerConnectionLease,
            _: Vec<RegistryEvent>,
        ) {
        }

        async fn revoke_disabled_sessions(
            &self,
            _: Vec<String>,
            _: Option<crate::server_session::ServerConnectionLease>,
            _: bool,
        ) -> bool {
            false
        }

        async fn evict_replaced_sessions(
            &self,
            _: &str,
            _: crate::server_session::ServerConnectionLease,
        ) -> bool {
            false
        }

        async fn prepare_peer_connected(
            &self,
            _: crate::server_session::ServerConnectionLease,
            _: crate::server_session::ServerPeerProjection,
        ) {
        }

        async fn notify_peer_connected(&self, _: String) {}

        async fn registry_events_applied(&self, _: Vec<RegistryEvent>) {}

        async fn peer_list(&self) -> Vec<ark_domain::types::PeerRecord> {
            Vec::new()
        }

        async fn entity_applied(&self, _: &str, _: SyncEntity) {}

        async fn discovered_peers(&self, _: Vec<ark_domain::types::PeerRecord>) {}

        async fn malformed_message(&self) {
            let _ = self.0.send(());
        }

        async fn task_merge_batch_applied(&self) {}
    }

    let (handle, mut actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    let (malformed_tx, mut malformed_rx) = tokio::sync::mpsc::unbounded_channel();
    actor.configure_host(Arc::new(MalformedNotifier(malformed_tx)));
    actor.schedule_peer_list_at(tokio::time::Instant::now(), vec![test_peer_record()]);

    let (transport_ready_tx, transport_ready_rx) = oneshot::channel();
    let (start_transport_tx, start_transport_rx) = oneshot::channel();
    let (release_queue_tx, release_queue_rx) = oneshot::channel();
    let (frames_tx, frames_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(1, |mut driver| {
        tokio::spawn(async move {
            let _bulk_outgoing = driver.outgoing;
            let _ = transport_ready_tx.send(());
            let _ = start_transport_rx.await;
            let _ = driver.incoming.send(SessionIoEvent::MalformedMessage).await;
            let _ = release_queue_rx.await;
            let first = driver.control_outgoing.recv().await;
            let second = driver.control_outgoing.recv().await;
            let duplicate =
                tokio::time::timeout(Duration::from_millis(40), driver.control_outgoing.recv())
                    .await
                    .ok()
                    .flatten();
            let _ = frames_tx.send((first, second, duplicate));
            let _ = driver.incoming.send(SessionIoEvent::Closed).await;
            driver.worker_completion.complete();
        })
    })
    .unwrap();
    transport_ready_rx.await.unwrap();
    session_io
        .sender()
        .try_send(LanSyncMessage::Pong { ts: 1 })
        .unwrap();
    start_transport_tx.send(()).unwrap();

    let run = tokio::spawn(async move {
        let end = run_connected_server_session_controlled(&mut session_io, &mut actor).await;
        session_io.finish().await.unwrap();
        end
    });
    tokio::time::timeout(Duration::from_millis(50), malformed_rx.recv())
        .await
        .expect("malformed input must be handled while peer-list retry is pending")
        .unwrap();
    release_queue_tx.send(()).unwrap();
    let (initial, peer_list, duplicate) = frames_rx.await.unwrap();
    assert!(matches!(initial, Some(LanSyncMessage::Pong { ts: 1 })));
    assert!(matches!(peer_list, Some(LanSyncMessage::PeerList { .. })));
    assert!(duplicate.is_none(), "peer list must be sent exactly once");
    assert_eq!(run.await.unwrap(), DirectServerSessionEnd::Closed);
    handle.revoke();
}

#[tokio::test]
async fn signed_frame_waits_for_bulk_capacity_without_being_dropped() {
    use crate::server_session::{ServerSessionHandle, ServerSessionIntent};

    let (handle, mut actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    let first_frame = signed_frame("first");
    let second_frame = signed_frame("second");
    let (first_seen_tx, first_seen_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let (frames_tx, frames_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(1, |mut driver| {
        tokio::spawn(async move {
            let first = driver.outgoing.recv().await;
            let _ = first_seen_tx.send(first);
            let _ = release_rx.await;
            let second = driver.outgoing.recv().await;
            let _ = frames_tx.send(second);
            let _ = driver.incoming.send(SessionIoEvent::Closed).await;
            driver.worker_completion.complete();
        })
    })
    .unwrap();
    session_io
        .sender()
        .try_send(LanSyncMessage::SignedIntegrationFrame { frame: first_frame })
        .unwrap();
    handle
        .try_send(ServerSessionIntent::SendSignedFrame(second_frame))
        .unwrap();

    let run = tokio::spawn(async move {
        let end = run_connected_server_session_controlled(&mut session_io, &mut actor).await;
        session_io.finish().await.unwrap();
        end
    });
    assert!(matches!(
        first_seen_rx.await.unwrap(),
        Some(LanSyncMessage::SignedIntegrationFrame { frame }) if frame.message_id == "first"
    ));
    release_tx.send(()).unwrap();
    assert!(matches!(
        frames_rx.await.unwrap(),
        Some(LanSyncMessage::SignedIntegrationFrame { frame }) if frame.message_id == "second"
    ));
    assert_eq!(run.await.unwrap(), DirectServerSessionEnd::Closed);
    handle.revoke();
}

fn signed_frame(message_id: &str) -> ark_domain::integration_replication::SignedSyncEnvelope {
    ark_domain::integration_replication::SignedSyncEnvelope::new(
        "space",
        "origin",
        "recipient",
        1,
        message_id,
        Vec::new(),
        "signature",
    )
}

fn test_peer_record() -> ark_domain::types::PeerRecord {
    ark_domain::types::PeerRecord {
        device_id: "peer".to_owned(),
        device_name: "Peer".to_owned(),
        addresses: vec!["127.0.0.1:1".to_owned()],
        last_seen: "2026-10-19T00:00:00.000Z".to_owned(),
        last_address: None,
        platform: None,
        app_version: None,
    }
}
