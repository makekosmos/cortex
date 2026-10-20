use super::*;

#[test]
fn deferred_commit_failure_rolls_back_authenticated_core_task_apply() {
    let (_database, mut connection) = database();
    let entity = core_task_entity(
        "failed-core-task-commit",
        "Task must roll back",
        "2026-10-15T00:00:00.000Z:000001:device-a",
    );
    let payload = serde_json::to_string(&entity).unwrap();
    receive_inbox_payload(&connection, "device-a", "task-message", &payload)
        .expect("receipt commits before task apply");
    connection
        .execute_batch(
            "CREATE TABLE deferred_task_probe (
                 parent_id TEXT REFERENCES objects(id) DEFERRABLE INITIALLY DEFERRED
             );
             CREATE TRIGGER fail_core_task_commit AFTER INSERT ON objects
             BEGIN
                 INSERT INTO deferred_task_probe(parent_id) VALUES ('missing-parent');
             END;",
        )
        .expect("deferred task commit failpoint installs");

    let failure = SqliteWriter::apply_authenticated_core_task_write_on_connection(
        &mut connection,
        "device-a",
        authenticated_task_write(&entity, "task-message"),
    )
    .expect_err("deferred foreign key rejects task commit");
    assert!(
        matches!(failure, StoreTransactionFailure::Commit(_)),
        "expected deferred commit failure, got {failure:?}"
    );
    assert!(connection.is_autocommit());
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
             WHERE peer_device_id = 'device-a' AND message_id = 'task-message'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("received task inbox survives failed apply");
    assert_eq!((state.as_str(), bytes > 0), ("received", true));
    assert!(
        !crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)
            .unwrap()
            .unwrap_or_default()
            .contains(&entity.id)
    );
}

#[test]
fn clear_all_removes_persisted_object_write_receipts() {
    let (_database, connection) = database();
    let receipt = NewObjectReceipt {
        entity_id: "cleared-object".into(),
        representation_kind: "full".into(),
        source_hlc: "2026-10-15T00:00:00.000Z:000001:device-a".into(),
        source_origin_device_id: Some("device-a".into()),
        type_id: "com.kosmos.note".into(),
        payload_sha256: "digest".into(),
        payload_json: "{}".into(),
        outcome: ReceiptOutcome::Applied,
        resulting_hlc: Some("2026-10-15T00:00:00.000Z:000001:device-a".into()),
        authenticated_peer_id: Some("device-a".into()),
    };
    crate::object_write_receipts::insert_receipt(&connection, receipt)
        .expect("object receipt persists");
    let mut writer = SqliteWriter::from_connection(connection);
    writer.clear_all().expect("clear all succeeds");
    writer.with_connection_mut(|connection| {
        assert_eq!(
            count(
                connection,
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                "cleared-object"
            ),
            0
        );
    });
}

#[test]
fn authenticated_task_replay_reuses_receipt_and_feed_persistence() {
    let (_database, mut connection) = database();
    let entity = core_task_entity(
        "receipt-replay-task",
        "Original title",
        "2026-10-15T00:00:00.000Z:000001:device-a",
    );
    let write = authenticated_task_write(&entity, "task-message");
    let applied = SqliteWriter::apply_authenticated_core_task_write_on_connection(
        &mut connection,
        "device-a",
        write.clone(),
    )
    .expect("authenticated task applies");
    assert_eq!(applied, RemoteCoreTaskOutcome::Applied);
    let replayed = SqliteWriter::apply_authenticated_core_task_write_on_connection(
        &mut connection,
        "device-a",
        write,
    )
    .expect("authenticated task replay is idempotent");
    assert_eq!(replayed, RemoteCoreTaskOutcome::Duplicate);

    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
            &entity.id
        ),
        1
    );
    let (state, bytes): (String, i64) = connection
        .query_row(
            "SELECT state, payload_bytes FROM ark_sync_inbox
             WHERE peer_device_id = 'device-a' AND message_id = 'task-message'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("applied inbox receipt reads");
    assert_eq!((state.as_str(), bytes), ("applied", 0));
    assert_eq!(
        count(
            &connection,
            "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id = ?1",
            &entity.id
        ),
        1
    );
    let title: String = connection
        .query_row(
            "SELECT title FROM objects WHERE id = ?1",
            [&entity.id],
            |row| row.get(0),
        )
        .expect("canonical object persists");
    assert_eq!(title, "Original title");
}
