use super::*;

#[test]
fn failed_inbox_apply_survives_writer_restart_and_replays_to_applied() {
    let (database, connection) = database();
    let entity = object_entity(
        "restart-receipt-object",
        "Replay after restart",
        "2026-10-15T00:00:00.000Z:000001:device-a",
    );
    let payload = serde_json::to_string(&entity).unwrap();
    receive_inbox_payload(&connection, "device-a", "restart-message", &payload)
        .expect("received receipt persists");
    connection
        .execute_batch(
            "CREATE TABLE deferred_restart_probe (
                 parent_id TEXT REFERENCES objects(id) DEFERRABLE INITIALLY DEFERRED
             );
             CREATE TRIGGER fail_restart_commit AFTER INSERT ON objects
             BEGIN
                 INSERT INTO deferred_restart_probe(parent_id) VALUES ('missing-parent');
             END;",
        )
        .expect("deferred commit failpoint installs");
    let mut writer = SqliteWriter::from_connection(connection);
    assert!(writer
        .apply_authenticated_inbound_entity(write(&entity, "restart-message"), entity.clone())
        .is_err());
    drop(writer);

    let reopened = database.open_connection().expect("database reopens");
    let (state, bytes): (String, i64) = reopened
        .query_row(
            "SELECT state, payload_bytes FROM ark_sync_inbox
             WHERE peer_device_id = 'device-a' AND message_id = 'restart-message'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("receipt survives writer restart");
    assert_eq!(state, "received");
    assert!(bytes > 0);
    reopened
        .execute_batch("DROP TRIGGER fail_restart_commit")
        .expect("failpoint is removed");
    let mut writer = SqliteWriter::from_connection(reopened);
    let result = writer
        .apply_authenticated_inbound_entity(write(&entity, "restart-message"), entity.clone())
        .expect("replayed receipt applies");
    assert_eq!(result.state, InboxState::Applied);
    writer.with_connection_mut(|connection| {
        let (state, bytes): (String, i64) = connection
            .query_row(
                "SELECT state, payload_bytes FROM ark_sync_inbox
                 WHERE peer_device_id = 'device-a' AND message_id = 'restart-message'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("applied receipt reads");
        assert_eq!((state.as_str(), bytes), ("applied", 0));
        assert_eq!(
            count(
                connection,
                "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id = ?1",
                &entity.id
            ),
            1
        );
    });
}
