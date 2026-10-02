use super::*;

#[tokio::test]
async fn disconnect_peer_stops_matching_outbound_client_and_emits_events() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    let shared = get_shared_conn(&state).unwrap();
    let backend = Arc::new(crate::db::SqliteStorageBackend::new(shared));
    backend.set_device_id("device-local").unwrap();

    let peer = PeerRecord {
        device_id: "peer-123".to_string(),
        device_name: "Peer 123".to_string(),
        addresses: vec!["192.168.1.20:21531".to_string()],
        last_seen: "2026-06-17T00:00:00.000Z".to_string(),
        last_address: None,
    };
    let client = Arc::new(SyncClient::new(
        backend.clone() as Arc<dyn crate::sync_server::StorageBackend>,
        peer,
        "device-local".to_string(),
        "Local Device".to_string(),
        "space-a".to_string(),
        vec![],
        None,
    ));

    let clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>> = Arc::new(TokioMutex::new(
        HashMap::from([("peer-123".to_string(), client.clone())]),
    ));
    let runtime = SyncRuntime {
        server: Arc::new(crate::sync_server::SyncServer::new(
            backend.clone() as Arc<dyn crate::sync_server::StorageBackend>
        )),
        storage: backend,
        clients: clients.clone(),
        relay: None,
        transport_choice: Some(TransportChoice::None),
        start_params: SyncStartParams {
            space_id: "space-a".to_string(),
            device_id: "device-local".to_string(),
            device_name: "Local Device".to_string(),
            port: None,
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: false,
            iroh_peer_ticket: None,
            discovery_enabled: true,
            bind: SyncBind::AllInterfaces,
        },
        iroh_our_ticket: None,
        beacon: Arc::new(crate::beacon::BroadcastDiscovery::new()),
        space_id: "space-a".to_string(),
        device_id: "device-local".to_string(),
        device_name: "Local Device".to_string(),
        auth_secret: None,
        own_addresses: Arc::new(TokioMutex::new(Vec::new())),
    };
    *state.sync.lock().await = Some(Arc::new(runtime));

    let mut event_rx = crate::events::subscribe();

    handle_disconnect_peer(&state, "peer-123".to_string())
        .await
        .unwrap();

    assert!(client.is_stopped());
    assert!(clients.lock().await.is_empty());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut saw_disconnect = false;
    let mut saw_list = false;
    while !(saw_disconnect && saw_list) {
        match event_rx.try_recv() {
            Ok(event) => {
                if event["event"] == "peer_disconnected" {
                    assert_eq!(event["device_id"], "peer-123");
                    saw_disconnect = true;
                } else if event["event"] == "peer_list_updated" {
                    saw_list = true;
                }
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
            | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                assert!(
                    std::time::Instant::now() <= deadline,
                    "timed out waiting for disconnect events"
                );
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                panic!("event bus is process-wide and never closes");
            }
        }
    }
}

#[tokio::test]
async fn upsert_object_broadcasts_live_change_to_peers() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    // Нужен объектный тип перед вставкой объекта (FK constraint)
    handle_request(
        &state,
        Request::UpsertObjectType {
            object_type: ObjectType {
                id: "note".to_string(),
                name: "Note".to_string(),
                schema_json: "{}".to_string(),
                ui_schema_json: "{}".to_string(),
                created_at: "2026-06-17T00:00:00.000Z".to_string(),
                updated_at: "2026-06-17T00:00:00.000Z".to_string(),
                system_locked: false,
            },
            device_id: Some("test-device".to_string()),
        },
    )
    .await
    .unwrap();

    let captured = setup_sync_with_capturing_transport(&state).await;

    let object = ArkObjectWrite {
        id: "live-obj-1".to_string(),
        type_id: "note".to_string(),
        type_version: Some("0.0.0-legacy".to_string()),
        title: "Live Test".to_string(),
        content_json: json!({ "type": "doc", "content": [] }),
        props_json: json!({}),
        created_at: "2026-06-17T00:00:00.000Z".to_string(),
        updated_at: "2026-06-17T00:00:00.000Z".to_string(),
        deleted_at: None,
    };
    handle_request(
        &state,
        Request::UpsertObject {
            object,
            expected_snapshot: None,
            device_id: Some("test-device".to_string()),
        },
    )
    .await
    .unwrap();

    // Небольшая пауза — broadcast_local_change запускается как spawn
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let msgs = captured.lock().await;
    let live_change = msgs.iter().find(|m| {
        matches!(m, crate::protocol::LanSyncMessage::LiveChange { entity, .. }
                if entity.id == "live-obj-1" && entity.entity_type == "object" &&
                    entity.deleted.is_none())
    });
    assert!(
        live_change.is_some(),
        "UpsertObject должен рассылать LiveChange(entity_type=object, id=live-obj-1, \
             deleted=None); \
             получено сообщений: {}, содержимое: {:?}",
        msgs.len(),
        msgs.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>()
    );

    handle_request(&state, Request::StopSync).await.unwrap();
}

#[tokio::test]
async fn delete_object_broadcasts_live_change_with_deleted_flag() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    // Нужен объектный тип
    handle_request(
        &state,
        Request::UpsertObjectType {
            object_type: ObjectType {
                id: "note".to_string(),
                name: "Note".to_string(),
                schema_json: "{}".to_string(),
                ui_schema_json: "{}".to_string(),
                created_at: "2026-06-17T00:00:00.000Z".to_string(),
                updated_at: "2026-06-17T00:00:00.000Z".to_string(),
                system_locked: false,
            },
            device_id: Some("test-device".to_string()),
        },
    )
    .await
    .unwrap();

    // Создаём объект сначала
    let object = ArkObjectWrite {
        id: "live-obj-del".to_string(),
        type_id: "note".to_string(),
        type_version: Some("0.0.0-legacy".to_string()),
        title: "To Delete".to_string(),
        content_json: json!({}),
        props_json: json!({}),
        created_at: "2026-06-17T00:00:00.000Z".to_string(),
        updated_at: "2026-06-17T00:00:00.000Z".to_string(),
        deleted_at: None,
    };
    handle_request(
        &state,
        Request::UpsertObject {
            object,
            expected_snapshot: None,
            device_id: Some("test-device".to_string()),
        },
    )
    .await
    .unwrap();

    let captured = setup_sync_with_capturing_transport(&state).await;

    handle_request(
        &state,
        Request::DeleteObject {
            id: "live-obj-del".to_string(),
            expected_snapshot: None,
            device_id: Some("test-device".to_string()),
        },
    )
    .await
    .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let msgs = captured.lock().await;
    let live_change = msgs.iter().find(|m| {
        matches!(m, crate::protocol::LanSyncMessage::LiveChange { entity, .. }
                if entity.id == "live-obj-del"
                    && entity.entity_type == "object"
                    && entity.deleted == Some(true))
    });
    assert!(
        live_change.is_some(),
        "DeleteObject должен рассылать LiveChange(deleted=Some(true)); \
             получено: {:?}",
        msgs.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>()
    );

    handle_request(&state, Request::StopSync).await.unwrap();
}

// -----------------------------------------------------------------------
// RED-тесты: fail-closed + атомарность entity/sync-meta (2026-06-18)
// -----------------------------------------------------------------------
