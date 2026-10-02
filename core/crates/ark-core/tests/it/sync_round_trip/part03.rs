#[tokio::test(flavor = "multi_thread", worker_threads = 2)]

async fn hmac_authenticated_sync_succeeds_with_matching_secret() {
    let storage_a = make_storage("device-a");
    let storage_b = make_storage("device-b");
    storage_a
        .apply_entity(&todo_entity("hmac-entity-a", "Protected", "device-a", 1))
        .await
        .unwrap();

    let server_a = Arc::new(SyncServer::new(storage_a.clone() as Arc<dyn StorageBackend>));
    server_a
        .set_auth_secret(Some("mesh-secret".to_string()))
        .await;

    let port_a = pick_port().await;
    server_a
        .start_with_addr(
            "space-hmac",
            "device-a",
            Some("Alpha"),
            Some(vec![format!("127.0.0.1:{port_a}")]),
            &format!("127.0.0.1:{port_a}"),
        )
        .await
        .expect("server A start");

    let peer = PeerRecord {
        device_id: "device-a".to_string(),
        device_name: "Alpha".to_string(),
        addresses: vec![format!("127.0.0.1:{port_a}")],
        last_seen: chrono::Utc::now().to_rfc3339(),
        last_address: None,
    };
    let client = Arc::new(SyncClient::new(
        storage_b.clone() as Arc<dyn StorageBackend>,
        peer,
        "device-b".to_string(),
        "Beta".to_string(),
        "space-hmac".to_string(),
        vec![],
        Some("mesh-secret".to_string()),
    ));
    client.start();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let loaded_b = storage_b.load_entities(&HashMap::new()).await;
    assert!(
        loaded_b.iter().any(|e| e.id == "hmac-entity-a"),
        "matching HMAC secret should allow initial sync; got {:?}",
        loaded_b.iter().map(|e| &e.id).collect::<Vec<_>>(),
    );
    assert_eq!(
        server_a.connected_peer_count().await,
        1,
        "server should authenticate exactly one matching-secret peer",
    );

    client.stop();
    server_a.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hmac_authenticated_sync_rejects_wrong_secret() {
    let storage_a = make_storage("device-a");
    let storage_b = make_storage("device-b");
    storage_a
        .apply_entity(&todo_entity("hmac-denied-a", "Denied", "device-a", 1))
        .await
        .unwrap();

    let server_a = Arc::new(SyncServer::new(storage_a.clone() as Arc<dyn StorageBackend>));
    server_a
        .set_auth_secret(Some("correct-secret".to_string()))
        .await;

    let port_a = pick_port().await;
    server_a
        .start_with_addr(
            "space-hmac-denied",
            "device-a",
            Some("Alpha"),
            Some(vec![format!("127.0.0.1:{port_a}")]),
            &format!("127.0.0.1:{port_a}"),
        )
        .await
        .expect("server A start");

    let peer = PeerRecord {
        device_id: "device-a".to_string(),
        device_name: "Alpha".to_string(),
        addresses: vec![format!("127.0.0.1:{port_a}")],
        last_seen: chrono::Utc::now().to_rfc3339(),
        last_address: None,
    };
    let client = Arc::new(SyncClient::new(
        storage_b.clone() as Arc<dyn StorageBackend>,
        peer,
        "device-b".to_string(),
        "Beta".to_string(),
        "space-hmac-denied".to_string(),
        vec![],
        Some("wrong-secret".to_string()),
    ));
    client.start();

    tokio::time::sleep(Duration::from_millis(500)).await;

    assert_eq!(
        server_a.connected_peer_count().await,
        0,
        "server must not authenticate a peer with the wrong HMAC secret",
    );
    let loaded_b = storage_b.load_entities(&HashMap::new()).await;
    assert!(
        !loaded_b.iter().any(|e| e.id == "hmac-denied-a"),
        "wrong-secret client must not receive protected sync data",
    );

    client.stop();
    server_a.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn usage_entities_sync_between_two_servers() {
    let storage_a = make_storage("device-a");
    let storage_b = make_storage("device-b");

    let tracked_app = tracked_app_entity("tracked-app-a", "device-a", 1);
    let session = usage_session_entity("usage-session-a", "tracked-app-a", "device-a", 2);
    let event = usage_event_entity(
        "usage-event-a",
        "tracked-app-a",
        "usage-session-a",
        "device-a",
        3,
    );

    storage_a.apply_entity(&tracked_app).await.unwrap();
    storage_a.apply_entity(&session).await.unwrap();
    storage_a.apply_entity(&event).await.unwrap();

    let mut vector_a: VersionVector = HashMap::new();
    vector_a.insert(tracked_app.id.clone(), tracked_app.hlc.clone());
    vector_a.insert(session.id.clone(), session.hlc.clone());
    vector_a.insert(event.id.clone(), event.hlc.clone());
    storage_a
        .set_kv(
            "lan_sync.version_vector",
            &serde_json::to_string(&vector_a).unwrap(),
        )
        .await;

    let server_a = Arc::new(SyncServer::new(storage_a.clone() as Arc<dyn StorageBackend>));
    let server_b = Arc::new(SyncServer::new(storage_b.clone() as Arc<dyn StorageBackend>));

    let port_a = pick_port().await;
    let port_b = pick_port().await;

    server_a
        .start_with_addr(
            "space-usage",
            "device-a",
            Some("Alpha"),
            Some(vec![format!("127.0.0.1:{port_a}")]),
            &format!("127.0.0.1:{port_a}"),
        )
        .await
        .expect("server A start");

    server_b
        .start_with_addr(
            "space-usage",
            "device-b",
            Some("Beta"),
            Some(vec![format!("127.0.0.1:{port_b}")]),
            &format!("127.0.0.1:{port_b}"),
        )
        .await
        .expect("server B start");

    let peer = PeerRecord {
        device_id: "device-a".to_string(),
        device_name: "Alpha".to_string(),
        addresses: vec![format!("127.0.0.1:{port_a}")],
        last_seen: chrono::Utc::now().to_rfc3339(),
        last_address: None,
    };
    let client = Arc::new(SyncClient::new(
        storage_b.clone() as Arc<dyn StorageBackend>,
        peer,
        "device-b".to_string(),
        "Beta".to_string(),
        "space-usage".to_string(),
        vec![format!("127.0.0.1:{port_b}")],
        None,
    ));
    client.start();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let loaded_b = storage_b.load_entities(&HashMap::new()).await;
    assert!(
        loaded_b
            .iter()
            .any(|entity| entity.entity_type == "tracked_app" && entity.id == "tracked-app-a"),
        "side B should receive tracked_app during initial sync; got {:?}",
        loaded_b
            .iter()
            .map(|entity| (&entity.entity_type, &entity.id))
            .collect::<Vec<_>>(),
    );
    assert!(
        loaded_b
            .iter()
            .any(|entity| entity.entity_type == "usage_session" && entity.id == "usage-session-a"),
        "side B should receive usage_session during initial sync; got {:?}",
        loaded_b
            .iter()
            .map(|entity| (&entity.entity_type, &entity.id))
            .collect::<Vec<_>>(),
    );
    assert!(
        loaded_b
            .iter()
            .any(|entity| entity.entity_type == "usage_event" && entity.id == "usage-event-a"),
        "side B should receive usage_event during initial sync; got {:?}",
        loaded_b
            .iter()
            .map(|entity| (&entity.entity_type, &entity.id))
            .collect::<Vec<_>>(),
    );

    let live_event = usage_event_entity(
        "usage-event-live",
        "tracked-app-a",
        "usage-session-a",
        "device-a",
        4,
    );
    storage_a.apply_entity(&live_event).await.unwrap();
    server_a
        .broadcast_live_change(live_event.clone(), None)
        .await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    let loaded_b_after = storage_b.load_entities(&HashMap::new()).await;
    assert!(
        loaded_b_after
            .iter()
            .any(|entity| entity.entity_type == "usage_event" && entity.id == "usage-event-live"),
        "side B should receive live usage_event; got {:?}",
        loaded_b_after
            .iter()
            .map(|entity| (&entity.entity_type, &entity.id))
            .collect::<Vec<_>>(),
    );

    client.stop();
    server_a.stop().await;
    server_b.stop().await;
}
