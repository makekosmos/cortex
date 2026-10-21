async fn free_loopback_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    listener.local_addr().unwrap().port()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn seed_client_adopts_authenticated_peer_identity() {
    let port = free_loopback_port().await;
    let (server_storage, server_endpoint) = make_storage_with_identity("device-server").await;
    let (client_storage, client_endpoint) = make_storage_with_identity("device-client").await;
    pair_storages(
        &server_storage,
        "device-server",
        &server_endpoint,
        &client_storage,
        "device-client",
        &client_endpoint,
    )
    .await;
    let server = SyncServer::new(server_storage.arc() as Arc<dyn SyncRuntimeStorage>);
    server
        .start_with_addr(
            "space-x",
            "device-server",
            Some("Server"),
            Some(vec![]),
            &format!("127.0.0.1:{port}"),
        )
        .await
        .unwrap();

    let client = new_direct_client(
        client_storage.arc() as Arc<dyn SyncRuntimeStorage>,
        PeerRecord {
            device_id: "seed-bootstrap".into(),
            device_name: "Bootstrap".into(),
            addresses: vec![format!("127.0.0.1:{port}")],
            last_seen: chrono::Utc::now().to_rfc3339(),
            last_address: None,
            platform: None,
            app_version: None,
        },
        "device-client".into(),
        "Client".into(),
        "space-x".into(),
        vec![],
        None,
    );
    client.start();

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let peer = client.current_peer().await;
        if peer.device_id == "device-server" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "client peer record stuck at {:?} after authentication",
            peer.device_id
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(client.peer_device_id(), "device-server");

    client.stop_and_join().await;
    server.stop().await;
    server_storage.shutdown().await;
    client_storage.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_address_is_pruned_but_other_addresses_keep_retrying() {
    // `self_server` deliberately runs on an address the client treats as its
    // own — simulates a peer record whose address now resolves to our own
    // listener (peer's old DHCP lease).
    let self_port = free_loopback_port().await;
    let self_storage = make_storage("device-me").await;
    let self_server = SyncServer::new(self_storage.arc() as Arc<dyn SyncRuntimeStorage>);
    self_server
        .start_with_addr(
            "space-x",
            "device-me",
            Some("Self"),
            Some(vec![]),
            &format!("127.0.0.1:{self_port}"),
        )
        .await
        .unwrap();

    let self_addr = format!("127.0.0.1:{self_port}");
    let dead_addr = "127.0.0.1:1".to_string(); // closed port — connect refused
    let client = new_direct_client(
        self_storage.arc() as Arc<dyn SyncRuntimeStorage>,
        PeerRecord {
            device_id: "peer-1".into(),
            device_name: "Peer".into(),
            addresses: vec![self_addr.clone(), dead_addr.clone()],
            last_seen: chrono::Utc::now().to_rfc3339(),
            last_address: None,
            platform: None,
            app_version: None,
        },
        "device-me".into(),
        "Me".into(),
        "space-x".into(),
        vec![self_addr.clone()], // own_addresses — client must skip this
        None,
    );
    client.start();

    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    loop {
        let peer = client.current_peer().await;
        if !peer.addresses.contains(&self_addr) {
            break;
        }
        assert!(
            !client.is_stopped(),
            "self address killed the whole client although other \
             addresses remained"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "self address never pruned from the peer record"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(!client.is_stopped());
    assert_eq!(client.current_peer().await.addresses, vec![dead_addr]);

    client.stop_and_join().await;
    self_server.stop().await;
    self_storage.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_only_record_still_stops_the_client() {
    // A record whose every address is ours is a true self-reference: the
    // client should still give up entirely in that case.
    let self_port = free_loopback_port().await;
    let self_storage = make_storage("device-me").await;
    let self_server = SyncServer::new(self_storage.arc() as Arc<dyn SyncRuntimeStorage>);
    self_server
        .start_with_addr(
            "space-x",
            "device-me",
            Some("Self"),
            Some(vec![]),
            &format!("127.0.0.1:{self_port}"),
        )
        .await
        .unwrap();

    let self_addr = format!("127.0.0.1:{self_port}");
    let client = new_direct_client(
        self_storage.arc() as Arc<dyn SyncRuntimeStorage>,
        PeerRecord {
            device_id: "peer-self".into(),
            device_name: "Phantom".into(),
            addresses: vec![self_addr.clone()],
            last_seen: chrono::Utc::now().to_rfc3339(),
            last_address: None,
            platform: None,
            app_version: None,
        },
        "device-me".into(),
        "Me".into(),
        "space-x".into(),
        vec![self_addr],
        None,
    );
    client.start();

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !client.is_stopped() {
        assert!(
            std::time::Instant::now() < deadline,
            "client never gave up on a pure self-reference record"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    client.stop_and_join().await;
    self_server.stop().await;
    self_storage.shutdown().await;
}
