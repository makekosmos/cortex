use super::*;

#[tokio::test]
async fn blocked_database_accepts_agenda_write_through_service() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("blocked.db");
    {
        let conn = Connection::open(&path).unwrap();
        init_schema_prerequisites_for_phase3(&conn).unwrap();
        phase3_legacy_fixtures::seed_historical_legacy_authorities(&conn);
        conn.execute_batch(
            "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,
             created_at,updated_at) VALUES('dangling-note','note_obj','0.0.0-legacy','n',
             '{}','{\"relatedNotes\":[\"missing-target\"]}',
             '2026-01-01T00:00:00.000Z','2026-01-01T00:00:00.000Z');",
        )
        .unwrap();
    }
    let service = ark_core::service::ArkService::open(path.to_str().unwrap())
        .await
        .unwrap();
    let object: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/agenda/task-inbox.json")).unwrap();
    assert_eq!(
        service
            .call(
                "upsert_object",
                json!({"object":object, "device_id":"test"})
            )
            .await
            .unwrap(),
        json!(true)
    );
    let stored = service.call("get_object", json!({"id":"t"})).await.unwrap();
    assert_eq!(stored["typeVersion"], "1.1.0");
    assert_eq!(
        service
            .call("delete_object", json!({"id":"t", "device_id":"test"}))
            .await
            .unwrap(),
        json!(true)
    );
}

#[test]
fn blocked_migration_still_installs_new_write_contracts_without_rewriting_legacy() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema_prerequisites_for_phase3(&conn).unwrap();
    phase3_legacy_fixtures::seed_historical_legacy_authorities(&conn);
    conn.execute_batch(
        "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,created_at,
         updated_at) VALUES('dangling-note','note_obj','0.0.0-legacy','n','{}',
         '{\"relatedNotes\":[\"missing-target\"]}','2026-01-01T00:00:00.000Z',
         '2026-01-01T00:00:00.000Z');",
    )
    .unwrap();
    for _ in 0..2 {
        init_schema(&conn).unwrap();
        for type_id in ["com.kosmos.task", "com.kosmos.project"] {
            assert!(
                ark_core::type_registry::get_type(&conn, type_id, Some("1.1.0"))
                    .unwrap()
                    .is_some()
            );
        }
        let write =
            serde_json::from_str(include_str!("../fixtures/agenda/task-inbox.json")).unwrap();
        let object = ark_core::canonical_types::ingress::prepare_object(&conn, write).unwrap();
        ark_core::db::upsert_object(&conn, &object).unwrap();
        let legacy: (String, String) = conn
            .query_row(
                "SELECT type_id,props_json FROM objects WHERE id='dangling-note'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(legacy.0, "note_obj");
        assert_eq!(legacy.1, "{\"relatedNotes\":[\"missing-target\"]}");
        assert_eq!(
            ark_core::canonical_types::migration::plan_phase3(&conn)
                .unwrap()
                .status,
            "blocked"
        );
    }
    // Once the bad fixture is repaired, installing contracts early must not
    // prevent the original migration from completing.
    conn.execute("DELETE FROM objects WHERE id='dangling-note'", [])
        .unwrap();
    assert_eq!(migrate_phase3(&conn).unwrap().status, "completed");
}
