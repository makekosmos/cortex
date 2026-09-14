use super::*;

#[tokio::test]
async fn opaque_worker_root_rejects_traversal_and_generation_cleanup() {
    let fixture = opaque_worker_fixture();
    let opened = dispatch(
        &fixture.supervisor.inner,
        &fixture.grant_a,
        &fixture.broker,
        None,
        &worker_call(
            &fixture.token_a,
            7,
            WorkerMethod::FilesystemRootOpen,
            serde_json::json!({"persistent_grant_id": fixture.persistent_a.as_str()}),
        ),
    )
    .await
    .expect("open opaque root");
    let root_id = opened["root_id"]
        .as_str()
        .expect("opaque root id")
        .to_owned();

    for operation in [
        WorkerMethod::FilesystemRead,
        WorkerMethod::FilesystemList,
        WorkerMethod::FilesystemWrite,
        WorkerMethod::FilesystemDelete,
        WorkerMethod::FilesystemCreateDir,
    ] {
        let mut params = serde_json::json!({
            "root_id": root_id.as_str(),
            "relative_path": "../escape"
        });
        if operation == WorkerMethod::FilesystemWrite {
            params["bytes"] =
                serde_json::json!(base64::engine::general_purpose::STANDARD.encode("escape"));
        }
        assert!(dispatch(
            &fixture.supervisor.inner,
            &fixture.grant_a,
            &fixture.broker,
            None,
            &worker_call(&fixture.token_a, 7, operation, params),
        )
        .await
        .is_err());
    }

    assert_eq!(fixture.authority.close_generation("session-a:pkg-a", 7), 2);
    assert!(dispatch(
        &fixture.supervisor.inner,
        &fixture.grant_a,
        &fixture.broker,
        None,
        &worker_call(
            &fixture.token_a,
            7,
            WorkerMethod::FilesystemRead,
            serde_json::json!({"root_id": root_id.as_str(), "relative_path": "safe.txt"}),
        ),
    )
    .await
    .is_err());
}
