use super::*;
use crate::db::{CommandContext, EnrollPairedDeviceCommand};
use ark_domain::device_registry::{DeviceId, DisplayName};

#[test]
fn device_registry_operations_have_a_stable_wire_shape() {
    let snapshot = serde_json::from_value::<Request>(json!({
        "operation": "device_registry.snapshot"
    }))
    .expect("snapshot operation should deserialize");
    assert!(matches!(snapshot, Request::DeviceRegistrySnapshot));

    let rename = serde_json::from_value::<Request>(json!({
        "operation": "device_registry.rename",
        "target_device_id": "device-a",
        "display_name": "Office Mac"
    }))
    .expect("rename operation should deserialize");
    assert!(matches!(rename, Request::DeviceRegistryRename { .. }));

    let disable = serde_json::from_value::<Request>(json!({
        "operation": "device_registry.disable",
        "target_device_id": "device-a"
    }))
    .expect("disable operation should deserialize");
    assert!(matches!(disable, Request::DeviceRegistryDisable { .. }));
}

#[tokio::test]
async fn engine_registry_operations_read_and_commit_signed_device_state() {
    let fixture = service_fixture();
    initialize_test_database(&fixture.state, fixture.db_path().to_string_lossy().into_owned(),)
    .await
    .expect("initialize service database");
    let db = get_db_handle(&fixture.state).expect("service database handle");
    db.set_local_sync_key_value(
        CommandContext::engine(),
        crate::db::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY.into(),
        "local-device".into(),
    )
    .await
    .expect("persist configured device identity");
    db.enroll_paired_device(
        CommandContext::engine(),
        EnrollPairedDeviceCommand::new(
            "local-device".into(),
            "remote-device".into(),
            ark_net::iroh::SecretKey::generate().public().to_string(),
        ),
    )
    .await
    .expect("seed an enrolled device");

    let initial = handle_request(&fixture.state, Request::DeviceRegistrySnapshot)
        .await
        .expect("load device registry through Engine API");
    assert_eq!(initial["localDeviceId"], "local-device");
    assert_eq!(initial["devices"].as_array().unwrap().len(), 2);

    let rename = handle_request(
        &fixture.state,
        Request::DeviceRegistryRename {
            target_device_id: DeviceId::parse("remote-device").unwrap(),
            display_name: DisplayName::parse("Office laptop").unwrap(),
        },
    )
    .await
    .expect("commit signed rename through Engine API");
    assert!(!rename.is_null());
    let renamed = handle_request(&fixture.state, Request::DeviceRegistrySnapshot)
        .await
        .expect("read renamed device");
    let remote = renamed["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|device| device["deviceId"] == "remote-device")
        .unwrap();
    assert_eq!(remote["displayName"], "Office laptop");
    assert_eq!(remote["enabled"], true);

    let disabled = handle_request(
        &fixture.state,
        Request::DeviceRegistryDisable {
            target_device_id: DeviceId::parse("remote-device").unwrap(),
        },
    )
    .await
    .expect("commit signed disable through Engine API");
    assert!(!disabled.is_null());
    let snapshot = handle_request(&fixture.state, Request::DeviceRegistrySnapshot)
        .await
        .expect("read disabled device");
    let remote = snapshot["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|device| device["deviceId"] == "remote-device")
        .unwrap();
    assert_eq!(remote["enabled"], false);
}
