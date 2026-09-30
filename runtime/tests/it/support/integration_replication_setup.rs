//! Deterministic, plaintext-free Core fixture for the integration replication tests.
//!
//! The caller owns the Core child lifecycle. This module only creates isolated
//! databases and seeds them through `ark-core`'s public database API before
//! `ArkHost::spawn` is called.
#![allow(dead_code, unused_imports)]

use ark_core::db::{
    init_schema, try_acquire_integration_refresh_lease, upsert_authorized_node,
    upsert_integration_configuration, upsert_integration_node_grant,
};
use ark_core::integration_replication::{
    AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationNodeGrant, NodeStatus,
};
use ed25519_dalek::{Signer, SigningKey};
use engine::package_service::credential_envelope::load_or_create_identity;
use rusqlite::Connection;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

/// Host HPKE identities live in the OS keyring under fixed node ids
/// ("origin-node", "recipient-node", "foreign-node"), and the HPKE test deletes
/// them when it finishes. Every test that creates or reads them holds this lock,
/// which serializes them under a plain `cargo test` (one process). The gate runs
/// each test in its own process, where the nextest `os-keyring` group in
/// .config/nextest.toml serializes them instead. A tokio mutex, because the
/// holders are async tests, and it does not poison, so one failure does not
/// cascade.
pub static HOST_IDENTITIES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub const INTEGRATION_ID: &str = "synthetic-provider";
const HLC_WALL: &str = "2026-09-06T00:00:00.000Z";

#[derive(Debug, Clone)]
pub struct NodeIdentity {
    pub node_id: String,
    pub signing_key: SigningKey,
    pub signing_public_key: String,
    pub transport_public_key: String,
    pub encryption_public_key: String,
}

#[derive(Debug)]
pub struct IntegrationReplicationSetup {
    pub _dir: TempDir,
    pub origin_db: PathBuf,
    pub recipient_db: PathBuf,
    pub origin: NodeIdentity,
    pub recipient: NodeIdentity,
    pub foreign: NodeIdentity,
}

#[path = "integration_replication_transport.rs"]
mod transport;
pub use transport::refresh_transport_public_keys_named;

pub fn new_setup_named(integration_id: &str) -> Result<IntegrationReplicationSetup, String> {
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let origin = identity("origin-node", 21);
    let recipient = identity("recipient-node", 22);
    let foreign = identity("foreign-node", 23);
    let origin_db = dir.path().join("origin.sqlite");
    let recipient_db = dir.path().join("recipient.sqlite");
    seed_database_named(
        &origin_db,
        &origin,
        &[&origin, &recipient, &foreign],
        true,
        integration_id,
    )?;
    seed_database_named(
        &recipient_db,
        &origin,
        &[&origin, &recipient],
        false,
        integration_id,
    )?;
    seed_recipient_baseline_named(&recipient_db, &origin, integration_id)?;
    let conn = Connection::open(&origin_db).map_err(|error| error.to_string())?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis() as u64;
    try_acquire_integration_refresh_lease(
        &conn,
        &ark_core::db::RefreshLeaseAcquireParams {
            integration_id: integration_id.into(),
            holder_node_id: origin.node_id.clone(),
            credential_generation: 1,
            now_ms,
            ttl_ms: 60_000,
            expected_fencing_token: 0,
            device_id: origin.node_id.clone(),
        },
    )?;
    Ok(IntegrationReplicationSetup {
        _dir: dir,
        origin_db,
        recipient_db,
        origin,
        recipient,
        foreign,
    })
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |byte: u8| {
                (byte as char)
                    .to_digit(16)
                    .expect("canonical bytes must be lowercase hexadecimal") as u8
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

pub fn seed_database_named(
    path: &Path,
    origin: &NodeIdentity,
    nodes: &[&NodeIdentity],
    seed_configuration: bool,
    integration_id: &str,
) -> Result<(), String> {
    let conn = Connection::open(path).map_err(|error| error.to_string())?;
    init_schema(&conn)?;
    if seed_configuration {
        let config = IntegrationConfiguration {
            integration_id: integration_id.into(),
            provider: "synthetic".into(),
            account_subject: "test-account".into(),
            public_scopes: Vec::new(),
            public_settings: json!({}),
            enabled: true,
            sync_cursor: None,
            revision: 1,
            hlc: format!("{HLC_WALL}:000001:origin-node"),
        };
        upsert_integration_configuration(&conn, &config, &origin.node_id)?;
    }
    for node in nodes {
        let authorized = authorized_node(node);
        let grant = IntegrationNodeGrant {
            integration_id: integration_id.into(),
            node_id: node.node_id.clone(),
            node_encryption_key: node.encryption_public_key.clone(),
            grant_epoch: 1,
            status: GrantStatus::Active,
            authorized_at: "2026-09-06T00:00:00Z".into(),
            revoked_at: None,
            revision: 1,
            hlc: format!("{HLC_WALL}:000002:{}", node.node_id),
        };
        upsert_authorized_node(&conn, &authorized, &origin.node_id)?;
        upsert_integration_node_grant(&conn, &grant, &origin.node_id)?;
    }
    Ok(())
}

pub async fn wait_for_recipient_state_named(
    path: &str,
    integration_id: &str,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut last_seen = None;
    loop {
        if let Ok(conn) = Connection::open(path) {
            if let Ok(Some(lease)) =
                ark_core::db::load_integration_refresh_lease(&conn, integration_id)
            {
                last_seen = Some(format!(
                    "holder={}, generation={}, fence={}",
                    lease.holder_node_id, lease.credential_generation, lease.fencing_token
                ));
                if lease.holder_node_id == "origin-node"
                    && lease.credential_generation == 1
                    && lease.fencing_token == 1
                {
                    return Ok(());
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "recipient did not persist the signed integration state; last={last_seen:?}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn seed_recipient_baseline_named(
    path: &Path,
    origin: &NodeIdentity,
    integration_id: &str,
) -> Result<(), String> {
    let conn = Connection::open(path).map_err(|error| error.to_string())?;
    let config = IntegrationConfiguration {
        integration_id: integration_id.into(),
        provider: "synthetic".into(),
        account_subject: "test-account".into(),
        public_scopes: Vec::new(),
        public_settings: json!({"before_send": true}),
        enabled: true,
        sync_cursor: None,
        revision: 1,
        hlc: format!("{HLC_WALL}:000000:recipient-node"),
    };
    upsert_integration_configuration(&conn, &config, &origin.node_id)
}

fn identity(node_id: &str, seed: u8) -> NodeIdentity {
    let signing_key = SigningKey::from_bytes(&[seed; 32]);
    let public = signing_key.verifying_key().to_bytes();
    let public_hex = hex(&public);
    let hpke = load_or_create_identity(node_id).expect("test-owned host HPKE identity");
    NodeIdentity {
        node_id: node_id.into(),
        signing_key,
        signing_public_key: public_hex.clone(),
        transport_public_key: public_hex.clone(),
        encryption_public_key: hpke.public_key,
    }
}

fn authorized_node(identity: &NodeIdentity) -> AuthorizedNode {
    AuthorizedNode {
        node_id: identity.node_id.clone(),
        key_fingerprint: format!(
            "sha256:{}",
            hex(&Sha256::digest(
                identity.signing_key.verifying_key().as_bytes()
            ))
        ),
        signing_public_key: identity.signing_public_key.clone(),
        encryption_public_key: identity.encryption_public_key.clone(),
        transport_public_key: Some(identity.transport_public_key.clone()),
        grant_epoch: 1,
        status: NodeStatus::Active,
        authorized_at: "2026-09-06T00:00:00Z".into(),
        revoked_at: None,
        revocation_epoch: None,
        revision: 1,
        hlc: format!("{HLC_WALL}:000001:{}", identity.node_id),
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "integration_replication_setup_tests.rs"]
mod tests;
