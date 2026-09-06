#![allow(clippy::unwrap_used)]

#[path = "support/integration_replication_setup.rs"]
mod support;

use ed25519_dalek::{Signer, SigningKey};
use kepler_backend::ark_host::{resolve_ark_core_rpc_path, ArkHost};
use kepler_backend::integrations::handle_operation;
use kepler_backend::package_service::PackageService;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::net::TcpListener;
use std::time::Duration;

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

async fn wait_for_peer(host: &ArkHost, device_id: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
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

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("canonical bytes must be lowercase hexadecimal"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sign_prepared(prepared: &Value, signing_key: &SigningKey) -> Value {
    let mut frame = prepared["envelope"].clone();
    let canonical = decode_hex(prepared["canonical_signing_bytes_hex"].as_str().unwrap());
    frame["signature"] = json!(hex(&signing_key.sign(&canonical).to_bytes()));
    frame
}

async fn wait_for_recipient_configuration(path: &str, expected_provider: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(conn) = Connection::open(path) {
            if let Ok(Some(config)) =
                ark_core::db::load_integration_configuration(&conn, support::INTEGRATION_ID)
            {
                if config.provider == expected_provider {
                    return;
                }
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "recipient did not persist the signed integration state"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn cortex_consumer_runs_signed_replication_over_two_core_nodes() {
    let setup: IntegrationReplicationSetup = support::new_setup().unwrap();
    let binary = resolve_ark_core_rpc_path().unwrap();
    let origin_db = setup.origin_db.to_string_lossy().into_owned();
    let recipient_db = setup.recipient_db.to_string_lossy().into_owned();
    let origin = ArkHost::spawn(&binary, &origin_db).await.unwrap();
    let recipient = ArkHost::spawn(&binary, &recipient_db).await.unwrap();
    let space_id = "integration-replication-headless";
    let auth_secret = "headless-two-node-auth-secret";
    start_iroh_sync(
        &origin,
        space_id,
        &setup.origin.node_id,
        free_loopback_port(),
        auth_secret,
        None,
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
    wait_for_peer(&recipient, &setup.origin.node_id).await;

    let packages_dir = tempfile::tempdir().unwrap();
    let data_dir = tempfile::tempdir().unwrap();
    let packages = PackageService::open(packages_dir.path()).unwrap();
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
    wait_for_recipient_configuration(&recipient_db, "synthetic").await;

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
}
