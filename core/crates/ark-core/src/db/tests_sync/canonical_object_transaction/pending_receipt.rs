#[tokio::test]
async fn pending_authenticated_object_keeps_a_durable_receipt_without_applied_state() {
    let backend = SqliteStorageBackend::for_test(Arc::new(Mutex::new(setup_db())));
    let mut entity = sync_object("pending-receipt", "unknown-type", "Future object");
    entity.data.insert("typeVersion".into(), json!("9.9.9"));

    backend
        .apply_authenticated_entity_with_id(&entity, "peer-a", "feed-change-1")
        .await
        .unwrap();

    let shared = backend.db.shared_connection_for_test();
    let conn = shared.lock().unwrap();
    let (state, payload_bytes): (String, i64) = conn
        .query_row(
            "SELECT state, payload_bytes FROM ark_sync_inbox
             WHERE peer_device_id = 'peer-a' AND message_id = 'feed-change-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, "pending");
    assert!(payload_bytes > 0, "pending payload must remain durable");
    assert!(get_object(&conn, &entity.id).unwrap().is_none());
}
