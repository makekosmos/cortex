#![allow(clippy::unwrap_used)]

use serde_json::json;

#[tokio::test]
async fn blocked_database_accepts_agenda_write_through_service() {
    let dir = crate::support::temp_dir();
    let path = dir.path().join("blocked.db");
    ark_store::test_support::create_blocked_write_legacy_fixture(&path).unwrap();
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
