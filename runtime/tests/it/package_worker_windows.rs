// Spawns both fixture binaries (worker + markdown bridge), so it needs
// both fixture features enabled to have CARGO_BIN_EXE_* defined.
#![cfg(all(
    windows,
    feature = "package-worker-fixture",
    feature = "markdown-bridge-fixture"
))]
#![allow(clippy::panic, clippy::unwrap_used)]

use std::{
    io::Write,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use engine::{
    ark_host::ArkHost,
    diagnostics::RpcDiagnostics,
    manager_api::ManagerState,
    package_manifest::{
        DataAction, FieldAccess, IntegrationManifest, IntegrationSetting, IntegrationSettingKind,
        ManifestData, ManifestTarget, ManifestV2, PackageKind, PackageManifest, SecretInjection,
        TargetOs, TargetRuntime, VersionedManifest,
    },
    package_service::PackageService,
    package_store::PackageStore,
    package_worker_process::{test_support, FailureStage},
    package_worker_protocol::BridgeWorkerConfig,
    package_worker_supervisor::{ArkRequestExecutor, PackageWorkerSupervisor, WorkerState},
    protocol_usage::ProtocolUsageStore,
    usage_tracker::UsageTrackerDiagnosticsState,
};
use httpmock::MockServer;
use sha2::{Digest, Sha256};
use zip::{write::FileOptions, ZipWriter};

// The crash reporter installs a process-global hook, so its test directory
// must outlive the test that installs it.
static CRASH_TEST_ROOT: OnceLock<tempfile::TempDir> = OnceLock::new();

/// Fixture env vars are process-wide and other test modules in this binary set
/// the same ones, so the env owns the engine's worker test lock while it exists.
struct FixtureEnv {
    _serialized: test_support::FailureGuard<'static>,
    entry: PathBuf,
    bootstrap: PathBuf,
    result: Option<PathBuf>,
}

impl Drop for FixtureEnv {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("MUNDUS_FIXTURE_ENTRY_MARKER");
            std::env::remove_var("MUNDUS_FIXTURE_BOOTSTRAP_MARKER");
            if self.result.is_some() {
                std::env::remove_var("MUNDUS_FAKE_PROVIDER_RESULT_MARKER");
            }
        }
    }
}

fn fixture_env(directory: &tempfile::TempDir) -> FixtureEnv {
    let serialized = test_support::serialized();
    let entry = directory.path().join("entry.marker");
    let bootstrap = directory.path().join("bootstrap.marker");
    unsafe {
        std::env::set_var("MUNDUS_FIXTURE_ENTRY_MARKER", &entry);
        std::env::set_var("MUNDUS_FIXTURE_BOOTSTRAP_MARKER", &bootstrap);
    }
    FixtureEnv {
        _serialized: serialized,
        entry,
        bootstrap,
        result: None,
    }
}

fn manifest(id: &str) -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: id.into(),
        name: "fixture".into(),
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: ">=1".into(),
        entrypoint: "package-worker-fixture.exe".into(),
        publisher: "kosmos".into(),
        permissions: if id.ends_with(".ark-write") {
            vec![engine::package_manifest::PermissionRequest {
                capability: "ark.write".into(),
                scopes: vec!["upsert_object_type".into()],
            }]
        } else if id.ends_with(".fake-provider") {
            vec![engine::package_manifest::PermissionRequest {
                capability: "network".into(),
                scopes: vec!["http://127.0.0.1/".into()],
            }]
        } else {
            vec![]
        },
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_package-worker-fixture"))
}
fn bridge_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ark-markdown-bridge"))
}

fn bridge_manifest() -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: "ark-markdown-bridge".into(),
        name: "ARK Markdown Bridge".into(),
        version: "1.0.0".into(),
        kind: PackageKind::Bridge,
        engine_api: ">=1".into(),
        entrypoint: "ark-markdown-bridge.exe".into(),
        publisher: "kosmos".into(),
        permissions: vec![
            engine::package_manifest::PermissionRequest {
                capability: "ark.read".into(),
                scopes: vec!["list_objects".into(), "get_object".into()],
            },
            engine::package_manifest::PermissionRequest {
                capability: "ark.write".into(),
                scopes: vec!["upsert_object".into(), "external_refs.upsert".into()],
            },
            engine::package_manifest::PermissionRequest {
                capability: "filesystem.read".into(),
                scopes: vec![],
            },
            engine::package_manifest::PermissionRequest {
                capability: "filesystem.write".into(),
                scopes: vec![],
            },
        ],
    }
}

fn versioned_manifest(manifest: &PackageManifest) -> VersionedManifest {
    VersionedManifest::V2(ManifestV2 {
        schema_version: 2,
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        description: None,
        version: manifest.version.clone(),
        kind: manifest.kind.clone(),
        engine_api: manifest.engine_api.clone(),
        entrypoint: manifest.entrypoint.clone(),
        icon: None,
        publisher: manifest.publisher.clone(),
        permissions: manifest.permissions.clone(),
        targets: vec![ManifestTarget {
            runtime: TargetRuntime::Worker,
            os: vec![TargetOs::Windows],
            arch: None,
            entrypoint: Some(manifest.entrypoint.clone()),
        }],
        data: ManifestData {
            access: (manifest.kind == PackageKind::Bridge)
                .then_some(engine::package_manifest::DataAccessRule {
                    type_id: "note".into(),
                    versions: "*".into(),
                    actions: vec![DataAction::Read, DataAction::Update],
                    fields: FieldAccess {
                        read: vec!["title".into(), "body".into()],
                        write: vec!["title".into(), "body".into()],
                    },
                    relations: None,
                })
                .into_iter()
                .collect(),
            defines: vec![],
            mappings: vec![],
        },
        integration: None,
    })
}

fn assert_owner_only_acl(path: &std::path::Path) {
    let output = std::process::Command::new("icacls")
        .arg(path)
        .output()
        .expect("icacls");
    assert!(output.status.success(), "icacls failed: {:?}", output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        !text.contains("(I)"),
        "inherited ACL remains for {path:?}: {text}"
    );
    for principal in ["BUILTIN\\Users", "Everyone", "Authenticated Users"] {
        assert!(
            !text.contains(principal),
            "broad ACL {principal} remains for {path:?}: {text}"
        );
    }
}

fn install_fixture(
    directory: &tempfile::TempDir,
    manifest: &PackageManifest,
) -> (Arc<PackageStore>, engine::package_store::InstalledPackage) {
    install_binary(directory, manifest, fixture())
}

fn install_binary(
    directory: &tempfile::TempDir,
    manifest: &PackageManifest,
    binary: PathBuf,
) -> (Arc<PackageStore>, engine::package_store::InstalledPackage) {
    let archive_path = directory.path().join("worker.kspkg");
    let file = std::fs::File::create(&archive_path).expect("archive");
    let mut archive = ZipWriter::new(file);
    let versioned = versioned_manifest(manifest);
    archive
        .start_file("manifest.json", FileOptions::default())
        .expect("manifest entry");
    archive
        .write_all(&serde_json::to_vec(&versioned).expect("manifest json"))
        .expect("manifest bytes");
    // Store the worker binary uncompressed: deflate costs seconds for the
    // multi-megabyte bridge fixture and the store re-inflates it on every
    // install and pre-launch integrity check.
    archive
        .start_file(
            versioned.entrypoint(),
            FileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .expect("worker entry");
    archive
        .write_all(&std::fs::read(binary).expect("fixture bytes"))
        .expect("worker bytes");
    archive.finish().expect("archive finish");
    let bytes = std::fs::read(&archive_path).expect("archive bytes");
    let hash = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let store = Arc::new(PackageStore::new(directory.path().join("store")).expect("store"));
    let installed = store
        .install_versioned(&archive_path, bytes.len() as u64, &hash, &versioned, 1)
        .expect("install");
    store
        .enable_worker(&manifest.id, &manifest.version)
        .expect("enable");
    (store, installed)
}

#[tokio::test]
async fn fixture_workers_validate_protocol_and_fail_closed() {
    let directory = tempfile::tempdir().expect("fixture marker directory");
    let markers = fixture_env(&directory);
    // Short injected backoff: the production schedule (1s/5s/30s, see
    // restart_policy_uses_bounded_backoff) is exercised shape-for-shape here,
    // including the assertion that retries are spaced, without spending 36 s
    // of wall time on sleeps.
    let restart_delays = vec![
        Duration::from_millis(250),
        Duration::from_millis(500),
        Duration::from_millis(750),
    ];
    let min_retry_wait: Duration = restart_delays.iter().sum();
    let supervisor = PackageWorkerSupervisor::with_restart_delays(1, restart_delays);
    let state = tempfile::tempdir().expect("worker state directory");
    let roots = [std::env::temp_dir()];
    let normal = manifest("fixture.normal");
    test_support::reset_resume_count();
    let mut barrier = test_support::pause_next_before_resume();
    let state_root = state.path().to_path_buf();
    let start_supervisor = supervisor.clone();
    let start_manifest = normal.clone();
    let launch = tokio::spawn(async move {
        start_supervisor
            .start(
                &start_manifest,
                fixture(),
                state_root,
                "hash".into(),
                &[std::env::temp_dir()],
                "corr".into(),
                None,
                None,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), barrier.suspended_ready())
        .await
        .expect("resume barrier");
    assert!(!launch.is_finished());
    assert!(!markers.entry.exists());
    assert!(!markers.bootstrap.exists());
    assert_eq!(test_support::resume_count(), 0);
    barrier.release();
    tokio::time::timeout(Duration::from_secs(3), launch)
        .await
        .expect("normal start timeout")
        .expect("start task")
        .expect("normal worker start");
    assert_eq!(test_support::resume_count(), 1);
    assert_eq!(
        supervisor.health(&normal.id, &normal.version).state,
        WorkerState::Running
    );
    assert!(
        markers.entry.exists(),
        "entry marker must precede bootstrap"
    );
    assert_eq!(
        std::fs::read_to_string(&markers.bootstrap).unwrap(),
        "bootstrap:1\n"
    );
    assert_eq!(
        supervisor
            .start(
                &normal,
                fixture(),
                state.path().to_path_buf(),
                "hash".into(),
                &roots,
                "corr".into(),
                None,
                None
            )
            .await,
        Err("already-running")
    );
    supervisor
        .stop(&normal.id, &normal.version)
        .await
        .expect("worker stop");

    let m = manifest("fixture.wrong-token");
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        supervisor.start(
            &m,
            fixture(),
            state.path().to_path_buf(),
            "hash".into(),
            &roots,
            "corr".into(),
            None,
            None,
        ),
    )
    .await
    .expect("fixture timeout");
    assert_eq!(result, Err("worker-unavailable"));
    assert!(
        started.elapsed() >= min_retry_wait,
        "retries must wait out the injected backoff schedule, elapsed {:?}",
        started.elapsed()
    );
    assert_eq!(
        supervisor.health(&m.id, &m.version).state,
        WorkerState::Failed
    );
    let diagnostic = supervisor
        .diagnostics()
        .into_iter()
        .find(|worker| worker.id == m.id)
        .expect("diagnostic");
    assert_eq!(diagnostic.generation, 4);
    assert_eq!(diagnostic.restart_count, 3);
    let health = format!("{:?}", supervisor.health(&m.id, &m.version));
    assert!(!health.contains("hash") && !health.contains("corr"));
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
    let _ = supervisor.stop_all().await;
}

#[tokio::test]
async fn stop_suppresses_initial_failure_retries() {
    let _lock = test_support::serialized();
    // Short first backoff lets one retry run fast; the long second delay is
    // what `stop` must cancel before the third generation ever launches.
    let supervisor = PackageWorkerSupervisor::with_restart_delays(
        1,
        vec![Duration::from_millis(400), Duration::from_secs(3)],
    );
    let m = manifest("fixture.initial-fail");
    let state = tempfile::tempdir().expect("worker state directory");
    let start_manifest = m.clone();
    let running = supervisor.clone();
    let start = tokio::spawn(async move {
        running
            .start(
                &start_manifest,
                fixture(),
                state.path().to_path_buf(),
                "hash".into(),
                &[std::env::temp_dir()],
                "corr".into(),
                None,
                None,
            )
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    supervisor
        .stop(&m.id, &m.version)
        .await
        .expect("worker stop");
    assert_eq!(start.await.expect("start task"), Err("worker-unavailable"));
    let diagnostic = supervisor
        .diagnostics()
        .into_iter()
        .find(|worker| worker.id == m.id)
        .expect("diagnostic");
    assert!(diagnostic.generation <= 2);
    assert_eq!(
        supervisor.health(&m.id, &m.version).state,
        WorkerState::Stopped
    );
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
    let _ = supervisor.stop_all().await;
}

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
    let object_type = tokio::time::timeout(std::time::Duration::from_secs(5), async {
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
    // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
    let _ = supervisor.stop_all().await;
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

    let collection = tokio::time::timeout(Duration::from_secs(5), async {
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
    // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
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
        .find(|registration| registration.type_id == "com.kosmos.note")
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
            "type_id":"com.kosmos.note",
            "type_version":"1.0.0",
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
            "kind":"read_object","type_id":"com.kosmos.note","type_version":"1.0.0",
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
            "kind":"create_object","type_id":"com.kosmos.game","type_version":"1.0.0",
            "object_id":"typed-worker-note","fields":[],"links":[]
        }),
    )
    .await;
    assert_eq!(collision, Err("conflict"));
    let wrong_type_delete = ArkRequestExecutor::request(
        &ark,
        "data.request",
        serde_json::json!({
            "kind":"delete_object","type_id":"com.kosmos.game","type_version":"1.0.0",
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
    assert_eq!(object.data["typeId"], "com.kosmos.note");
}

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
    let projected = tokio::time::timeout(Duration::from_secs(8), async {
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
    tokio::time::timeout(Duration::from_secs(5), async {
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
    let round_trip = tokio::time::timeout(Duration::from_secs(8), async {
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
    tokio::time::sleep(Duration::from_millis(600)).await;
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
    let retried = tokio::time::timeout(std::time::Duration::from_secs(4), async {
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

    tokio::time::timeout(Duration::from_secs(3), async {
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
            "WORKER_SECRET_UNIQUE ARK_MARKDOWN_BODY_UNIQUE RAW_REQUEST_PAYLOAD_UNIQUE \\
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

#[tokio::test]
async fn prepublication_wait_failures_quarantine_and_reap_exact_startup() {
    let _lock = test_support::serialized();
    for (suffix, cleanup_failure) in [
        ("wait-timeout", FailureStage::WaitTimeout),
        ("wait-failed", FailureStage::WaitFailed),
    ] {
        let state = tempfile::tempdir().expect("worker state directory");
        test_support::reset();
        let supervisor = PackageWorkerSupervisor::new(1);
        let worker = manifest(&format!("fixture.{suffix}"));
        test_support::fail_next(FailureStage::PreResume);
        test_support::fail_cleanup_next(cleanup_failure);
        assert_eq!(
            supervisor
                .test_start_once(
                    &worker,
                    fixture(),
                    state.path().to_path_buf(),
                    "hash".into(),
                    &[],
                    suffix.into(),
                )
                .await,
            Err("process-cleanup-failed")
        );
        assert_eq!(supervisor.test_registry_snapshot(), (0, 1, 0, 0));
        assert_eq!(
            supervisor.test_startup_reservation_snapshot(&worker.id, &worker.version, 1),
            Some((true, true))
        );
        assert_eq!(
            supervisor
                .test_start_once(
                    &worker,
                    fixture(),
                    state.path().to_path_buf(),
                    "hash".into(),
                    &[],
                    "replacement-before-reap".into(),
                )
                .await,
            Err("already-running")
        );
        test_support::reset();
        supervisor
            .test_reap_worker_startup(&worker.id, &worker.version)
            .await;
        assert_eq!(supervisor.test_registry_snapshot(), (0, 0, 0, 0));
        test_support::capture_next_process();
        assert_eq!(
            supervisor
                .test_start_once(
                    &worker,
                    fixture(),
                    state.path().to_path_buf(),
                    "hash".into(),
                    &[],
                    "replacement-after-reap".into(),
                )
                .await,
            Ok(())
        );
        let running = supervisor
            .test_worker_snapshot(&worker.id, &worker.version)
            .await;
        assert!(running.3, "replacement did not publish a real process");
        assert!(supervisor.stop(&worker.id, &worker.version).await.is_ok());
        assert!(test_support::take_captured_process()
            .expect("replacement process")
            .wait_object_0());
        assert_eq!(supervisor.test_registry_snapshot(), (0, 0, 0, 0));
        // Drain every supervisor registry so no task keeps a worker child or
        // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
        let _ = supervisor.stop_all().await;
    }
    test_support::reset();
}

#[tokio::test]
async fn supervisor_pid_unavailable_rolls_back_all_worker_reservations() {
    let _lock = test_support::serialized();
    let supervisor = PackageWorkerSupervisor::new(1);
    let worker = manifest("fixture.pid-unavailable");
    let state = tempfile::tempdir().expect("worker state directory");
    test_support::fail_next(engine::package_worker_process::FailureStage::PidUnavailable);

    assert_eq!(
        supervisor
            .test_start_once(
                &worker,
                fixture(),
                state.path().to_path_buf(),
                "hash".into(),
                &[],
                "pid-unavailable".into(),
            )
            .await,
        Err("pid-unavailable")
    );
    assert_eq!(supervisor.test_registry_snapshot(), (0, 0, 0, 0));
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
    let _ = supervisor.stop_all().await;
}

#[tokio::test]
async fn supervisor_cleanup_failure_retains_holder_and_disables_replacement() {
    let _lock = test_support::serialized();
    let supervisor = PackageWorkerSupervisor::new(1);
    let worker = manifest("fixture.cleanup-retained");
    let state = tempfile::tempdir().expect("worker state directory");
    supervisor
        .start(
            &worker,
            fixture(),
            state.path().to_path_buf(),
            "hash".into(),
            &[],
            "cleanup-retained".into(),
            None,
            None,
        )
        .await
        .expect("worker start");

    test_support::fail_cleanup_next(engine::package_worker_process::FailureStage::Terminate);
    assert_eq!(
        supervisor.stop(&worker.id, &worker.version).await,
        Err("cleanup-failed")
    );
    let snapshot = supervisor
        .test_worker_snapshot(&worker.id, &worker.version)
        .await;
    assert_eq!(snapshot.0, WorkerState::Failed);
    assert_eq!(snapshot.1.as_deref(), Some("cleanup-failed"));
    assert!(!snapshot.2);
    assert!(snapshot.3);
    assert_eq!(
        supervisor
            .start(
                &worker,
                fixture(),
                state.path().to_path_buf(),
                "hash".into(),
                &[],
                "replacement".into(),
                None,
                None,
            )
            .await,
        Err("already-running")
    );
    assert!(supervisor.stop(&worker.id, &worker.version).await.is_ok());
    assert!(
        !supervisor
            .test_worker_snapshot(&worker.id, &worker.version)
            .await
            .3
    );
    test_support::capture_next_process();
    assert!(supervisor
        .start(
            &worker,
            fixture(),
            state.path().to_path_buf(),
            "hash".into(),
            &[],
            "replacement-after-stop".into(),
            None,
            None,
        )
        .await
        .is_ok());
    let replacement = supervisor
        .test_worker_snapshot(&worker.id, &worker.version)
        .await;
    assert!(replacement.3, "replacement did not retain a real process");
    let mut holder_gate = supervisor
        .test_hold_process_holder(&worker.id, &worker.version)
        .expect("holder");
    holder_gate.ready().await;
    let terminations = test_support::termination_count();
    assert_eq!(
        supervisor
            .test_stop_all_until(std::time::Instant::now() + Duration::from_millis(1))
            .await,
        Err("cleanup-failed")
    );
    assert_eq!(test_support::termination_count(), terminations);
    holder_gate.release();
    assert!(supervisor.stop(&worker.id, &worker.version).await.is_ok());
    assert!(test_support::take_captured_process()
        .expect("replacement process")
        .wait_object_0());
    assert!(
        !supervisor
            .test_worker_snapshot(&worker.id, &worker.version)
            .await
            .3
    );
}

#[tokio::test]
async fn cancellation_after_process_publication_reaps_without_losing_holder() {
    let _lock = test_support::serialized();
    let supervisor = PackageWorkerSupervisor::new(1);
    let worker = manifest("fixture.cancel-after-launch");
    let warmup = manifest("fixture.handle-warmup");
    let state = tempfile::tempdir().expect("worker state directory");
    supervisor
        .test_start_once(
            &warmup,
            fixture(),
            state.path().to_path_buf(),
            "hash".into(),
            &[],
            "warmup".into(),
        )
        .await
        .expect("worker handle accounting warmup");
    supervisor
        .stop(&warmup.id, &warmup.version)
        .await
        .expect("warmup stop");
    let baseline_handles = test_support::process_handle_count();
    test_support::capture_next_process();
    let mut gate = supervisor.test_pause_after_launch();
    let task_supervisor = supervisor.clone();
    let task_worker = worker.clone();
    let task_state = state.path().to_path_buf();
    let launch = tokio::spawn(async move {
        task_supervisor
            .test_start_once(
                &task_worker,
                fixture(),
                task_state,
                "hash".into(),
                &[],
                "cancel-after-launch".into(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), gate.ready())
        .await
        .expect("after-launch gate");
    assert!(!launch.is_finished());
    assert_eq!(
        supervisor.test_startup_reservation_snapshot(&worker.id, &worker.version, 1),
        Some((true, true))
    );
    assert_eq!(supervisor.test_registry_snapshot().1, 1);

    assert_eq!(
        supervisor
            .test_start_once(
                &worker,
                fixture(),
                state.path().to_path_buf(),
                "replacement".into(),
                &[],
                "replacement-while-starting".into(),
            )
            .await,
        Err("worker-unavailable")
    );
    let cancel_supervisor = supervisor.clone();
    let cancel_worker = worker.clone();
    let cancel = tokio::spawn(async move {
        cancel_supervisor
            .test_reap_worker_startup(&cancel_worker.id, &cancel_worker.version)
            .await;
    });
    tokio::task::yield_now().await;
    assert_eq!(
        supervisor.test_startup_reservation_snapshot(&worker.id, &worker.version, 1),
        Some((true, true))
    );
    assert_eq!(supervisor.test_registry_snapshot().1, 1);
    assert!(!launch.is_finished());

    gate.release();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(10), launch)
            .await
            .expect("launch cleanup")
            .expect("launch task"),
        Err("worker-unavailable")
    ));
    tokio::time::timeout(Duration::from_secs(10), cancel)
        .await
        .expect("startup cancellation")
        .expect("cancellation task");
    assert!(test_support::take_captured_process()
        .expect("published process")
        .wait_object_0());
    assert_eq!(test_support::process_handle_count(), baseline_handles);
    assert_eq!(
        supervisor.test_startup_reservation_snapshot(&worker.id, &worker.version, 1),
        None
    );
    assert_eq!(supervisor.test_registry_snapshot(), (0, 0, 0, 0));
}
