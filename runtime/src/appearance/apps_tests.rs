#[tokio::test]
async fn scoped_get_initializes_from_global_without_persisting() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    store
        .set(
            &json!({"mode": "light", "font_size": 15.0}),
            &manager_client(),
        )
        .await
        .unwrap();

    let scoped = store
        .get(&json!({"app_id": "ordo"}), &client_with_class("ordo"))
        .await
        .unwrap();
    assert_eq!(scoped["app_id"], "ordo");
    assert_eq!(scoped["settings"]["mode"], "light");
    assert_eq!(scoped["settings"]["font_size"], 15.0);
    assert_eq!(scoped["settings"]["revision"], 0);
    assert!(scoped["settings"].get("follow_apps").is_none());
    assert_eq!(scoped["policy"]["following"], false);
    assert_eq!(scoped["policy"]["editable"], true);
    assert_eq!(scoped["policy"]["can_set_follow_apps"], false);
    assert!(
        !store.app_path("ordo").exists(),
        "a read alone must not create a per-app file"
    );
}

#[tokio::test]
async fn scoped_writes_are_isolated_per_app() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());

    store
        .set(
            &json!({"app_id": "ordo", "patch": {"mode": "light"}}),
            &client_with_class("ordo"),
        )
        .await
        .unwrap();
    store
        .set(
            &json!({"app_id": "imago", "patch": {"mode": "dark"}}),
            &client_with_class("imago"),
        )
        .await
        .unwrap();

    let ordo = store
        .get(&json!({"app_id": "ordo"}), &client_with_class("ordo"))
        .await
        .unwrap();
    let imago = store
        .get(&json!({"app_id": "imago"}), &client_with_class("imago"))
        .await
        .unwrap();
    assert_eq!(ordo["settings"]["mode"], "light");
    assert_eq!(imago["settings"]["mode"], "dark");

    let global = store.get(&json!({}), &anon_client()).await.unwrap();
    assert_eq!(
        global["settings"]["mode"],
        AppearanceSettings::default().mode
    );
}

#[tokio::test]
async fn scoped_writes_persist_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    store
        .set(
            &json!({"app_id": "ordo", "patch": {"mode": "light", "font_size": 14.0}}),
            &client_with_class("ordo"),
        )
        .await
        .unwrap();

    let reopened = AppearanceStore::new(dir.path().to_path_buf());
    let settings = reopened
        .get(&json!({"app_id": "ordo"}), &client_with_class("ordo"))
        .await
        .unwrap();
    assert_eq!(settings["settings"]["mode"], "light");
    assert_eq!(settings["settings"]["font_size"], 14.0);
    assert_eq!(settings["settings"]["revision"], 1);
}

#[tokio::test]
async fn follow_apps_locks_scoped_writes_but_preserves_stored_prefs() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    store
        .set(
            &json!({"app_id": "ordo", "patch": {"mode": "light"}}),
            &client_with_class("ordo"),
        )
        .await
        .unwrap();

    store
        .set(&json!({"follow_apps": true}), &manager_client())
        .await
        .unwrap();

    // Scoped writes are rejected while following.
    assert!(store
        .set(
            &json!({"app_id": "ordo", "patch": {"mode": "dark"}}),
            &client_with_class("ordo"),
        )
        .await
        .is_err());

    // Scoped reads return the shared global settings with the locked policy.
    let following = store
        .get(&json!({"app_id": "ordo"}), &client_with_class("ordo"))
        .await
        .unwrap();
    assert_eq!(
        following["settings"]["mode"],
        store.get(&json!({}), &anon_client()).await.unwrap()["settings"]["mode"]
    );
    assert_eq!(following["policy"]["following"], true);
    assert_eq!(following["policy"]["editable"], false);
    assert_eq!(following["policy"]["can_set_follow_apps"], false);

    // Turning follow_apps back off restores the stored app preference.
    store
        .set(&json!({"follow_apps": false}), &manager_client())
        .await
        .unwrap();
    let restored = store
        .get(&json!({"app_id": "ordo"}), &client_with_class("ordo"))
        .await
        .unwrap();
    assert_eq!(restored["settings"]["mode"], "light");
}

#[tokio::test]
async fn scoped_write_rejects_reserved_and_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    for patch in [
        json!({"revision": 5}),
        json!({"schema_version": 2}),
        json!({"follow_apps": true}),
        json!({"unknown": true}),
        json!({"mode": "wrong"}),
        json!({"font_size": 100.0}),
        json!({"accent_color": "red"}),
        json!({"material": "bogus"}),
    ] {
        assert!(
            store
                .set(
                    &json!({"app_id": "ordo", "patch": patch.clone()}),
                    &client_with_class("ordo"),
                )
                .await
                .is_err(),
            "{patch}"
        );
    }
}

#[tokio::test]
async fn scoped_write_rejects_unsafe_app_ids() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    for app_id in ["../escape", "a/b", "..", ".", "", "with/slash", "a\0b"] {
        let client = client_with_class(app_id);
        assert!(
            store
                .set(
                    &json!({"app_id": app_id, "patch": {"mode": "light"}}),
                    &client,
                )
                .await
                .is_err(),
            "{app_id}"
        );
        assert!(
            store
                .get(&json!({"app_id": app_id}), &client)
                .await
                .is_err(),
            "{app_id}"
        );
    }
    assert!(!dir.path().join(APPS_DIR).exists());
}

#[tokio::test]
async fn scoped_write_requires_matching_client_class_or_manager() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());

    assert!(store
        .set(
            &json!({"app_id": "ordo", "patch": {"mode": "light"}}),
            &client_with_class("imago"),
        )
        .await
        .is_err());
    assert!(store
        .set(
            &json!({"app_id": "ordo", "patch": {"mode": "light"}}),
            &anon_client(),
        )
        .await
        .is_err());

    assert!(store
        .set(
            &json!({"app_id": "ordo", "patch": {"mode": "light"}}),
            &client_with_class("ordo"),
        )
        .await
        .is_ok());
    assert!(store
        .set(
            &json!({"app_id": "ordo", "patch": {"mode": "dark"}}),
            &manager_client(),
        )
        .await
        .is_ok());
}

#[tokio::test]
async fn scoped_get_is_readable_by_any_client() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    store
        .set(
            &json!({"app_id": "ordo", "patch": {"mode": "light"}}),
            &client_with_class("ordo"),
        )
        .await
        .unwrap();

    let from_other = store
        .get(&json!({"app_id": "ordo"}), &client_with_class("imago"))
        .await
        .unwrap();
    assert_eq!(from_other["settings"]["mode"], "light");
}

#[tokio::test]
async fn scoped_writes_are_serialized_under_the_same_lock() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    let guard = store.write_lock.try_lock();
    assert!(guard.is_ok(), "lock must start unheld");
    drop(guard);

    let _held = store.write_lock.lock().await;
    let result = tokio::time::timeout(
        Duration::from_millis(50),
        store.set(
            &json!({"app_id": "ordo", "patch": {"mode": "light"}}),
            &client_with_class("ordo"),
        ),
    )
    .await;
    assert!(
        result.is_err(),
        "scoped set must wait for the shared write lock"
    );
}
