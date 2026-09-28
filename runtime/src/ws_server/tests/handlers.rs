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
async fn calculator_op_returns_result_or_quiet_null() {
    let data_dir = tempfile::tempdir().unwrap();
    let result = handle_calculator_op(
        "evaluate",
        serde_json::json!({ "query": "1200 * 1.2" }),
        data_dir.path(),
    )
    .await;
    assert!(result.ok);
    assert_eq!(result.data["result"], "1440");
    assert_eq!(result.data["expression"], "1200 * 1.2");

    let search_text = handle_calculator_op(
        "evaluate",
        serde_json::json!({ "query": "settings" }),
        data_dir.path(),
    )
    .await;
    assert!(search_text.ok);
    assert!(search_text.data["result"].is_null());
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

    let status = handle_package_op("trust_status", serde_json::Value::Null, &service).await;
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
