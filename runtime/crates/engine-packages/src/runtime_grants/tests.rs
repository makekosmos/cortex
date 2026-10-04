use super::*;

#[test]
fn manifest_v2_compiles_only_matching_dictation_permissions() {
    let raw = concat!(
        r#"{
            "schema_version": 2,
            "id": "com.kosmos.demo",
            "name": "Demo",
            "version": "1.0.0",
            "kind": "app",
            "engine_api": "*",
            "entrypoint": "index.html",
            "publisher": "kosmos",
            "permissions": [
                {"capability": "ark.read", "#,
        r#""scopes": ["dictation.get_state", "dictation.start_recording"]},
                {"capability": "ark.write", "scopes": ["dictation.cancel", "dictation.get_config"]}
            ],
            "targets": [{"runtime": "standalone", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#,
    );
    let crate::package_manifest::VersionedManifest::V2(manifest) =
        crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
    else {
        panic!("expected v2 manifest");
    };

    let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
        .expect("compiled grant");
    assert!(grant.allows_dictation_operation("dictation.get_state"));
    assert!(grant.allows_dictation_operation("dictation.cancel"));
    assert!(!grant.allows_dictation_operation("dictation.get_config"));
    assert!(!grant.allows_dictation_operation("dictation.start_recording"));
}

#[test]
fn manifest_v2_grants_dictation_config_updates_only_with_write_scope() {
    assert_eq!(
        dictation_operation_capability("dictation.update_config"),
        Some("ark.write")
    );
    assert_eq!(
        dictation_operation_capability("dictation.submit_audio"),
        None
    );
    assert_eq!(
        dictation_operation_capability("dictation.capture.future"),
        None
    );
}

#[test]
fn dictation_worker_contract_uses_only_control_scopes() {
    assert_eq!(
        dictation_operation_capability("dictation.capture.start"),
        Some("dictation.control")
    );
    assert_eq!(
        dictation_operation_capability("dictation.lifecycle.set_autostart"),
        Some("dictation.control")
    );
    assert_eq!(
        dictation_operation_capability("dictation.submit_audio"),
        None
    );
}

#[test]
fn manifest_v2_grants_named_network_scopes_only() {
    let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.memoria", "name": "Memoria",
            "version": "0.6.9", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [{"capability": "network", "scopes": ["bookMetadata", "images"]}],
            "targets": [{"runtime": "standalone", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
    let crate::package_manifest::VersionedManifest::V2(manifest) =
        crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
    else {
        panic!("expected v2 manifest");
    };

    let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
        .expect("compiled grant");
    for operation in [
        "bookMetadata.lookupIsbn",
        "bookMetadata.fetchPage",
        "images.fetch",
        "images.dominantColor",
        "images.storeCover",
    ] {
        let scope = app_network_operation_scope(operation).unwrap();
        assert!(grant.allows_app_network_scope(scope), "{operation}");
    }
    assert_eq!(app_network_operation_scope("bookMetadata.evil"), None);
    assert_eq!(app_network_operation_scope("images.deleteAll"), None);
}

#[test]
fn manifest_v2_without_network_scope_denies_app_network_ops() {
    let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.demo", "name": "Demo",
            "version": "1.0.0", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [{"capability": "network", "scopes": ["https://api.example.com"]}],
            "targets": [{"runtime": "standalone", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
    let crate::package_manifest::VersionedManifest::V2(manifest) =
        crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
    else {
        panic!("expected v2 manifest");
    };
    let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
        .expect("compiled grant");
    assert!(!grant.allows_app_network_scope("bookMetadata"));
    assert!(!grant.allows_app_network_scope("images"));
}

#[test]
fn manifest_v2_grants_only_requested_worker_operations() {
    let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.arcadia", "name": "Arcadia",
            "version": "0.1.0", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [{"capability": "worker.invoke", "scopes": ["games.*"]}],
            "targets": [{"runtime": "standalone", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
    let crate::package_manifest::VersionedManifest::V2(manifest) =
        crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
    else {
        panic!("expected v2 manifest");
    };

    let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
        .expect("compiled grant");
    assert!(grant.allows_worker_operation("games.list"));
    assert!(grant.allows_worker_operation("games.rawg.search"));
    assert!(!grant.allows_worker_operation("dictation.get_config"));
}

#[test]
fn manifest_v2_grants_only_requested_focus_operations() {
    let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.focus", "name": "Focus",
            "version": "0.1.0", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [
                {"capability": "ark.read", "scopes": ["pomodoro.get_state"]},
                {"capability": "ark.write", "scopes": ["pomodoro.start", "focus.set_active_state"]}
            ],
            "targets": [{"runtime": "standalone", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
    let crate::package_manifest::VersionedManifest::V2(manifest) =
        crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
    else {
        panic!("expected v2 manifest");
    };

    let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
        .expect("compiled grant");
    assert!(grant.allows_focus_operation("pomodoro.get_state"));
    assert!(grant.allows_focus_operation("pomodoro.start"));
    assert!(grant.allows_focus_operation("focus.set_active_state"));
    assert!(!grant.allows_focus_operation("pomodoro.stop"));
    assert!(!grant.allows_focus_operation("focus.delete_blocklist"));
}

#[test]
fn manifest_v2_grants_only_exact_agents_operations_by_capability() {
    let raw = concat!(
        r#"{
            "schema_version": 2, "id": "com.kosmos.daedalus", "name": "Daedalus",
            "version": "0.1.0", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [
                {"capability": "ark.read", "#,
        r#""scopes": ["agents.projects.list", "agents.models.list", "#,
        r#""agents.sessions.create", "agents.unknown"]},
                {"capability": "ark.write", "#,
        r#""scopes": ["agents.sessions.create", "#,
        r#""agents.editors.open", "agents.projects.list"]}
            ],
            "targets": [{"runtime": "standalone", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#,
    );
    let crate::package_manifest::VersionedManifest::V2(manifest) =
        crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
    else {
        panic!("expected v2 manifest");
    };

    let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
        .expect("compiled grant");
    assert!(grant.allows_agents_operation("agents.projects.list"));
    assert!(grant.allows_agents_operation("agents.sessions.create"));
    assert!(grant.allows_agents_operation("agents.editors.open"));
    assert!(!grant.allows_agents_operation("agents.unknown"));
    assert_eq!(
        agents_operation_capability("agents.projects.list"),
        Some("ark.read")
    );
    assert_eq!(
        agents_operation_capability("agents.sessions.create"),
        Some("ark.write")
    );
    assert_eq!(agents_operation_capability("agents.unknown"), None);
}
