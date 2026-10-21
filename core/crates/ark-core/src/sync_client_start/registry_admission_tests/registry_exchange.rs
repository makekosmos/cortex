#[tokio::test]
async fn direct_client_and_server_exchange_registry_before_sync_readiness() {
    let (client_storage, client_key, _client_database) = registry_backend("client-device").await;
    let (server_storage, server_key, _server_database) = registry_backend("server-device").await;
    let (observer_storage, observer_key, _observer_database) =
        registry_backend("observer-device").await;
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
    let client = create_direct_client(
        client_storage.clone() as Arc<dyn crate::sync_server::SyncRuntimeStorage>,
        DirectClientConfig::new(
            PeerRecord {
                device_id: "seed-server-device".into(),
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
        ),
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
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if server_storage
                .accepted_policy_identity_for_peer("client-device")
                .await
                .expect("load accepted client policy")
                .is_some()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("client policy must arrive after authenticating its seed peer record");

    let observer = create_direct_client(
        observer_storage.clone() as Arc<dyn crate::sync_server::SyncRuntimeStorage>,
        DirectClientConfig::new(
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
        ),
    );
    observer.start();
    tokio::time::timeout(Duration::from_secs(5), async {
        while sync_server.connected_peer_count().await != 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("second direct peer should complete signed admission");

    let additional_key = ark_net::iroh::SecretKey::generate();
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

    let child_key = ark_net::iroh::SecretKey::generate();
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
            &ark_domain::device_registry::DeviceId::parse("server-device").unwrap(),
            server_head.actor_key_epoch(),
            &server_public_key,
        )
        .unwrap()
        .event_hash()
        .unwrap();
    let disable_client = ark_domain::device_registry::RegistryEvent::sign(
        ark_domain::device_registry::RegistryEventDraft {
            actor_device_id: ark_domain::device_registry::DeviceId::parse("server-device").unwrap(),
            actor_sequence: server_head.actor_sequence() + 1,
            previous_event_hash: Some(previous_event_hash),
            target_device_id: ark_domain::device_registry::DeviceId::parse("client-device")
                .unwrap(),
            actor_key_epoch: server_head.actor_key_epoch(),
            actor_admission_epoch: server_head.actor_admission_epoch(),
            hlc: crate::hlc::HLC::now("server-device").to_string(),
            operation: ark_domain::device_registry::RegistryOperation::Disable,
        },
        &ed25519_dalek::SigningKey::from_bytes(&server_key.to_bytes()),
    )
    .unwrap();
    client.stop_and_join().await;
    server_storage
        .apply_device_registry_events("server-device", std::slice::from_ref(&disable_client))
        .await
        .expect("commit signed disable while the device is offline");
    let server_only_entity = SyncEntity {
        entity_type: "tracked_app".into(),
        id: "revoked-client-must-not-receive".into(),
        data: serde_json::json!({
            "platform": "linux",
            "exePath": "/opt/revoked-client-must-not-receive",
            "normalizedExePath": "/opt/revoked-client-must-not-receive",
            "processName": "revoked-client-must-not-receive",
            "displayName": null,
            "publisher": null,
            "iconRef": null,
            "firstSeenAt": "2026-10-16T00:00:00Z",
            "lastSeenAt": "2026-10-16T00:00:00Z"
        })
        .as_object()
        .expect("entity payload should be an object")
        .clone(),
        hlc: crate::hlc::HLC::now("server-device").to_string(),
        deleted: None,
        origin_device_id: Some("server-device".into()),
        origin_seq: Some(1),
    };
    server_storage
        .apply_entity(&server_only_entity)
        .await
        .expect("seed user data that a mistakenly admitted peer could receive");
    let connected_callbacks = Arc::new(AtomicUsize::new(0));
    let reconnecting_client = create_direct_client(
        client_storage.clone() as Arc<dyn crate::sync_server::SyncRuntimeStorage>,
        DirectClientConfig::new(
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
        ),
    );
    let callback_count = connected_callbacks.clone();
    reconnecting_client
        .set_on_connected(Arc::new(move |_| {
            callback_count.fetch_add(1, Ordering::SeqCst);
        }))
        .await;
    reconnecting_client.start();
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
    .expect("offline device should learn its signed disable during control catch-up");
    tokio::time::timeout(Duration::from_secs(2), async {
        while !reconnecting_client.is_stopped() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("revoked device should stop reconnecting after control catch-up");
    assert_eq!(sync_server.connected_peer_count().await, 1);
    reconnecting_client.stop_and_join().await;
    assert_eq!(sync_server.connected_peer_count().await, 1);
    let (_, _, client_events) = client_storage
        .load_device_registry_events_page(0, None, 32)
        .await
        .expect("load client registry after revocation");
    assert!(client_events
        .iter()
        .any(|event| event.event_id() == disable_client.event_id()));
    assert_eq!(
        connected_callbacks.load(Ordering::SeqCst),
        0,
        "a disabled peer must not become data-ready during registry catch-up"
    );
    assert!(client_storage
        .load_entities(&VersionVector::new())
        .await
        .expect("load client entities after control catch-up")
        .iter()
        .all(|entity| entity.id != server_only_entity.id));

    observer.stop_and_join().await;
    sync_server.stop().await;
    server_storage
        .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();
    client_storage
        .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();
    observer_storage
        .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();
}
