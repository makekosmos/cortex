// Legacy-app migration (KOS-267): installer-marker and kspkg-record
// evidence, outcome persistence and retry semantics. Helpers live in
// native_tests.rs — both files are included into the same test module.

// MIGRATION(KOS-267): remove after 2026-11-01
#[tokio::test]
async fn legacy_package_record_triggers_native_migration_once() {
    let dir = tempdir().expect("temp dir");
    let zip = native_zip(dir.path(), "agenda.zip", &agenda().executable(TARGET));
    let server = httpmock::MockServer::start_async().await;
    stub_release(&server, agenda(), "0.1.1", &zip, None).await;
    let service = native_service(&dir);
    // Seed a legacy 0.9.x package-store record for com.kosmos.agenda.
    let mut legacy = manifest();
    legacy.id = "com.kosmos.agenda".into();
    let legacy_versioned = VersionedManifest::V2(legacy);
    let (kspkg, kspkg_hash, kspkg_size) =
        archive_with_versioned_manifest(dir.path(), &legacy_versioned);
    service
        .store
        .install_versioned(&kspkg, kspkg_size, &kspkg_hash, &legacy_versioned, 1)
        .expect("legacy package install");

    service
        .migrate_legacy_native_apps_with(&probe(&server))
        .await;
    assert!(service.native_app_executable(agenda()).is_ok());
    // The legacy kspkg record is removed once the native app is live.
    assert!(service
        .store
        .list()
        .unwrap()
        .iter()
        .all(|package| package.id != "com.kosmos.agenda"));
    let marker = dir
        .path()
        .join("packages")
        .join("native-apps-migration.json");
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker).expect("marker")).expect("marker json");
    assert_eq!(
        marker["apps"]["com.kosmos.agenda"]["reason"].as_str(),
        Some("installed")
    );

    // Idempotent: a second run changes nothing.
    service
        .migrate_legacy_native_apps_with(&probe(&server))
        .await;
    let apps = service
        .native_apps_with(&probe(&server), false)
        .await
        .expect("list");
    let row = apps
        .iter()
        .find(|row| row.id == "com.kosmos.agenda")
        .unwrap();
    assert_eq!(row.installed_version.as_deref(), Some("0.1.1"));
}

// MIGRATION(KOS-267): remove after 2026-11-01
#[tokio::test]
async fn installer_marker_component_triggers_migration() {
    // The installer recorded `agenda` as a bundled component before wiping
    // the old payload — the marker is the evidence, not the deleted dirs.
    let dir = tempdir().expect("temp dir");
    let zip = native_zip(dir.path(), "agenda.zip", &agenda().executable(TARGET));
    let server = httpmock::MockServer::start_async().await;
    stub_release(&server, agenda(), "0.1.1", &zip, None).await;
    let service = native_service(&dir);
    // Marker sits next to Apps/ under the (here: temp) Mundus local dir.
    fs::write(
        dir.path().join("legacy-components.json"),
        r#"{"schema_version":1,"components":{"agenda":true,"memoria":false,"dictation":false}}"#,
    )
    .expect("marker");

    service
        .migrate_legacy_native_apps_with(&probe(&server))
        .await;
    assert!(service.native_app_executable(agenda()).is_ok());
}

// MIGRATION(KOS-267): remove after 2026-11-01
#[tokio::test]
async fn unreadable_marker_defers_instead_of_settling() {
    // A corrupt marker is undecidable — the app must stay unresolved and be
    // retried next start, never recorded "not-present".
    let dir = tempdir().expect("temp dir");
    let service = native_service(&dir);
    fs::write(dir.path().join("legacy-components.json"), b"not json").expect("marker");

    service.migrate_legacy_native_apps_with(&dead_probe()).await;
    let marker_path = dir
        .path()
        .join("packages")
        .join("native-apps-migration.json");
    let resolved = fs::read(&marker_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|marker| marker["apps"]["com.kosmos.agenda"]["resolved"].as_bool())
        .unwrap_or(false);
    assert!(!resolved, "corrupt marker must not settle the app");
    assert!(service.native_app_executable(agenda()).is_err());
}

// MIGRATION(KOS-267): remove after 2026-11-01
#[tokio::test]
async fn migration_without_legacy_records_is_a_noop() {
    let dir = tempdir().expect("temp dir");
    let service = native_service(&dir);
    service.migrate_legacy_native_apps_with(&dead_probe()).await;
    // No legacy record and no marker → everything resolves "not-present".
    let apps = service
        .native_apps_with(&dead_probe(), false)
        .await
        .expect("list");
    assert!(apps.iter().all(|row| !row.installed));
    let marker_path = dir
        .path()
        .join("packages")
        .join("native-apps-migration.json");
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker_path).expect("marker")).expect("marker json");
    for desc in crate::native_apps::NATIVE_APPS {
        assert_eq!(
            marker["apps"][desc.id]["reason"].as_str(),
            Some("not-present"),
            "{} should resolve as not-present",
            desc.id
        );
    }
}
