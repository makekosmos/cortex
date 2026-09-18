#![allow(dead_code)]

use super::support::IntegrationReplicationSetup;
use ark_core::integration_replication::encryption_key_id;
use kepler_backend::ark_host::ArkHost;
use kepler_backend::integrations::handle_operation;
use kepler_backend::package_service::PackageService;
use serde_json::json;
use std::path::Path;
use std::time::Duration;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub async fn wait_for_peer(host: &ArkHost, device_id: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    loop {
        let response = host
            .request("get_connected_peers", json!({}))
            .await
            .unwrap();
        assert!(response.ok, "Core peer status failed: {:?}", response.error);
        if response.data.as_array().is_some_and(|peers| {
            peers
                .iter()
                .any(|peer| peer.get("device_id") == Some(&json!(device_id)))
        }) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Core did not authenticate peer {device_id} within 10s"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub async fn verify_source_offline_refresh(
    setup: &IntegrationReplicationSetup,
    recipient: &ArkHost,
    data_dir: &Path,
    packages: &PackageService,
) {
    let offline_now = now_ms().saturating_add(61_000);
    let status_before_refresh = handle_operation(
        "replication_verification_status",
        json!({
            "integration_id": super::support::INTEGRATION_ID,
            "local_node_id": setup.recipient.node_id,
            "now_ms": offline_now,
        }),
        recipient,
        data_dir,
        packages,
    )
    .await
    .unwrap();
    assert_eq!(status_before_refresh["envelope_generation"], 1);
    assert_eq!(status_before_refresh["ready_for_collection"], true);
    assert_eq!(status_before_refresh["ready_for_refresh"], true);

    let lease = handle_operation(
        "replication_acquire_refresh_lease",
        json!({
            "integration_id": super::support::INTEGRATION_ID,
            "holder_node_id": setup.recipient.node_id,
            "credential_generation": 2,
            "now_ms": offline_now,
            "ttl_ms": 60_000,
            "expected_fencing_token": 1,
            "device_id": setup.recipient.node_id,
        }),
        recipient,
        data_dir,
        packages,
    )
    .await
    .unwrap();
    assert_eq!(lease["holder_node_id"], setup.recipient.node_id);
    assert_eq!(lease["credential_generation"], 2);
    assert_eq!(lease["fencing_token"], 2);

    assert!(
        handle_operation(
            "replication_acquire_refresh_lease",
            json!({
                "integration_id": super::support::INTEGRATION_ID,
                "holder_node_id": setup.recipient.node_id,
                "credential_generation": 2,
                "now_ms": offline_now,
                "ttl_ms": 60_000,
                "expected_fencing_token": 1,
                "device_id": setup.recipient.node_id,
            }),
            recipient,
            data_dir,
            packages,
        )
        .await
        .is_err(),
        "a stale refresh contender must not replace the current fence"
    );

    let refreshed_envelope = json!({
        "integration_id": super::support::INTEGRATION_ID,
        "recipient_node_id": setup.recipient.node_id,
        "grant_epoch": 2,
        "credential_generation": 2,
        "refresh_fencing_token": 2,
        "key_id": encryption_key_id(&setup.recipient.encryption_public_key),
        "algorithm": "opaque-test",
        "nonce": "recipient-nonce-2",
        "ciphertext": "opaque-recipient-ciphertext-2",
        "authenticated_metadata": {"content_type": "oauth"},
        "issuer_node_id": setup.recipient.node_id,
        "issued_at": "2026-09-06T00:00:00Z",
        "revision": 2,
        "hlc": "2026-09-06T00:00:00.000Z:000004:recipient-node"
    });
    assert_eq!(
        handle_operation(
            "replication_publish_credential_envelope",
            json!({
                "envelope": refreshed_envelope,
                "device_id": setup.recipient.node_id,
                "now_ms": offline_now,
            }),
            recipient,
            data_dir,
            packages,
        )
        .await
        .unwrap()["published"],
        true
    );
    let loaded = handle_operation(
        "replication_load_latest_credential_envelope",
        json!({
            "integration_id": super::support::INTEGRATION_ID,
            "recipient_node_id": setup.recipient.node_id,
        }),
        recipient,
        data_dir,
        packages,
    )
    .await
    .unwrap();
    assert_eq!(loaded["credential_generation"], 2);
    assert_eq!(loaded["ciphertext"], "opaque-recipient-ciphertext-2");
    assert!(loaded.get("plaintext").is_none());
    assert!(!serde_json::to_string(&loaded)
        .unwrap()
        .contains("oauth-token"));
}
