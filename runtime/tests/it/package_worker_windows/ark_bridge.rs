use super::*;

#[tokio::test]
async fn worker_ark_write_uses_host_and_advances_sync_state() {
    let _lock = test_support::serialized();
    let directory = tempfile::tempdir().expect("temporary directory");
    let worker_manifest = manifest("fixture.ark-write");
    let (store, installed) = install_fixture(&directory, &worker_manifest);
    let executable = store
        .immutable_entrypoint(&installed)
        .expect("immutable entrypoint");
    let db_path = directory.path().join("ark.db");

    let ark = Arc::new(
        ArkHost::open(db_path.to_str().expect("db path"))
            .await
            .expect("ark host"),
    );
    let supervisor = PackageWorkerSupervisor::with_ark(1, ark.clone());
    let state = tempfile::tempdir().expect("worker state directory");
    supervisor.bind_store(store);
    supervisor
        .start(
            &worker_manifest,
            executable,
            state.path().to_path_buf(),
            installed.hash,
            &[],
            "ark-write-test".into(),
            None,
            None,
        )
        .await
        .expect("worker start");
    let object_type = tokio::time::timeout(HANG_GUARD, async {
        loop {
            let response = ark
                .request(
                    "get_object_type",
                    serde_json::json!({ "id": "worker_fixture_type" }),
                )
                .await
                .expect("ark request");
            if response.ok && response.data.get("id").is_some() {
                break response.data;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("worker ARK dispatch timeout");
    assert_eq!(object_type["id"], "worker_fixture_type");
    let sync = ark
        .request(
            "get_sync_kv",
            serde_json::json!({ "key": "lan_sync.version_vector" }),
        )
        .await
        .expect("sync state request");
    assert!(sync.ok && sync.data.as_str().is_some());
    supervisor
        .stop(&worker_manifest.id, &worker_manifest.version)
        .await
        .expect("worker stop");
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup.
    let _ = supervisor.stop_all().await;
    drop(supervisor);
    shutdown_ark(ark).await;
}

#[tokio::test]
async fn fake_provider_collection_uses_keyring_secret_and_broker_injection() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let mut markers = fixture_env(&directory);
    let result_marker = directory.path().join("fake-provider-result.json");
    unsafe {
        std::env::set_var("MUNDUS_FAKE_PROVIDER_RESULT_MARKER", &result_marker);
    }
    markers.result = Some(result_marker.clone());

    let server = MockServer::start_async().await;
    let credential = "local-fake-provider-credential";
    let provider = server
        .mock_async(|when, then| {
            when.method("GET")
                .path("/collect")
                .header("authorization", format!("Bearer {credential}"));
            then.status(200).json_body(serde_json::json!({
                "provider": "fake",
                "items": [{"id": "local-item"}]
            }));
        })
        .await;
    let endpoint = server.url("/collect");
    let origin = server.url("/");

    // Same service `secret_store::package_integration_entry` uses in the
    // shipped engine; the fixture-only account name keeps real credentials
    // untouched, and `CredentialCleanup` deletes the entry on drop.
    let keyring_entry = keyring::Entry::new(
        engine::brand::KEYRING_SERVICE,
        "package-integration:fixture.fake-provider:1.0.0:session",
    )
    .expect("keyring entry");
    keyring_entry
        .set_password(credential)
        .expect("write local fake credential");
    let credential_from_keyring = keyring_entry
        .get_password()
        .expect("read local fake credential");
    struct CredentialCleanup(keyring::Entry);
    impl Drop for CredentialCleanup {
        fn drop(&mut self) {
            let _ = self.0.delete_credential();
        }
    }
    let _credential_cleanup = CredentialCleanup(keyring_entry);

    let mut worker_manifest = manifest("fixture.fake-provider");
    worker_manifest.permissions[0].scopes = vec![origin.clone()];
    let (store, installed) = install_fixture(&directory, &worker_manifest);
    let executable = store
        .immutable_entrypoint(&installed)
        .expect("immutable entrypoint");
    let supervisor = PackageWorkerSupervisor::new(1);
    supervisor.bind_store(store);
    let state = tempfile::tempdir().expect("worker state");
    let start_result = supervisor
        .start(
            &worker_manifest,
            executable,
            state.path().to_path_buf(),
            installed.hash.clone(),
            &[],
            "fake-provider-test".into(),
            None,
            Some(engine::package_worker_supervisor::IntegrationLaunchConfig {
                manifest: IntegrationManifest {
                    settings: vec![
                        IntegrationSetting {
                            key: "endpoint".into(),
                            label: "Provider endpoint".into(),
                            kind: IntegrationSettingKind::Text,
                            description: None,
                            required: true,
                            injection: None,
                        },
                        IntegrationSetting {
                            key: "session".into(),
                            label: "Session".into(),
                            kind: IntegrationSettingKind::Secret,
                            description: None,
                            required: true,
                            injection: Some(SecretInjection::Header {
                                origins: vec![origin],
                                name: "Authorization".into(),
                                prefix: "Bearer ".into(),
                            }),
                        },
                    ],
                    login: None,
                    schedule: None,
                },
                values: [("endpoint".into(), endpoint)].into_iter().collect(),
                secrets: [("session".into(), credential_from_keyring)]
                    .into_iter()
                    .collect(),
            }),
        )
        .await;
    assert_eq!(
        start_result,
        Ok(()),
        "fake provider worker start: {start_result:?}; diagnostics: {:?}",
        supervisor.diagnostics()
    );
    supervisor
        .run_now("fixture.fake-provider", "1.0.0")
        .expect("sync now");

    let collection = tokio::time::timeout(HANG_GUARD, async {
        loop {
            if result_marker.is_file() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        collection.is_ok(),
        "fake provider collection timeout; diagnostics: {:?}",
        supervisor.diagnostics()
    );
    let result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&result_marker).expect("provider result"))
            .expect("provider JSON");
    assert_eq!(
        result,
        serde_json::json!({
            "provider": "fake",
            "items": [{"id": "local-item"}]
        }),
        "provider collection failed"
    );
    assert!(!result.to_string().contains(credential));
    let bootstrap = std::fs::read_to_string(&markers.bootstrap).expect("bootstrap marker");
    assert!(bootstrap.contains("secret_handles"));
    assert!(!bootstrap.contains(credential));
    provider.assert_async().await;
    supervisor
        .stop("fixture.fake-provider", "1.0.0")
        .await
        .expect("fake provider worker stop");
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup.
    let _ = supervisor.stop_all().await;
}

#[tokio::test]
async fn typed_data_request_translates_to_canonical_ark_operations() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let db_path = directory.path().join("ark.db");

    let ark = ArkHost::open(db_path.to_str().expect("db path"))
        .await
        .expect("ark host");
    let note_registration = ark_core::canonical_types::definitions::canonical_type_registrations()
        .expect("canonical registrations")
        .into_iter()
        .find(|registration| registration.type_id == "core.note")
        .expect("note registration");
    assert!(
        ark.request(
            "types.registerPackageDefinitions",
            serde_json::json!({"registrations":[note_registration]})
        )
        .await
        .expect("register note type")
        .ok
    );
    ArkRequestExecutor::request(
        &ark,
        "data.request",
        serde_json::json!({
            "kind":"create_object",
            "type_id":"core.note",
            "type_version":"0.1.0",
            "object_id":"typed-worker-note",
            "fields":[
                {"field_id":"title","value":"Typed worker note"},
                {"field_id":"content","value":{"type":"doc","content":[]}},
                {"field_id":"props.description","value":null},
                {"field_id":"props.extensions","value":{}}
            ],
            "links":[]
        }),
    )
    .await
    .expect("typed create request");

    let projected = ArkRequestExecutor::request(
        &ark,
        "data.request",
        serde_json::json!({
            "kind":"read_object","type_id":"core.note","type_version":"0.1.0",
            "object_id":"typed-worker-note","fields":["title"],"relations":[]
        }),
    )
    .await
    .expect("typed read request");
    assert_eq!(projected["title"], "Typed worker note");
    assert_eq!(projected["propsJson"], serde_json::json!({}));

    let collision = ArkRequestExecutor::request(
        &ark,
        "data.request",
        serde_json::json!({
            "kind":"create_object","type_id":"core.game","type_version":"0.1.0",
            "object_id":"typed-worker-note","fields":[],"links":[]
        }),
    )
    .await;
    assert_eq!(collision, Err("conflict"));
    let wrong_type_delete = ArkRequestExecutor::request(
        &ark,
        "data.request",
        serde_json::json!({
            "kind":"delete_object","type_id":"core.game","type_version":"0.1.0",
            "object_id":"typed-worker-note","expected_hlc":null
        }),
    )
    .await;
    assert_eq!(wrong_type_delete, Err("not-found"));

    let object = ark
        .request("get_object", serde_json::json!({"id":"typed-worker-note"}))
        .await
        .expect("get object");
    assert!(object.ok);
    assert_eq!(object.data["title"], "Typed worker note");
    assert_eq!(object.data["typeId"], "core.note");
    ark.shutdown(Instant::now() + Duration::from_secs(30))
        .await
        .unwrap();
}
