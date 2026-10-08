use ark_core::data_platform;

fn selective_object_entity(id: &str, type_id: &str, title: &str, counter: u64) -> SyncEntity {
    SyncEntity {
        entity_type: "object".into(),
        id: id.into(),
        data: serde_json::from_value(json!({
            "typeId": type_id,
            "typeVersion": "1.0.0",
            "title": title,
            "contentJson": {"type":"doc","content":[{"type":"paragraph"}]},
            "propsJson": if type_id == "com.kosmos.game" {
                json!(
                    {"playStatus":null,
                    "userRating":null,
                    "genres":[],
                    "platforms":[],
                    "released":null,
                    "description":null,
                    "extensions":{"localState":{"path":"C:/device-only"}}})
            } else if type_id == "com.kosmos.task" {
                json!(
                    {"status":"todo",
                    "priority":"medium",
                    "scheduledAt":null,
                    "dueAt":null,
                    "reminderAt":null,
                    "completedAt":null,
                    "canceledAt":null,
                    "recurrence":null,
                    "checklist":[],
                    "extensions":{"vendor":{"opaque":true},
                    "localState":{"path":"C:/device-only"}}})
            } else {
                json!({"description":null,"extensions":{"localState":{"path":"C:/device-only"}}})
            },
            "createdAt": "2026-09-08T00:00:00Z",
            "updatedAt": "2026-09-08T00:00:00Z",
            "deletedAt": null
        }))
        .unwrap(),
        hlc: format!("2026-09-08T00:00:00.000Z:{counter:06}:device-a"),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selective_profile_two_db_round_trip_reconnect_and_narrowing_are_non_destructive() {
    let storage_a = make_storage("device-a");
    let storage_b = make_storage("device-b");
    let note = selective_object_entity("sync-note", "com.kosmos.note", "Sync note", 1);
    let task = selective_object_entity("sync-task", "com.kosmos.task", "Sync task", 2);
    let game = selective_object_entity("sync-game", "com.kosmos.game", "Sync game", 3);
    for (type_id, name, counter) in [
        ("com.kosmos.note", "Note", 10),
        ("com.kosmos.task", "Task", 11),
        ("com.kosmos.game", "Game", 12),
    ] {
        storage_a
            .apply_entity(&SyncEntity {
                entity_type: "object_type".into(),
                id: type_id.into(),
                data: serde_json::from_value(json!({
                    "name": name,
                    "schemaJson": "{}",
                    "uiSchemaJson": "{}",
                    "createdAt": "2026-09-08T00:00:00Z",
                    "updatedAt": "2026-09-08T00:00:00Z",
                    "systemLocked": false
                }))
                .unwrap(),
                hlc: format!("2026-09-08T00:00:00.000Z:{counter:06}:device-a"),
                deleted: None,
                origin_device_id: None,
                origin_seq: None,
            })
            .await
            .unwrap();
    }
    storage_a.apply_entity(&note).await.unwrap();
    storage_a.apply_entity(&task).await.unwrap();
    storage_a.apply_entity(&game).await.unwrap();
    storage_a
        .apply_entity(&SyncEntity {
            entity_type: "object_link".into(),
            id: "sync-link".into(),
            data: serde_json::from_value(json!({
                "sourceObjectId": "sync-note",
                "targetObjectId": "sync-game",
                "linkType": "related",
                "createdAt": "2026-09-08T00:00:00Z"
            }))
            .unwrap(),
            hlc: "2026-09-08T00:00:00.000Z:000003:device-a".into(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        })
        .await
        .unwrap();
    storage_a
        .apply_entity(&tracked_app_entity("tracked-app-a", "device-a", 3))
        .await
        .unwrap();
    storage_a
        .apply_entity(&usage_session_entity(
            "usage-session-a",
            "tracked-app-a",
            "device-a",
            4,
        ))
        .await
        .unwrap();
    storage_a
        .apply_entity(&usage_event_entity(
            "sync-usage",
            "tracked-app-a",
            "usage-session-a",
            "device-a",
            5,
        ))
        .await
        .unwrap();

    let mut profile = data_platform::SelectiveSyncProfile::default();
    profile.set_rule("type", "com.kosmos.note", data_platform::SyncMode::Full);
    profile.set_rule("type", "com.kosmos.task", data_platform::SyncMode::Full);
    profile.set_rule("type", "com.kosmos.game", data_platform::SyncMode::Metadata);
    profile.set_rule("dataset", "usage", data_platform::SyncMode::None);
    storage_a.set_selective_sync_profile(Some(profile.clone()));
    storage_b.set_selective_sync_profile(Some(profile));

    let server_a = Arc::new(SyncServer::new(storage_a.clone() as Arc<dyn StorageBackend>));
    let server_b = Arc::new(SyncServer::new(storage_b.clone() as Arc<dyn StorageBackend>));
    let port_a = pick_port().await;
    let port_b = pick_port().await;
    server_a
        .start_with_addr(
            "space-selective",
            "device-a",
            Some("Alpha"),
            Some(vec![format!("127.0.0.1:{port_a}")]),
            &format!("127.0.0.1:{port_a}"),
        )
        .await
        .unwrap();
    server_b
        .start_with_addr(
            "space-selective",
            "device-b",
            Some("Beta"),
            Some(vec![format!("127.0.0.1:{port_b}")]),
            &format!("127.0.0.1:{port_b}"),
        )
        .await
        .unwrap();

    let peer = PeerRecord {
        device_id: "device-a".into(),
        device_name: "Alpha".into(),
        addresses: vec![format!("127.0.0.1:{port_a}")],
        last_seen: chrono::Utc::now().to_rfc3339(),
        last_address: None,
        platform: None,
        app_version: None,
    };
    let client = Arc::new(SyncClient::new(
        storage_b.clone() as Arc<dyn StorageBackend>,
        peer.clone(),
        "device-b".into(),
        "Beta".into(),
        "space-selective".into(),
        vec![format!("127.0.0.1:{port_b}")],
        None,
    ));
    client.start();
    tokio::time::sleep(Duration::from_millis(700)).await;

    let loaded = storage_b.load_entities(&HashMap::new()).await;
    assert!(loaded.iter().any(|entity| entity.id == note.id));
    let task_copy = loaded.iter().find(|entity| entity.id == task.id).unwrap();
    assert!(task_copy.data.contains_key("contentJson"));
    let game_copy = loaded.iter().find(|entity| entity.id == game.id).unwrap();
    assert!(!game_copy.data.contains_key("contentJson"));
    assert!(!loaded.iter().any(|entity| entity.id == "sync-usage"));
    assert!(loaded
        .iter()
        .any(|entity| entity.entity_type == "object_link" && entity.id == "sync-link"));
    let type_pos = loaded
        .iter()
        .position(|entity| entity.entity_type == "object_type" && entity.id == "com.kosmos.game")
        .expect("selected type definition should be received");
    let object_pos = loaded
        .iter()
        .position(|entity| entity.entity_type == "object" && entity.id == "sync-game")
        .expect("selected object should be received");
    assert!(
        type_pos < object_pos,
        "type definitions must precede objects in transport"
    );

    client.stop();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let reconnect = Arc::new(SyncClient::new(
        storage_b.clone() as Arc<dyn StorageBackend>,
        peer,
        "device-b".into(),
        "Beta".into(),
        "space-selective".into(),
        vec![format!("127.0.0.1:{port_b}")],
        None,
    ));
    reconnect.start();
    tokio::time::sleep(Duration::from_millis(700)).await;
    let replayed = storage_b.load_entities(&HashMap::new()).await;
    assert_eq!(
        replayed
            .iter()
            .filter(|entity| entity.id == note.id)
            .count(),
        1
    );

    storage_a.set_selective_sync_profile(Some(data_platform::SelectiveSyncProfile::default()));
    server_a.broadcast_live_change(game.clone(), None).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(storage_b
        .load_entities(&HashMap::new())
        .await
        .iter()
        .any(|entity| entity.id == "sync-game"));
    storage_a.set_selective_sync_profile(Some(data_platform::SelectiveSyncProfile {
        rules: [
            (
                ("type".into(), "com.kosmos.note".into()),
                data_platform::SyncMode::Full,
            ),
            (
                ("type".into(), "com.kosmos.game".into()),
                data_platform::SyncMode::Metadata,
            ),
        ]
        .into_iter()
        .collect(),
    }));
    assert!(storage_a
        .load_entities(&HashMap::new())
        .await
        .iter()
        .any(|entity| entity.id == "sync-game"));

    // Deletes use the same live transport and retain type metadata for
    // profile filtering at the receiver.
    let mut deleted_game = selective_object_entity("sync-game", "com.kosmos.game", "Sync game", 6);
    deleted_game.deleted = Some(true);
    storage_a.apply_entity(&deleted_game).await.unwrap();
    server_a.broadcast_live_change(deleted_game, None).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let tombstones_b = storage_b.load_entities(&HashMap::new()).await;
    let game_tombstone = tombstones_b
        .iter()
        .find(|entity| entity.deleted == Some(true) && entity.id == "sync-game")
        .expect("selected object tombstone should cross the live transport");
    assert_eq!(
        game_tombstone
            .data
            .get("typeId")
            .and_then(|value| value.as_str()),
        Some("com.kosmos.game")
    );

    reconnect.stop();
    server_a.stop().await;
    server_b.stop().await;
}
