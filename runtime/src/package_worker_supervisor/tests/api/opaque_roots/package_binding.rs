use super::*;

#[tokio::test]
async fn opaque_worker_root_binds_package_session_and_generation() {
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

    dispatch(
        &fixture.supervisor.inner,
        &fixture.grant_a,
        &fixture.broker,
        None,
        &worker_call(
            &fixture.token_a,
            7,
            WorkerMethod::FilesystemCreateDir,
            serde_json::json!({"root_id": root_id.as_str(), "relative_path": "nested"}),
        ),
    )
    .await
    .expect("mkdir relative");
    dispatch(
        &fixture.supervisor.inner,
        &fixture.grant_a,
        &fixture.broker,
        None,
        &worker_call(
            &fixture.token_a,
            7,
            WorkerMethod::FilesystemWrite,
            serde_json::json!({
                "root_id": root_id.as_str(),
                "relative_path": "nested/note.txt",
                "bytes": base64::engine::general_purpose::STANDARD.encode("opaque")
            }),
        ),
    )
    .await
    .expect("write relative");

    let (other_manifest, _) = opaque_manifest("pkg-a", &fixture.selected_root);
    let (other_session, other_token) = Grant::derive(
        &other_manifest,
        fixture.grant_a.hash.clone(),
        41,
        7,
        "other-session".into(),
        std::slice::from_ref(&fixture.selected_root),
    )
    .expect("other session grant");
    for (grant, token, generation) in [
        (&other_session, other_token.as_str(), 7),
        (&fixture.grant_a, fixture.token_a.as_str(), 8),
        (&fixture.grant_b, fixture.token_b.as_str(), 7),
    ] {
        assert!(dispatch(
            &fixture.supervisor.inner,
            grant,
            &fixture.broker,
            None,
            &worker_call(
                token,
                generation,
                WorkerMethod::FilesystemRead,
                serde_json::json!({"root_id": root_id.as_str(), "relative_path": "nested/note.txt"}),
            ),
        )
        .await
        .is_err());
    }
}
