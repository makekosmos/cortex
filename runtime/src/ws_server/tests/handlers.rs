use super::*;
#[test]
fn app_index_entry_uses_icon_ref_without_inline_data_url() {
    // Regression: 2026-06-09. app_index.search must not read/base64 top-N icons.
    let app = App {
        id: "calc".into(),
        name: "Calculator".into(),
        exec_path: "C:\\Windows\\System32\\calc.exe".into(),
        icon_path: Some("C:\\Mundus\\icons\\calc.png".into()),
        icon_source: None,
        kind: AppKind::Win32,
        source: "test".into(),
        mtime: 1,
    };

    let entry = app_index_entry_json(&app);

    assert_eq!(entry["icon_path"], serde_json::Value::Null);
    assert_eq!(entry["icon_ref"], "mundus-icon://app/calc");
}

#[tokio::test]
async fn file_index_diagnostics_op_returns_enriched_payload() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("diag-note.md"), "v1").unwrap();
    let index =
        Arc::new(FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap());
    index.rescan().await.unwrap();

    let response = handle_file_index_op("diagnostics", serde_json::Value::Null, &index).await;

    assert!(response.ok);
    assert_eq!(response.data["roots_count"], 1);
    assert_eq!(response.data["files_count"], 1);
    assert!(response.data.get("db_size_bytes").is_some());
}

#[tokio::test]
async fn file_index_estimate_root_op_returns_estimate_payload() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("notes.md"), "v1").unwrap();
    std::fs::write(root.path().join("photo.png"), "v1").unwrap();
    let index =
        Arc::new(FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap());

    let response = handle_file_index_op(
        "estimate_root",
        serde_json::json!({ "path": root.path().to_string_lossy() }),
        &index,
    )
    .await;

    assert!(response.ok);
    assert_eq!(response.data["indexable_text_files_count"], 1);
    assert_eq!(response.data["metadata_only_media_files_count"], 1);
}

#[tokio::test]
async fn package_api_returns_bounded_metadata_without_trust_material_or_paths() {
    let data = tempfile::tempdir().unwrap();
    let service = Arc::new(PackageService::open(data.path()).unwrap());

    let status = handle_package_op("catalog_status", serde_json::Value::Null, &service).await;
    assert!(status.ok);
    let status_json = status.data.to_string();
    for forbidden in [
        "public_key",
        "signature",
        "archive_path",
        "entrypoint",
        "permissions",
        "sha256",
    ] {
        assert!(!status_json.contains(forbidden), "{status_json}");
    }

    let list = handle_package_op("list", serde_json::Value::Null, &service).await;
    assert!(list.ok);
    assert_eq!(list.data["total"], 0);
    assert_eq!(list.data["truncated"], false);

    let private_path = r"C:\Users\alice\private\package.kspkg";
    let install = handle_package_op(
        "install",
        serde_json::json!({
            "id": "com.kosmos.demo",
            "version": "1.0.0",
            "archive_path": private_path,
        }),
        &service,
    )
    .await;
    assert!(!install.ok);
    assert!(!install.error.unwrap_or_default().contains(private_path));
}

/// `packages.open` mints a fresh launch lease per call through the registry
/// the Engine shares in — and every failure is a typed
/// `packages.open: <code>` wire error, never a bare string.
#[tokio::test]
async fn package_open_mints_per_tab_leases_and_reports_typed_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
    let service = Arc::new(service);

    let open = |params: serde_json::Value| {
        let service = service.clone();
        async move {
            let service = service;
            handle_package_op("open", params, &service).await
        }
    };

    // No launch surface wired (an engine without its HTTP API) — the op
    // must fail closed with a typed code, not panic.
    let response = open(serde_json::json!({"package_id": "com.kosmos.demo"})).await;
    assert!(!response.ok);
    assert_eq!(
        response.error.as_deref(),
        Some("packages.open: unavailable")
    );

    // Unknown id is `not-installed`, a malformed id is `invalid-request`,
    // and a missing id never reaches the resolver.
    for (params, expected) in [
        (
            serde_json::json!({"package_id": "com.kosmos.missing"}),
            "packages.open: not-installed",
        ),
        (
            serde_json::json!({"package_id": "com.kosmos.demo", "version": "9.9.9"}),
            "packages.open: not-installed",
        ),
        (
            serde_json::json!({"package_id": "com.kosmos.\u{7}demo"}),
            "packages.open: invalid-request",
        ),
        (serde_json::json!({}), "packages.open: invalid-request"),
    ] {
        let response = open(params).await;
        assert!(!response.ok);
        assert_eq!(response.error.as_deref(), Some(expected));
    }

    let leases = Arc::new(std::sync::Mutex::new(
        crate::package_launch::LaunchLeaseRegistry::default(),
    ));
    service.configure_launch_surface(crate::package_launch::LaunchSurface {
        leases: leases.clone(),
        http_port: 12_345,
    });

    let first = open(serde_json::json!({"package_id": "com.kosmos.demo"})).await;
    assert!(
        first.ok,
        "open must mint a lease: {}",
        first.error.unwrap_or_default()
    );
    // The reply is just what the Manager opens: the launch URL carries the
    // one-time bootstrap code in the fragment — no token crosses to it.
    let launch_url = first.data["launch_url"].as_str().expect("launch_url");
    let origin_host = crate::package_launch::package_origin_host("com.kosmos.demo");
    assert!(
        launch_url.starts_with(&format!("http://{origin_host}:12345/v1/apps/assets/"))
            && launch_url.contains("#launch=")
            && launch_url.contains("&code="),
        "{launch_url}"
    );
    assert!(first.data["broker_token"].is_null());
    assert!(first.data["launch_id"].is_null());

    // Every open is a new session — a second tab shares no token, so its
    // pagehide revoke cannot kill the first tab's session.
    let second = open(serde_json::json!({"package_id": "com.kosmos.demo"})).await;
    assert!(second.ok);
    assert_ne!(
        second.data["launch_url"].as_str(),
        first.data["launch_url"].as_str()
    );

    // Disabled app → `disabled`.
    service
        .set_enabled("com.kosmos.demo", "1.0.0", false)
        .await
        .expect("disable");
    let response = open(serde_json::json!({"package_id": "com.kosmos.demo"})).await;
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("packages.open: disabled"));
}

/// KOS-353: Manager package rows are keyed by id and call
/// set_enabled/uninstall/disclosure without `version`. With a single
/// installed (or singly catalog-listed) version the Engine resolves it —
/// the id-only call must work, and only an absent/ambiguous id may fail
/// `invalid-request`.
#[tokio::test]
async fn package_state_ops_resolve_version_for_id_only_calls() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
    let service = Arc::new(service);

    let disclosure = handle_package_op(
        "disclosure",
        serde_json::json!({"package_id": "com.kosmos.demo"}),
        &service,
    )
    .await;
    assert!(disclosure.ok, "{:?}", disclosure.error);

    let disabled = handle_package_op(
        "set_enabled",
        serde_json::json!({"package_id": "com.kosmos.demo", "enabled": false}),
        &service,
    )
    .await;
    assert!(disabled.ok, "{:?}", disabled.error);
    assert_eq!(disabled.data["enabled"], false);

    let enabled = handle_package_op(
        "set_enabled",
        serde_json::json!({"package_id": "com.kosmos.demo", "enabled": true}),
        &service,
    )
    .await;
    assert!(enabled.ok, "{:?}", enabled.error);
    assert_eq!(enabled.data["enabled"], true);

    // An unknown or absent id still fails closed.
    for params in [
        serde_json::json!({"package_id": "com.kosmos.missing", "enabled": true}),
        serde_json::json!({"enabled": true}),
    ] {
        let response = handle_package_op("set_enabled", params, &service).await;
        assert!(!response.ok);
        assert_eq!(
            response.error.as_deref(),
            Some("packages.set_enabled: invalid-request")
        );
    }

    let uninstalled = handle_package_op(
        "uninstall",
        serde_json::json!({"package_id": "com.kosmos.demo"}),
        &service,
    )
    .await;
    assert!(uninstalled.ok, "{:?}", uninstalled.error);
    assert_eq!(uninstalled.data["uninstalled"], true);
}
