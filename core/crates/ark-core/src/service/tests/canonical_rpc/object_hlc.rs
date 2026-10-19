use super::*;

#[tokio::test]
async fn canonical_task_write_keeps_legacy_local_hlc_fallback() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let db_path = fixture.dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
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
    with_conn(&state, |conn| {
        let revision =
            ark_store::sync_versions::object_revision(conn, "canonical-task-hlc-fallback")?
                .ok_or_else(|| "missing object revision".to_string())?;
        assert_eq!(
            crate::hlc::HLC::from_string(&revision).device_id,
            "ark-core-rpc-local"
        );
        Ok(())
    })
    .unwrap();
    fixture.shutdown().await;
}
