use std::path::Path;

use super::integration_replication_snapshot::IntegrationReplicationReadSnapshot;
use crate::writer::SqliteWriter;
use ark_application::integration_admin::{
    IntegrationAdminWrite, NodeAuthorizationOperation, PersistNodeAuthorization,
    PublishCredentialEnvelope, RefreshLeaseAcquire,
};
use ark_domain::integration_replication::{
    AuthorizedNode, IntegrationConfiguration, IntegrationCredentialEnvelope, IntegrationNodeGrant,
    IntegrationRefreshLease, NodeStatus,
};

#[derive(Debug, Clone)]
pub struct RefreshLeaseAcquireFixture {
    pub integration_id: String,
    pub holder_node_id: String,
    pub credential_generation: u64,
    pub now_ms: u64,
    pub ttl_ms: u64,
    pub expected_fencing_token: u64,
    pub device_id: String,
}

/// Acquires a refresh lease through the same Store transaction and application
/// policy used by production integration administration.
pub fn acquire_refresh_lease(
    path: &Path,
    request: RefreshLeaseAcquireFixture,
) -> Result<IntegrationRefreshLease, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
    let mut writer = SqliteWriter::open(path)?;
    let output = writer
        .apply_integration_admin(IntegrationAdminWrite::AcquireRefreshLease(
            RefreshLeaseAcquire {
                integration_id: request.integration_id,
                holder_node_id: request.holder_node_id,
                credential_generation: request.credential_generation,
                now_ms: request.now_ms,
                ttl_ms: request.ttl_ms,
                expected_fencing_token: request.expected_fencing_token,
                device_id: request.device_id,
            },
        ))
        .map_err(|error| format!("integration fixture lease acquisition failed: {error:?}"))?;
    match output {
        ark_application::integration_admin::IntegrationAdminOutput::Lease(lease) => Ok(lease),
        ark_application::integration_admin::IntegrationAdminOutput::Complete => {
            Err("integration fixture lease acquisition returned no lease".into())
        }
    }
}

/// Publishes a typed credential envelope through Store's production transaction path.
pub fn publish_credential_envelope(
    path: &Path,
    envelope: IntegrationCredentialEnvelope,
    device_id: &str,
    now_ms: u64,
) -> Result<(), String> {
    let path = path
        .to_str()
        .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
    let mut writer = SqliteWriter::open(path)?;
    writer
        .apply_integration_admin(IntegrationAdminWrite::PublishCredentialEnvelope(
            PublishCredentialEnvelope {
                envelope,
                device_id: device_id.to_owned(),
                now_ms,
            },
        ))
        .map(|_| ())
        .map_err(|error| format!("integration fixture envelope publication failed: {error:?}"))
}

pub fn persist_node_authorization(
    path: &Path,
    operation: NodeAuthorizationOperation,
    node: AuthorizedNode,
    grant: Option<IntegrationNodeGrant>,
    device_id: &str,
) -> Result<(), String> {
    let path = path
        .to_str()
        .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
    let mut writer = SqliteWriter::open(path)?;
    writer
        .apply_integration_admin(IntegrationAdminWrite::PersistNodeAuthorization(
            PersistNodeAuthorization {
                operation,
                node,
                grant,
                device_id: device_id.to_owned(),
            },
        ))
        .map(|_| ())
        .map_err(|error| format!("integration fixture authorization failed: {error:?}"))
}

pub fn load_credential_envelope(
    path: &Path,
    integration_id: &str,
    recipient_node_id: &str,
    credential_generation: u64,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
    let connection = crate::open::open_db(path)?;
    crate::integration_replication::load_integration_credential_envelope_record(
        &connection,
        integration_id,
        recipient_node_id,
        credential_generation,
    )
}

pub fn load_latest_credential_envelope(
    path: &Path,
    integration_id: &str,
    recipient_node_id: &str,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
    let connection = crate::open::open_db(path)?;
    crate::integration_replication::load_latest_integration_credential_envelope_record(
        &connection,
        integration_id,
        recipient_node_id,
    )
}

pub fn load_refresh_lease(
    path: &Path,
    integration_id: &str,
) -> Result<Option<IntegrationRefreshLease>, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
    let connection = crate::open::open_db(path)?;
    crate::integration_replication::load_integration_refresh_lease(&connection, integration_id)
}

pub fn load_version_vector(
    path: &Path,
) -> Result<Option<ark_domain::types::VersionVector>, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
    let connection = crate::open::open_db(path)?;
    crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)?
        .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
        .transpose()
}

pub struct IntegrationReplicationTestFixture {
    database: super::TestDatabase,
}

impl IntegrationReplicationTestFixture {
    pub fn create() -> Result<Self, String> {
        Ok(Self {
            database: super::TestDatabase::create()?,
        })
    }

    pub fn path(&self) -> &Path {
        self.database.path()
    }

    pub fn replace_version_vector_for_test(
        &self,
        vector: &ark_domain::types::VersionVector,
    ) -> Result<(), String> {
        let connection = self.database.open_connection()?;
        let value = serde_json::to_string(vector).map_err(|error| error.to_string())?;
        crate::sync_kv::set_sync_kv(&connection, crate::VERSION_VECTOR_KEY, &value)
    }

    pub fn admit_peer_for_test(&self, node_id: &str) -> Result<(), String> {
        use rusqlite::params;

        let connection = self.database.open_connection()?;
        let node = crate::integration_replication::load_authorized_node(&connection, node_id)?
            .ok_or_else(|| "test peer is not authorized".to_string())?;
        let signing_key = decode_public_key(&node.signing_public_key)
            .map_err(|error| format!("test peer signing key is invalid: {error}"))?;
        connection
            .execute(
                "INSERT OR IGNORE INTO device_registry_projection
                 (device_id, signing_public_key, key_epoch, admission_epoch, display_name,
                  disabled_event_id, display_name_hlc, updated_hlc, updated_event_id)
                 VALUES (?1, ?2, 1, 1, NULL, NULL, NULL, NULL, NULL)",
                params![node_id, signing_key],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn read_snapshot(&self) -> Result<IntegrationReplicationReadSnapshot, String> {
        IntegrationReplicationReadSnapshot::load(self.path())
    }

    pub fn upsert_configuration(
        &self,
        configuration: &IntegrationConfiguration,
        device_id: &str,
    ) -> Result<(), String> {
        let connection = self.database.open_connection()?;
        crate::integration_replication::upsert_integration_configuration(
            &connection,
            configuration,
            device_id,
        )
    }

    pub fn upsert_node(&self, node: &AuthorizedNode, device_id: &str) -> Result<(), String> {
        let connection = self.database.open_connection()?;
        crate::integration_replication::upsert_authorized_node(&connection, node, device_id)
    }

    pub fn upsert_integration_grant(
        &self,
        grant: &IntegrationNodeGrant,
        device_id: &str,
    ) -> Result<(), String> {
        let connection = self.database.open_connection()?;
        crate::integration_replication::upsert_integration_node_grant(&connection, grant, device_id)
    }

    pub fn seed_authorization_records(
        &self,
        nodes: &[AuthorizedNode],
        grants: &[IntegrationNodeGrant],
        device_id: &str,
    ) -> Result<(), String> {
        let mut connection = self.database.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        for node in nodes {
            crate::integration_replication::upsert_authorized_node(&transaction, node, device_id)?;
        }
        for grant in grants {
            crate::integration_replication::upsert_integration_node_grant(
                &transaction,
                grant,
                device_id,
            )?;
        }
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn validate_outbound_signed_frame(
        &self,
        envelope: &ark_domain::integration_replication::SignedSyncEnvelope,
        space_id: &str,
        origin_node_id: &str,
    ) -> Result<(), String> {
        let path = self
            .path()
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let mut reader = crate::read_snapshot::SqliteReader::open_read_only(path)
            .map_err(|error| error.to_string())?;
        reader
            .with_read_snapshot(|capabilities| {
                ark_application::integration_sync::validate_outbound_signed_sync(
                    &capabilities.integration_replication(),
                    space_id,
                    origin_node_id,
                    envelope,
                )
            })
            .map_err(|error| match error {
                crate::read_snapshot::ReadSnapshotFailure::Operation(error) => error,
                error => error.to_string(),
            })
    }

    /// Applies a signed frame through Store's production transaction boundary.
    pub fn apply_signed_frame(
        &self,
        envelope: &ark_domain::integration_replication::SignedSyncEnvelope,
        expected_space_id: &str,
        expected_recipient_node_id: &str,
        authenticated_transport_public_key: Option<&str>,
        reserved_at_ms: u64,
    ) -> Result<(), ark_domain::integration_replication::SignedSyncError> {
        use crate::writer::{SignedIntegrationFrameWriteError, SqliteWriter, TransactionFailure};
        use ark_application::signed_integration::ApplySignedIntegrationFrame;
        use ark_domain::integration_replication::SignedSyncError;

        let mut connection = self
            .database
            .open_connection()
            .map_err(SignedSyncError::Storage)?;

        let request = ApplySignedIntegrationFrame {
            envelope: envelope.clone(),
            expected_space_id: expected_space_id.into(),
            expected_recipient_node_id: expected_recipient_node_id.into(),
            authenticated_transport_public_key: authenticated_transport_public_key
                .map(str::to_owned),
            reserved_at_ms,
        };
        SqliteWriter::apply_signed_integration_frame_on_connection(
            &mut connection,
            &envelope.origin_node_id,
            request,
        )
        .map_err(|failure| match failure {
            TransactionFailure::Begin(error) | TransactionFailure::Commit(error) => {
                SignedSyncError::Storage(error.to_string())
            }
            TransactionFailure::Operation(SignedIntegrationFrameWriteError::Apply(error)) => error,
            TransactionFailure::Operation(SignedIntegrationFrameWriteError::PeerNotAdmitted) => {
                SignedSyncError::Apply("authenticated peer is not admitted".into())
            }
        })
    }

    /// Seeds typed authorization records for Core's outbound validation tests.
    pub fn seed_authorization(
        path: &Path,
        nodes: &[AuthorizedNode],
        grants: &[IntegrationNodeGrant],
        device_id: &str,
    ) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        for node in nodes {
            crate::integration_replication::upsert_authorized_node(&connection, node, device_id)?;
        }
        for grant in grants {
            crate::integration_replication::upsert_integration_node_grant(
                &connection,
                grant,
                device_id,
            )?;
        }
        Ok(())
    }

    /// Replaces a typed grant record through Store's integration persistence API.
    pub fn upsert_grant(
        path: &Path,
        grant: &IntegrationNodeGrant,
        device_id: &str,
    ) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        crate::integration_replication::upsert_integration_node_grant(&connection, grant, device_id)
    }

    /// Reports whether every persisted grant still authorizes the node's current key.
    pub fn node_may_receive_frames(path: &Path, node_id: &str) -> Result<bool, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let Some(node) =
            crate::integration_replication::load_authorized_node(&connection, node_id)?
        else {
            return Ok(false);
        };
        if node.status != NodeStatus::Active {
            return Ok(false);
        }
        crate::integration_replication::node_grants_match_current_authorization(
            &connection,
            node_id,
            node.grant_epoch,
            &node.encryption_public_key,
        )
    }

    pub fn acquire_refresh_lease(
        &self,
        request: RefreshLeaseAcquireFixture,
    ) -> Result<IntegrationRefreshLease, String> {
        acquire_refresh_lease(self.path(), request)
    }

    pub fn publish_credential_envelope(
        &self,
        envelope: IntegrationCredentialEnvelope,
        device_id: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        publish_credential_envelope(self.path(), envelope, device_id, now_ms)
    }

    pub fn seed_authorized_pair(
        path: &Path,
        origin_signing_key: &str,
        recipient_signing_key: &str,
        origin_transport_key: Option<&str>,
        recipient_transport_key: Option<&str>,
    ) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let origin = authorized_node("node-a", origin_signing_key, "enc-a", origin_transport_key);
        let recipient = authorized_node(
            "node-b",
            recipient_signing_key,
            "enc-b",
            recipient_transport_key,
        );
        crate::integration_replication::upsert_authorized_node(&connection, &origin, "bootstrap")?;
        crate::integration_replication::upsert_authorized_node(
            &connection,
            &recipient,
            "bootstrap",
        )?;
        crate::integration_replication::upsert_integration_node_grant(
            &connection,
            &integration_grant("node-a", "enc-a"),
            "bootstrap",
        )?;
        crate::integration_replication::upsert_integration_node_grant(
            &connection,
            &integration_grant("node-b", "enc-b"),
            "bootstrap",
        )?;
        Ok(())
    }

    pub fn load_configuration(
        path: &Path,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        crate::integration_replication::load_integration_configuration(&connection, integration_id)
    }
}

fn decode_public_key(
    value: &str,
) -> Result<Vec<u8>, ark_domain::integration_replication::SignedSyncError> {
    use ark_domain::integration_replication::SignedSyncError;

    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        return Err(SignedSyncError::Storage(
            "authorized node signing key must be 32-byte hex".into(),
        ));
    }
    (0..bytes.len())
        .step_by(2)
        .map(|index| {
            let pair = std::str::from_utf8(&bytes[index..index + 2])
                .map_err(|error| SignedSyncError::Storage(error.to_string()))?;
            u8::from_str_radix(pair, 16)
                .map_err(|error| SignedSyncError::Storage(error.to_string()))
        })
        .collect()
}

#[path = "seed_helpers.rs"]
mod seed_helpers;
use seed_helpers::{authorized_node, integration_grant};

#[cfg(test)]
#[path = "integration_replication_tests.rs"]
mod tests;
