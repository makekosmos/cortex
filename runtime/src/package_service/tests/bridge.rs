#[test]
fn bridge_config_requires_one_real_vault_and_persists_owner_state() {
    let dir = tempdir().expect("tempdir");
    let vault = dir.path().join("vault");
    fs::create_dir(&vault).expect("vault");
    let config = BridgeConfig {
        vault_root: vault.to_string_lossy().into_owned(),
        selected_types: vec!["com.kosmos.note".into()],
        editable_fields: vec!["title".into(), "body".into()],
        readonly_fields: vec!["machine_output".into()],
    };
    assert!(validate_bridge_config(&config).is_ok());
    let state = BridgeConfigState {
        configs: HashMap::from([("ark-markdown-bridge@1.0.0".into(), config)]),
    };
    let packages = dir.path().join("packages");
    fs::create_dir(&packages).expect("packages");
    write_owner_only_json(&packages.join("bridge-config.json"), &state).expect("write");
    assert_eq!(read_bridge_configs(&packages).configs, state.configs);
    assert!(validate_bridge_config(&BridgeConfig {
        vault_root: r"\\server\vault".into(),
        selected_types: vec!["note".into()],
        editable_fields: vec![],
        readonly_fields: vec![]
    })
    .is_err());
}

#[cfg(windows)]
#[tokio::test]
async fn catalog_bridge_runs_through_service() {
    use crate::{ark_host::ArkHost, package_worker_supervisor::PackageWorkerSupervisor};
    use std::sync::Arc;
    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault");
    fs::create_dir(&vault).unwrap();
    let manifest = ManifestV2 {
        schema_version: 2,
        id: "ark-markdown-bridge".into(),
        name: "ARK Markdown Bridge".into(),
        description: None,
        version: "1.0.0".into(),
        kind: PackageKind::Bridge,
        engine_api: ">=1".into(),
        entrypoint: "ark-markdown-bridge.exe".into(),
        icon: None,
        publisher: "kosmos".into(),
        permissions: vec![
            PermissionRequest {
                capability: "ark.read".into(),
                scopes: vec!["list_objects".into(), "get_object".into()],
            },
            PermissionRequest {
                capability: "ark.write".into(),
                scopes: vec!["upsert_object".into(), "external_refs.upsert".into()],
            },
            PermissionRequest {
                capability: "filesystem.read".into(),
                scopes: vec![],
            },
            PermissionRequest {
                capability: "filesystem.write".into(),
                scopes: vec![],
            },
        ],
        targets: vec![crate::package_manifest::ManifestTarget {
            runtime: crate::package_manifest::TargetRuntime::Worker,
            os: vec![
                crate::package_manifest::TargetOs::Windows,
                crate::package_manifest::TargetOs::Macos,
                crate::package_manifest::TargetOs::Linux,
            ],
            arch: None,
            entrypoint: None,
        }],
        data: crate::package_manifest::ManifestData {
            access: vec![],
            defines: vec![],
            mappings: vec![],
        },
        integration: None,
        store: None,
    };
    let versioned = VersionedManifest::V2(manifest.clone());
    let (archive, hash, size) = archive_bridge_binary(dir.path(), &versioned);
    let mut service = PackageService::open_for_test(dir.path()).unwrap();
    let catalog = CatalogDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        packages: vec![CatalogEntry {
            manifest: VersionedManifest::V2(manifest.clone()),
            archives: vec![catalog_archive(
                "https://packages.kosmos.dev/bridge.kspkg",
                hash,
                size,
            )],
        }],
        external_apps: vec![],
        revoked: vec![],
    };
    let bytes = document_bytes(&catalog);
    service.apply_catalog(&bytes).unwrap();
    service
        .install_from_path(&manifest.id, &manifest.version, archive)
        .unwrap();
    let ark = Arc::new(
        ArkHost::open(dir.path().join("ark.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let note_registration = ark_core::canonical_types::definitions::canonical_type_registrations()
        .unwrap()
        .into_iter()
        .find(|registration| registration.type_id == "com.kosmos.note")
        .unwrap();
    assert!(
        ark.request(
            "types.registerPackageDefinitions",
            serde_json::json!({"registrations":[note_registration]})
        )
        .await
        .unwrap()
        .ok
    );
    assert!(
        ark.request(
            "upsert_object",
            serde_json::json!(
                    {"object":{"id":"service-note",
                    "typeId":"com.kosmos.note",
                    "typeVersion":"1.0.0",
                    "title":"Service note",
                    "contentJson":{"type":"doc",
                    "content":[{"type":"paragraph",
                    "content":[{"type":"text",
                    "text":"from ark"}]}]},
                    "propsJson":{"description":null,
                    "extensions":{}},
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-01T00:00:00Z",
                    "deletedAt":null},
                    "device_id":"bridge-service"}),
        )
        .await
        .unwrap()
        .ok
    );
    let objects = ark
        .request("list_objects", serde_json::Value::Null)
        .await
        .unwrap()
        .data;
    let note = objects
        .as_array()
        .and_then(|objects| objects.iter().find(|object| object["id"] == "service-note"))
        .expect("created note must be visible to the bridge");
    assert_eq!(note["typeId"], "com.kosmos.note");
    let supervisor = PackageWorkerSupervisor::with_ark(1, ark.clone());
    service.configure_workers(supervisor.clone(), vec![], "bridge-service".into());
    service
        .set_bridge_config(
            &manifest.id,
            &manifest.version,
            BridgeConfig {
                vault_root: vault.to_string_lossy().into_owned(),
                selected_types: vec!["com.kosmos.note".into()],
                editable_fields: vec!["title".into(), "body".into()],
                readonly_fields: vec![],
            },
        )
        .await
        .unwrap();
    if let Err(error) = service
        .set_enabled(&manifest.id, &manifest.version, true)
        .await
    {
        panic!(
            "bridge enable failed: {error:?}; diagnostics: {:?}",
            supervisor.diagnostics()
        );
    }
    let markdown = vault.join("Service note-service-note.md");
    let bridge_state = dir
        .path()
        .join("packages")
        .join("bridge-state")
        .join(&manifest.id)
        .join(&manifest.version)
        .join("state.json");
    // Generous hang guard only — the files appearing are the event; the
    // cap must outlast worker spawn under parallel gate load (KOS-308).
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        // Wait for durable provenance before editing the first snapshot;
        // otherwise the worker can classify the edit as initial state.
        while !markdown.exists() || !bridge_state.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "bridge did not write markdown: {:?}",
            supervisor.diagnostics(),
        )
    });
    let content = fs::read_to_string(&markdown).unwrap();
    fs::write(&markdown, content.replace("from ark", "from vault")).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            let current = ark
                .request("get_object", serde_json::json!({"id":"service-note"}))
                .await
                .unwrap();
            if current.data["contentJson"]["content"][0]["content"][0]["text"] == "from vault" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "bridge did not sync vault edit: {:?}",
            supervisor.diagnostics()
        )
    });

    let mut replacement = manifest.clone();
    replacement.name = "ARK Markdown Bridge Updated".into();
    let (replacement_archive, replacement_hash, replacement_size) =
        archive_bridge_binary(dir.path(), &VersionedManifest::V2(replacement.clone()));
    let update = CatalogDocument {
        schema_version: 1,
        sequence: 2,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        packages: vec![CatalogEntry {
            manifest: VersionedManifest::V2(replacement.clone()),
            archives: vec![catalog_archive(
                "https://packages.kosmos.dev/bridge-update.kspkg",
                replacement_hash,
                replacement_size,
            )],
        }],
        external_apps: vec![],
        revoked: vec![],
    };
    let bytes = document_bytes(&update);
    service.apply_catalog(&bytes).unwrap();
    let updated = service
        .install_from_path_with_worker_stop(
            &replacement.id,
            &replacement.version,
            replacement_archive,
        )
        .await
        .unwrap();
    assert!(updated.enabled);
    assert_eq!(updated.worker_state, WorkerState::Running);
    assert_eq!(
        service
            .store
            .installed(&replacement.id, &replacement.version)
            .unwrap()
            .manifest
            .common_manifest()
            .name,
        replacement.name
    );

    let previous = service
        .store
        .installed(&replacement.id, &replacement.version)
        .unwrap();
    let mut broken = replacement.clone();
    broken.name = "ARK Markdown Bridge Broken Update".into();
    let (broken_archive, broken_hash, broken_size) =
        archive_bridge_binary(dir.path(), &VersionedManifest::V2(broken.clone()));
    let broken_update = CatalogDocument {
        schema_version: 1,
        sequence: 3,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        packages: vec![CatalogEntry {
            manifest: VersionedManifest::V2(broken.clone()),
            archives: vec![catalog_archive(
                "https://packages.kosmos.dev/bridge-broken.kspkg",
                broken_hash,
                broken_size,
            )],
        }],
        external_apps: vec![],
        revoked: vec![],
    };
    let bytes = document_bytes(&broken_update);
    service.apply_catalog(&bytes).unwrap();
    supervisor.test_fail_next_start();
    let failed_update = service
        .install_from_path_with_worker_stop(&broken.id, &broken.version, broken_archive)
        .await;
    assert!(
        matches!(failed_update, Err(PackageError::Worker("unavailable"))),
        "unexpected failed update result: {failed_update:?}"
    );
    assert_eq!(
        service
            .store
            .installed(&replacement.id, &replacement.version)
            .unwrap(),
        previous
    );
    assert_eq!(
        supervisor
            .health(&replacement.id, &replacement.version)
            .state,
        WorkerState::Running
    );
    assert_eq!(
        supervisor
            .diagnostics()
            .into_iter()
            .find(|worker| worker.id == replacement.id && worker.version == replacement.version)
            .and_then(|worker| worker.hash),
        Some(previous.hash.clone())
    );
    service
        .set_enabled(&manifest.id, &manifest.version, false)
        .await
        .unwrap();
    assert_eq!(
        supervisor.health(&manifest.id, &manifest.version).state,
        WorkerState::Stopped
    );
    // Drain worker tasks so nothing holds the test dir's files past
    // tempdir cleanup (KOS-270).
    supervisor.stop_all().await.expect("stop all workers");
}

// Native-app tests live in package_service/native_tests.rs (KOS-265) —
// gated on the host target they exercise so nothing passes vacuously.
