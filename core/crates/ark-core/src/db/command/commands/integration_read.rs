use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};

fn serialize<T: serde::Serialize>(value: Result<T, String>) -> Result<serde_json::Value, DbError> {
    let value = value.map_err(DbError::Storage)?;
    serde_json::to_value(value).map_err(|error| DbError::Storage(error.to_string()))
}

#[derive(serde::Serialize)]
pub(crate) enum IntegrationReadQuery {
    PrepareSignedSync {
        space_id: String,
        origin_node_id: String,
        integration_id: String,
        recipient_node_id: String,
        message_id: String,
    },
    ValidateOutbound {
        space_id: String,
        origin_node_id: String,
        frame: crate::integration_replication::SignedSyncEnvelope,
    },
    LatestCredentialEnvelope {
        integration_id: String,
        recipient_node_id: String,
    },
    IssuerEncryptionKey {
        space_id: String,
        integration_id: String,
        recipient_node_id: String,
        issuer_node_id: String,
        credential_generation: u64,
        expected_issuer_key_id: String,
    },
    CredentialFence {
        space_id: String,
        integration_id: String,
        recipient_node_id: String,
        issuer_node_id: String,
        credential_generation: u64,
        refresh_fencing_token: u64,
        expected_issuer_key_id: String,
    },
    IssuerEncryptionKeyForPublish {
        space_id: String,
        integration_id: String,
        recipient_node_id: String,
        issuer_node_id: String,
        expected_issuer_key_id: String,
    },
    VerificationStatus {
        integration_id: String,
        local_node_id: String,
        now_ms: u64,
    },
}

impl sealed::Sealed for IntegrationReadQuery {}

impl StoreReadQuery for IntegrationReadQuery {
    type Output = serde_json::Value;

    fn class(&self) -> CommandClass {
        CommandClass::Interactive
    }

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let integration = capabilities.integration_replication();
        match self {
            Self::PrepareSignedSync {
                space_id,
                origin_node_id,
                integration_id,
                recipient_node_id,
                message_id,
            } => serialize(crate::integration_replication::prepare_signed_sync(
                &integration,
                &space_id,
                &origin_node_id,
                &integration_id,
                &recipient_node_id,
                &message_id,
            )),
            Self::ValidateOutbound {
                space_id,
                origin_node_id,
                frame,
            } => crate::integration_replication::validate_outbound_signed_sync(
                &integration,
                &space_id,
                &origin_node_id,
                &frame,
            )
            .map(|()| serde_json::json!(true))
            .map_err(DbError::from_application_error),
            Self::LatestCredentialEnvelope {
                integration_id,
                recipient_node_id,
            } => serialize(crate::db::load_latest_integration_credential_envelope(
                &integration,
                &integration_id,
                &recipient_node_id,
            )),
            Self::IssuerEncryptionKey {
                space_id,
                integration_id,
                recipient_node_id,
                issuer_node_id,
                credential_generation,
                expected_issuer_key_id,
            } => serialize(crate::db::load_issuer_encryption_key(
                &integration,
                &space_id,
                &integration_id,
                &recipient_node_id,
                &issuer_node_id,
                credential_generation,
                &expected_issuer_key_id,
            )),
            Self::CredentialFence {
                space_id,
                integration_id,
                recipient_node_id,
                issuer_node_id,
                credential_generation,
                refresh_fencing_token,
                expected_issuer_key_id,
            } => crate::db::check_integration_credential_fence(
                &integration,
                &crate::db::CredentialFenceExpectation {
                    space_id: &space_id,
                    integration_id: &integration_id,
                    recipient_node_id: &recipient_node_id,
                    issuer_node_id: &issuer_node_id,
                    credential_generation,
                    refresh_fencing_token,
                    expected_issuer_key_id: &expected_issuer_key_id,
                },
            )
            .map(|()| serde_json::json!(true))
            .map_err(DbError::from_application_error),
            Self::IssuerEncryptionKeyForPublish {
                space_id,
                integration_id,
                recipient_node_id,
                issuer_node_id,
                expected_issuer_key_id,
            } => serialize(crate::db::load_issuer_encryption_key_for_publish(
                &integration,
                &space_id,
                &integration_id,
                &recipient_node_id,
                &issuer_node_id,
                &expected_issuer_key_id,
            )),
            Self::VerificationStatus {
                integration_id,
                local_node_id,
                now_ms,
            } => serialize(crate::db::integration_verification_status(
                &integration,
                &integration_id,
                &local_node_id,
                now_ms,
            )),
        }
    }
}
