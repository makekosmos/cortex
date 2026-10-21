use super::*;

#[test]
fn deferred_commit_failure_rolls_back_canonical_apply_and_preserves_received_inbox() {
    let (_database, mut connection) = database();
    let entity = object_entity(
        "failed-object-commit",
        "Commit must roll back",
        "2026-10-15T00:00:00.000Z:000001:device-a",
    );
    let payload = serde_json::to_string(&entity).unwrap();
    receive_inbox_payload(&connection, "device-a", "message-a", &payload)
        .expect("receipt commits before apply");
    connection
        .execute_batch(
            "CREATE TABLE deferred_object_probe (
                 parent_id TEXT REFERENCES objects(id) DEFERRABLE INITIALLY DEFERRED
             );
             CREATE TRIGGER fail_object_commit AFTER INSERT ON objects
             BEGIN
                 INSERT INTO deferred_object_probe(parent_id) VALUES ('missing-parent');
             END;",
        )
        .expect("deferred commit failpoint installs");

    let failure = SqliteWriter::apply_authenticated_inbound_entity_on_connection(
        &mut connection,
        write(&entity, "message-a"),
        entity.clone(),
    )
    .expect_err("deferred foreign key rejects commit");
    assert!(
        matches!(failure, StoreTransactionFailure::Commit(_)),
        "expected deferred commit failure, got {failure:?}"
    );
    assert!(
        connection.is_autocommit(),
        "failed commit releases transaction"
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM objects WHERE id = ?1",
            &entity.id
        ),
        0
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
            &entity.id
        ),
        0
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM object_search_fts WHERE object_id = ?1",
            &entity.id
        ),
        0
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM object_sync_versions WHERE object_id = ?1",
            &entity.id
        ),
        0
    );
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id = ?1",
            &entity.id
        ),
        0
    );
    let (state, bytes): (String, i64) = connection
        .query_row(
            "SELECT state, payload_bytes FROM ark_sync_inbox
             WHERE peer_device_id = 'device-a' AND message_id = 'message-a'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("received inbox survives failed apply");
    assert_eq!((state.as_str(), bytes > 0), ("received", true));
    assert!(
        !crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)
            .unwrap()
            .unwrap_or_default()
            .contains(&entity.id)
    );

    connection
        .execute_batch("DROP TRIGGER fail_object_commit")
        .expect("deferred commit failpoint is removed");
    let next = object_entity(
        "after-object-commit-failure",
        "Next write",
        "2026-10-15T00:00:00.000Z:000002:device-a",
    );
    let next_payload = serde_json::to_string(&next).unwrap();
    receive_inbox_payload(&connection, "device-a", "message-b", &next_payload)
        .expect("next receipt persists");
    let mut writer = SqliteWriter::from_connection(connection);
    let applied = writer
        .apply_authenticated_inbound_entity(write(&next, "message-b"), next.clone())
        .expect("writer accepts a subsequent apply");
    assert_eq!(applied.outcome, InboundApplyOutcome::Applied);
    writer.with_connection_mut(|connection| {
        assert_eq!(
            count(
                connection,
                "SELECT COUNT(*) FROM objects WHERE id = ?1",
                &next.id
            ),
            1
        );
    });
}

#[test]
fn pending_authenticated_object_retains_payload_without_canonical_state() {
    let (_database, connection) = database();
    let mut entity = object_entity(
        "pending-receipt",
        "Future object",
        "2026-10-15T00:00:00.000Z:000001:device-a",
    );
    entity.data.insert("typeId".into(), json!("unknown-type"));
    entity.data.insert("typeVersion".into(), json!("9.9.9"));
    let mut writer = SqliteWriter::from_connection(connection);
    let result = writer
        .apply_authenticated_inbound_entity(write(&entity, "message-pending"), entity.clone())
        .expect("unknown type receipt becomes pending");
    assert_eq!(result.state, InboxState::Pending);
    writer.with_connection_mut(|connection| {
        let (state, bytes): (String, i64) = connection
            .query_row(
                "SELECT state, payload_bytes FROM ark_sync_inbox
                 WHERE peer_device_id = 'device-a' AND message_id = 'message-pending'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("pending receipt reads");
        assert_eq!(state, "pending");
        assert!(bytes > 0);
        assert_eq!(
            count(
                connection,
                "SELECT COUNT(*) FROM objects WHERE id = ?1",
                &entity.id
            ),
            0
        );
        assert_eq!(
            count(
                connection,
                "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id = ?1",
                &entity.id
            ),
            0
        );
    });
}
