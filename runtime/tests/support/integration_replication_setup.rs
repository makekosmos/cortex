//! Deterministic, plaintext-free Core fixture for the integration replication tests.
//!
//! The caller owns the Core child lifecycle. This module only creates isolated
//! databases and seeds them through `ark-core`'s public database API before
//! `ArkHost::spawn` is called.

use ark_core::db::{
    init_schema, try_acquire_integration_refresh_lease, upsert_authorized_node,
    upsert_integration_configuration, upsert_integration_node_grant,
};
use ark_core::integration_replication::{
    AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationNodeGrant, NodeStatus,
};
use ed25519_dalek::{Signer, SigningKey};
use rusqlite::Connection;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

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
pub use transport::refresh_transport_public_keys;

pub fn new_setup() -> Result<IntegrationReplicationSetup, String> {
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let origin = identity("origin-node", 21);
    let recipient = identity("recipient-node", 22);
    let foreign = identity("foreign-node", 23);
    let origin_db = dir.path().join("origin.sqlite");
    let recipient_db = dir.path().join("recipient.sqlite");
    seed_database(&origin_db, &origin, &[&origin, &recipient, &foreign], true)?;
    seed_database(&recipient_db, &origin, &[&origin, &recipient], false)?;
    seed_recipient_baseline(&recipient_db, &origin)?;
    let conn = Connection::open(&origin_db).map_err(|error| error.to_string())?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis() as u64;
    try_acquire_integration_refresh_lease(
        &conn,
        INTEGRATION_ID,
        &origin.node_id,
        1,
        now_ms,
        60_000,
        0,
        &origin.node_id,
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

pub fn seed_database(
    path: &Path,
    origin: &NodeIdentity,
    nodes: &[&NodeIdentity],
    seed_configuration: bool,
) -> Result<(), String> {
    let conn = Connection::open(path).map_err(|error| error.to_string())?;
    init_schema(&conn)?;
    if seed_configuration {
        let config = IntegrationConfiguration {
            integration_id: INTEGRATION_ID.into(),
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
            integration_id: INTEGRATION_ID.into(),
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

pub async fn wait_for_recipient_state(path: &str) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut last_seen = None;
    loop {
        if let Ok(conn) = Connection::open(path) {
            if let Ok(Some(lease)) =
                ark_core::db::load_integration_refresh_lease(&conn, INTEGRATION_ID)
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

fn seed_recipient_baseline(path: &Path, origin: &NodeIdentity) -> Result<(), String> {
    let conn = Connection::open(path).map_err(|error| error.to_string())?;
    let config = IntegrationConfiguration {
        integration_id: INTEGRATION_ID.into(),
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
    NodeIdentity {
        node_id: node_id.into(),
        signing_key,
        signing_public_key: public_hex.clone(),
        transport_public_key: public_hex.clone(),
        encryption_public_key: public_hex,
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_seeds_both_isolated_databases() {
        let setup = new_setup().unwrap();
        for path in [&setup.origin_db, &setup.recipient_db] {
            let conn = Connection::open(path).unwrap();
            assert!(
                ark_core::db::load_integration_configuration(&conn, INTEGRATION_ID)
                    .unwrap()
                    .is_some()
            );
            assert!(
                ark_core::db::load_authorized_node(&conn, &setup.origin.node_id)
                    .unwrap()
                    .is_some()
            );
            assert!(ark_core::db::load_integration_node_grant(
                &conn,
                INTEGRATION_ID,
                &setup.recipient.node_id,
            )
            .unwrap()
            .is_some());
        }
        let conn = Connection::open(&setup.origin_db).unwrap();
        assert!(
            ark_core::db::load_authorized_node(&conn, &setup.foreign.node_id)
                .unwrap()
                .is_some()
        );
        let signature = setup.origin.signing_key.sign(b"integration-replication");
        setup
            .origin
            .signing_key
            .verifying_key()
            .verify_strict(b"integration-replication", &signature)
            .unwrap();
        let conn = Connection::open(&setup.origin_db).unwrap();
        let prepared = ark_core::integration_replication::prepare_signed_sync(
            &conn,
            "synthetic-space",
            &setup.origin.node_id,
            INTEGRATION_ID,
            &setup.recipient.node_id,
            "synthetic-message",
        )
        .unwrap();
        assert!(!prepared.envelope.payload.is_empty());
    }
}
