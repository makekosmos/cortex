#![cfg(all(windows, feature = "package-worker-fixture"))]
#![allow(clippy::unwrap_used)]

#[path = "support/integration_replication_hpke_finish.rs"]
mod finish;
#[path = "support/integration_replication_hpke_fixture.rs"]
mod fixture;

use crate::{offline, support};

use ed25519_dalek::Signer;
use engine::{
    ark_host::ArkHost, integrations::handle_operation, package_service::PackageService,
    package_worker_supervisor::PackageWorkerSupervisor,
};
use httpmock::MockServer;
use serde_json::json;
use std::path::Path;
use tempfile::tempdir;

const PACKAGE_ID: &str = "com.kosmos.test.hpke-replication";
const PACKAGE_VERSION: &str = "1.0.0";
const SETTING: &str = "session";
const SPACE_ID: &str = "integration-replication-hpke";

#[tokio::test]
async fn signed_hpke_replication_reaches_offline_provider_and_rejects_stale_inputs() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("provider-result.json");
    let _cleanup = fixture::cleanup(&marker).await;
    let provider = MockServer::start_async().await;
    let endpoint = provider.url("/collect");
    let origin = provider.url("/");
    let _mock = provider
        .mock_async(|when, then| {
            when.method("GET")
                .path("/collect")
                .header("authorization", "Bearer replicated-provider-secret");
            then.status(200)
                .json_body(json!({"provider":"offline-test","items":[{"id":"replicated-item"}]}));
        })
        .await;
    let versioned = fixture::manifest(&origin);
    let (archive, size, hash) = fixture::archive(dir.path(), &versioned);
    let (catalog, signatures, root, release) = fixture::signed_catalog(&origin, size, &hash);
    let setup = support::new_setup_named(PACKAGE_ID).unwrap();

    let (origin_host, recipient_host) = fixture::start_hosts(&setup).await;
    let origin_dir = dir.path().join("origin-runtime");
    let recipient_dir = dir.path().join("recipient-runtime");
    let mut origin_packages =
        PackageService::open_with_test_trust(&origin_dir, root.clone(), vec![release.clone()])
            .unwrap();
    origin_packages
        .apply_catalog(&catalog, signatures.clone())
        .unwrap();
    origin_packages
        .install_from_path(PACKAGE_ID, PACKAGE_VERSION, &archive)
        .unwrap();
    origin_packages.configure_workers(
        PackageWorkerSupervisor::new(1),
        vec![],
        "origin-correlation".into(),
    );
    origin_packages
        .set_integration_value(PACKAGE_ID, Some("endpoint"), None, &endpoint)
        .await
        .unwrap();
    origin_packages
        .set_integration_value(PACKAGE_ID, Some(SETTING), None, "origin-secret")
        .await
        .unwrap();
    let mut recipient_packages =
        PackageService::open_with_test_trust(&recipient_dir, root, vec![release]).unwrap();
    recipient_packages
        .apply_catalog(&catalog, signatures)
        .unwrap();
    recipient_packages
        .install_from_path(PACKAGE_ID, PACKAGE_VERSION, &archive)
        .unwrap();
    recipient_packages.configure_workers(
        PackageWorkerSupervisor::new(1),
        vec![],
        "recipient-correlation".into(),
    );
    recipient_packages
        .set_integration_value(PACKAGE_ID, Some("endpoint"), None, &endpoint)
        .await
        .unwrap();
    recipient_packages
        .set_integration_value(PACKAGE_ID, Some(SETTING), None, "bootstrap-placeholder")
        .await
        .unwrap();
    // Both isolated hosts share the test machine keyring namespace. Restore
    // the issuer value after recipient bootstrap so publication encrypts the
    // intended source credential; receive overwrites it with the decrypted
    // value before the provider run.
    origin_packages
        .set_integration_value(
            PACKAGE_ID,
            Some(SETTING),
            None,
            "replicated-provider-secret",
        )
        .await
        .unwrap();

    let issuer = engine::package_service::credential_envelope::load_or_create_identity(
        &setup.origin.node_id,
    )
    .unwrap();
    let debug_lookup = origin_host
        .request(
            "integration.lookup_issuer_encryption_key_for_publish",
            json!({
                "space_id": SPACE_ID,
                "integration_id": PACKAGE_ID,
                "recipient_node_id": setup.recipient.node_id,
                "issuer_node_id": setup.origin.node_id,
                "expected_issuer_key_id": issuer.key_id,
            }),
        )
        .await
        .unwrap();
    assert!(
        debug_lookup.ok,
        "prepublish lookup: {:?}",
        debug_lookup.error
    );
    let publish = handle_operation(
        "replication_publish_credential_envelope_v2",
        json!({
            "space_id": SPACE_ID,
            "integration_id": PACKAGE_ID,
            "package_version": "forged-version",
            "setting": "forged-setting",
            "issuer_node_id": setup.origin.node_id,
            "recipient_node_id": setup.recipient.node_id,
            "recipient_public_key": setup.recipient.encryption_public_key,
            "grant_epoch": 2,
            "credential_generation": 1,
            "refresh_fencing_token": 1,
            "issued_at": "2026-09-07T00:00:00Z",
            "revision": 1,
            "hlc": "2026-09-07T00:00:00.000Z:000001:origin-node",
            "device_id": setup.origin.node_id,
            "now_ms": support::now_ms(),
        }),
        &origin_host,
        origin_dir.as_path(),
        &origin_packages,
    )
    .await
    .unwrap();
    assert_eq!(publish["version"], 2);
    let prepared = handle_operation(
        "replication_prepare_signed_sync",
        json!({
            "space_id": SPACE_ID,
            "origin_node_id": setup.origin.node_id,
            "integration_id": PACKAGE_ID,
            "recipient_node_id": setup.recipient.node_id,
            "message_id": "hpke-positive",
        }),
        &origin_host,
        origin_dir.as_path(),
        &origin_packages,
    )
    .await
    .unwrap();
    let canonical = support::decode_hex(prepared["canonical_signing_bytes_hex"].as_str().unwrap());
    assert!(
        prepared["envelope"]["payload"]
            .as_array()
            .is_some_and(|payload| !payload.is_empty()),
        "signed frame did not contain a payload: {}",
        prepared["envelope"]
    );
    let mut frame = prepared["envelope"].clone();
    frame["signature"] = json!(support::hex(
        &setup.origin.signing_key.sign(&canonical).to_bytes()
    ));
    let accepted = handle_operation(
        "replication_validate_outbound_signed_sync",
        json!({
            "space_id": SPACE_ID,
            "origin_node_id": setup.origin.node_id,
            "frame": frame,
        }),
        &origin_host,
        origin_dir.as_path(),
        &origin_packages,
    )
    .await
    .unwrap();
    assert_eq!(accepted["accepted"], true);
    handle_operation(
        "replication_send_signed_sync",
        json!({"frame": frame}),
        &origin_host,
        origin_dir.as_path(),
        &origin_packages,
    )
    .await
    .unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let response = recipient_host
            .request(
                "integration.load_latest_credential_envelope",
                json!({
                    "integration_id": PACKAGE_ID,
                    "recipient_node_id": setup.recipient.node_id,
                }),
            )
            .await
            .unwrap();
        if response.ok
            && response
                .data
                .get("credential_generation")
                .and_then(serde_json::Value::as_u64)
                == Some(1)
        {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            let conn = rusqlite::Connection::open(&setup.recipient_db).unwrap();
            let persisted = ark_core::db::load_integration_credential_envelope(
                &conn,
                PACKAGE_ID,
                &setup.recipient.node_id,
                1,
            )
            .unwrap();
            assert!(
                response.ok && persisted.is_some(),
                "recipient did not persist the signed HPKE envelope: \
                    response={:?} db={:?} panics={}",
                response.error,
                persisted,
                recipient_host.panic_count()
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    drop(origin_host);

    finish::receive_and_validate(
        &marker,
        &setup,
        &issuer,
        &recipient_host,
        recipient_dir.as_path(),
        &mut recipient_packages,
    )
    .await;
    // The origin's provider worker was launched by the replicated credential;
    // disable the package so the supervisor stops it before the fixture's
    // tempdir is removed (KOS-270).
    origin_packages
        .set_enabled(PACKAGE_ID, PACKAGE_VERSION, false)
        .await
        .unwrap();
}
