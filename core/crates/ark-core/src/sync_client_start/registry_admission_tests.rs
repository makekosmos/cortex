use super::*;
use crate::application::sync_storage::SyncEntityReadPort;
use crate::replication_storage::PeerIdentityBackend;
use crate::sync_server::SyncServer;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn registry_backend(device_id: &str) -> (Arc<crate::db::SqliteStorageBackend>, iroh::SecretKey) {
    let connection = rusqlite::Connection::open_in_memory().expect("open test database");
    crate::db::init_schema(&connection).expect("initialize test schema");
    let secret = crate::iroh_transport::load_or_generate_secret_key(&connection)
        .expect("persist local signing identity");
    let backend = Arc::new(crate::db::SqliteStorageBackend::for_test(Arc::new(
        Mutex::new(connection),
    )));
    backend
        .set_device_id(device_id)
        .expect("set local device id");
    (backend, secret)
}

#[tokio::test]
async fn direct_client_and_server_exchange_registry_before_sync_readiness() {
    let (client_storage, client_key) = registry_backend("client-device");
    let (server_storage, server_key) = registry_backend("server-device");
    let (observer_storage, observer_key) = registry_backend("observer-device");
    client_storage
        .enroll_paired_device(
            "client-device",
            "server-device",
            &server_key.public().to_string(),
        )
        .await
        .expect("client must have a signed server enrollment");
    server_storage
        .enroll_paired_device(
            "server-device",
            "client-device",
            &client_key.public().to_string(),
        )
        .await
        .expect("server must have a signed client enrollment");
    observer_storage
        .enroll_paired_device(
            "observer-device",
            "server-device",
            &server_key.public().to_string(),
        )
        .await
        .expect("observer must have a signed server enrollment");
    server_storage
        .enroll_paired_device(
            "server-device",
            "observer-device",
            &observer_key.public().to_string(),
        )
        .await
        .expect("server must have a signed observer enrollment");

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind port");
    let address = listener.local_addr().expect("read port");
    drop(listener);
    let sync_server =
        SyncServer::new(server_storage.clone() as Arc<dyn crate::sync_server::SyncRuntimeStorage>);
    sync_server
        .start_with_addr(
            "space-test",
            "server-device",
            Some("Server"),
            None,
            &address.to_string(),
        )
        .await
        .expect("start direct server");

    let (connected_tx, connected_rx) = tokio::sync::oneshot::channel();
    let connected_tx = Arc::new(Mutex::new(Some(connected_tx)));
    let callback_tx = connected_tx.clone();
    let client = SyncClient::new(
        client_storage.clone() as Arc<dyn crate::sync_server::SyncRuntimeStorage>,
        PeerRecord {
            device_id: "server-device".into(),
            device_name: "Server".into(),
            addresses: vec![address.to_string()],
            last_seen: String::new(),
            last_address: None,
            platform: None,
            app_version: None,
        },
        "client-device".into(),
        "Client".into(),
        "space-test".into(),
        Vec::new(),
        None,
    );
    client
        .set_on_connected(Arc::new(move |peer| {
            if let Some(connected_tx) = callback_tx.lock().unwrap().take() {
                let _ = connected_tx.send(peer);
            }
        }))
        .await;
    client.start();

    let connected_peer = tokio::time::timeout(Duration::from_secs(5), connected_rx)
        .await
        .expect("signed admission should finish promptly")
        .expect("connection callback should run after admission");
    assert_eq!(connected_peer.device_id, "server-device");
    tokio::time::timeout(Duration::from_secs(2), async {
        while sync_server.connected_peer_count().await != 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("server readiness should follow the mutual registry digest");
    assert_eq!(sync_server.connected_peer_count().await, 1);

    let observer = SyncClient::new(
        observer_storage.clone() as Arc<dyn crate::sync_server::SyncRuntimeStorage>,
        PeerRecord {
            device_id: "server-device".into(),
            device_name: "Server".into(),
            addresses: vec![address.to_string()],
            last_seen: String::new(),
            last_address: None,
            platform: None,
            app_version: None,
        },
        "observer-device".into(),
        "Observer".into(),
        "space-test".into(),
        Vec::new(),
        None,
    );
    observer.start();
    tokio::time::timeout(Duration::from_secs(5), async {
        while sync_server.connected_peer_count().await != 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("second direct peer should complete signed admission");

    let additional_key = iroh::SecretKey::generate();
    server_storage
        .enroll_paired_device(
            "server-device",
            "additional-device",
            &additional_key.public().to_string(),
        )
        .await
        .expect("commit a signed event after admission");
    let (_, _, server_events) = server_storage
        .load_device_registry_events_page(0, None, 32)
        .await
        .expect("load server registry events");
    let update = server_events
        .last()
        .expect("additional enrollment should append one event")
        .clone();
    sync_server.broadcast_registry_event(update.clone()).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (_, _, events) = client_storage
                .load_device_registry_events_page(0, None, 32)
                .await
                .expect("load client registry events");
            if events
                .iter()
                .any(|event| event.event_id() == update.event_id())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("connected direct peer should apply signed registry update");

    let child_key = iroh::SecretKey::generate();
    client_storage
        .enroll_paired_device(
            "client-device",
            "client-child",
            &child_key.public().to_string(),
        )
        .await
        .expect("client should commit a registry event for relay");
    let (_, _, client_events) = client_storage
        .load_device_registry_events_page(0, None, 32)
        .await
        .expect("load client registry history");
    let relayed_event = client_events
        .iter()
        .find(|event| event.target_device_id().as_str() == "client-child")
        .expect("client child enrollment should append one event")
        .clone();
    client.broadcast_registry_event(relayed_event.clone()).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (_, _, events) = observer_storage
                .load_device_registry_events_page(0, None, 32)
                .await
                .expect("load observer registry history");
            if events
                .iter()
                .any(|event| event.event_id() == relayed_event.event_id())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("server should relay a newly applied event to another direct peer");

    let server_head = server_events
        .iter()
        .rfind(|event| event.actor_device_id().as_str() == "server-device")
        .expect("server should own its local enrollment history");
    let server_public_key = ed25519_dalek::VerifyingKey::from_bytes(server_key.public().as_bytes())
        .expect("server signing public key should be valid");
    let previous_event_hash = server_head
        .verify(
            &crate::domain::device_registry::DeviceId::parse("server-device").unwrap(),
            server_head.actor_key_epoch(),
            &server_public_key,
        )
        .unwrap()
        .event_hash()
        .unwrap();
    let disable_client = crate::domain::device_registry::RegistryEvent::sign(
        crate::domain::device_registry::RegistryEventDraft {
            actor_device_id: crate::domain::device_registry::DeviceId::parse("server-device")
                .unwrap(),
            actor_sequence: server_head.actor_sequence() + 1,
            previous_event_hash: Some(previous_event_hash),
            target_device_id: crate::domain::device_registry::DeviceId::parse("client-device")
                .unwrap(),
            actor_key_epoch: server_head.actor_key_epoch(),
            actor_admission_epoch: server_head.actor_admission_epoch(),
            hlc: crate::hlc::HLC::now("server-device").to_string(),
            operation: crate::domain::device_registry::RegistryOperation::Disable,
        },
        &ed25519_dalek::SigningKey::from_bytes(&server_key.to_bytes()),
    )
    .unwrap();
    server_storage
        .apply_device_registry_events("server-device", std::slice::from_ref(&disable_client))
        .await
        .expect("commit a signed disable before notifying peers");
    sync_server
        .broadcast_registry_event(disable_client.clone())
        .await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (_, _, client_events) = client_storage
                .load_device_registry_events_page(0, None, 32)
                .await
                .expect("load client registry after revocation");
            if client_events
                .iter()
                .any(|event| event.event_id() == disable_client.event_id())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("connected direct peer should apply the signed disable event");
    assert_eq!(sync_server.connected_peer_count().await, 1);
    let (_, _, client_events) = client_storage
        .load_device_registry_events_page(0, None, 32)
        .await
        .expect("load client registry after revocation");
    assert!(client_events
        .iter()
        .any(|event| event.event_id() == disable_client.event_id()));

    client.stop_and_join().await;
    observer.stop_and_join().await;
    sync_server.stop().await;
}

#[tokio::test]
async fn direct_server_sends_no_data_or_ack_before_registry_admission() {
    let (server_storage, _server_key) = registry_backend("server-device");
    let client_key = iroh::SecretKey::generate();
    server_storage
        .enroll_paired_device(
            "server-device",
            "client-device",
            &client_key.public().to_string(),
        )
        .await
        .expect("server must trust the paired client identity");

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind port");
    let address = listener.local_addr().expect("read port");
    drop(listener);
    let sync_server =
        SyncServer::new(server_storage.clone() as Arc<dyn crate::sync_server::SyncRuntimeStorage>);
    sync_server
        .start_with_addr(
            "space-test",
            "server-device",
            Some("Server"),
            None,
            &address.to_string(),
        )
        .await
        .expect("start direct server");

    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}"))
        .await
        .expect("connect direct client");
    let offer = ProtocolOffer::current_with_registry_history(true);
    socket
        .send(Message::Text(
            serialize_message(&LanSyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: "client-device".into(),
                device_name: "Client".into(),
                space_id: "space-test".into(),
                addresses: None,
                auth_nonce: None,
                auth_hmac: None,
                platform: None,
                app_version: None,
                supported_protocol_versions: Some(offer.version_ids()),
                supported_capabilities: Some(offer.capability_ids()),
                negotiated_capabilities: None,
                negotiation_hmac: None,
            })
            .into(),
        ))
        .await
        .expect("send client Hello");
    let hello = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("server Hello timeout")
        .expect("server closed before Hello")
        .expect("read server Hello");
    let Message::Text(text) = hello else {
        panic!("expected the server Hello frame");
    };
    assert!(matches!(
        deserialize_message(&text),
        Some(LanSyncMessage::Hello { .. })
    ));
    assert_no_data_frame(&mut socket).await;

    let entity = SyncEntity {
        entity_type: "todo".into(),
        id: "pre-admission-object".into(),
        data: serde_json::Map::new(),
        hlc: "2026-10-16T00:00:00.000Z:000001:client-device".into(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    };
    socket
        .send(Message::Text(
            serialize_message(&LanSyncMessage::SyncChanges {
                batch_id: "pre-admission-batch".into(),
                entities: vec![entity.clone()],
                is_last: true,
                origin_device_id: None,
                usage_complete_through: None,
            })
            .into(),
        ))
        .await
        .expect("send premature entity batch");
    assert_no_data_frame(&mut socket).await;
    assert!(
        server_storage
            .load_entities(&VersionVector::new())
            .await
            .expect("read server entities")
            .iter()
            .all(|stored| stored.id != entity.id),
        "the server must not apply an entity before registry admission"
    );

    socket.close(None).await.expect("close websocket");
    sync_server.stop().await;
}

async fn assert_no_data_frame(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) {
    let frame = tokio::time::timeout(Duration::from_millis(100), socket.next()).await;
    assert!(
        !matches!(
            frame,
            Ok(Some(Ok(Message::Text(ref text))))
                if matches!(
                    deserialize_message(text),
                    Some(
                        LanSyncMessage::VersionVector { .. }
                            | LanSyncMessage::SyncChanges { .. }
                            | LanSyncMessage::LiveChange { .. }
                            | LanSyncMessage::PeerList { .. }
                            | LanSyncMessage::SyncAck { .. }
                            | LanSyncMessage::LiveAck { .. }
                            | LanSyncMessage::LiveReceiptAck { .. }
                    )
                )
        ),
        "the pending direct session emitted or acknowledged user data: {frame:?}"
    );
}
