// Native app icons (KOS-285): the descriptor's embedded product PNG is
// materialized under `<Apps>/icons/<id>.png` and reported as `icon_path` on
// every `apps.list` row — installed or not. Included into the same
// `package_service::tests` module as native_tests.rs, which owns the shared
// `native_service`/`probe` helpers.

#[tokio::test]
async fn native_list_materializes_descriptor_icons() {
    let dir = tempdir().expect("temp dir");
    let server = httpmock::MockServer::start_async().await;
    let service = native_service(&dir);
    let apps = service
        .native_apps_with(&probe(&server), false)
        .await
        .expect("list");
    for row in &apps {
        let icon = row.icon_path.as_deref().expect("icon path");
        assert!(icon.ends_with(".png"), "{icon}");
        assert!(!icon.starts_with(r"\\?\"), "{icon}");
        let bytes = fs::read(icon).expect("icon file exists");
        let desc = crate::native_apps::app_descriptor(&row.id).expect("descriptor");
        assert_eq!(bytes, desc.icon_png, "{icon} must hold the product icon");
    }
    // The icons dir is inside the store root but never surfaces as an app —
    // `list()` skips it because it carries no install.json.
    assert!(dir.path().join("apps").join("icons").is_dir());
    assert_eq!(
        service
            .native_store()
            .expect("store")
            .list()
            .iter()
            .filter(|record| record.id == "icons")
            .count(),
        0
    );
}

#[tokio::test]
async fn icon_write_failure_degrades_to_no_icon_path() {
    // A blocked icons dir must not fail the list — the row reports no
    // icon_path and the Manager renders its letter placeholder. The warning
    // is logged by ensure_native_icon (not asserted here).
    let dir = tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join("apps")).expect("apps dir");
    // A regular file named `icons` makes create_dir_all fail for every app.
    fs::write(dir.path().join("apps").join("icons"), b"occupied").expect("blocker");
    let server = httpmock::MockServer::start_async().await;
    let service = native_service(&dir);
    let apps = service
        .native_apps_with(&probe(&server), false)
        .await
        .expect("list must not fail on icon io errors");
    assert!(!apps.is_empty());
    for row in &apps {
        assert_eq!(row.icon_path, None, "{:?} icon_path", row.id);
    }
}
