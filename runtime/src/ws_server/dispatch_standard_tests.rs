use super::{inject_usage_windows_dir, replication_requires_authority, system_windows_dir};

#[test]
fn usage_analytics_params_carry_real_windows_dir() {
    let injected = inject_usage_windows_dir(
        "get_usage_analytics",
        serde_json::json!({ "range_days": 30 }),
    );
    assert_eq!(
        injected["windows_dir"].as_str(),
        system_windows_dir(),
        "the op must receive the OS-resolved dir, never a hard-coded one"
    );
    #[cfg(target_os = "windows")]
    assert!(injected["windows_dir"].as_str().is_some());
}

#[test]
fn other_ops_and_non_object_params_are_untouched() {
    let other = inject_usage_windows_dir("app_index.list_all", serde_json::json!({}));
    assert!(other.get("windows_dir").is_none());
    let bare = inject_usage_windows_dir("get_usage_analytics", serde_json::json!(null));
    assert_eq!(bare, serde_json::Value::Null);
}

#[test]
fn every_replication_operation_requires_desktop_authority() {
    for operation in [
        "replication_acquire_refresh_lease",
        "replication_send_signed_sync",
        "replication_publish_credential_envelope_v2",
    ] {
        assert!(replication_requires_authority(operation));
    }
    assert!(!replication_requires_authority("list"));
}
