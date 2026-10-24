use super::*;

#[tokio::test]
async fn canonical_task_write_keeps_legacy_local_hlc_fallback() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let db_path = fixture.dir.path().join("ark.db");
    initialize_test_database(&state, db_path.to_string_lossy().to_string())
        .await
        .unwrap();
    let mut object = canonical_task_object(Some("0.1.0"), canonical_task_props());
    object.id = "canonical-task-hlc-fallback".into();
    handle_request(
        &state,
        Request::UpsertObject {
            object,
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("  \t ".into()),
        },
    )
    .await
    .unwrap();
    let version_vector = handle_request(
        &state,
        Request::GetSyncKv {
            key: "lan_sync.version_vector".into(),
        },
    )
    .await
    .unwrap();
    let version_vector: ark_domain::types::VersionVector =
        serde_json::from_str(version_vector.as_str().unwrap()).unwrap();
    let revision = version_vector
        .get("canonical-task-hlc-fallback")
        .expect("object revision is in the public sync version vector");
    assert_eq!(
        crate::hlc::HLC::from_string(revision).device_id,
        "ark-core-rpc-local"
    );
    fixture.shutdown().await;
}
