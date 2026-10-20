#![allow(clippy::unwrap_used)]

use crate::phase3_legacy_fixtures;
use rusqlite::Connection;
use serde_json::json;

#[tokio::test]
async fn blocked_database_accepts_agenda_write_through_service() {
    let dir = crate::support::temp_dir();
    let path = dir.path().join("blocked.db");
    {
        let conn = Connection::open(&path).unwrap();
        ark_store::open::init_schema_prerequisites(&conn).unwrap();
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
    assert_eq!(stored["typeVersion"], "0.1.0");
    assert_eq!(
        service
            .call("delete_object", json!({"id":"t", "device_id":"test"}))
            .await
            .unwrap(),
        json!(true)
    );
    service
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}
