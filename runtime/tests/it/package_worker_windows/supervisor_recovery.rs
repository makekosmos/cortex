use super::*;

#[tokio::test]
async fn signed_bridge_worker_projects_real_ark_and_restarts_idempotently() {
    let _lock = test_support::serialized();
    let directory = tempfile::tempdir().expect("temporary directory");
    let vault = directory.path().join("vault");
    let state_directory = tempfile::tempdir().expect("worker state directory");
    let state = state_directory.path().join("state");
    std::fs::create_dir(&vault).expect("vault");
    std::fs::create_dir(&state).expect("state");
    let manifest = bridge_manifest();
    let (store, installed) = install_binary(&directory, &manifest, bridge_fixture());
    let executable = store
        .immutable_entrypoint(&installed)
        .expect("immutable entrypoint");

    let ark = Arc::new(
        ArkHost::open(
            directory
                .path()
                .join("ark.db")
                .to_str()
                .expect("db path is valid UTF-8"),
        )
        .await
        .expect("ark host"),
    );
    assert!(
        ark.request(
            "upsert_object_type",
            serde_json::json!(
                {"object_type":{"id":"note",
                "name":"Note",
                "schemaJson":"{}",
                "uiSchemaJson":"{}",
                "createdAt":"2026-01-01T00:00:00Z",
                "updatedAt":"2026-01-01T00:00:00Z",
                "systemLocked":false},
                "device_id":"bridge-e2e"}),
        )
        .await
        .expect("type")
        .ok
    );
    let object = ark
        .request(
            "upsert_object",
            serde_json::json!(
                {"object":{"id":"bridge-note",
                "typeId":"note",
                "title":"Bridge note",
                "contentJson":{},
                "propsJson":{"body":"from ark"},
                "createdAt":"2026-01-01T00:00:00Z",
                "updatedAt":"2026-01-01T00:00:00Z",
                "deletedAt":null},
                "device_id":"bridge-e2e"}),
        )
        .await
        .expect("object");
    assert!(object.ok, "{:?}", object.error);
    let supervisor = PackageWorkerSupervisor::with_ark(1, ark.clone());
    supervisor.bind_store(store);
    let config = BridgeWorkerConfig {
        vault_root: vault.to_string_lossy().into_owned(),
        state_root: state.to_string_lossy().into_owned(),
        selected_types: vec!["note".into()],
        editable_fields: vec!["title".into(), "body".into()],
        readonly_fields: vec![],
    };
    let roots = [vault.clone(), state.clone()];
    supervisor
        .start(
            &manifest,
            executable.clone(),
            state.clone(),
            installed.hash.clone(),
            &roots,
            "bridge-e2e".into(),
            Some(config.clone()),
            None,
        )
        .await
        .expect("bridge start");
    let markdown_path = vault.join("Bridge note-bridge-note.md");
    let projected = tokio::time::timeout(HANG_GUARD, async {
        while !markdown_path.exists() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        projected.is_ok(),
        "projection diagnostics: {:?}",
        supervisor.diagnostics()
    );
    tokio::time::timeout(HANG_GUARD, async {
        while !state.join("state.json").exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("bridge state file");
    assert_owner_only_acl(&state);
    assert_owner_only_acl(&state.join("state.json"));
    let markdown = std::fs::read_to_string(&markdown_path).expect("markdown");
    assert!(markdown.contains("ark_id: \"bridge-note\""));
    std::fs::write(&markdown_path, markdown.replace("from ark", "from vault")).expect("edit vault");
    let round_trip = tokio::time::timeout(HANG_GUARD, async {
        loop {
            let response = ark
                .request("get_object", serde_json::json!({"id":"bridge-note"}))
                .await
                .expect("read");
            if response.data["propsJson"]["body"] == "from vault" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        round_trip.is_ok(),
        "round trip: markdown={} state={}",
        std::fs::read_to_string(&markdown_path).unwrap_or_default(),
        std::fs::read_to_string(state.join("state.json")).unwrap_or_default()
    );
    let sync = ark
        .request(
            "get_sync_kv",
            serde_json::json!({"key":"lan_sync.version_vector"}),
        )
        .await
        .expect("sync state");
    assert!(sync.ok && sync.data.as_str().is_some());
    supervisor
        .stop(&manifest.id, &manifest.version)
        .await
        .expect("worker stop");
    let after_first = std::fs::read(&markdown_path).expect("projected bytes");
    supervisor
        .start(
            &manifest,
            executable,
            state.clone(),
            installed.hash,
            &roots,
            "bridge-e2e-restart".into(),
            Some(config),
            None,
        )
        .await
        .expect("bridge restart");
    // The restarted worker reports `bridge_status` in its first heartbeat only
    // after completing a full sync pass, so Some(..) proves the projection ran
    // — no fixed sleep that CPU starvation could stretch or compress (KOS-308).
    tokio::time::timeout(HANG_GUARD, async {
        loop {
            if supervisor
                .diagnostics()
                .into_iter()
                .find(|worker| worker.id == manifest.id)
                .is_some_and(|worker| worker.bridge_status.is_some())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("restarted bridge never completed a sync pass");
    assert_eq!(
        std::fs::read(&markdown_path).expect("restarted bytes"),
        after_first
    );
    supervisor
        .stop(&manifest.id, &manifest.version)
        .await
        .expect("worker stop");
    assert_eq!(
        supervisor.health(&manifest.id, &manifest.version).state,
        WorkerState::Stopped
    );
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
    let _ = supervisor.stop_all().await;
    drop(supervisor);
    shutdown_ark(ark).await;
}

#[tokio::test]
async fn activated_worker_restarts_once_and_stop_cancels_more_retries() {
    let _lock = test_support::serialized();
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let directory = tempfile::tempdir().expect("temporary directory");
    let worker_manifest = manifest("fixture.crash");
    let (store, installed) = install_fixture(&directory, &worker_manifest);
    let executable = store
        .immutable_entrypoint(&installed)
        .expect("immutable entrypoint");
    let supervisor = PackageWorkerSupervisor::new(1);
    let state = tempfile::tempdir().expect("worker state directory");
    supervisor.bind_store(store);
    supervisor
        .start(
            &worker_manifest,
            executable,
            state.path().to_path_buf(),
            installed.hash,
            &[],
            "restart-test".into(),
            None,
            None,
        )
        .await
        .expect("initial start");
    assert!(supervisor.activate(&worker_manifest.id, &worker_manifest.version));
    let retried = tokio::time::timeout(HANG_GUARD, async {
        loop {
            let diagnostic = supervisor
                .diagnostics()
                .into_iter()
                .find(|worker| worker.id == worker_manifest.id)
                .expect("worker diagnostics");
            if diagnostic.generation == 2 && diagnostic.state == WorkerState::Running {
                assert_eq!(diagnostic.restart_count, 1);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        retried.is_ok(),
        "first retry: {:?}",
        supervisor.diagnostics()
    );
    supervisor
        .stop(&worker_manifest.id, &worker_manifest.version)
        .await
        .expect("worker stop");
    supervisor
        .stop_all()
        .await
        .expect("all worker registries quiesced");
    let (calls, lifecycles) = supervisor.test_registry_counts();
    assert_eq!(calls, 0);
    assert_eq!(lifecycles, 0);
    assert_eq!(
        supervisor
            .health(&worker_manifest.id, &worker_manifest.version)
            .state,
        WorkerState::Stopped
    );
}

#[tokio::test]
async fn secret_bearing_worker_failure_is_redacted_end_to_end() {
    let _lock = test_support::serialized();
    let directory = tempfile::tempdir().expect("temporary directory");
    let worker_manifest = manifest("fixture.secret-fail");
    let (store, installed) = install_fixture(&directory, &worker_manifest);
    let executable = store
        .immutable_entrypoint(&installed)
        .expect("immutable entrypoint");
    let supervisor = PackageWorkerSupervisor::new(1);
    let state = tempfile::tempdir().expect("worker state directory");
    supervisor.bind_store(store);
    supervisor
        .start(
            &worker_manifest,
            executable,
            state.path().to_path_buf(),
            installed.hash,
            &[],
            "secret-redaction-e2e".into(),
            None,
            None,
        )
        .await
        .expect("secret fixture should complete authenticated hello");
    assert!(supervisor.activate(&worker_manifest.id, &worker_manifest.version));

    tokio::time::timeout(HANG_GUARD, async {
        loop {
            if supervisor
                .health(&worker_manifest.id, &worker_manifest.version)
                .state
                == WorkerState::Failed
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("secret fixture failure was not observed");
    supervisor
        .stop(&worker_manifest.id, &worker_manifest.version)
        .await
        .expect("worker stop");

    let diagnostics = supervisor.diagnostics();
    let worker = diagnostics
        .iter()
        .find(|item| item.id == worker_manifest.id)
        .expect("worker diagnostics");
    let diagnostics_json = serde_json::to_string(&diagnostics).expect("diagnostics json");
    for forbidden in [
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "ARK_MARKDOWN_BODY_UNIQUE",
        "MARKDOWN_CONTENT_UNIQUE",
        "RAW_REQUEST_PAYLOAD_UNIQUE",
        "WORKER_SECRET_UNIQUE",
        "secret-user",
        "fixture.kspkg",
    ] {
        assert!(
            !diagnostics_json.contains(forbidden),
            "worker diagnostics leaked {forbidden}: {diagnostics_json}"
        );
    }
    assert!(
        diagnostics_json.contains("[REDACTED]"),
        "worker tails should retain redaction markers: {diagnostics_json}"
    );
    assert!(worker
        .stdout_tail
        .iter()
        .any(|line| line.contains("[REDACTED]")));
    assert!(worker
        .stderr_tail
        .iter()
        .any(|line| line.contains("[REDACTED]")));

    // Exercise the Manager diagnostics and support-bundle surfaces with the
    // same Engine-owned supervisor output.
    let manager_dir = directory.path().join("manager-data");
    let mut packages = PackageService::open(&manager_dir).expect("manager package service");
    packages.configure_workers(supervisor.clone(), vec![], "secret-redaction-e2e".into());
    let packages = std::sync::Arc::new(packages);
    let manager = ManagerState::new(manager_dir.clone());
    let snapshot = manager
        .diagnostics_snapshot(
            &std::sync::Arc::new(RpcDiagnostics::new()),
            &std::sync::Arc::new(UsageTrackerDiagnosticsState::default()),
            &std::sync::Arc::new(ProtocolUsageStore::open(&manager_dir).expect("protocol usage")),
            &packages,
        )
        .await;
    let manager_json = snapshot.to_string();
    for forbidden in [
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "ARK_MARKDOWN_BODY_UNIQUE",
        "MARKDOWN_CONTENT_UNIQUE",
        "RAW_REQUEST_PAYLOAD_UNIQUE",
        "WORKER_SECRET_UNIQUE",
        "secret-user",
        "fixture.kspkg",
    ] {
        assert!(
            !manager_json.contains(forbidden),
            "manager diagnostics leaked {forbidden}"
        );
    }

    let handle = manager
        .create_bundle(
            snapshot,
            serde_json::json!(
                {"worker_stdout": worker.stdout_tail,
                "worker_stderr": worker.stderr_tail}
            ),
        )
        .await
        .expect("create support bundle")["handle"]
        .as_str()
        .expect("bundle handle")
        .to_owned();
    let bundle_path = manager_dir.join("bundle.json");
    manager
        .save_bundle(&handle, bundle_path.to_str().expect("bundle path"))
        .await
        .expect("save support bundle");
    let bundle = std::fs::read_to_string(&bundle_path).expect("bundle bytes");
    for forbidden in [
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "ARK_MARKDOWN_BODY_UNIQUE",
        "MARKDOWN_CONTENT_UNIQUE",
        "RAW_REQUEST_PAYLOAD_UNIQUE",
        "WORKER_SECRET_UNIQUE",
        "secret-user",
        "fixture.kspkg",
    ] {
        assert!(
            !bundle.contains(forbidden),
            "support bundle leaked {forbidden}"
        );
    }
    assert!(bundle.contains("[REDACTED]"));

    // A real crash artifact must preserve only bounded metadata, never the
    // worker's panic payload or user data.
    let crash_root = CRASH_TEST_ROOT
        .get_or_init(|| tempfile::tempdir().expect("crash data directory"))
        .path()
        .to_path_buf();
    engine::crash_reporter::install(
        crash_root.clone(),
        "00000000-0000-4000-8000-000000000001".into(),
    );
    let panic = std::thread::spawn(|| {
        panic!(
            "WORKER_SECRET_UNIQUE ARK_MARKDOWN_BODY_UNIQUE RAW_REQUEST_PAYLOAD_UNIQUE \
                 C:\\Users\\secret-user\\vault\\private-note.md"
        )
    })
    .join();
    // The hook is process-wide and deliberately drops panic messages; every
    // integration test shares this process, so restore the default hook to keep
    // later failures readable. The crash file is already written by now.
    drop(std::panic::take_hook());
    assert!(panic.is_err());
    let crash_file = std::fs::read_dir(crash_root.join("crashes"))
        .expect("crash directory")
        .flatten()
        .find(|entry| entry.path().is_file())
        .expect("crash report");
    let crash = std::fs::read_to_string(crash_file.path()).expect("crash report bytes");
    for forbidden in [
        "WORKER_SECRET_UNIQUE",
        "ARK_MARKDOWN_BODY_UNIQUE",
        "RAW_REQUEST_PAYLOAD_UNIQUE",
        "secret-user",
        "private-note.md",
    ] {
        assert!(
            !crash.contains(forbidden),
            "crash report leaked {forbidden}: {crash}"
        );
    }
    assert!(crash.contains("[REDACTED_PANIC_PAYLOAD]"));

    // The OnceLock'd TempDir is never dropped (statics don't drop), so its
    // directory would leak into %TEMP%. The production panic hook was
    // restored above, so nothing writes into the root after this test —
    // remove it explicitly.
    std::fs::remove_dir_all(&crash_root).expect("crash test root cleanup");
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
    let _ = supervisor.stop_all().await;
}
