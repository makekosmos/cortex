//! Dispatcher tests for `apps.*` — id resolution (A1) and typed error
//! mapping (A5). The heavy install/uninstall behavior lives in the
//! package_service native tests; here we assert the boundary.

use super::*;
use serde_json::json;
use std::sync::Arc;

fn service(dir: &tempfile::TempDir) -> Arc<PackageService> {
    Arc::new(
        PackageService::from_parts(dir.path().join("packages"), Some(dir.path().join("apps")))
            .expect("service"),
    )
}

fn id_param(id: &str) -> Value {
    json!({ "id": id })
}

fn assert_not_found(response: &LocalResponse, op: &str) {
    assert!(!response.ok);
    assert_eq!(
        response.error.as_deref(),
        Some(format!("apps.{op}: not-found").as_str()),
        "expected {op} not-found, got {:?}",
        response.error
    );
}

#[tokio::test]
async fn id_must_name_a_known_descriptor() {
    // A1: "..", "." and unknown ids resolve to not-found at the boundary —
    // the store layer never sees a raw id that could name a path.
    let dir = tempfile::tempdir().expect("temp dir");
    let service = service(&dir);
    for subop in ["install", "uninstall", "open"] {
        for id in ["..", ".", "com.kosmos.unknown", "../x", "AGENDA"] {
            let response = handle_apps_op(subop, id_param(id), &service).await;
            assert_not_found(&response, subop);
        }
    }
    // Nothing may have been touched under the store root — no dirs/files
    // created for rejected ids.
    let entries = std::fs::read_dir(dir.path().join("apps"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(entries, 0);
}

#[tokio::test]
async fn missing_and_typed_params_are_not_found() {
    let dir = tempfile::tempdir().expect("temp dir");
    let service = service(&dir);
    for params in [
        json!({}),
        json!({"id": 42}),
        json!({"app_id": "com.kosmos.agenda"}),
    ] {
        let response = handle_apps_op("uninstall", params, &service).await;
        assert_not_found(&response, "uninstall");
    }
}

#[tokio::test]
async fn uninstall_of_known_but_absent_app_is_ok() {
    // The descriptor resolves; the store reports "nothing to remove" —
    // uninstall is idempotent.
    let dir = tempfile::tempdir().expect("temp dir");
    let service = service(&dir);
    let response = handle_apps_op("uninstall", id_param("com.kosmos.agenda"), &service).await;
    assert!(
        response.ok,
        "uninstall response error: {:?}",
        response.error
    );
}

#[tokio::test]
async fn open_of_absent_app_is_not_found() {
    let dir = tempfile::tempdir().expect("temp dir");
    let service = service(&dir);
    let response = handle_apps_op("open", id_param("com.kosmos.agenda"), &service).await;
    assert!(!response.ok);
    assert_eq!(
        response.error.as_deref(),
        Some("apps.open: not-found"),
        "expected apps.open: not-found, got {:?}",
        response.error
    );
}

#[tokio::test]
async fn unknown_subop_is_prefixed_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let service = service(&dir);
    let response = handle_apps_op("bogus", json!({}), &service).await;
    assert!(!response.ok);
    assert_eq!(
        response.error.as_deref(),
        Some("apps.bogus: unknown sub-operation")
    );
}
