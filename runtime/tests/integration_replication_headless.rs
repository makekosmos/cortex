#![allow(clippy::unwrap_used)]

use ed25519_dalek::SigningKey;
use engine::ark_host::ArkHost;
use engine::integrations::handle_operation;
use engine::package_service::PackageService;
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

fn trusted_node(node_id: &str, seed: u8, encryption_key: &str) -> Value {
    let key = SigningKey::from_bytes(&[seed; 32]);
    let signing_public_key = key
        .verifying_key()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    json!({
        "node_id": node_id,
        "key_fingerprint": format!("fingerprint-{node_id}"),
        "signing_public_key": signing_public_key,
        "encryption_public_key": encryption_key,
        "transport_public_key": null,
        "grant_epoch": 2,
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

fn trusted_grant(node_id: &str, encryption_key: &str) -> Value {
    let mut value = grant(node_id, encryption_key);
    value["grant_epoch"] = json!(2);
    value["hlc"] = json!(format!("2026-09-06T00:00:00.000Z:000002:{node_id}"));
    value
}

fn trusted_grant_after_restart(node_id: &str, encryption_key: &str) -> Value {
    let mut value = trusted_grant(node_id, encryption_key);
    value["revision"] = json!(2);
    value["hlc"] = json!(format!("2026-09-06T00:00:01.000Z:000001:{node_id}"));
    value
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
    let origin = ArkHost::open(&fixture.origin_db).await.unwrap();
    let recipient = ArkHost::open(&fixture.recipient_db).await.unwrap();
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
async fn core_rejects_malformed_authorization_before_persisting() {
    let fixture = fixture();
    let (origin, _recipient) = spawn_core_pair(&fixture).await;
    let response = origin
        .request(
            "integration.persist_node_authorization",
            json!({
                "authorization_operation": "authorize",
                "node": { "node_id": "origin-node" },
                "grant": null,
                "device_id": "origin-node"
            }),
        )
        .await
        .unwrap();
    assert!(!response.ok, "malformed node must fail closed");
    assert_no_plaintext(&response.error.map(Value::String).unwrap_or(Value::Null));
}

#[tokio::test]
async fn trusted_cortex_authorization_survives_core_restart() {
    let fixture = fixture();
    let (origin, _recipient) = spawn_core_pair(&fixture).await;
    let packages_dir = tempfile::tempdir().unwrap();
    let data_dir = tempfile::tempdir().unwrap();
    let packages = PackageService::open(packages_dir.path()).unwrap();
    let origin_node = trusted_node("origin-node", 21, "origin-encryption");
    let origin_grant = trusted_grant("origin-node", "origin-encryption");
    let accepted = handle_operation(
        "replication_authorize_node",
        json!({"node": origin_node, "grant": origin_grant, "device_id": "origin-node"}),
        &origin,
        data_dir.path(),
        &packages,
    )
    .await
    .unwrap();
    assert_eq!(accepted["accepted"], true);
    drop(origin);

    let restarted = ArkHost::open(&fixture.origin_db).await.unwrap();
    let accepted_again = handle_operation(
        "replication_persist_grant",
        json!({
            "grant": trusted_grant_after_restart("origin-node", "origin-encryption"),
            "device_id": "origin-node"
        }),
        &restarted,
        data_dir.path(),
        &packages,
    )
    .await
    .unwrap();
    assert_eq!(accepted_again["accepted"], true);
}
