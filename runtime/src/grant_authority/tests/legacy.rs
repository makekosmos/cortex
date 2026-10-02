use super::super::*;
use super::owner;

#[test]
fn legacy_revoke_preserves_unknown_fields_and_unrelated_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grant-authority.json");
    let records = serde_json::json!([
        {
            "version": 1,
            "persistent_grant_id": "legacy",
            "extension_id": "eden",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 1, "secondary": 2},
            "exact_file_identity": null,
            "revoked": false,
            "future_scope": {"read": ["a"], "write": null},
            "future_marker": "preserve-me"
        },
        {
            "version": 1,
            "persistent_grant_id": "unrelated",
            "extension_id": "other",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 1, "secondary": 2},
            "exact_file_identity": null,
            "revoked": false,
            "future_marker": "untouched"
        }
    ]);
    fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

    let revoked = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
        .revoke_legacy_records(&["eden"])
        .unwrap();
    assert_eq!(revoked, 1);

    let persisted: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(persisted[0]["revoked"], true);
    assert_eq!(persisted[0]["future_scope"]["read"][0], "a");
    assert_eq!(persisted[0]["future_marker"], "preserve-me");
    assert_eq!(persisted[1]["revoked"], false);
    assert_eq!(persisted[1]["future_marker"], "untouched");
}

#[test]
fn malformed_records_abort_legacy_revoke_before_write_or_handle_clear() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grant-authority.json");
    let before = concat!(
        r#"[{"version":1,"persistent_grant_id":"legacy","extension_id":"eden","#,
        r#""provenance":"native-dialog","exact_file":false,"selected_path":"/tmp","#,
        r#""root_identity":{"dev":1,"ino":2},"exact_file_identity":null,"revoked":false},"#,
        r#"{"version":99}]"#,
    )
    .as_bytes();
    fs::write(&path, before).unwrap();

    let registry = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
    assert_eq!(
        registry.revoke_legacy_records(&["eden"]),
        Err(GrantError::Persistence)
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(registry.len(), 0);
}

#[test]
fn legacy_revoke_clears_only_live_handles_for_allowlisted_owners() {
    let dir = tempfile::tempdir().unwrap();
    let legacy_root = dir.path().join("legacy");
    let unrelated_root = dir.path().join("unrelated");
    fs::create_dir(&legacy_root).unwrap();
    fs::create_dir(&unrelated_root).unwrap();
    let registry = GrantAuthorityRegistry::with_data_dir(dir.path().join("engine"));
    registry
        .register(
            &owner(1),
            "eden",
            &legacy_root,
            false,
            GrantProvenance::NativeDialog,
            None,
        )
        .unwrap();
    registry
        .register(
            &owner(2),
            "other",
            &unrelated_root,
            false,
            GrantProvenance::NativeDialog,
            None,
        )
        .unwrap();

    assert_eq!(registry.revoke_legacy_records(&["eden"]).unwrap(), 1);
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.close_owner(owner(2)), 1);
    assert_eq!(registry.len(), 0);
}

#[test]
fn scoped_legacy_revoke_leaves_other_legacy_identity_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grant-authority.json");
    let records = serde_json::json!([
        {
            "version": 1,
            "persistent_grant_id": "eden-grant",
            "extension_id": "eden",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 1, "secondary": 2},
            "exact_file_identity": null,
            "revoked": false,
            "unknown": "keep"
        },
        {
            "version": 1,
            "persistent_grant_id": "delphi-grant",
            "extension_id": "delphi",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 1, "secondary": 2},
            "exact_file_identity": null,
            "revoked": false,
            "unknown": "untouched"
        }
    ]);
    fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

    assert_eq!(
        GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .revoke_legacy_records(&["eden"])
            .unwrap(),
        1
    );
    let persisted: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(persisted[0]["persistent_grant_id"], "eden-grant");
    assert_eq!(persisted[0]["revoked"], true);
    assert_eq!(persisted[0]["unknown"], "keep");
    assert_eq!(persisted[1]["persistent_grant_id"], "delphi-grant");
    assert_eq!(persisted[1]["revoked"], false);
    assert_eq!(persisted[1]["unknown"], "untouched");
}

#[test]
fn invalid_scope_is_rejected_without_touching_persisted_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grant-authority.json");
    let before = concat!(
        r#"[{"version":1,"persistent_grant_id":"legacy","extension_id":"eden","#,
        r#""provenance":"native-dialog","exact_file":false,"selected_path":"/tmp","#,
        r#""root_identity":{"dev":1,"ino":2},"exact_file_identity":null,"revoked":false}]"#,
    )
    .as_bytes();
    fs::write(&path, before).unwrap();

    let registry = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
    assert_eq!(
        registry.revoke_legacy_records(&["eden", "eden"]),
        Err(GrantError::Invalid)
    );
    assert_eq!(
        registry.revoke_legacy_records(&["not-a-legacy-id"]),
        Err(GrantError::Invalid)
    );
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn legacy_grant_transaction_restores_only_changed_records_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grant-authority.json");
    let records = serde_json::json!([
        {
            "version": 1,
            "persistent_grant_id": "eden-live",
            "extension_id": "eden",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 1, "secondary": 2},
            "exact_file_identity": null,
            "revoked": false,
            "future_scope": {"read": ["vault"]}
        },
        {
            "version": 1,
            "persistent_grant_id": "eden-already-revoked",
            "extension_id": "eden",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 3, "secondary": 4},
            "exact_file_identity": null,
            "revoked": true,
            "future_marker": "do-not-resurrect"
        },
        {
            "version": 1,
            "persistent_grant_id": "delphi-untouched",
            "extension_id": "delphi",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 5, "secondary": 6},
            "exact_file_identity": null,
            "revoked": false,
            "future_marker": "unrelated"
        }
    ]);
    fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

    let result = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
        .revoke_legacy_records_transaction(&["eden"])
        .unwrap();
    assert_eq!(result.revoked, 1);
    let token = result.transaction_token.unwrap();
    let revoked: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(revoked[0]["revoked"], true);
    assert_eq!(revoked[0]["future_scope"]["read"][0], "vault");
    assert_eq!(revoked[1]["revoked"], true);
    assert_eq!(revoked[2]["revoked"], false);

    let restarted = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
    assert_eq!(
        restarted
            .restore_migration_snapshot(&["eden"], Some(&token))
            .unwrap()
            .restored,
        1
    );
    let restored: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(restored[0]["revoked"], false);
    assert_eq!(restored[0]["future_scope"]["read"][0], "vault");
    assert_eq!(restored[1]["revoked"], true);
    assert_eq!(restored[1]["future_marker"], "do-not-resurrect");
    assert_eq!(restored[2]["revoked"], false);
    assert_eq!(
        restarted
            .restore_migration_snapshot(&["eden"], Some(&token))
            .unwrap()
            .restored,
        0
    );
}

#[test]
fn legacy_grant_restore_failure_is_retryable_after_restart_and_commit_is_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grant-authority.json");
    let records = serde_json::json!([{
        "version": 1,
        "persistent_grant_id": "eden-grant",
        "extension_id": "eden",
        "provenance": "NativeDialog",
        "exact_file": false,
        "selected_path": dir.path().to_string_lossy(),
        "root_identity": {"primary": 1, "secondary": 2},
        "exact_file_identity": null,
        "revoked": false,
        "opaque": {"keep": true}
    }]);
    fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

    let result = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
        .revoke_legacy_records_transaction(&["eden"])
        .unwrap();
    let token = result.transaction_token.unwrap();

    FAIL_PERSIST_AFTER_FSYNC.with(|fail| fail.set(true));
    assert_eq!(
        GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .restore_migration_snapshot(&["eden"], Some(&token))
            .map(|result| result.restored),
        Err(GrantError::Persistence)
    );
    FAIL_PERSIST_AFTER_FSYNC.with(|fail| fail.set(false));

    let restarted = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
    assert_eq!(
        restarted
            .restore_migration_snapshot(&["eden"], Some(&token))
            .unwrap()
            .restored,
        1
    );
    assert_eq!(
        restarted.commit_legacy_records(&token),
        Err(GrantError::Invalid)
    );

    fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    let committed = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
        .revoke_legacy_records_transaction(&["eden"])
        .unwrap();
    let committed_token = committed.transaction_token.unwrap();
    GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
        .commit_legacy_records(&committed_token)
        .unwrap();
    assert_eq!(
        GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .restore_migration_snapshot(&["eden"], Some(&committed_token))
            .map(|result| result.restored),
        Err(GrantError::Invalid)
    );
    let final_records: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(final_records[0]["revoked"], true);
    assert_eq!(final_records[0]["opaque"]["keep"], true);
}

#[test]
fn migration_snapshot_resolves_active_transaction_without_persisted_token() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grant-authority.json");
    let records = serde_json::json!([{
        "version": 1,
        "persistent_grant_id": "eden-grant",
        "extension_id": "eden",
        "provenance": "NativeDialog",
        "exact_file": false,
        "selected_path": dir.path().to_string_lossy(),
        "root_identity": {"primary": 1, "secondary": 2},
        "exact_file_identity": null,
        "revoked": false
    }]);
    fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

    let revoked = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
        .revoke_legacy_records_transaction(&["eden"])
        .unwrap();
    assert_eq!(revoked.revoked, 1);
    let restarted = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
    let restored = restarted
        .restore_migration_snapshot(&["eden"], None)
        .unwrap();
    assert_eq!(
        restored,
        MigrationGrantRestoration {
            snapshot_found: true,
            restored: 1
        }
    );
}

#[test]
fn stale_snapshot_token_cannot_restore_by_source_set() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grant-authority.json");
    let records = serde_json::json!([{
        "version": 1,
        "persistent_grant_id": "eden-grant",
        "extension_id": "eden",
        "provenance": "NativeDialog",
        "exact_file": false,
        "selected_path": dir.path().to_string_lossy(),
        "root_identity": {"primary": 1, "secondary": 2},
        "exact_file_identity": null,
        "revoked": false
    }]);
    fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
        .revoke_legacy_records_transaction(&["eden"])
        .unwrap();

    let restarted = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
    assert_eq!(
        restarted.restore_migration_snapshot(&["eden"], Some("stale-token")),
        Err(GrantError::NotFound)
    );
    let persisted: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(persisted[0]["revoked"], true);
}
