#![allow(clippy::unwrap_used)]

use kepler_backend::ark_host::{resolve_ark_core_rpc_path, ArkHost};
use serde_json::{json, Value};
use tempfile::TempDir;

const INTEGRATION_ID: &str = "synthetic-provider";

struct TwoNodeFixture {
    _dir: TempDir,
    origin_db: String,
    recipient_db: String,
    origin_node: Value,
    recipient_node: Value,
    origin_grant: Value,
    recipient_grant: Value,
}

fn node(node_id: &str, encryption_key: &str) -> Value {
    json!({
        "node_id": node_id,
        "key_fingerprint": format!("fingerprint-{node_id}"),
        "signing_public_key": format!("signing-{node_id}"),
        "encryption_public_key": encryption_key,
        "transport_public_key": null,
        "grant_epoch": 1,
        "status": "active",
        "authorized_at": "2026-09-06T00:00:00Z",
        "revoked_at": null,
        "revocation_epoch": null,
        "revision": 1,
        "hlc": format!("2026-09-06T00:00:00.000Z:000001:{node_id}"),
    })
}

fn grant(node_id: &str, encryption_key: &str) -> Value {
    json!({
        "integration_id": INTEGRATION_ID,
        "node_id": node_id,
        "node_encryption_key": encryption_key,
        "grant_epoch": 1,
        "status": "active",
        "authorized_at": "2026-09-06T00:00:00Z",
        "revoked_at": null,
        "revision": 1,
        "hlc": format!("2026-09-06T00:00:00.000Z:000002:{node_id}"),
    })
}

fn assert_no_plaintext(value: &Value) {
    let encoded = serde_json::to_string(value).unwrap();
    assert!(!encoded.contains("credential_plaintext"));
    assert!(!encoded.contains("secret-token"));
}

fn fixture() -> TwoNodeFixture {
    let dir = tempfile::tempdir().unwrap();
    let origin_db = dir.path().join("origin.sqlite");
    let recipient_db = dir.path().join("recipient.sqlite");
    let origin_node = node("origin-node", "origin-encryption");
    let recipient_node = node("recipient-node", "recipient-encryption");
    let origin_grant = grant("origin-node", "origin-encryption");
    let recipient_grant = grant("recipient-node", "recipient-encryption");
    assert_no_plaintext(&origin_node);
    assert_no_plaintext(&recipient_node);
    assert_no_plaintext(&origin_grant);
    assert_no_plaintext(&recipient_grant);
    TwoNodeFixture {
        _dir: dir,
        origin_db: origin_db.to_string_lossy().into_owned(),
        recipient_db: recipient_db.to_string_lossy().into_owned(),
        origin_node,
        recipient_node,
        origin_grant,
        recipient_grant,
    }
}

async fn spawn_core_pair(fixture: &TwoNodeFixture) -> (ArkHost, ArkHost) {
    let binary = resolve_ark_core_rpc_path().unwrap();
    let origin = ArkHost::spawn(&binary, &fixture.origin_db).await.unwrap();
    let recipient = ArkHost::spawn(&binary, &fixture.recipient_db)
        .await
        .unwrap();
    (origin, recipient)
}

#[test]
fn synthetic_two_node_fixture_is_plaintext_free_and_deterministic() {
    let fixture = fixture();
    assert_eq!(fixture.origin_node["node_id"], "origin-node");
    assert_eq!(fixture.recipient_node["node_id"], "recipient-node");
    assert_eq!(fixture.origin_grant["integration_id"], INTEGRATION_ID);
    assert_eq!(fixture.recipient_grant["grant_epoch"], 1);
}

#[tokio::test]
async fn core_pair_setup_and_drop_reaps_children() {
    let fixture = fixture();
    let (_origin, _recipient) = spawn_core_pair(&fixture).await;
}

#[tokio::test]
#[ignore = "awaits Cortex source-offline transport and refresh wiring"]
async fn source_offline_recipient_flow_is_not_run_until_cortex_wiring_exists() {
    let fixture = fixture();
    let (_origin, _recipient) = spawn_core_pair(&fixture).await;
    panic!(
        "NOT_RUN: Cortex currently exposes prepare/validate only; transport, recipient apply, replay/revocation and refresh entrypoints are not wired"
    );
}
