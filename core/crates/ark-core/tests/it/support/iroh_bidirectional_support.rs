use std::sync::Arc;
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use tokio::sync::mpsc;

use ark_core::integration_replication::{
    encode_hex, AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationNodeGrant,
    IntegrationReplicationChange, IntegrationReplicationEntity, NodeStatus, SignedSyncEnvelope,
};
use ark_core::iroh_transport::{IrohConfig, IrohTransport};
use ark_core::protocol::{generate_id, LanSyncMessage};
use ark_core::sync_bind::SyncBind;
use ark_core::sync_server::StorageBackend;
use ark_core::sync_transport::{SyncTransport, TransportEvent};
use ark_core::types::SyncEntity;
use iroh::RelayMode;

struct TestStorage {
    inner: Arc<ark_core::SqliteStorageBackend>,
    first_check: Option<Arc<tokio::sync::Notify>>,
    release_first: Option<Arc<tokio::sync::Notify>>,
}

#[async_trait::async_trait]
impl StorageBackend for TestStorage {
    async fn validate_outbound_signed_integration_frame(
        &self,
        _frame: &SignedSyncEnvelope,
        _space: &str,
        _origin: &str,
    ) -> Result<(), String> {
        self.inner
            .validate_outbound_signed_integration_frame(_frame, _space, _origin)
            .await?;
        if let (Some(first), Some(release)) = (&self.first_check, &self.release_first) {
            first.notify_one();
            release.notified().await;
        }
        Ok(())
    }
    async fn validate_outbound_signed_integration_frame_with_transport(
        &self,
        frame: &SignedSyncEnvelope,
        space: &str,
        origin: &str,
        transport: &str,
    ) -> Result<(), String> {
        self.inner
            .validate_outbound_signed_integration_frame_with_transport(
                frame, space, origin, transport,
            )
            .await?;
        if let (Some(first), Some(release)) = (&self.first_check, &self.release_first) {
            first.notify_one();
            release.notified().await;
        }
        Ok(())
    }
    async fn load_entities(&self, _vector: &ark_core::types::VersionVector) -> Vec<SyncEntity> {
        Vec::new()
    }
    async fn load_entities_page(
        &self,
        _vector: &ark_core::types::VersionVector,
        _offset: usize,
        _limit: usize,
    ) -> Vec<SyncEntity> {
        Vec::new()
    }
    async fn apply_entity(&self, _entity: &SyncEntity) -> Result<(), String> {
        Ok(())
    }
    async fn get_kv(&self, _key: &str) -> Option<String> {
        None
    }
    async fn set_kv(&self, _key: &str, _value: &str) {}
}

fn authorized_storage() -> (
    Arc<ark_core::SqliteStorageBackend>,
    Arc<std::sync::Mutex<rusqlite::Connection>>,
) {
    let conn = Arc::new(std::sync::Mutex::new(
        rusqlite::Connection::open_in_memory().unwrap(),
    ));
    {
        let guard = conn.lock().unwrap();
        ark_core::db::init_schema(&guard).unwrap();
        let origin = AuthorizedNode {
            node_id: "device-B".into(),
            key_fingerprint: "fp-b".into(),
            signing_public_key: encode_hex(
                SigningKey::from_bytes(&[7; 32]).verifying_key().as_bytes(),
            ),
            encryption_public_key: "enc-b".into(),
            transport_public_key: None,
            grant_epoch: 1,
            status: NodeStatus::Active,
            authorized_at: "2026-08-31T00:00:00Z".into(),
            revoked_at: None,
            revocation_epoch: None,
            revision: 1,
            hlc: "2026-08-31T00:00:00.000Z:000001:device-B".into(),
        };
        let recipient = AuthorizedNode {
            node_id: "device-A".into(),
            key_fingerprint: "fp-a".into(),
            signing_public_key: "00".into(),
            encryption_public_key: "enc-a".into(),
            transport_public_key: None,
            grant_epoch: 1,
            status: NodeStatus::Active,
            authorized_at: "2026-08-31T00:00:00Z".into(),
            revoked_at: None,
            revocation_epoch: None,
            revision: 1,
            hlc: "2026-08-31T00:00:00.000Z:000001:device-A".into(),
        };
        for node in [&origin, &recipient] {
            ark_core::db::upsert_authorized_node(&guard, node, "device-B").unwrap();
        }
        for node_id in ["device-A", "device-B"] {
            ark_core::db::upsert_integration_node_grant(
                &guard,
                &IntegrationNodeGrant {
                    integration_id: "integration".into(),
                    node_id: node_id.into(),
                    node_encryption_key: format!(
                        "enc-{}",
                        if node_id == "device-A" { "a" } else { "b" }
                    ),
                    grant_epoch: 1,
                    status: GrantStatus::Active,
                    authorized_at: "2026-08-31T00:00:00Z".into(),
                    revoked_at: None,
                    revision: 1,
                    hlc: format!("2026-08-31T00:00:00.000Z:000001:{node_id}"),
                },
                "device-B",
            )
            .unwrap();
        }
        ark_core::db::upsert_integration_configuration(
            &guard,
            &IntegrationConfiguration {
                integration_id: "integration".into(),
                provider: "fatsecret".into(),
                account_subject: "account".into(),
                public_scopes: vec![],
                public_settings: serde_json::json!({}),
                enabled: true,
                sync_cursor: None,
                revision: 1,
                hlc: "2026-08-31T00:00:00.000Z:000001:device-B".into(),
            },
            "device-B",
        )
        .unwrap();
    }
    (
        Arc::new(ark_core::SqliteStorageBackend::new(conn.clone())),
        conn,
    )
}

fn signed_frame() -> SignedSyncEnvelope {
    let mut frame = SignedSyncEnvelope::new(
        "test-space-iroh-bidirectional",
        "device-B",
        "device-A",
        1,
        "addressed-1",
        vec![IntegrationReplicationChange {
            entity: IntegrationReplicationEntity::IntegrationConfiguration(
                IntegrationConfiguration {
                    integration_id: "integration".into(),
                    provider: "fatsecret".into(),
                    account_subject: "account".into(),
                    public_scopes: vec![],
                    public_settings: serde_json::json!({}),
                    enabled: true,
                    sync_cursor: None,
                    revision: 1,
                    hlc: "2026-08-31T00:00:00.000Z:000001:device-B".into(),
                },
            ),
            vector_hlc: "2026-08-31T00:00:00.000Z:000001:device-B".into(),
        }],
        "",
    );
    let key = SigningKey::from_bytes(&[7; 32]);
    frame.signature = encode_hex(
        &key.sign(&frame.canonical_signing_bytes().unwrap())
            .to_bytes(),
    );
    frame
}

fn set_recipient_transport_key(
    conn: &Arc<std::sync::Mutex<rusqlite::Connection>>,
    transport_key: &str,
    status: NodeStatus,
    epoch: u64,
) {
    let guard = conn.lock().unwrap();
    let status = match status {
        NodeStatus::Active => "active",
        NodeStatus::Revoked => "revoked",
    };
    let revoked_at = (status == "revoked").then_some("2026-08-31T00:01:00Z");
    let revocation_epoch = (status == "revoked").then_some(epoch as i64);
    guard
        .execute(
            concat!(
                "UPDATE authorized_nodes SET transport_public_key = ?1, status = ?2, ",
                "grant_epoch = ?3, revoked_at = ?4, revocation_epoch = ?5 WHERE node_id = ?6"
            ),
            rusqlite::params![
                transport_key,
                status,
                epoch as i64,
                revoked_at,
                revocation_epoch,
                "device-A"
            ],
        )
        .unwrap();
}

/// Вспомогательная функция: дренирует события из `rx` до тех пор, пока не
/// найдёт `MessageReceived` с нужным условием, либо не истечёт таймаут.
/// Пропускает `Connected`/`Disconnected` и `MessageReceived` других типов.
async fn find_message<F>(
    rx: &mut mpsc::UnboundedReceiver<TransportEvent>,
    timeout: Duration,
    predicate: F,
) -> Option<(String, LanSyncMessage)>
where
    F: Fn(&str, &LanSyncMessage) -> bool,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(
                TransportEvent::MessageReceived {
                    from_device_id,
                    msg,
                }
                | TransportEvent::MessageReceivedFromTransport {
                    from_device_id,
                    msg,
                    ..
                },
            )) => {
                if predicate(&from_device_id, &msg) {
                    return Some((from_device_id, msg));
                }
            }
            Ok(Some(_)) => {}
            Ok(None) => return None, // канал закрыт
            Err(_) => return None,   // таймаут
        }
    }
}
