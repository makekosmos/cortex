use ark_application::sync_storage::{PeerProgressPort, SyncEntityMutationPort, SyncEntityReadPort};
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]

async fn round_trip_sync_between_two_servers() {
    // ---------------------------------------------------------------------
    // Server A — holds one todo already; Server B — empty.
    // After sync, B should contain the todo.
    // ---------------------------------------------------------------------
    let (storage_a, endpoint_a) = make_storage_with_identity("device-a");
    let (storage_b, endpoint_b) = make_storage_with_identity("device-b");
    pair_storages(
        &storage_a,
        "device-a",
        &endpoint_a,
        &storage_b,
        "device-b",
        &endpoint_b,
    )
    .await;

    // Seed one todo on side A.
    let seed = todo_entity("entity-a-1", "Seeded on A", "device-a", 1);
    storage_a.apply_entity(&seed).await.unwrap();

    // Also stamp it into the local version vector so load_entities emits the
    // right HLC. Seed application-owned progress as the sync server would
    // normally do this.
    let mut vector_a: VersionVector = HashMap::new();
    vector_a.insert(seed.id.clone(), seed.hlc.clone());
    storage_a.replace_version_vector(&vector_a).await.unwrap();

    let server_a = Arc::new(SyncServer::new(
        storage_a.clone() as Arc<dyn SyncRuntimeStorage>
    ));
    let server_b = Arc::new(SyncServer::new(
        storage_b.clone() as Arc<dyn SyncRuntimeStorage>
    ));

    // Use two separate loopback ports.
    let port_a = pick_port().await;
    let port_b = pick_port().await;

    let server_a_changes: Arc<Mutex<Vec<SyncEntity>>> = Arc::new(Mutex::new(Vec::new()));
    let server_b_changes: Arc<Mutex<Vec<SyncEntity>>> = Arc::new(Mutex::new(Vec::new()));
    let server_a_peers: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    {
        let changes = server_a_changes.clone();
        server_a
            .set_on_change(Arc::new(move |e| {
                let changes = changes.clone();
                tokio::spawn(async move {
                    changes.lock().await.push(e);
                });
            }))
            .await;
    }
    {
        let changes = server_b_changes.clone();
        server_b
            .set_on_change(Arc::new(move |e| {
                let changes = changes.clone();
                tokio::spawn(async move {
                    changes.lock().await.push(e);
                });
            }))
            .await;
    }
    {
        let peers = server_a_peers.clone();
        server_a
            .set_on_peer_connect(Arc::new(move |id| {
                let peers = peers.clone();
                tokio::spawn(async move {
                    peers.lock().await.push(id);
                });
            }))
            .await;
    }

    server_a
        .start_with_addr(
            "space-int",
            "device-a",
            Some("Alpha"),
            Some(vec![format!("127.0.0.1:{port_a}")]),
            &format!("127.0.0.1:{port_a}"),
        )
        .await
        .expect("server A start");

    server_b
        .start_with_addr(
            "space-int",
            "device-b",
            Some("Beta"),
            Some(vec![format!("127.0.0.1:{port_b}")]),
            &format!("127.0.0.1:{port_b}"),
        )
        .await
        .expect("server B start");

    // ---------------------------------------------------------------------
    // DirectClient on side B dialing server A
    // ---------------------------------------------------------------------
    let peer = PeerRecord {
        device_id: "device-a".to_string(),
        device_name: "Alpha".to_string(),
        addresses: vec![format!("127.0.0.1:{port_a}")],
        last_seen: chrono::Utc::now().to_rfc3339(),
        last_address: None,
        platform: None,
        app_version: None,
    };
    let client = Arc::new(new_direct_client(
        storage_b.clone() as Arc<dyn SyncRuntimeStorage>,
        peer,
        "device-b".to_string(),
        "Beta".to_string(),
        "space-int".to_string(),
        vec![format!("127.0.0.1:{port_b}")],
        None,
    ));
    client.start();

    let loaded_b = wait_for_entities(storage_b.as_ref(), |entities| {
        entities.iter().any(|entity| entity.id == "entity-a-1")
    })
    .await;
    assert!(
        loaded_b.iter().any(|e| e.id == "entity-a-1"),
        "side B should have received seeded todo after initial sync; got {:?}",
        loaded_b.iter().map(|e| &e.id).collect::<Vec<_>>(),
    );

    // Side A peer-connect callback should have fired at least once.
    assert!(
        !server_a_peers.lock().await.is_empty(),
        "server A should have received a peer_connected event",
    );

    // ---------------------------------------------------------------------
    // Live change: broadcast from server A → should appear on B.
    // ---------------------------------------------------------------------
    let live = todo_entity("entity-a-2", "Live from A", "device-a", 2);
    storage_a.apply_entity(&live).await.unwrap();
    server_a.broadcast_live_change(None).await;

    let loaded_b_after = wait_for_entities(storage_b.as_ref(), |entities| {
        entities.iter().any(|entity| entity.id == "entity-a-2")
    })
    .await;
    assert!(
        loaded_b_after.iter().any(|e| e.id == "entity-a-2"),
        "side B should have received live change; got {:?}",
        loaded_b_after.iter().map(|e| &e.id).collect::<Vec<_>>(),
    );

    client.stop();
    server_a.stop().await;
    server_b.stop().await;

    println!("INTEGRATION OK");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_connect_is_rejected() {
    let storage = make_storage("device-self");
    let server = Arc::new(SyncServer::new(
        storage.clone() as Arc<dyn SyncRuntimeStorage>
    ));

    let port = pick_port().await;
    server
        .start_with_addr(
            "space-s",
            "device-self",
            Some("Self"),
            Some(vec![format!("127.0.0.1:{port}")]),
            &format!("127.0.0.1:{port}"),
        )
        .await
        .expect("server start");

    let peer = PeerRecord {
        device_id: "some-other-id".to_string(),
        device_name: "FakePeer".to_string(),
        addresses: vec![format!("127.0.0.1:{port}")],
        last_seen: chrono::Utc::now().to_rfc3339(),
        last_address: None,
        platform: None,
        app_version: None,
    };
    // The DirectClient will connect and send a hello with device_id = "device-self"
    // — same as the server. The server must reject the connection and NOT
    // emit a peer_connected event.
    let client = Arc::new(new_direct_client(
        storage.clone() as Arc<dyn SyncRuntimeStorage>,
        peer,
        "device-self".to_string(),
        "Self".to_string(),
        "space-s".to_string(),
        vec![format!("127.0.0.1:{port}")],
        None,
    ));

    let connect_counter = Arc::new(Mutex::new(0usize));
    {
        let counter = connect_counter.clone();
        server
            .set_on_peer_connect(Arc::new(move |_id| {
                let counter = counter.clone();
                tokio::spawn(async move {
                    *counter.lock().await += 1;
                });
            }))
            .await;
    }

    client.start();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let connected = server.connected_peer_count().await;
    assert_eq!(
        connected, 0,
        "server must have zero authenticated peers after self-connect",
    );
    assert_eq!(
        *connect_counter.lock().await,
        0,
        "server must not emit peer_connected for a self hello",
    );

    client.stop();
    server.stop().await;
}
