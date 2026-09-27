use super::filesystem::*;
use crate::engine_dispatch::DispatchClient;
use crate::grant_authority::GrantAuthorityRegistry;
use base64::Engine as _;
use serde_json::json;
use std::sync::Arc;

fn client(pid: u32) -> DispatchClient {
    DispatchClient {
        pid: Some(pid),
        class: Some("memoria-gpui".to_string()),
        version: None,
        correlation_id: None,
        connection_id: Some(1),
        desktop_authorized: false,
    }
}

#[tokio::test]
async fn open_read_export_close_roundtrip() {
    let grants = Arc::new(GrantAuthorityRegistry::new());
    let vault = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(vault.path().join("notes")).unwrap();
    std::fs::write(vault.path().join("notes/a.md"), "# A").unwrap();
    std::fs::write(vault.path().join("pic.png"), b"\x89PNG").unwrap();
    let opened = open_vault(
        json!({ "path": vault.path().to_str().unwrap() }),
        &client(7),
        &grants,
        true,
    )
    .await;
    assert!(opened.ok, "{:?}", opened.error);
    let root_id = opened.data["rootId"].as_str().unwrap().to_string();
    assert_eq!(opened.data["files"][0]["relativePath"], "notes/a.md");
    assert_eq!(opened.data["images"][0]["mimeType"], "image/png");
    // Absolute paths never leave Engine.
    let serialized = opened.data.to_string();
    assert!(!serialized.contains(vault.path().to_str().unwrap()));

    let read = read_vault_file(
        json!({ "rootId": root_id, "path": "pic.png" }),
        &client(7),
        &grants,
    )
    .await;
    assert!(read.ok);
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(read.data["bytesBase64"].as_str().unwrap())
            .unwrap(),
        b"\x89PNG"
    );

    // Another process (different pid) cannot touch this root.
    let foreign = read_vault_file(
        json!({ "rootId": root_id, "path": "notes/a.md" }),
        &client(8),
        &grants,
    )
    .await;
    assert_eq!(foreign.error.as_deref(), Some("forbidden"));

    let exported = export_vault(
        json!({
            "rootId": root_id,
            "files": [
                { "relativePath": "out/b.md", "content": "hi" },
                { "relativePath": "../escape.md", "content": "x" },
            ],
        }),
        &client(7),
        &grants,
    )
    .await;
    assert_eq!(exported.error.as_deref(), Some("invalid-request"));
    let exported = export_vault(
        json!({
            "rootId": root_id,
            "files": [{ "relativePath": "out/b.md", "content": "hi" }],
        }),
        &client(7),
        &grants,
    )
    .await;
    assert!(exported.ok, "{:?}", exported.error);
    assert_eq!(exported.data["exportedCount"], 1);
    assert_eq!(std::fs::read(vault.path().join("out/b.md")).unwrap(), b"hi");

    let closed = handle_filesystem_op(
        "vault.close",
        json!({ "rootId": root_id }),
        &client(7),
        &grants,
    )
    .await;
    assert_eq!(closed.data["closed"], true);
    let gone = read_vault_file(
        json!({ "rootId": root_id, "path": "notes/a.md" }),
        &client(7),
        &grants,
    )
    .await;
    assert_eq!(gone.error.as_deref(), Some("not-found"));
}

#[tokio::test]
async fn open_rejects_relative_and_missing_paths() {
    let grants = Arc::new(GrantAuthorityRegistry::new());
    let relative = open_vault(json!({ "path": "some/dir" }), &client(1), &grants, true).await;
    assert_eq!(relative.error.as_deref(), Some("invalid-request"));
    let missing = open_vault(
        json!({ "path": "/definitely/not/here" }),
        &client(1),
        &grants,
        true,
    )
    .await;
    assert!(!missing.ok);
}
