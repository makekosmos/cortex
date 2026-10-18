use crate::application::signed_integration::{
    ReplayApply, SignedIntegrationFrameSession, SignedIntegrationFrameStore,
};
use crate::integration_replication::{
    AuthorizedNode, IntegrationConfiguration, IntegrationNodeGrant, IntegrationReplicationChange,
    IntegrationReplicationEntity, SignedSyncError, VerifiedSyncEnvelope,
};
use crate::types::VersionVector;
use rusqlite::{params, Transaction};

const REPLAY_SAVEPOINT: &str = "ark_signed_sync_replay";

pub(crate) struct SqliteSignedIntegrationFrameSession<'a, 'connection> {
    transaction: &'a Transaction<'connection>,
}

impl<'a, 'connection> SqliteSignedIntegrationFrameSession<'a, 'connection> {
    pub(crate) fn new(transaction: &'a Transaction<'connection>) -> Self {
        Self { transaction }
    }
}

impl SignedIntegrationFrameStore for SqliteSignedIntegrationFrameSession<'_, '_> {
    fn load_authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String> {
        ark_store::integration_replication::load_authorized_node(self.transaction, node_id)
    }

    fn load_integration_grant(
        &self,
        integration_id: &str,
        node_id: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String> {
        crate::db::load_integration_node_grant(self.transaction, integration_id, node_id)
    }

    fn load_configuration(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String> {
        crate::db::load_integration_configuration(self.transaction, integration_id)
    }

    fn load_current_entity(
        &self,
        entity: &IntegrationReplicationEntity,
    ) -> Result<Option<IntegrationReplicationEntity>, String> {
        let current = match entity {
            IntegrationReplicationEntity::IntegrationConfiguration(value) => {
                crate::db::load_integration_configuration(self.transaction, &value.integration_id)?
                    .map(IntegrationReplicationEntity::IntegrationConfiguration)
            }
            IntegrationReplicationEntity::AuthorizedNode(value) => {
                ark_store::integration_replication::load_authorized_node(
                    self.transaction,
                    &value.node_id,
                )?
                .map(IntegrationReplicationEntity::AuthorizedNode)
            }
            IntegrationReplicationEntity::IntegrationNodeGrant(value) => {
                crate::db::load_integration_node_grant(
                    self.transaction,
                    &value.integration_id,
                    &value.node_id,
                )?
                .map(IntegrationReplicationEntity::IntegrationNodeGrant)
            }
            IntegrationReplicationEntity::IntegrationRefreshLease(value) => {
                ark_store::integration_replication::load_integration_refresh_lease(
                    self.transaction,
                    &value.integration_id,
                )?
                .map(IntegrationReplicationEntity::IntegrationRefreshLease)
            }
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => {
                crate::db::load_integration_credential_envelope(
                    self.transaction,
                    &value.integration_id,
                    &value.recipient_node_id,
                    value.credential_generation,
                )?
                .map(IntegrationReplicationEntity::IntegrationCredentialEnvelope)
            }
        };
        Ok(current)
    }

    fn load_version_vector(&self) -> Result<VersionVector, String> {
        ark_store::sync_kv::get_sync_kv(self.transaction, crate::db::VERSION_VECTOR_KEY)?
            .filter(|value| !value.trim().is_empty())
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    fn persist_entity(
        &self,
        change: &IntegrationReplicationChange,
        origin_node_id: &str,
        received_at_ms: u64,
    ) -> Result<(), String> {
        match &change.entity {
            IntegrationReplicationEntity::IntegrationConfiguration(value) => {
                crate::db::upsert_integration_configuration(self.transaction, value, origin_node_id)
            }
            IntegrationReplicationEntity::AuthorizedNode(value) => {
                crate::db::upsert_authorized_node(self.transaction, value, origin_node_id)
            }
            IntegrationReplicationEntity::IntegrationNodeGrant(value) => {
                crate::db::upsert_integration_node_grant(self.transaction, value, origin_node_id)
            }
            IntegrationReplicationEntity::IntegrationRefreshLease(value) => {
                crate::db::upsert_integration_refresh_lease_record(
                    self.transaction,
                    value,
                    origin_node_id,
                    true,
                )
            }
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => {
                crate::db::upsert_integration_credential_envelope_record(
                    self.transaction,
                    value,
                    origin_node_id,
                    received_at_ms,
                )
            }
        }
    }

    fn persist_version_vector(&self, vector: &VersionVector) -> Result<(), String> {
        let encoded = serde_json::to_string(vector).map_err(|error| error.to_string())?;
        ark_store::sync_kv::set_sync_kv(self.transaction, crate::db::VERSION_VECTOR_KEY, &encoded)
            .map_err(|error| error.to_string())
    }
}

impl SignedIntegrationFrameSession for SqliteSignedIntegrationFrameSession<'_, '_> {
    fn load_authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String> {
        ark_store::integration_replication::load_authorized_node(self.transaction, node_id)
    }

    fn with_replay_reservation(
        &mut self,
        verified: &VerifiedSyncEnvelope,
        reserved_at_ms: u64,
        apply: ReplayApply<'_>,
    ) -> Result<(), SignedSyncError> {
        let reserved_at_ms = i64::try_from(reserved_at_ms).map_err(|_| {
            SignedSyncError::Storage("reserved_at_ms exceeds SQLite INTEGER range".into())
        })?;
        let key_epoch = i64::try_from(verified.key_epoch).map_err(|_| {
            SignedSyncError::Storage("key_epoch exceeds SQLite INTEGER range".into())
        })?;
        self.transaction
            .execute_batch(&format!("SAVEPOINT {REPLAY_SAVEPOINT}"))
            .map_err(|error| SignedSyncError::Storage(error.to_string()))?;
        let insertion = self.transaction.execute(
            "INSERT INTO sync_signed_replay_reservations
             (space_id, message_id, origin_node_id, key_epoch, reserved_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &verified.space_id,
                &verified.message_id,
                &verified.origin_node_id,
                key_epoch,
                reserved_at_ms,
            ],
        );
        match insertion {
            Ok(1) => match apply(self) {
                Ok(()) => match self
                    .transaction
                    .execute_batch(&format!("RELEASE SAVEPOINT {REPLAY_SAVEPOINT}"))
                {
                    Ok(()) => Ok(()),
                    Err(error) => Err(self.rollback_replay(error.to_string())),
                },
                Err(error) => Err(self.rollback_replay_with(SignedSyncError::Apply(error))),
            },
            Ok(_) => Err(self.rollback_replay_with(SignedSyncError::Replay)),
            Err(error) => {
                let replay = is_unique_constraint(&error);
                let message = error.to_string();
                match self.rollback_savepoint() {
                    Ok(()) if replay => Err(SignedSyncError::Replay),
                    Ok(()) => Err(SignedSyncError::Storage(message)),
                    Err(rollback_error) => Err(SignedSyncError::Storage(format!(
                        "{message}; replay savepoint rollback failed: {rollback_error}"
                    ))),
                }
            }
        }
    }
}

impl SqliteSignedIntegrationFrameSession<'_, '_> {
    fn rollback_savepoint(&self) -> Result<(), rusqlite::Error> {
        self.transaction.execute_batch(&format!(
            "ROLLBACK TO SAVEPOINT {REPLAY_SAVEPOINT}; RELEASE SAVEPOINT {REPLAY_SAVEPOINT}"
        ))
    }

    fn rollback_replay(&self, cause: String) -> SignedSyncError {
        match self.rollback_savepoint() {
            Ok(()) => SignedSyncError::Storage(cause),
            Err(error) => SignedSyncError::Storage(format!(
                "{cause}; replay savepoint rollback failed: {error}"
            )),
        }
    }

    fn rollback_replay_with(&self, cause: SignedSyncError) -> SignedSyncError {
        match self.rollback_savepoint() {
            Ok(()) => cause,
            Err(error) => SignedSyncError::Storage(format!(
                "{cause}; replay savepoint rollback failed: {error}"
            )),
        }
    }
}

fn is_unique_constraint(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(code, _)
        if code.code == rusqlite::ffi::ErrorCode::ConstraintViolation
            && matches!(code.extended_code,
                rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
                    | rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE))
}
