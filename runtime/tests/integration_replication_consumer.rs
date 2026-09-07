#![allow(clippy::unwrap_used)]

#[path = "support/integration_replication_offline.rs"]
mod offline;
#[path = "support/integration_replication_setup.rs"]
mod support;

use ed25519_dalek::{Signer, SigningKey};
use kepler_backend::ark_host::{resolve_ark_core_rpc_path, ArkHost};
use kepler_backend::integrations::handle_operation;
use kepler_backend::package_service::PackageService;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::net::TcpListener;

use support::IntegrationReplicationSetup;

fn free_loopback_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.local_addr().unwrap().port()
}

async fn start_iroh_sync(
    host: &ArkHost,
    space_id: &str,
    device_id: &str,
    port: u16,
    auth_secret: &str,
    peer_ticket: Option<&str>,
) {
    let response = host
        .request(
            "start_sync",
            json!({
                "space_id": space_id,
                "device_id": device_id,
                "device_name": device_id,
                "port": port,
                "auth_secret": auth_secret,
                "use_iroh": true,
                "iroh_peer_ticket": peer_ticket,
                "discovery_enabled": false,
            }),
        )
        .await
        .unwrap();
    assert!(
        response.ok,
        "Core start_sync failed for {device_id}: {:?}",
        response.error
    );
}

async fn own_iroh_ticket(host: &ArkHost) -> String {
    let response = host
        .request("get_own_iroh_ticket", json!({}))
        .await
        .unwrap();
    assert!(
        response.ok,
        "Core ticket request failed: {:?}",
        response.error
    );
    let ticket = response.data.as_str().unwrap_or_default().to_owned();
    assert!(
        !ticket.is_empty(),
        "Core must expose a non-empty Iroh ticket"
    );
    ticket
}

fn sign_prepared(prepared: &Value, signing_key: &SigningKey) -> Value {
    let mut frame = prepared["envelope"].clone();
    let canonical = support::decode_hex(prepared["canonical_signing_bytes_hex"].as_str().unwrap());
    frame["signature"] = json!(support::hex(&signing_key.sign(&canonical).to_bytes()));
    frame
}

#[tokio::test]
async fn cortex_consumer_runs_signed_replication_over_two_core_nodes() {
    let setup: IntegrationReplicationSetup =
        support::new_setup_named(support::INTEGRATION_ID).unwrap();
    let binary = resolve_ark_core_rpc_path().unwrap();
    let origin_db = setup.origin_db.to_string_lossy().into_owned();
    let recipient_db = setup.recipient_db.to_string_lossy().into_owned();
    let origin = ArkHost::spawn(&binary, &origin_db).await.unwrap();
    let recipient = ArkHost::spawn(&binary, &recipient_db).await.unwrap();
    let space_id = "integration-replication-headless";
    let auth_secret = "headless-two-node-auth-secret";
    let bootstrap_ticket = std::env::var("KOSMOS_BOOTSTRAP_TICKET").ok();
    start_iroh_sync(
        &origin,
        space_id,
        &setup.origin.node_id,
        free_loopback_port(),
        auth_secret,
        bootstrap_ticket.as_deref(),
    )
    .await;
    let origin_ticket = own_iroh_ticket(&origin).await;
    start_iroh_sync(
        &recipient,
        space_id,
        &setup.recipient.node_id,
        free_loopback_port(),
        auth_secret,
        Some(&origin_ticket),
    )
    .await;
    let recipient_ticket = own_iroh_ticket(&recipient).await;
    support::refresh_transport_public_keys_named(
        &setup,
        &origin_ticket,
        &recipient_ticket,
        support::INTEGRATION_ID,
    )
    .unwrap();
    start_iroh_sync(
        &origin,
        space_id,
        &setup.origin.node_id,
        free_loopback_port(),
        auth_secret,
        Some(&recipient_ticket),
    )
    .await;
    start_iroh_sync(
        &recipient,
        space_id,
        &setup.recipient.node_id,
        free_loopback_port(),
        auth_secret,
        Some(&origin_ticket),
    )
    .await;
    offline::wait_for_peer(&recipient, &setup.origin.node_id).await;

    let packages_dir = tempfile::tempdir().unwrap();
    let data_dir = tempfile::tempdir().unwrap();
    let packages = PackageService::open(packages_dir.path()).unwrap();
    let origin_config = ark_core::db::load_integration_configuration(
        &Connection::open(&origin_db).unwrap(),
        support::INTEGRATION_ID,
    )
    .unwrap()
    .unwrap();
    assert_eq!(origin_config.public_settings, json!({}));
    let published_at = support::now_ms();
    let origin_envelope = json!({
        "integration_id": support::INTEGRATION_ID,
        "recipient_node_id": setup.recipient.node_id,
        "grant_epoch": 2,
        "credential_generation": 1,
        "refresh_fencing_token": 1,
        "key_id": ark_core::integration_replication::encryption_key_id(
            &setup.recipient.encryption_public_key
        ),
        "algorithm": "opaque-test",
        "nonce": "origin-nonce-1",
        "ciphertext": "opaque-origin-ciphertext-1",
        "authenticated_metadata": {"content_type": "oauth"},
        "issuer_node_id": setup.origin.node_id,
        "issued_at": "2026-09-06T00:00:00Z",
        "revision": 1,
        "hlc": "2026-09-06T00:00:00.000Z:000003:origin-node"
    });
    assert_eq!(
        handle_operation(
            "replication_publish_credential_envelope",
            json!({
                "envelope": origin_envelope,
                "device_id": setup.origin.node_id,
                "now_ms": published_at,
            }),
            &origin,
            data_dir.path(),
            &packages,
        )
        .await
        .unwrap()["published"],
        true
    );
    let prepared = handle_operation(
        "replication_prepare_signed_sync",
        json!({
            "space_id": space_id,
            "origin_node_id": setup.origin.node_id,
            "integration_id": support::INTEGRATION_ID,
            "recipient_node_id": setup.recipient.node_id,
            "message_id": "cortex-positive-message",
        }),
        &origin,
        data_dir.path(),
        &packages,
    )
    .await
    .unwrap();
    let frame = sign_prepared(&prepared, &setup.origin.signing_key);
    let accepted = handle_operation(
        "replication_validate_outbound_signed_sync",
        json!({"space_id": space_id, "origin_node_id": setup.origin.node_id, "frame": frame}),
        &origin,
        data_dir.path(),
        &packages,
    )
    .await
    .unwrap();
    assert_eq!(accepted["accepted"], true);
    let sent = handle_operation(
        "replication_send_signed_sync",
        json!({"frame": frame}),
        &origin,
        data_dir.path(),
        &packages,
    )
    .await
    .unwrap();
    assert_eq!(sent["sent"], true);
    let recipient_state =
        support::wait_for_recipient_state_named(&recipient_db, support::INTEGRATION_ID).await;
    assert!(
        recipient_state.is_ok(),
        "recipient state failed: {recipient_state:?}"
    );

    let prepared_tamper = handle_operation(
        "replication_prepare_signed_sync",
        json!({
            "space_id": space_id,
            "origin_node_id": setup.origin.node_id,
            "integration_id": support::INTEGRATION_ID,
            "recipient_node_id": setup.recipient.node_id,
            "message_id": "cortex-tamper-message",
        }),
        &origin,
        data_dir.path(),
        &packages,
    )
    .await
    .unwrap();
    let mut tampered = sign_prepared(&prepared_tamper, &setup.origin.signing_key);
    tampered["signature"] = json!(format!(
        "00{}",
        &tampered["signature"].as_str().unwrap()[2..]
    ));
    assert!(handle_operation(
        "replication_validate_outbound_signed_sync",
        json!({"space_id": space_id, "origin_node_id": setup.origin.node_id, "frame": tampered}),
        &origin,
        data_dir.path(),
        &packages,
    )
    .await
    .is_err());

    let prepared_foreign = handle_operation(
        "replication_prepare_signed_sync",
        json!({
            "space_id": space_id,
            "origin_node_id": setup.origin.node_id,
            "integration_id": support::INTEGRATION_ID,
            "recipient_node_id": setup.foreign.node_id,
            "message_id": "cortex-foreign-message",
        }),
        &origin,
        data_dir.path(),
        &packages,
    )
    .await
    .unwrap();
    let foreign = sign_prepared(&prepared_foreign, &setup.origin.signing_key);
    assert_eq!(
        handle_operation(
            "replication_validate_outbound_signed_sync",
            json!({"space_id": space_id, "origin_node_id": setup.origin.node_id, "frame": foreign}),
            &origin,
            data_dir.path(),
            &packages,
        )
        .await
        .unwrap()["accepted"],
        true
    );
    assert!(
        handle_operation(
            "replication_send_signed_sync",
            json!({"frame": foreign}),
            &origin,
            data_dir.path(),
            &packages,
        )
        .await
        .is_err(),
        "valid frame for an authorized but disconnected foreign recipient must fail before wire"
    );

    drop(origin);
    offline::verify_source_offline_refresh(&setup, &recipient, data_dir.path(), &packages).await;
}
